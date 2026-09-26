//! Background worker that drains `notification_queue` and sends referrer-facing
//! Telegram messages.
//!
//! Loop #21: referral lifecycle pushes (friend joined; friend ordered and
//! milestone until 2026-09-26) are decoupled from the request path. The worker polls every 30 seconds, sends
//! up to 50 queued messages per tick, and marks rows `processed_at` on success.
//! Each pass counts an attempt before the send, so three end a row either way.

use std::collections::HashMap;
use std::sync::Arc;
use teloxide::prelude::*;
use teloxide::types::{ChatId, InlineKeyboardMarkup};

use crate::bot::{miniapp_deep_link, url_btn};
use crate::config::Config;
use crate::db::Database;
use crate::locales::get_locale;

const POLL_INTERVAL_SECS: u64 = 30;
const BATCH_SIZE: usize = 50;
const MAX_ATTEMPTS: i32 = 3;

/// Spawn the notification worker loop.
pub fn spawn_notification_worker(
    orm: sea_orm::DatabaseConnection,
    bot: Arc<Bot>,
    config: Arc<Config>,
) {
    tokio::spawn(async move {
        let db = Arc::new(Database::from_conn(orm.clone()));
        let mut interval =
            tokio::time::interval(std::time::Duration::from_secs(POLL_INTERVAL_SECS));
        interval.tick().await; // discard cold-start tick

        // Held ACROSS ticks on purpose: a row whose attempt the column would
        // not take is bounded by nothing else. See `UncountedAttempts`.
        let mut uncounted = UncountedAttempts::default();
        loop {
            interval.tick().await;
            match process_batch(&orm, &bot, &config, &db, BATCH_SIZE, &mut uncounted).await {
                // silent-tick: deliberate, not an omission -- ticks every 30s (POLL_INTERVAL_SECS) = 2880 lines/day, and Ok(0) here means zero messages DELIVERED, not zero due.
                // Reporting "nothing due" here would announce health where there may be
                // total delivery failure, which is worse than the silence it replaced.
                Ok(0) => {}
                Ok(n) => tracing::info!("notification worker: delivered {} message(s)", n),
                Err(e) => tracing::warn!("notification worker batch failed: {}", e),
            }
        }
    });
}

async fn process_batch(
    orm: &sea_orm::DatabaseConnection,
    bot: &Bot,
    config: &Config,
    db: &Arc<Database>,
    limit: usize,
    uncounted: &mut UncountedAttempts,
) -> Result<usize, anyhow::Error> {
    let kinds = DeliverableKind::names();
    let rows =
        crate::db::notifications::pending_notifications(orm, limit, MAX_ATTEMPTS, &kinds).await?;
    let mut delivered = 0usize;

    for row in rows {
        // A row of a kind this worker does not deliver is HELD: the retired
        // garden's `friend_watered`, or any kind nothing here writes (owner
        // rulings of 2026-09-24 and 2026-09-25, see `DeliverableKind`). The
        // scan above already leaves such rows out; this is the second lock,
        // and it stands above every write, so a held row leaves the loop
        // exactly as it is stored -- no attempt, no `processed_at`, no send.
        let Some(deliverable) = DeliverableKind::of(&row.kind) else {
            tracing::warn!(
                "notification worker: {} kind={} is held and not sent (owner rulings of 2026-09-24/25), yet the scan handed it out",
                row.id,
                row.kind
            );
            continue;
        };
        let telegram_id = row.telegram_id;
        let kind = row.kind.clone();
        let payload = row.payload.clone();

        // What this row has cost so far, read from BOTH places the number can
        // live: its own column, and whatever this process had to hold for it
        // because that column refused the write.
        let spent_before = attempts_spent(row.attempts, uncounted.remembered(row.id));
        // The scan filters on the column alone, so a row whose column is stuck
        // keeps being handed out after its budget is gone. Nothing more is
        // spent on such a row and nothing is sent -- but it is not dropped in
        // silence either: the terminal write below is still attempted, because
        // the tick an UPDATE finally lands is the tick it can leave the queue.
        let budget_gone = budget_is_spent(spent_before);

        // The attempt is counted BEFORE it is spent, and the answer is CARRIED
        // rather than obeyed. Two defects meet on this line and they are not
        // the same defect.
        //
        // The first: until 2026-09-21 only the send-failed arm moved this
        // counter, so a send that SUCCEEDED and whose mark_delivered then
        // failed left `attempts` unchanged. The next tick re-read the row,
        // re-sent it, and the give-up below was unreachable because `attempts`
        // stayed 0. One customer received one referral message every
        // POLL_INTERVAL_SECS seconds for as long as that second write kept
        // failing. Counting here is what makes the bound reachable on every
        // way out of this body -- delivered, failed or skipped.
        //
        // The second was the FIRST CUT of that fix, and it is what this shape
        // repairs: it refused to send when the increment failed, which handed
        // outbound delivery a single point of failure it never had. Revoke
        // UPDATE on notification_queue and every row was skipped, for ever,
        // with one warn per row per tick and not one message out. An UPDATE
        // that will not land is now a condition this loop SEES and reports and
        // then carries: the attempt is held in memory (`UncountedAttempts`),
        // `attempts_spent` adds the two halves, and the bound is reached on
        // that path as well.
        let mut column_now = row.attempts;
        let mut counted_in_memory = false;
        if !budget_gone {
            match crate::db::notifications::increment_attempts(orm, row.id).await {
                Ok(()) => column_now = attempts_after_pass(row.attempts),
                Err(e) => {
                    counted_in_memory = true;
                    let held = uncounted.remember(row.id);
                    tracing::warn!(
                        "notification worker: the attempts column refused the write for {}; this pass is counted in memory instead ({} held for this row, and a restart forgets them): {}",
                        row.id,
                        held,
                        e
                    );
                }
            }
        }
        let attempts = if budget_gone {
            spent_before
        } else {
            attempts_after_pass(spent_before)
        };

        if telegram_id == 0 {
            // A row with no chat reaches nobody; the mark is what takes it out
            // of the queue. The write is no longer fire-and-forget: the attempt
            // above bounds the repeat either way, but three silent passes over
            // a row that cannot be marked look like nothing happening at all.
            match crate::db::notifications::mark_delivered(orm, row.id).await {
                Ok(()) => uncounted.forget(row.id),
                Err(e) => tracing::warn!(
                    "notification worker: {} has no chat id and could not be marked: {}",
                    row.id,
                    e
                ),
            }
            continue;
        }

        // A row whose budget is gone is not sent again, and everything the
        // send needs -- the language, the text, the button -- is work for a
        // message that is not going out, so the whole block is skipped rather
        // than built and dropped.
        let sent = if budget_gone {
            None
        } else {
            let lang = db
                .get_user_lang(telegram_id)
                .await
                .unwrap_or_else(|| "en".to_string());
            let locale = get_locale(&lang);

            let text = build_message(deliverable, &payload, &locale);
            // This button carried `startapp=garden` until D5 removed the screen.
            // Every notification this worker sends is about a friend — joined,
            // ordered — so `referrals` is not a substitute destination, it is the
            // one the message was always about; the garden merely hosted the panel.
            // The deep link is the reason `Target::Referrals` exists in
            // `trios::deeplink`: the bot answers `/start referrals` with a button,
            // the app parses it to `/referrals`, and the server serves that path.
            let markup = InlineKeyboardMarkup::new(vec![vec![url_btn(
                &locale.referrals_open_app,
                &miniapp_deep_link(&config.bot_username, "referrals"),
            )]]);

            Some(
                bot.send_message(ChatId(telegram_id), text)
                    .reply_markup(markup)
                    .await,
            )
        };
        if let Some(Err(e)) = &sent {
            tracing::warn!(
                "notification worker: send failed for telegram_id={} kind={} ({} of {} attempts spent{}): {}",
                telegram_id,
                kind,
                attempts,
                MAX_ATTEMPTS,
                if counted_in_memory {
                    ", the last of them in memory"
                } else {
                    ""
                },
                e
            );
        }
        let sent_ok = matches!(sent, Some(Ok(_)));

        // One terminal write, reached from both ways a pass can end the row:
        // a send the transport acknowledged, and a budget that is spent. It is
        // the same `processed_at` in both cases, so the row cannot say
        // afterwards which happened -- which is why the reason is said here.
        let mut marked = false;
        let mut confirmed = false;
        if sent_ok || budget_is_spent(attempts) {
            match crate::db::notifications::mark_delivered(orm, row.id).await {
                Ok(()) => {
                    marked = true;
                    confirmed = sent_ok;
                }
                Err(e) => tracing::warn!(
                    "notification worker: processed_at not written for {} ({} of {} attempts spent): {}",
                    row.id,
                    attempts,
                    MAX_ATTEMPTS,
                    e
                ),
            }
        }
        // Nothing needs remembering once the scan stops handing the row out,
        // and it stops for two separate reasons: the row is marked, or the
        // COLUMN alone has reached the ceiling the scan filters on.
        if marked || budget_is_spent(column_now) {
            uncounted.forget(row.id);
        }
        if confirmed {
            crate::metrics::friend_activity_pushed(&kind);
            delivered += 1;
        }
        if !confirmed && !budget_gone && budget_is_spent(attempts) {
            // A message that stops being retried leaves a line naming it, and
            // leaves it ONCE: the pass that spends the last attempt is the one
            // that says so. The row itself records only that it is no longer
            // pending, and when even that write failed the row stays pending
            // and the scan's `attempts < MAX_ATTEMPTS` filter is what holds it
            // back -- unless the counter is the write that is failing, in
            // which case nothing holds the SCAN back and `budget_gone` above
            // is what holds the SEND back on every later pass.
            tracing::warn!(
                "notification worker: giving up on {} kind={} after {} attempt(s){}: {}",
                row.id,
                kind,
                attempts,
                if counted_in_memory {
                    ", the last of them in memory"
                } else {
                    ""
                },
                if sent_ok {
                    "it was sent and the delivery mark never succeeded"
                } else {
                    "every send failed"
                }
            );
        }
    }

    Ok(delivered)
}

/// Attempts this process spent that the `attempts` column would not hold.
///
/// The bound this worker keeps is a number in a column, and an UPDATE that
/// fails leaves that number where it was. The first cut of the 2026-09-21
/// resend fix answered that by refusing to send such a row, which is how one
/// column became able to stop outbound delivery altogether. This is the other
/// answer: the pass is sent, and the attempt it spent is held here until the
/// row leaves the queue.
///
/// What this is NOT: a second home for the bound. It lives in this process, so
/// a restart forgets it and a restarted worker spends the budget again on a
/// row whose column is still stuck. That is a repeat bounded by restarts
/// rather than the unbounded one a missing count gives, it is said out loud in
/// the warn at the call site rather than left to be discovered, and the
/// persisted failure record that would close it properly is the same store
/// `specs/turbobaby/notification_queue.t27` DEAD_LETTER_NOTE already asks for.
///
/// Size: one entry per row whose increment failed AND which is still pending.
/// An entry is dropped the moment the row's `processed_at` write lands or its
/// column alone reaches the ceiling -- either of which stops the scan handing
/// it out. The scan is `ORDER BY scheduled_at LIMIT BATCH_SIZE`, so in the
/// world where no UPDATE lands at all the same head-of-queue rows come back
/// every tick and this map stays at batch size.
#[derive(Default)]
struct UncountedAttempts {
    per_row: HashMap<uuid::Uuid, i32>,
}

impl UncountedAttempts {
    /// Attempts held here for a row; zero for a row that never needed it.
    fn remembered(&self, id: uuid::Uuid) -> i32 {
        self.per_row.get(&id).copied().unwrap_or(0)
    }

    /// Hold one more attempt for a row, and say how many are now held.
    fn remember(&mut self, id: uuid::Uuid) -> i32 {
        let held = self.per_row.entry(id).or_insert(0);
        *held = attempts_after_pass(*held);
        *held
    }

    /// Drop a row the scan will not hand out again.
    fn forget(&mut self, id: uuid::Uuid) {
        self.per_row.remove(&id);
    }
}

/// What a row has cost so far, from the two places the number can live.
///
/// The column is the durable half and the only half the SCAN can read
/// (`pending_notifications` filters on it). `remembered` is the half this
/// process is holding because an UPDATE would not take it. The give-up bound
/// is one number applied to their sum, so a column that stopped moving cannot
/// quietly buy a row more sends than a column that works.
fn attempts_spent(column: i32, remembered: i32) -> i32 {
    column.saturating_add(remembered)
}

/// The `attempts` value one pass over a row leaves behind.
///
/// Always one more than it found, and that is the whole fix: until 2026-09-21
/// the counter moved only when a send FAILED, so the one outcome that repeats
/// a message -- a send that succeeded and a `processed_at` write that did not
/// -- advanced nothing and repeated for ever. An attempt is an attempt however
/// it ended.
///
/// This lives beside the loop rather than inside it because `process_batch`
/// needs a live Postgres and a live Telegram, so nothing could execute its
/// retry rule; the tests below drive this one over as many passes as it takes,
/// and `tests/notification_drain_wiring.rs` is what says the loop still asks.
fn attempts_after_pass(attempts_before: i32) -> i32 {
    attempts_before.saturating_add(1)
}

/// Whether a row whose pass ended without a confirmed delivery must be given
/// up on.
///
/// The same number the scan filters on: `process_batch` hands `MAX_ATTEMPTS`
/// to `pending_notifications`, so a row this says yes about is also a row the
/// next scan will not hand out. That overlap is deliberate -- it means the
/// bound still holds on the one path where the give-up write itself fails.
fn budget_is_spent(attempts: i32) -> bool {
    attempts >= MAX_ATTEMPTS
}

fn build_message(
    kind: DeliverableKind,
    payload: &serde_json::Value,
    locale: &crate::locales::Locale,
) -> String {
    let name = payload
        .get("referred_name")
        .and_then(|v| v.as_str())
        .unwrap_or("Friend");

    match kind {
        DeliverableKind::FriendJoined => format!(
            "{}\n\n{}",
            locale.referral_friend_joined.replace("{name}", name),
            locale.referral_invite_progress_hint
        ),
        // The `friend_ordered` and `milestone` arms stood here until
        // 2026-09-26: they announced the referral bonus and the milestone
        // bonus the owner stopped that day (R3: «Убрать, только скидка 10%»).
        // Their producers went with them, so their rows are held (below).
    }
}

/// The kinds this worker delivers, and the only ones it can render.
///
/// Exactly the kinds a producer writes today: `enqueue_friend_joined` in
/// src/db/notifications.rs. A row of any other kind is HELD -- not sent, not
/// counted, not marked -- and three sorts of row fall there.
///
/// * `friend_watered`, the retired garden's report that a friend watered
///   their plant. Its writer went with the garden (D5) and its message went
///   with the owner's ruling of 2026-09-25 that nothing cannabis-related may
///   appear anywhere (answer 12, and the second list's answer 3, read by the
///   operator as: analyse all of it and take it out of customers' sight,
///   deleting nothing). Until then this worker still rendered it for rows
///   queued before D5.
/// * `friend_ordered` and `milestone`, the referral bonus's and the milestone
///   bonus's announcements. Both bonuses stopped on 2026-09-26 (owner, R3:
///   «Убрать, только скидка 10%»), their producers were deleted, and a row
///   queued before that is held rather than announcing money no longer paid.
/// * A kind nothing here has ever written: this database forked from another
///   shop's bot (DECISIONS.md D19). Until 2026-09-26 such a row went out
///   through a catch-all arm that showed the customer the raw kind column.
///
/// Held, not terminated. The queue has no status or reason column, and its one
/// terminal write, `processed_at`, is the very write a delivery makes
/// (`mark_delivered`): writing it would record a message nobody received as
/// delivered, and rewrite a stored row, which the owner's ruling of
/// 2026-09-24 (nothing deleted) and the rulings above do not allow. So the row
/// stays as it was stored, and `names()` is handed to the scan
/// (`pending_notifications`), which never hands such a row out -- no retry,
/// and no head-of-queue slot spent on it. The contract is
/// specs/turbobaby/notification_queue.t27, `HELD_KINDS_DECIDED_AT`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DeliverableKind {
    FriendJoined,
}

impl DeliverableKind {
    /// Every variant, in the order their names are handed to the scan.
    const ALL: [DeliverableKind; 1] = [Self::FriendJoined];

    /// The kind a row carries, when it is one this worker delivers. The
    /// default arm refuses: an unknown kind is held, never rendered.
    fn of(kind: &str) -> Option<Self> {
        match kind {
            "friend_joined" => Some(Self::FriendJoined),
            _ => None,
        }
    }

    /// The `kind` column value this variant is written under.
    fn name(self) -> &'static str {
        match self {
            Self::FriendJoined => "friend_joined",
        }
    }

    /// What the scan may hand out: the names of every variant.
    fn names() -> [&'static str; 1] {
        Self::ALL.map(Self::name)
    }
}

#[cfg(test)]
mod tests {
    use super::{
        attempts_after_pass, attempts_spent, budget_is_spent, build_message, DeliverableKind,
        MAX_ATTEMPTS,
    };

    /// What one row cost the drain, walked over as many ticks as it takes.
    #[derive(Debug, PartialEq, Eq)]
    struct Walk {
        /// Passes that actually put a message on the wire. This is the number
        /// the customer feels.
        sends: usize,
        /// Passes the drain spent on the row before the scan stopped handing
        /// it out. `None` means it was still being handed out after `cap`
        /// ticks, which for the callers below means for ever.
        passes: Option<usize>,
    }

    /// Walk one row through ticks of the drain.
    ///
    /// `sent`, `marked` and `counted` are whether the Telegram send, the
    /// `processed_at` write and the `attempts` UPDATE succeed on every pass.
    /// The three rules the decision rests on -- `attempts_spent`,
    /// `attempts_after_pass`, `budget_is_spent` -- are the production ones, so
    /// changing any of them moves these numbers. That the LOOP still asks them
    /// is a separate claim and a separate file:
    /// `tests/notification_drain_wiring.rs`, because `process_batch` needs a
    /// live Postgres and a live Telegram and no test here can execute it.
    fn walk(sent: bool, marked: bool, counted: bool, cap: usize) -> Walk {
        let mut column = 0i32; // the row's `attempts` column
        let mut remembered = 0i32; // `UncountedAttempts`, for this one row
        let mut processed = false; // `processed_at` is not null
        let mut sends = 0usize;
        for pass in 1..=cap {
            // The scan reads the COLUMN and nothing else
            // (`pending_notifications`, filtered on the same MAX_ATTEMPTS the
            // drain hands it).
            if processed || column >= MAX_ATTEMPTS {
                return Walk {
                    sends,
                    passes: Some(pass - 1),
                };
            }
            let spent_before = attempts_spent(column, remembered);
            let budget_gone = budget_is_spent(spent_before);
            let sent_ok = if budget_gone {
                false
            } else {
                if counted {
                    column = attempts_after_pass(column);
                } else {
                    remembered = attempts_after_pass(remembered);
                }
                sends += 1;
                sent
            };
            let attempts = if budget_gone {
                spent_before
            } else {
                attempts_after_pass(spent_before)
            };
            // `processed_at` is written after a send the transport
            // acknowledged, and again when the budget is spent. Those are the
            // two writes `process_batch` makes and it makes no others.
            if (sent_ok || budget_is_spent(attempts)) && marked {
                processed = true;
            }
        }
        Walk {
            sends,
            passes: None,
        }
    }

    /// The defect this module was rewritten for, stated as the customer felt
    /// it: a referral message that WAS delivered, whose `processed_at` write
    /// then failed, used to be sent again every POLL_INTERVAL_SECS seconds for
    /// as long as that write kept failing. The Ok arm advanced nothing, so the
    /// give-up branch was never reached and the row never left the queue.
    #[test]
    fn a_delivered_row_whose_mark_keeps_failing_stops_being_sent() {
        let walked = walk(true, false, true, 1_000);
        assert_eq!(
            walked.passes,
            Some(MAX_ATTEMPTS as usize),
            "a delivered row whose mark never succeeds is still being re-sent after 1000 ticks"
        );
        assert_eq!(walked.sends, MAX_ATTEMPTS as usize);
    }

    /// The same bound on the other unbounded path: when the give-up write
    /// itself fails there is nothing left to mark the row with, so the scan's
    /// own `attempts < MAX_ATTEMPTS` filter has to be what stops it.
    #[test]
    fn a_row_whose_giveup_write_keeps_failing_stops_being_sent() {
        assert_eq!(
            walk(false, false, true, 1_000).passes,
            Some(MAX_ATTEMPTS as usize),
            "a row nobody could mark is still being re-sent after 1000 ticks"
        );
    }

    /// The bound is a ceiling and not a schedule: a confirmed delivery still
    /// costs exactly one pass, and a fix that re-sent a delivered message two
    /// more times to use up its budget would pass the two tests above.
    #[test]
    fn a_confirmed_delivery_costs_one_pass() {
        assert_eq!(
            walk(true, true, true, 1_000),
            Walk {
                sends: 1,
                passes: Some(1)
            }
        );
    }

    /// Three sends per row, unchanged: the budget was already three and this
    /// change makes it reachable rather than larger.
    #[test]
    fn a_failing_send_is_retried_to_the_published_budget() {
        assert_eq!(
            walk(false, true, true, 1_000).passes,
            Some(MAX_ATTEMPTS as usize)
        );
        assert_eq!(MAX_ATTEMPTS, 3);
    }

    /// The second defect of 2026-09-21, and the one this shape exists for. The
    /// first cut of the bound above refused to send a row whose `attempts`
    /// UPDATE failed, so revoking UPDATE on the table stopped outbound
    /// delivery entirely -- a single point of failure the queue never had. A
    /// counter nobody can write is now carried, not obeyed: the message still
    /// goes out.
    #[test]
    fn a_counter_that_refuses_the_write_does_not_stop_the_message() {
        let walked = walk(true, true, false, 1_000);
        assert_eq!(
            walked.sends, 1,
            "an UPDATE the database would not take decided that the customer hears nothing"
        );
        assert_eq!(walked.passes, Some(1));
    }

    /// And it is carried WITH a bound. A column stuck at its old value keeps
    /// the scan handing the row out for ever -- that half is honest and is
    /// recorded in the contract's UNCOUNTABLE_ROW_NOTE -- but what this
    /// process remembers is what stops the SENDING at the published budget.
    #[test]
    fn a_counter_that_never_takes_the_write_still_stops_the_resend() {
        let walked = walk(true, false, false, 1_000);
        assert_eq!(
            walked.sends, MAX_ATTEMPTS as usize,
            "the resend ran past the budget on the path where the column never moved"
        );
        assert_eq!(
            walked.passes, None,
            "the SCAN is bounded here by nothing, and this test records that rather than \
             pretending otherwise: the column never reaches the ceiling it filters on, so \
             the row is re-read every tick and refused a send by what the process remembers"
        );
    }

    /// Both halves at once, over every combination of outcomes the three
    /// writes can have. At least one send: no single write can cancel a
    /// delivery. Never more than the budget: no single write can buy an extra
    /// one either.
    #[test]
    fn no_single_failed_write_cancels_a_delivery_or_buys_an_extra_one() {
        for sent in [true, false] {
            for marked in [true, false] {
                for counted in [true, false] {
                    let walked = walk(sent, marked, counted, 1_000);
                    assert!(
                        walked.sends >= 1,
                        "sent={sent} marked={marked} counted={counted}: the row was never sent at all"
                    );
                    assert!(
                        walked.sends <= MAX_ATTEMPTS as usize,
                        "sent={sent} marked={marked} counted={counted}: {} sends against a budget of {MAX_ATTEMPTS}",
                        walked.sends
                    );
                }
            }
        }
    }

    /// The rule underneath all of them, on its own: whatever happened on a
    /// pass, the row ends it one attempt poorer.
    #[test]
    fn every_pass_counts_an_attempt() {
        for before in 0..MAX_ATTEMPTS {
            assert_eq!(
                attempts_after_pass(before),
                before + 1,
                "a pass over a row at attempts={before} left the counter where it was"
            );
        }
    }

    /// One bound over two places. A row whose column is stuck at 1 and for
    /// which this process is holding two attempts has spent three, not one,
    /// and the give-up must read it that way -- otherwise an UPDATE that stops
    /// landing quietly grants an unlimited retry.
    #[test]
    fn an_attempt_the_column_would_not_take_is_still_an_attempt() {
        assert_eq!(attempts_spent(0, 0), 0);
        assert_eq!(attempts_spent(1, 0), 1);
        assert_eq!(attempts_spent(0, MAX_ATTEMPTS), MAX_ATTEMPTS);
        assert_eq!(attempts_spent(1, 2), MAX_ATTEMPTS);
        assert!(!budget_is_spent(attempts_spent(1, 1)));
        assert!(budget_is_spent(attempts_spent(1, 2)));
        // A stuck row is walked for as long as the process lives, so the sum
        // must not wrap back under the bound and hand it a fresh budget.
        assert_eq!(attempts_spent(i32::MAX, 5), i32::MAX);
        assert!(budget_is_spent(attempts_spent(i32::MAX, 5)));
    }

    /// The give-up and the scan agree on the value, not merely on the name:
    /// the worker hands `MAX_ATTEMPTS` to the query, so the row this abandons
    /// is the row the next scan declines to hand out.
    #[test]
    fn the_giveup_and_the_scan_stop_at_the_same_value() {
        assert!(!budget_is_spent(MAX_ATTEMPTS - 1));
        assert!(budget_is_spent(MAX_ATTEMPTS));
        assert!(budget_is_spent(attempts_after_pass(MAX_ATTEMPTS - 1)));
    }

    // --- Held kinds (owner rulings of 2026-09-24 and 2026-09-25) -----------

    /// The retired garden's kind cannot become a message: the renderer takes
    /// a `DeliverableKind` and there is none for it, and the scan is never
    /// handed its name, so its rows are not read at all.
    #[test]
    fn the_retired_garden_kind_is_held_and_never_rendered() {
        assert_eq!(DeliverableKind::of("friend_watered"), None);
        assert!(!DeliverableKind::names().contains(&"friend_watered"));
    }

    /// The two kinds that announced the stopped referral bonuses are held the
    /// same way since 2026-09-26 (R3): no variant, no name for the scan.
    #[test]
    fn the_stopped_bonus_announcements_are_held_and_never_rendered() {
        for kind in ["friend_ordered", "milestone"] {
            assert_eq!(DeliverableKind::of(kind), None, "{kind} would be delivered");
            assert!(
                !DeliverableKind::names().contains(&kind),
                "{kind} would be handed out by the scan"
            );
        }
    }

    /// A kind nothing here writes is held too. Until 2026-09-26 it went out
    /// through a catch-all arm that showed the customer the raw kind column;
    /// the refusing default arm of `DeliverableKind::of` is what replaced it.
    /// The match is exact: a near miss is a different kind, and is held.
    #[test]
    fn a_kind_nothing_here_writes_is_held_too() {
        for kind in [
            "",
            "garden_water_reminder",
            "garden_harvest_ready",
            "garden_reward_expiry",
            "Friend_Joined",
            "friend_joined ",
            "milestones",
            "referral_bonus",
        ] {
            assert_eq!(
                DeliverableKind::of(kind),
                None,
                "{kind:?} would be delivered"
            );
            assert!(
                !DeliverableKind::names().contains(&kind),
                "{kind:?} would be handed out by the scan"
            );
        }
    }

    /// What the scan is handed and what the loop accepts are one list, so a
    /// row the scan hands out is never refused by the loop, and a row the
    /// loop would refuse is never handed out.
    #[test]
    fn the_scan_and_the_loop_accept_the_same_kinds() {
        let names = DeliverableKind::names();
        assert_eq!(names, ["friend_joined"]);
        for kind in DeliverableKind::ALL {
            assert_eq!(DeliverableKind::of(kind.name()), Some(kind));
        }
        for name in names {
            assert_eq!(
                DeliverableKind::of(name).map(DeliverableKind::name),
                Some(name)
            );
        }
    }

    /// Each kind still delivered renders its own shipped template in both
    /// published languages, with every placeholder filled -- the held kinds
    /// took nothing with them that a live kind used.
    #[test]
    fn every_deliverable_kind_renders_its_own_message() {
        let payload = serde_json::json!({
            "referred_name": "Ann",
            "bonus": 50.0,
            "milestone": 5,
            "bonus_amount": 100.0,
        });
        for lang in ["ru", "en"] {
            let locale = crate::locales::get_locale(lang);
            for kind in DeliverableKind::ALL {
                let text = build_message(kind, &payload, &locale);
                let head = match kind {
                    DeliverableKind::FriendJoined => {
                        locale.referral_friend_joined.replace("{name}", "Ann")
                    }
                };
                assert!(
                    text.starts_with(&head),
                    "{lang} {kind:?} rendered {text:?}, not its own template"
                );
                assert!(
                    !text.contains('{'),
                    "{lang} {kind:?} left a placeholder: {text:?}"
                );
                assert!(
                    !text.contains("50") && !text.contains("100"),
                    "{lang} {kind:?} printed an amount from the payload: {text:?}"
                );
            }
        }
    }
}
