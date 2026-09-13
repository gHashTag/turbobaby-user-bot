use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Order {
    pub id: String,
    pub telegram_id: Option<i64>,
    pub customer_name: Option<String>,
    pub customer_phone: Option<String>,
    pub customer_telegram: Option<String>,
    pub items: Value,
    pub subtotal: f64,
    pub bonus_used: f64,
    pub stars_used: i64,
    pub total: f64,
    pub status: String,
    pub shop_id: Option<String>,
    pub delivery_address: Option<String>,
    pub delivery_notes: Option<String>,
    pub age_confirmed: bool,
    pub delivery_zone_id: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// D9: the only conversion this module performs on a money figure.
///
/// An absent, NaN, infinite or negative figure **stays absent**. It is
/// deliberately not clamped to `0.0`: a zero rate reads as "free" and a zero
/// deposit reads as "nothing owed", while `None` reads as "not computed yet"
/// and renders as a dash. The three constructs in this tree that turn an
/// unknown number into a confident zero — the `clamp` closure in
/// `db/strains.rs`, `NOT NULL DEFAULT 0` in SQL, and `try_get_warn!` (fail-open
/// by design) — are all bypassed on purpose.
pub(crate) fn finite_money(raw: Option<f64>) -> Option<f64> {
    raw.filter(|v| v.is_finite() && *v >= 0.0)
}

// Cycle #85: SeaORM `order::Model` → wire `Order` conversion. Entity has
// identical column set; `DateTimeWithTimeZone` converts to
// `chrono::DateTime<chrono::Utc>` via `.into()` (re-zones).
impl From<crate::db::entities::order::Model> for Order {
    fn from(m: crate::db::entities::order::Model) -> Self {
        // `subtotal` / `bonus_used` / `total` are `NOT NULL DEFAULT 0` columns
        // (`migrations/001_initial.sql:64-66`) and 001 is frozen (D2), so this
        // wire shape cannot express "absent" for them the way the nullable
        // bike money on `OrderItem` can. What it can stop doing is lying
        // quietly: the old closure turned a NaN — which Postgres
        // `DOUBLE PRECISION` accepts and `serde_json` cannot serialise at all
        // — into a confident `0` with no trace. Name the order and the field,
        // then fall back to 0 only because the response must serialise. New
        // money goes through `finite_money` and stays absent (D9).
        let money = |v: f64, field: &'static str| -> f64 {
            match finite_money(Some(v)) {
                Some(ok) => ok,
                None => {
                    tracing::error!(
                        order_id = %m.id,
                        field = field,
                        value = v,
                        "order money column is not a finite non-negative number; \
                         reported as 0 because the wire field cannot be null"
                    );
                    0.0
                }
            }
        };
        let subtotal = money(m.subtotal, "subtotal");
        let bonus_used = money(m.bonus_used, "bonus_used");
        let total = money(m.total, "total");
        Self {
            id: m.id,
            telegram_id: m.telegram_id,
            customer_name: m.customer_name,
            customer_phone: m.customer_phone,
            customer_telegram: m.customer_telegram,
            items: m.items,
            subtotal,
            bonus_used,
            stars_used: m.stars_used.max(0),
            total,
            status: m.status,
            shop_id: m.shop_id,
            delivery_address: m.delivery_address,
            delivery_notes: m.delivery_notes,
            age_confirmed: m.age_confirmed,
            delivery_zone_id: m.delivery_zone_id,
            created_at: m.created_at.with_timezone(&chrono::Utc),
        }
    }
}

/// Atomically mark an order as completed and update the customer's loyalty profile.
/// Returns `Some((telegram_id, is_first_order))` if the order was newly completed,
/// or `None` if it was already completed (idempotent).
///
/// Cycle #88: migrated to SeaORM transaction. The flow stays identical —
/// FOR UPDATE the order row (race guard against concurrent admin actions),
/// count prior completions for `is_first`, flip status, accumulate
/// total_spent on loyalty_profiles via upsert, recompute tier with a
/// CASE-WHEN that consults loyalty_config thresholds. Drop = auto-rollback;
/// Pure helper: extract the cashback percent for a tier from the
/// `loyalty_config.config` JSONB value. Testable without a DB.
///
/// D9 note: unlike the bike money fields, this substitutes a default when the
/// config is absent or malformed, because the caller credits a percentage and
/// has no dash to render. The defaults below are the shop's standing policy,
/// not a measurement — so every substitution is logged rather than made
/// silently, which is how a half-migrated `loyalty_config` becomes visible
/// instead of quietly paying everyone the bronze rate.
pub(crate) fn cashback_pct_for_tier(config: &serde_json::Value, tier: &str) -> f64 {
    let key = match tier {
        "gold" => "gold_cashback_pct",
        "silver" => "silver_cashback_pct",
        "bronze" => "bronze_cashback_pct",
        _ => "progressive_cashback",
    };
    let default = match tier {
        "gold" => 10.0,
        "silver" => 7.0,
        "bronze" => 5.0,
        _ => 2.0,
    };
    let raw = config.get(key).cloned().unwrap_or(serde_json::Value::Null);
    let parsed = if raw.is_array() {
        // `progressive_cashback` is an array indexed by completed-order
        // count. Without that context, use the first (lowest) value.
        raw.as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_f64())
    } else {
        raw.as_f64()
    };
    match parsed.filter(|pct| pct.is_finite() && *pct >= 0.0) {
        Some(pct) => pct,
        None => {
            tracing::warn!(
                tier = tier,
                config_key = key,
                default_pct = default,
                "loyalty_config has no usable cashback percent for this tier; \
                 crediting the standing default"
            );
            default
        }
    }
}

/// only `commit()` on the happy path.
/// Outcome of a successful order completion. Returned so callers can
/// drive user-facing side effects (Telegram notifications, etc.)
/// without re-running DB lookups the completion function already did.
#[derive(Debug, Clone)]
#[allow(unreachable_pub)] // Returned by `complete_order_and_update_loyalty` which integration tests use.
pub struct OrderCompletion {
    pub customer_telegram_id: i64,
    /// Currently unread — kept for analytics / future-caller hooks
    /// (e.g. "first-order welcome bonus" or signup-completion
    /// metrics). The cycle-#171 callers use
    /// `referral_bonus_credited.is_some()` as a stricter predicate
    /// for the referral-notification path.
    #[allow(dead_code)]
    pub is_first_order: bool,
    /// `Some(amount)` if this was a first order from a referred user
    /// AND `confirm_referral` succeeded. `None` if not-first,
    /// not-referred, or the credit failed (logged inside the
    /// function; callers receive `None` and skip downstream side
    /// effects like referrer notifications).
    pub referral_bonus_credited: Option<f64>,
    /// `Some((pct, amount))` if the order earned automatic tier-based
    /// cashback. Loop #10: credits `bonus_balance` and writes a
    /// `bonus_transactions` row inside the completion tx.
    pub cashback_credited: Option<(f64, f64)>,
}

#[allow(unreachable_pub)] // Used by tests/integration_use_bonus.rs to set up scenarios.
pub async fn complete_order_and_update_loyalty(
    orm: &sea_orm::DatabaseConnection,
    order_id: &str,
    referred_welcome_bonus: f64,
) -> Result<Option<OrderCompletion>, sea_orm::DbErr> {
    use crate::db::entities::{
        loyalty_profile::{ActiveModel as LpAm, Column as LpCol, Entity as LpEntity},
        order::{Column as OrderCol, Entity as OrderEntity},
    };
    use sea_orm::sea_query::OnConflict;
    use sea_orm::{
        ActiveValue::Set, ColumnTrait, ConnectionTrait, DbBackend, EntityTrait, QueryFilter,
        QuerySelect, Statement, TransactionTrait,
    };

    let tx = orm.begin().await?;

    // 1. Lock the order row (FOR UPDATE) and read its current state.
    let order = OrderEntity::find_by_id(order_id.to_string())
        .lock_exclusive()
        .one(&tx)
        .await?;

    let Some(order) = order else {
        tx.commit().await?;
        return Ok(None);
    };
    if order.status == "completed" {
        // Idempotent: already completed.
        tx.commit().await?;
        return Ok(None);
    }
    let Some(cid) = order.telegram_id else {
        // No customer attribution — flip status but no loyalty side-effect.
        OrderEntity::update_many()
            .col_expr(
                OrderCol::Status,
                sea_orm::sea_query::Expr::value("completed"),
            )
            .filter(OrderCol::Id.eq(order_id))
            .exec(&tx)
            .await?;
        tx.commit().await?;
        return Ok(None);
    };
    // D9: `total` drives `total_spent`, the tier recompute and the cashback
    // credit. A non-finite column value used to become `0.0` here, which
    // completed the order, accumulated nothing and credited cashback on a
    // number nobody owed. Refuse instead — the fn already returns `DbErr`,
    // `validate_create_order` rejects a non-finite total at the door, and an
    // order that cannot be priced must not be turned into loyalty money.
    let Some(total) = finite_money(Some(order.total)) else {
        return Err(sea_orm::DbErr::Custom(format!(
            "complete_order: order {} has a non-finite total ({}); refusing to \
             complete it rather than crediting loyalty on a substituted 0",
            order_id, order.total
        )));
    };

    // 2. Count prior completions BEFORE the flip — gives the correct
    //    `is_first_order` even under concurrent admin actions (the FOR
    //    UPDATE earlier serialises this).
    let count_row = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*) AS cnt FROM orders WHERE telegram_id = $1 AND status = 'completed' AND id != $2",
            [cid.into(), order_id.into()],
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("COUNT(*) returned no rows".into()))?;
    // Fail loud: a silent `.unwrap_or(0)` here would mark an *existing*
    // customer's order as their first on any read error, wrongly granting
    // first-order tier promotion + referral-bonus eligibility. The fn already
    // returns DbErr, so propagate.
    let count_before: i64 = count_row.try_get("", "cnt")?;
    let is_first = count_before == 0;

    // 3. Flip the order to completed.
    OrderEntity::update_many()
        .col_expr(
            OrderCol::Status,
            sea_orm::sea_query::Expr::value("completed"),
        )
        .filter(OrderCol::Id.eq(order_id))
        .exec(&tx)
        .await?;

    // 4. Accumulate total_spent + first_purchase_at on the loyalty profile.
    //    `total_spent = COALESCE(existing, 0) + new_total` via column-expr
    //    UPDATE inside the OnConflict clause (pattern #10 in seaorm-patterns).
    let lp_am = LpAm {
        telegram_id: Set(cid),
        total_spent: Set(Some(total)),
        first_purchase_at: Set(Some(chrono::Utc::now().into())),
        ..Default::default()
    };
    LpEntity::insert(lp_am)
        .on_conflict(
            OnConflict::column(LpCol::TelegramId)
                .value(
                    LpCol::TotalSpent,
                    sea_orm::sea_query::Expr::cust_with_values(
                        "COALESCE(loyalty_profiles.total_spent, 0) + $1",
                        [total],
                    ),
                )
                .value(
                    LpCol::FirstPurchaseAt,
                    sea_orm::sea_query::Expr::cust(
                        "COALESCE(loyalty_profiles.first_purchase_at, NOW())",
                    ),
                )
                .to_owned(),
        )
        .exec(&tx)
        .await?;

    // 5. Recompute tier from the loyalty_config JSONB thresholds. The
    //    CASE-WHEN-subquery shape is too custom for the typed builder; raw
    //    Statement (pattern #15) keeps the original logic intact.
    tx.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        "UPDATE loyalty_profiles SET tier = CASE \
            WHEN loyalty_profiles.total_spent >= (SELECT (config->>'gold_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'gold' \
            WHEN loyalty_profiles.total_spent >= (SELECT (config->>'silver_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'silver' \
            WHEN loyalty_profiles.total_spent >= (SELECT (config->>'bronze_threshold')::float8 FROM loyalty_config WHERE id = 1 LIMIT 1) THEN 'bronze' \
            ELSE 'none' \
         END \
         WHERE telegram_id = $1",
        [cid.into()],
    ))
    .await?;

    // 5a. Loop #10: automatic cashback. Read the freshly-computed tier and
    //     the matching cashback percent from loyalty_config, then credit
    //     bonus_balance and append a bonus_transactions ledger row inside
    //     the same tx. This keeps the bookkeeping equation
    //     `SUM(amount) == bonus_balance` intact (cycle #161).
    // 5a. Loop #10: automatic cashback. Read the freshly-computed tier and
    //     the matching cashback percent from loyalty_config, then credit
    //     bonus_balance and append a bonus_transactions ledger row inside
    //     the same tx. This keeps the bookkeeping equation
    //     `SUM(amount) == bonus_balance` intact (cycle #161).
    let tier_row = tx
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT tier FROM loyalty_profiles WHERE telegram_id = $1",
            [cid.into()],
        ))
        .await?;
    let tier: String = tier_row
        .and_then(|r| r.try_get::<Option<String>>("", "tier").ok().flatten())
        .unwrap_or_else(|| "none".to_string());
    let config_row = tx
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT config FROM loyalty_config WHERE id = 1".to_string(),
        ))
        .await?;
    let config: serde_json::Value = config_row
        .and_then(|r| {
            r.try_get::<Option<serde_json::Value>>("", "config")
                .ok()
                .flatten()
        })
        .unwrap_or_else(|| serde_json::json!({}));
    let cashback_pct = cashback_pct_for_tier(&config, &tier).min(100.0);
    let cashback_amount = (total * cashback_pct / 100.0).max(0.0);
    if cashback_amount > 0.01 {
        use crate::db::entities::bonus_transaction::{
            ActiveModel as BtAm, Entity as BonusTxEntity,
        };
        let tx_id = uuid::Uuid::new_v4().to_string();
        let bt_am = BtAm {
            id: Set(tx_id),
            telegram_id: Set(cid),
            amount: Set(cashback_amount),
            tx_type: Set("order_cashback".to_string()),
            description: Set(Some(format!(
                "Cashback {}% for order {}",
                cashback_pct, order_id
            ))),
            related_order_id: Set(Some(order_id.to_string())),
            ..Default::default()
        };
        BonusTxEntity::insert(bt_am).exec(&tx).await?;

        let updated = LpEntity::update_many()
            .col_expr(
                LpCol::BonusBalance,
                sea_orm::sea_query::Expr::cust_with_values("bonus_balance + $1", [cashback_amount]),
            )
            .filter(LpCol::TelegramId.eq(cid))
            .exec(&tx)
            .await?;
        if updated.rows_affected == 0 {
            return Err(sea_orm::DbErr::Custom(format!(
                "complete_order: loyalty profile missing for telegram_id={} during cashback credit",
                cid
            )));
        }
    }

    // 6. D5: the garden seed side-effect used to run here. It planted a
    //    virtual seed for the first cannabis item in the order and gated the
    //    INSERT on an `EXISTS` union over the six cannabis catalogs
    //    (`strains`, `sets`, `accessory_sets`, `tea_sets`, `accessories`,
    //    `tea_products`) — the last place in this module that named a
    //    cannabis table. A motorbike rental has no botanical analogue, so the
    //    mechanic is deleted rather than repointed: no bike family is planted,
    //    watered or harvested, and completing a rental now has exactly the
    //    loyalty side-effects above and nothing else.

    tx.commit().await?;

    // 7. Cycle #171: referral bonus credit, lifted from the two
    //    completion callers into the canonical completion function.
    //    Pre-cycle, only the bot-callback path (`bot/callbacks.rs`)
    //    called `confirm_referral` — cycle #170 mirrored it in
    //    `update_order_status`, but a future third completion path
    //    (payment webhook, batch completion, etc.) could miss it
    //    again. Lifting closes the bug class.
    //
    //    Side-effect chain: read referral_bonus from loyalty_config,
    //    call confirm_referral (which credits the referrer's
    //    bonus_balance + writes a `bonus_transactions` ledger row
    //    + sets `referral_events.status = 'paid'`). Outside the
    //    completion tx — confirm_referral has its own transaction
    //    semantics and a failure here shouldn't roll back the order
    //    completion (the order is already committed; referrals can
    //    be reconciled).
    let referral_bonus_credited: Option<f64> = if is_first {
        let bonus = match orm
            .query_one(Statement::from_string(
                DbBackend::Postgres,
                "SELECT config->>'referral_bonus' AS bonus FROM loyalty_config WHERE id = 1"
                    .to_string(),
            ))
            .await
        {
            Ok(Some(row)) => row
                .try_get::<Option<String>>("", "bonus")
                .ok()
                .flatten()
                .and_then(|s| s.parse().ok())
                .unwrap_or(200.0),
            Ok(None) | Err(_) => 200.0,
        };
        match crate::db::referrals::confirm_referral(orm, cid, bonus, referred_welcome_bonus).await
        {
            Ok(_) => {
                // Loop #21: award 1/3/5-referral milestones and notify the
                // referrer that their friend ordered. These are best-effort
                // after the order tx commits; failures are logged, not fatal.
                if let Some(referrer_id) = crate::db::referrals::get_referrer_of(orm, cid)
                    .await
                    .ok()
                    .flatten()
                {
                    if let Err(e) =
                        crate::db::referrals::maybe_award_referral_milestones(orm, referrer_id)
                            .await
                    {
                        tracing::warn!(
                            "complete_order: maybe_award_referral_milestones failed for referrer={}: {}",
                            referrer_id, e
                        );
                    }
                    let name = crate::db::users::first_name_for(orm, cid)
                        .await
                        .unwrap_or_else(|_| "Friend".to_string());
                    if let Err(e) = crate::db::notifications::enqueue_friend_ordered(
                        orm,
                        referrer_id,
                        &name,
                        bonus,
                    )
                    .await
                    {
                        tracing::warn!(
                            "complete_order: enqueue_friend_ordered failed for referrer={}: {}",
                            referrer_id,
                            e
                        );
                    }
                    crate::metrics::garden_invite_funnel("ordered");
                }
                Some(bonus)
            }
            Err(e) => {
                tracing::error!(
                    "complete_order: confirm_referral failed for cid={}: {:?}",
                    cid,
                    e
                );
                None
            }
        }
    } else {
        None
    };

    Ok(Some(OrderCompletion {
        customer_telegram_id: cid,
        is_first_order: is_first,
        referral_bonus_credited,
        cashback_credited: if cashback_amount > 0.01 {
            Some((cashback_pct, cashback_amount))
        } else {
            None
        },
    }))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(unreachable_pub)] // Used as a field of pub structs in src/api/*.rs request bodies; pub(crate) would cascade.
pub struct OrderItem {
    pub strain_id: Option<String>,
    pub strain_name: Option<String>,
    pub accessory_id: Option<String>,
    pub accessory_name: Option<String>,
    pub tea_id: Option<String>,
    pub tea_name: Option<String>,
    pub set_id: Option<String>,
    pub set_name: Option<String>,
    /// How much of this line. On a bike line (`bike.is_some()`) this is the
    /// number of UNITS of the family being booked or bought, and the API
    /// requires a whole number; on a legacy line it is the catalog quantity.
    pub quantity: f64,
    /// Unit price captured at the time the order was created. Lets the
    /// "reorder" feature rebuild the cart without fetching every catalog.
    /// Not used by bike lines: a rental is priced per DAY and a sale per
    /// unit by a human, so both live in [`BikeLine`] where they can be
    /// absent without being mistaken for a line total.
    #[serde(default)]
    pub unit_price: Option<f64>,
    pub is_set: Option<bool>,
    pub is_accessory: Option<bool>,
    pub is_tea: Option<bool>,
    pub is_tea_set: Option<bool>,
    /// D7 (`fulfillment`, two l's — the shipped spelling wins over the spec's
    /// `fulfilment`, and a serde split here would silently drop the field on
    /// every order): how this line is handed over.
    ///
    /// * legacy drink lines — "dine_in" (на месте) or "takeaway" (с собой)
    /// * bike lines — "delivery" (to the accommodation) or "pickup" (at the
    ///   Kamala office, which is the shop's default); the API rejects any
    ///   other value on a bike line
    ///
    /// Stored in the order `items` JSONB so staff see how each line is served.
    #[serde(default)]
    pub fulfillment: Option<String>,
    /// The bike half of the line: present on a rental or a sale, `None` on the
    /// legacy (pre-rebrand) lines the `items` JSONB still holds.
    #[serde(default)]
    pub bike: Option<BikeLine>,
}

/// One bike on an order — the model the customer chose, and which kind of deal
/// it is. Rental and sale are the same catalog family seen two ways, so they
/// share the identity fields and differ only inside [`BikeDeal`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[allow(unreachable_pub)] // Reachable through `OrderItem`, which is itself pub for the api/*.rs request bodies.
pub struct BikeLine {
    /// `bikes.key` — the FAMILY key (`"nmax-155"`), never a
    /// `bike_units.unit_code`. D8: the customer books a model and the shop
    /// assigns the unit, so a line that named a unit would promise something
    /// checkout cannot keep.
    pub bike_key: String,
    /// Brand + model as it read when the line was created, kept so an old
    /// order still renders after the family is renamed. `None` falls back to
    /// `bike_key` at render time — never to a guess at the model.
    #[serde(default)]
    pub bike_name: Option<String>,
    /// Rental or sale. An enum, so no order line can carry rental dates and a
    /// sale price at the same time.
    pub deal: BikeDeal,
}

/// Rental or sale, tagged on the wire with the same `kind` vocabulary D8 gives
/// `cart_items` (`bike_rental`), so a cart line maps to an order line without
/// a translation table.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[allow(unreachable_pub)] // Reachable through `OrderItem`, which is itself pub for the api/*.rs request bodies.
pub enum BikeDeal {
    /// `kind = "bike_rental"`.
    BikeRental {
        /// First rental day, inclusive.
        rental_start: chrono::NaiveDate,
        /// Last rental day, inclusive. Equal to `rental_start` for one day.
        rental_end: chrono::NaiveDate,
        /// The per-day rate ACTUALLY QUOTED for this booking, in THB.
        ///
        /// D11: the quote comes from the door (the owner's live sheet), never
        /// from arithmetic over a file — the published tariff in
        /// `data/fleet_seed.json` is pre-class-discount and pre-term-discount,
        /// and the term bands are ranges rather than multipliers. `None` means
        /// the door was silent, i.e. a human still has to quote this rental; it
        /// renders as a dash and must never be filled in with a computed
        /// number, an average or a "from" price.
        #[serde(default)]
        rate_thb_day: Option<f64>,
        /// The deposit actually agreed, and in which form. `None` means no
        /// deposit has been agreed yet — not that none is owed.
        #[serde(default)]
        deposit: Option<DepositForm>,
    },
    /// `kind = "bike_sale"`.
    BikeSale {
        /// The sale price actually quoted for one unit, in THB. `None` means
        /// unquoted and renders as a dash: no family in the seed publishes a
        /// sale price, and D14 forbids deriving one from what the shop paid
        /// for the bike, so an invented number here would be both wrong and
        /// a leak.
        #[serde(default)]
        price_thb: Option<f64>,
    },
}

/// The form a rental deposit took.
///
/// The seed's rule is verbatim: "Deposit is EITHER money OR the passport -
/// never both." One enum, so the illegal state is unrepresentable — two
/// nullable columns could hold a money amount *and* a held passport, and the
/// staff member reading the order could not tell which one to return.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "form", rename_all = "snake_case")]
#[allow(unreachable_pub)] // Reachable through `OrderItem`, which is itself pub for the api/*.rs request bodies.
pub enum DepositForm {
    /// Money was taken. It must be returned by the same method and at the same
    /// fixed amount that was agreed, which is why the method is recorded next
    /// to the figure rather than inferred later.
    Money {
        /// The figure agreed, in `currency`. `None` means it has not been
        /// agreed or computed yet — a dash, never `0`, which would read as
        /// "no deposit owed" (D9).
        #[serde(default)]
        amount: Option<f64>,
        /// Currency of `amount` as agreed: "THB" for the published tiers, or
        /// the USD/EUR equivalent the seed allows for a foreign-currency
        /// deposit. `None` when nothing has been agreed.
        #[serde(default)]
        currency: Option<String>,
        /// How it was taken, and therefore how it must be returned — one of
        /// the accepted forms in the seed: cash THB, bank transfer, USDT.
        #[serde(default)]
        method: Option<String>,
    },
    /// The passport stands in place of money. Carries no amount at all: that
    /// is the whole point of the enum.
    Passport,
}

// The two accessors below have no caller yet: the checkout leg that will
// read them (issue #10) is unwired; each carries an `#[allow(dead_code)]`
// naming that. Unlike the free-function parks in db/bikes.rs, an impl
// method with `#[expect(dead_code)]` roots itself live, so the expectation
// cannot be used here — `allow` is the honest form.
#[allow(unreachable_pub)] // Methods on a type reachable from the api/*.rs request bodies; pub(crate) would cascade.
impl DepositForm {
    /// The money figure agreed, filtered through [`finite_money`] so a NaN
    /// written by an older client cannot reach a renderer as `0`.
    ///
    /// `None` for the passport form — which owes no money by construction —
    /// and also `None` for a money deposit whose figure is not agreed yet.
    /// A caller that must tell those two apart matches on the enum; that is
    /// what it is for.
    #[allow(dead_code)] // No caller yet — the checkout leg is issue #10.
    pub fn agreed_amount(&self) -> Option<f64> {
        match self {
            DepositForm::Money { amount, .. } => finite_money(*amount),
            DepositForm::Passport => None,
        }
    }

    /// True when the passport is being held instead of money.
    #[allow(dead_code)] // No caller yet — the checkout leg is issue #10.
    pub fn is_passport(&self) -> bool {
        matches!(self, DepositForm::Passport)
    }
}

#[allow(unreachable_pub)] // Methods on a type reachable from the api/*.rs request bodies; pub(crate) would cascade.
impl BikeDeal {
    /// `(rental_start, rental_end)` for a rental, `None` for a sale.
    pub fn rental_dates(&self) -> Option<(chrono::NaiveDate, chrono::NaiveDate)> {
        match self {
            BikeDeal::BikeRental {
                rental_start,
                rental_end,
                ..
            } => Some((*rental_start, *rental_end)),
            BikeDeal::BikeSale { .. } => None,
        }
    }

    /// How many calendar days the rental covers, both ends inclusive: a
    /// booking that starts and ends on the same day is 1 day, and Monday to
    /// Sunday is 7.
    ///
    /// Used only to bound the term (and to read it back to staff). It is
    /// deliberately NOT multiplied by anything: the total a customer pays is
    /// the door's quote, and `rate_thb_day × span_days` would be exactly the
    /// invented number D11 forbids, since the door applies class and term
    /// discounts this function knows nothing about.
    pub fn span_days(&self) -> Option<i64> {
        let (start, end) = self.rental_dates()?;
        let span = (end - start).num_days();
        if span < 0 {
            // Backwards dates are rejected at the API boundary; report the
            // span as absent rather than as a plausible-looking negative.
            return None;
        }
        Some(span + 1)
    }
}

// ─── Audit-table TTL sweeps (cycles #58 / #63 / #66) ──────────────────────
//
// The three append-only audit tables — order_idempotency_keys (24 h),
// order_fraud_events (30 d), block_history (90 d) — each got their own
// near-identical sweep builder. After the third copy (cycle #66) the
// rule-of-three (Fowler, "Refactoring" §3.4) said: extract. The generic
// helper below is the only place the actual DELETE template lives. Each
// specific builder remains as a thin alias so call-sites stay
// self-documenting at their use point.

/// Build the canonical `DELETE` SQL fragment for any append-only audit
/// table swept by `created_at`. Pure, unit-testable.
///
/// **Security note:** `table` is interpolated as a SQL identifier (cannot
/// be parameterised). Callers MUST pass a static `&str` literal —
/// internal-only at the time of writing. `interval_clause` is the body
/// of `INTERVAL '...'` (e.g. `"24 hours"`, `"30 days"`).
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) fn audit_sweep_sql(table: &str, interval_clause: &str) -> String {
    format!(
        "DELETE FROM {} WHERE created_at < NOW() - INTERVAL '{}'",
        table, interval_clause
    )
}

/// Build the `DELETE` SQL fragment for the idempotency-key TTL sweep.
/// Thin alias over [`audit_sweep_sql`] so the existing tests + callers
/// keep their idempotency-specific naming.
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) fn idempotency_sweep_sql(retention_hours: u32) -> String {
    audit_sweep_sql(
        "order_idempotency_keys",
        &format!("{} hours", retention_hours),
    )
}

/// Delete `order_idempotency_keys` rows older than `retention_hours`.
/// Returns the number of rows deleted (for metrics / structured logs).
///
/// Idempotent and safe to run concurrently with `create_order` — Postgres
/// handles concurrent DELETE/INSERT on the same table cleanly, and the
/// 24 h cutoff is far older than any in-flight order's retry window.
///
/// Cycle #86: signature `&Pool` → `&sea_orm::DatabaseConnection`. The
/// SQL builder (`idempotency_sweep_sql`) stays — its INTERVAL literal
/// is unit-tested in the same module and switching the call site to
/// `Statement::from_string` is the minimal change that preserves the
/// builder's contract.
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) async fn cleanup_old_idempotency_keys(
    orm: &sea_orm::DatabaseConnection,
    retention_hours: u32,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let sql = idempotency_sweep_sql(retention_hours);
    let res = orm
        .execute(Statement::from_string(DbBackend::Postgres, sql))
        .await?;
    Ok(res.rows_affected())
}

// ─── Fraud-event TTL sweep (cycle #63 / A) ───────────────────────────────
//
// `order_fraud_events` is append-only (cycle #59). 30-day retention is
// longer than the idempotency table's 24h because trend / pattern review
// often goes back weeks (Pareto offenders, repeat tampering, etc.), but
// after 30 days the data is more noise than signal — admin acts on the
// /engage 24h window. Same pure-builder pattern as the idempotency sweep
// so the INTERVAL literal is unit-testable.

/// Build the `DELETE` SQL fragment for the fraud-event TTL sweep.
/// Thin alias over [`audit_sweep_sql`].
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) fn fraud_events_sweep_sql(retention_days: u32) -> String {
    audit_sweep_sql("order_fraud_events", &format!("{} days", retention_days))
}

/// Delete `order_fraud_events` rows older than `retention_days`. Returns
/// the row count for the spawn-loop's structured log line.
///
/// Cycle #86: same migration pattern as [`cleanup_old_idempotency_keys`].
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) async fn cleanup_old_fraud_events(
    orm: &sea_orm::DatabaseConnection,
    retention_days: u32,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let sql = fraud_events_sweep_sql(retention_days);
    let res = orm
        .execute(Statement::from_string(DbBackend::Postgres, sql))
        .await?;
    Ok(res.rows_affected())
}

// ─── block_history TTL sweep (cycle #66) ─────────────────────────────────
//
// `block_history` is the third append-only audit table (after
// `order_idempotency_keys` from cycle #58/A and `order_fraud_events` from
// cycle #63/A). Same shape as both: pure SQL builder, async cleanup, daily
// tokio::spawn loop. Retention is the longest of the three — 90 days —
// because block decisions are operational/compliance records that admins
// genuinely look back at across quarters ("did we wrongly block user X
// three months ago?"). Beyond 90 days a row is more noise than signal.

/// Build the `DELETE` SQL fragment for the block-history TTL sweep.
/// Thin alias over [`audit_sweep_sql`].
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) fn block_history_sweep_sql(retention_days: u32) -> String {
    audit_sweep_sql("block_history", &format!("{} days", retention_days))
}

/// Delete `block_history` rows older than `retention_days`. Returns the
/// number of rows deleted for the spawn-loop's structured log line.
///
/// Cycle #86: same migration pattern as [`cleanup_old_idempotency_keys`].
#[allow(dead_code)] // Called only from main.rs's spawn_ttl_sweep loop (bin); lib has no user.
pub(crate) async fn cleanup_old_block_history(
    orm: &sea_orm::DatabaseConnection,
    retention_days: u32,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let sql = block_history_sweep_sql(retention_days);
    let res = orm
        .execute(Statement::from_string(DbBackend::Postgres, sql))
        .await?;
    Ok(res.rows_affected())
}

// ─── Fraud-event audit log (cycle #59) ────────────────────────────────────
//
// Every 422 reject in `create_order` emits a structured `tracing::warn!`,
// but those lines live in stdout — invisible to an admin who only opens
// the Telegram bot. This append-only table mirrors the same data so the
// `/engage` panel can show "Suspicious activity (24h)" without grep.
//
// Inserts are best-effort: a failure here MUST NOT block the reject path
// the customer is already seeing. Worst case we lose a row, not a request.

/// Stable codes for `order_fraud_events.code`. They mirror the JSON error
/// codes the client UI may eventually parse for friendly messages.
pub(crate) const FRAUD_CODE_SUBTOTAL_MISMATCH: &str = "subtotal_mismatch";
pub(crate) const FRAUD_CODE_UNKNOWN_ITEM: &str = "unknown_item";
pub(crate) const FRAUD_CODE_UNAVAILABLE: &str = "unavailable";
pub(crate) const FRAUD_CODE_MALFORMED: &str = "malformed";

/// Number of `subtotal_mismatch` events in the lookback window that triggers
/// an automatic block (cycle #60). 3 is conservative: a real shopper hitting
/// stale-cart prices would clear and retry, not produce 3+ price tampering
/// events in a day. 1–2 might be a buggy client or race condition; 3+ is a
/// sustained pattern that warrants an automatic stop-the-bleeding action.
pub(crate) const FRAUD_AUTO_BLOCK_THRESHOLD: i64 = 3;
/// Lookback window for the auto-block decision. 24 h matches the rest of
/// the audit pipeline (/engage panel, idempotency TTL).
pub(crate) const FRAUD_AUTO_BLOCK_LOOKBACK_HOURS: i32 = 24;

/// Pure decision: should the auto-blocker engage given the count of
/// `subtotal_mismatch` events seen for this user in the lookback window?
/// Extracted as a standalone function so the threshold semantics are
/// table-tested instead of buried inside an async DB-bound function.
pub(crate) fn should_auto_block_for_fraud(subtotal_mismatch_events_24h: i64) -> bool {
    subtotal_mismatch_events_24h >= FRAUD_AUTO_BLOCK_THRESHOLD
}

/// Insert one row into `order_fraud_events`. Returns `Result<(), ...>` so
/// the caller can log a warning, but the caller MUST NOT propagate — the
/// 422 reject path is more important than the audit row landing.
///
/// Cycle #60: after a successful `subtotal_mismatch` insert with a known
/// telegram_id, this function also fires the auto-block check. The check
/// runs serially (not spawned) so a race between two concurrent mismatches
/// can't both decide independently to skip the block — Postgres
/// `pg_advisory_xact_lock` inside the helper serialises them.
pub(crate) async fn record_fraud_event(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: Option<i64>,
    code: &str,
    catalog: Option<&str>,
    item_id: Option<&str>,
    claimed_subtotal: Option<f64>,
    expected_subtotal: Option<f64>,
) -> Result<(), sea_orm::DbErr> {
    use crate::db::entities::order_fraud_event::{ActiveModel as FraudAm, Entity as FraudEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};
    // Cycle #89: SeaORM ActiveModel insert. The `drop(client)` dance for
    // freeing the deadpool connection before the recursive call to
    // `auto_block_for_fraud` is gone — `DatabaseConnection` clones cheaply
    // and SeaORM handles connection lifecycle internally.
    let am = FraudAm {
        telegram_id: Set(telegram_id),
        code: Set(code.to_string()),
        catalog: Set(catalog.map(|s| s.to_string())),
        item_id: Set(item_id.map(|s| s.to_string())),
        claimed_subtotal: Set(claimed_subtotal),
        expected_subtotal: Set(expected_subtotal),
        ..Default::default()
    };
    FraudEntity::insert(am).exec(orm).await?;

    // Auto-block trigger (cycle #60). Only fires for the strongest fraud
    // signal — `subtotal_mismatch` — and only when we have a telegram_id
    // to block. Anonymous mismatches are caught by the per-IP rate limit.
    if code == FRAUD_CODE_SUBTOTAL_MISMATCH {
        if let Some(tid) = telegram_id {
            if let Err(e) = auto_block_for_fraud(orm, tid).await {
                tracing::warn!("auto_block check failed for telegram_id={}: {}", tid, e);
            }
        }
    }
    Ok(())
}

/// Parse a `/unblock <arg>` argument into a positive telegram_id.
///
/// Pure helper extracted so the parsing rules can be exhaustively tested
/// (off-by-one, leading whitespace, dot decimals, `0` and negative) without
/// needing a Telegram bot harness. The handler then dispatches to
/// [`manual_unblock`] using the returned `i64`.
///
/// Returns `None` for empty, non-numeric, zero, or negative inputs.
/// Negative is rejected explicitly so a future SQL rewrite like
/// `WHERE telegram_id > $1` can't accidentally unblock the entire user
/// base when admin types `/unblock -1`.
pub(crate) fn parse_unblock_arg(arg: &str) -> Option<i64> {
    let trimmed = arg.trim();
    if trimmed.is_empty() {
        return None;
    }
    let parsed: i64 = trimmed.parse().ok()?;
    if parsed <= 0 {
        return None;
    }
    Some(parsed)
}

/// One row from `query_blocked_users` — currently-blocked user plus the
/// most recent fraud event we have on file for them (when any).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct BlockedUserRow {
    pub telegram_id: i64,
    /// `created_at` of the most recent `order_fraud_events` row for this
    /// user. `None` when blocked by hand (e.g. SQL or pre-cycle-#60 setups)
    /// — the audit table simply has no row to point at.
    pub last_fraud_at: Option<chrono::DateTime<chrono::Utc>>,
    /// Code from that same row. Lets admin see "what got them blocked"
    /// at a glance — usually `subtotal_mismatch`.
    pub last_fraud_code: Option<String>,
}

/// Cap on how many rows `/blocks` lists in one Telegram message. 50 keeps
/// the message well under Telegram's 4096-char limit even with long codes.
pub(crate) const BLOCKED_USERS_LIST_LIMIT: i64 = 50;

/// Fetch currently-blocked users enriched with the most recent fraud-event
/// context for each. Single round-trip — `LEFT JOIN LATERAL` lets the
/// planner index-scan `idx_fraud_events_created_at` per user instead of
/// doing a window scan across the whole table.
pub(crate) async fn query_blocked_users(
    orm: &sea_orm::DatabaseConnection,
    limit: i64,
) -> Result<Vec<BlockedUserRow>, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    // Cycle #89: raw Statement (pattern #15) — LATERAL JOIN + NULLS LAST
    // ordering doesn't have a typed builder equivalent in SeaORM 1.1.
    let rows = orm
        .query_all(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT lp.telegram_id, \
                    fe.created_at AS last_fraud_at, \
                    fe.code       AS last_fraud_code \
             FROM loyalty_profiles lp \
             LEFT JOIN LATERAL ( \
                 SELECT created_at, code \
                 FROM order_fraud_events \
                 WHERE telegram_id = lp.telegram_id \
                 ORDER BY created_at DESC LIMIT 1 \
             ) fe ON TRUE \
             WHERE lp.is_blocked = TRUE \
             ORDER BY fe.created_at DESC NULLS LAST, lp.telegram_id \
             LIMIT $1",
            [limit.into()],
        ))
        .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        out.push(BlockedUserRow {
            telegram_id: r.try_get::<i64>("", "telegram_id").unwrap_or(0),
            last_fraud_at: r
                .try_get::<Option<chrono::DateTime<chrono::Utc>>>("", "last_fraud_at")
                .ok()
                .flatten(),
            last_fraud_code: r
                .try_get::<Option<String>>("", "last_fraud_code")
                .ok()
                .flatten(),
        });
    }
    Ok(out)
}

/// Format the `/blocks` response as Telegram-flavored HTML. Pure helper —
/// testable without a Telegram client or a DB. Caller wraps in
/// `parse_mode(Html)`.
///
/// `total_count` may exceed `rows.len()` when the query was truncated to
/// `BLOCKED_USERS_LIST_LIMIT`; in that case a tail line tells admin how
/// many entries didn't fit so they don't think the list is complete.
pub(crate) fn format_blocks_message(rows: &[BlockedUserRow], total_count: usize) -> String {
    use crate::util::html_escape;
    if rows.is_empty() {
        return "🛡 <b>Blocked users</b>\n━━━━━━━━━━━━━━━━\n✅ <i>none</i>".to_string();
    }
    let mut s = String::from("🛡 <b>Blocked users</b>\n━━━━━━━━━━━━━━━━\n");
    for r in rows {
        let when = r
            .last_fraud_at
            .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
            .unwrap_or_else(|| "—".to_string());
        let code = r
            .last_fraud_code
            .as_deref()
            .map(html_escape)
            .unwrap_or_else(|| "—".to_string());
        s.push_str(&format!(
            "👤 <code>{}</code> · {}\n   📝 {}\n",
            r.telegram_id, when, code,
        ));
    }
    if total_count > rows.len() {
        s.push_str(&format!(
            "\n…and <b>{}</b> more (use SQL for full list)",
            total_count - rows.len()
        ));
    }
    s.push_str("\n<i>Unblock with /unblock &lt;telegram_id&gt;</i>");
    s
}

// ─── block_history audit log (cycle #64) ─────────────────────────────────
//
// migration 031 (`block_history`) has been on disk since cycle #46, but no
// code wrote to it. Cycles #60 and #61 emit `tracing::warn!` / `info!` on
// every auto-block and `/unblock` action, but those lines disappear into
// stdout. This wires every state transition into the table so:
//   * Compliance: "who unblocked telegram_id 42 on date X" is one query.
//   * Forensics: scan repeat offenders across multiple block cycles.
//   * Future /engage extensions can show "N auto-blocks today".

/// Stable values for `block_history.action`. Kept here next to the writers
/// so a future cycle that adds a new transition (e.g. admin manual block)
/// has a single source of truth.
pub(crate) const BLOCK_ACTION_AUTO: &str = "auto_block";
pub(crate) const BLOCK_ACTION_UNBLOCK: &str = "unblock";

/// Append one row to `block_history`. Returns `Result<(), _>` so callers can
/// log a warning on failure, but the caller MUST swallow — losing an audit
/// row is preferable to blocking the underlying block / unblock action.
pub(crate) async fn record_block_history(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
    action: &str,
    reason: Option<&str>,
    actor_admin_id: Option<i64>,
) -> Result<(), sea_orm::DbErr> {
    use crate::db::entities::block_history::{ActiveModel as BhAm, Entity as BhEntity};
    use sea_orm::{ActiveValue::Set, EntityTrait};
    let am = BhAm {
        telegram_id: Set(telegram_id),
        action: Set(action.to_string()),
        reason: Set(reason.map(|s| s.to_string())),
        actor_admin_id: Set(actor_admin_id),
        ..Default::default()
    };
    BhEntity::insert(am).exec(orm).await?;
    Ok(())
}

/// Manually clear `is_blocked` for `telegram_id`. Counterpart to the
/// automatic blocker in [`auto_block_for_fraud`] — admins drive this via
/// the `/unblock` bot command (cycle #61).
///
/// Returns `Ok(true)` if the row was actually flipped, `Ok(false)` if the
/// user was not blocked (or has no profile). The `AND is_blocked = true`
/// guard means an admin can spam `/unblock 123` without each call writing
/// a fresh `is_blocked = false` UPDATE.
pub(crate) async fn manual_unblock(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<bool, sea_orm::DbErr> {
    use crate::db::entities::loyalty_profile::{Column as LpCol, Entity as LpEntity};
    use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
    // The `is_blocked = TRUE` guard means an admin can spam `/unblock 123`
    // without each call writing a fresh UPDATE.
    let result = LpEntity::update_many()
        .col_expr(LpCol::IsBlocked, sea_orm::sea_query::Expr::value(false))
        .filter(LpCol::TelegramId.eq(telegram_id))
        .filter(LpCol::IsBlocked.eq(true))
        .exec(orm)
        .await?;
    Ok(result.rows_affected > 0)
}

/// If the user has accumulated >= `FRAUD_AUTO_BLOCK_THRESHOLD`
/// `subtotal_mismatch` events in the last `FRAUD_AUTO_BLOCK_LOOKBACK_HOURS`,
/// flip `loyalty_profiles.is_blocked = true`. After that the existing
/// `check_not_blocked` guard at the top of `create_order` rejects every
/// subsequent attempt with 403 before any DB work happens.
///
/// Returns `Ok(true)` when the user was newly blocked by this call,
/// `Ok(false)` when no action was needed (count below threshold or already
/// blocked). Defensive: an error here is logged by the caller and ignored —
/// auto-block is defence in depth, not the primary 422 protection.
async fn auto_block_for_fraud(
    orm: &sea_orm::DatabaseConnection,
    telegram_id: i64,
) -> Result<bool, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    // Count only `subtotal_mismatch` — other codes (`unavailable`,
    // `unknown_item`, `malformed`) are mostly stale-cart / client-bug and
    // would auto-block honest users. Subtotal mismatch alone is the
    // can't-happen-by-accident signal.
    let row = orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS n FROM order_fraud_events \
             WHERE telegram_id = $1 \
               AND code = $2 \
               AND created_at > NOW() - ($3 * INTERVAL '1 hour')",
            [
                telegram_id.into(),
                FRAUD_CODE_SUBTOTAL_MISMATCH.into(),
                FRAUD_AUTO_BLOCK_LOOKBACK_HOURS.into(),
            ],
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("auto_block_for_fraud: COUNT no rows".into()))?;
    // Fail loud: a silent `.unwrap_or(0)` here would read as "0 fraud events"
    // on any error → the auto-block gate silently disables itself (fail-open
    // security hole), letting a fraudster past the threshold. The fn returns
    // DbErr, so propagate and let the caller decide.
    let count: i64 = row.try_get("", "n")?;
    if !should_auto_block_for_fraud(count) {
        return Ok(false);
    }
    // UPSERT so anonymous-ish accounts without a loyalty profile row still
    // get blocked the moment they cross the threshold. The
    // `WHERE loyalty_profiles.is_blocked = FALSE` action filter is the
    // "only flip if currently unblocked" guard — keeps `rows > 0` as the
    // "newly blocked" signal that drives the audit log + warn line.
    //
    // Cycle #89 note: SeaORM's `OnConflict::value(...)` doesn't support a
    // conditional `WHERE` on the action clause; the raw Statement keeps
    // the original semantics intact. This is pattern #15 (aggregates +
    // unusual conflict shapes use raw Statement).
    let result = orm
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO loyalty_profiles (telegram_id, bonus_balance, total_spent, is_blocked) \
             VALUES ($1, 0, 0, TRUE) \
             ON CONFLICT (telegram_id) DO UPDATE SET is_blocked = TRUE \
             WHERE loyalty_profiles.is_blocked = FALSE",
            [telegram_id.into()],
        ))
        .await?;
    let newly_blocked = result.rows_affected() > 0;
    if newly_blocked {
        tracing::warn!(
            telegram_id,
            count_24h = count,
            threshold = FRAUD_AUTO_BLOCK_THRESHOLD,
            "auto_block: user blocked for repeated subtotal_mismatch"
        );
        // Cycle #89: no more drop(client) dance — SeaORM connections are
        // pooled internally and `&orm` clones for free.
        if let Err(e) = record_block_history(
            orm,
            telegram_id,
            BLOCK_ACTION_AUTO,
            Some("subtotal_mismatch_threshold"),
            None, // server-initiated, no admin actor
        )
        .await
        {
            tracing::warn!(
                telegram_id,
                "auto_block: block_history audit insert failed: {}",
                e
            );
        }
    }
    Ok(newly_blocked)
}

/// 24-hour aggregate for the `/engage` orders block (cycle #63 / B).
#[derive(Debug, Default, Clone)]
pub(crate) struct OrderStats24h {
    pub total_orders: i64,
    /// Sum of `total` across all 24h orders. Float because the column is
    /// `DOUBLE PRECISION`; admin rendering rounds to a baht integer.
    pub revenue: f64,
    pub unique_buyers: i64,
    /// `total_orders > 0` ? `revenue / total_orders` : 0. Pre-computed so
    /// the render side doesn't have to deal with div-by-zero.
    pub avg_order_value: f64,
    /// Currently `pending` orders — admin's "right now" backlog signal.
    /// Includes orders older than 24h.
    pub pending_total: i64,
}

/// One round-trip aggregate over `orders` for the last 24h, plus a tail
/// `pending_total` for the right-now view. Uses partial indexes already on
/// `created_at` and `status`; cheap even on large `orders` tables.
pub(crate) async fn order_stats_24h(
    orm: &sea_orm::DatabaseConnection,
) -> Result<OrderStats24h, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT \
                COUNT(*)::bigint                                    AS total_orders, \
                COALESCE(SUM(total::float8), 0)::float8             AS revenue, \
                COUNT(DISTINCT telegram_id) \
                    FILTER (WHERE telegram_id IS NOT NULL)::bigint  AS unique_buyers \
             FROM orders WHERE created_at > NOW() - INTERVAL '24 hours'"
                .to_string(),
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("order_stats_24h: aggregate row missing".into()))?;
    let total_orders: i64 = crate::try_get_warn!(row, "total_orders", 0_i64);
    let revenue: f64 = crate::try_get_warn!(row, "revenue", 0.0);
    // Cycle #147 fix → cycle #148 helper. `SUM(total::float8)` could
    // in principle return NaN if a `total` row got NaN-poisoned, and
    // the `/engage` admin panel would then render "Revenue: NaN ฿".
    // `avg_order_value` was already guarded; `revenue` wasn't.
    let revenue = crate::trios::validation::clamp_finite_non_negative(revenue);
    let unique_buyers: i64 = crate::try_get_warn!(row, "unique_buyers", 0_i64);

    let avg_order_value = if total_orders > 0 && revenue.is_finite() {
        revenue / total_orders as f64
    } else {
        0.0
    };

    let pending_row = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS pending FROM orders WHERE status = 'pending'".to_string(),
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("order_stats_24h: pending row missing".into()))?;
    let pending_total: i64 = pending_row.try_get("", "pending").unwrap_or(0);

    Ok(OrderStats24h {
        total_orders,
        revenue,
        unique_buyers,
        avg_order_value,
        pending_total,
    })
}

/// 24-hour aggregate for the `/engage` blocks panel (cycle #68).
/// Mirrors the shape of `OrderStats24h` / `FraudStats24h` so the bot
/// handler can stack all three panels symmetrically.
#[derive(Debug, Default, Clone)]
pub(crate) struct BlockStats24h {
    pub auto_blocks: i64,
    pub unblocks: i64,
    /// Admin who issued the most `/unblock` commands in the window, as
    /// a stringified telegram_id (matches FraudStats24h::top_offender).
    /// None when no manual unblocks happened.
    pub top_actor_admin: Option<String>,
    pub top_actor_count: i64,
}

/// One round-trip aggregate over `block_history` for the last 24h.
/// Uses `COUNT(*) FILTER (WHERE action = ...)` so the per-action counts
/// come from a single index scan over `(created_at)`.
pub(crate) async fn block_stats_24h(
    orm: &sea_orm::DatabaseConnection,
) -> Result<BlockStats24h, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    let row = orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT \
                COUNT(*) FILTER (WHERE action = $1)::bigint AS auto_blocks, \
                COUNT(*) FILTER (WHERE action = $2)::bigint AS unblocks \
             FROM block_history WHERE created_at > NOW() - INTERVAL '24 hours'",
            [BLOCK_ACTION_AUTO.into(), BLOCK_ACTION_UNBLOCK.into()],
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("block_stats_24h: aggregate row missing".into()))?;
    let mut s = BlockStats24h {
        auto_blocks: row.try_get("", "auto_blocks").unwrap_or(0),
        unblocks: row.try_get("", "unblocks").unwrap_or(0),
        top_actor_admin: None,
        top_actor_count: 0,
    };
    // Top admin actor — scoped to manual `unblock` rows because
    // `auto_block` events have NULL `actor_admin_id` by design.
    if let Ok(Some(top)) = orm
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT actor_admin_id::text AS aid, COUNT(*)::bigint AS n \
             FROM block_history \
             WHERE created_at > NOW() - INTERVAL '24 hours' \
               AND action = $1 \
               AND actor_admin_id IS NOT NULL \
             GROUP BY actor_admin_id ORDER BY n DESC LIMIT 1",
            [BLOCK_ACTION_UNBLOCK.into()],
        ))
        .await
    {
        s.top_actor_admin = top.try_get::<Option<String>>("", "aid").ok().flatten();
        s.top_actor_count = top.try_get("", "n").unwrap_or(0);
    }
    Ok(s)
}

/// Pure helper: render the `/engage` blocks block as Telegram HTML.
/// Symmetric with `format_order_stats` / fraud rendering inside the
/// engage handler. Empty window → ✅ none so admin doesn't see a panel
/// full of zeros every quiet day.
pub(crate) fn format_block_stats(s: &BlockStats24h) -> String {
    use crate::util::html_escape;
    if s.auto_blocks == 0 && s.unblocks == 0 {
        return "<b>🚫 Blocks (24h)</b>\n✅ <i>none</i>".to_string();
    }
    let top = s
        .top_actor_admin
        .as_deref()
        .map(|t| {
            format!(
                "<code>{}</code> ({} unblocks)",
                html_escape(t),
                s.top_actor_count
            )
        })
        .unwrap_or_else(|| "—".to_string());
    format!(
        "<b>🚫 Blocks (24h)</b>\n\
         🤖 Auto-blocks: <b>{}</b>\n\
         🔓 Manual unblocks: <b>{}</b>\n\
         👮 Top admin: {}",
        s.auto_blocks, s.unblocks, top,
    )
}

/// Pure helper: render the `/engage` orders block as Telegram HTML.
/// Caller wraps in `parse_mode(Html)`. Extracted from the bot handler so
/// the layout is unit-testable.
pub(crate) fn format_order_stats(s: &OrderStats24h) -> String {
    format!(
        "<b>📦 Orders (24h)</b>\n\
         🛒 Total: <b>{}</b>\n\
         💰 Revenue: <b>{:.0} ฿</b>\n\
         👥 Unique buyers: <b>{}</b>\n\
         📊 Avg order: <b>{:.0} ฿</b>\n\
         ⏳ Pending right now: <b>{}</b>",
        s.total_orders, s.revenue, s.unique_buyers, s.avg_order_value, s.pending_total,
    )
}

/// 24-hour aggregate for the `/engage` fraud panel. Keep this struct narrow:
/// /engage's text rendering reads each field once.
#[derive(Debug, Default, Clone)]
pub(crate) struct FraudStats24h {
    pub subtotal_mismatch: i64,
    pub unknown_item: i64,
    pub unavailable: i64,
    pub malformed: i64,
    /// telegram_id with the most events in the window. None if no events
    /// had a telegram_id (anonymous-only). String to allow easy `format!`
    /// without an extra cast.
    pub top_offender: Option<String>,
    pub top_offender_count: i64,
}

/// One round-trip aggregate over `order_fraud_events` for the last 24h.
pub(crate) async fn fraud_stats_24h(
    orm: &sea_orm::DatabaseConnection,
) -> Result<FraudStats24h, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, DbBackend, Statement};
    // Per-code counts. COUNT(*) FILTER (...) keeps the whole thing in one
    // index scan over `idx_fraud_events_created_at`.
    let row = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT \
                COUNT(*) FILTER (WHERE code = 'subtotal_mismatch')::bigint AS subtotal_mismatch, \
                COUNT(*) FILTER (WHERE code = 'unknown_item')::bigint        AS unknown_item, \
                COUNT(*) FILTER (WHERE code = 'unavailable')::bigint         AS unavailable, \
                COUNT(*) FILTER (WHERE code = 'malformed')::bigint           AS malformed \
             FROM order_fraud_events WHERE created_at > NOW() - INTERVAL '24 hours'"
                .to_string(),
        ))
        .await?
        .ok_or_else(|| sea_orm::DbErr::Custom("fraud_stats_24h: aggregate row missing".into()))?;
    let mut s = FraudStats24h {
        subtotal_mismatch: row.try_get("", "subtotal_mismatch").unwrap_or(0),
        unknown_item: row.try_get("", "unknown_item").unwrap_or(0),
        unavailable: row.try_get("", "unavailable").unwrap_or(0),
        malformed: row.try_get("", "malformed").unwrap_or(0),
        top_offender: None,
        top_offender_count: 0,
    };
    // Top offender — separate cheap query because it's bounded LIMIT 1.
    if let Ok(Some(top)) = orm
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT telegram_id::text AS tid, COUNT(*)::bigint AS n \
             FROM order_fraud_events \
             WHERE created_at > NOW() - INTERVAL '24 hours' \
               AND telegram_id IS NOT NULL \
             GROUP BY telegram_id ORDER BY n DESC LIMIT 1"
                .to_string(),
        ))
        .await
    {
        s.top_offender = top.try_get::<Option<String>>("", "tid").ok().flatten();
        s.top_offender_count = top.try_get("", "n").unwrap_or(0);
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::{
        audit_sweep_sql, cashback_pct_for_tier, finite_money, idempotency_sweep_sql, BikeDeal,
        BikeLine, DepositForm, OrderItem,
    };

    // ── cashback_pct_for_tier (Loop #10) ───────────────────────────────

    #[test]
    fn cashback_pct_uses_config_values_per_tier() {
        let config = serde_json::json!({
            "bronze_cashback_pct": 5.0,
            "silver_cashback_pct": 8.0,
            "gold_cashback_pct": 12.0,
            "progressive_cashback": [2.0, 3.0],
        });
        assert_eq!(cashback_pct_for_tier(&config, "none"), 2.0);
        assert_eq!(cashback_pct_for_tier(&config, "bronze"), 5.0);
        assert_eq!(cashback_pct_for_tier(&config, "silver"), 8.0);
        assert_eq!(cashback_pct_for_tier(&config, "gold"), 12.0);
    }

    #[test]
    fn cashback_pct_falls_back_to_defaults_on_missing_config() {
        let config = serde_json::json!({});
        assert_eq!(cashback_pct_for_tier(&config, "gold"), 10.0);
        assert_eq!(cashback_pct_for_tier(&config, "none"), 2.0);
    }

    #[test]
    fn cashback_pct_ignores_non_finite_config() {
        let config = serde_json::json!({ "gold_cashback_pct": -50.0 });
        assert_eq!(cashback_pct_for_tier(&config, "gold"), 10.0);
    }

    // ── audit_sweep_sql (cycle #67) ─────────────────────────────────────
    //
    // All three table-specific builders now delegate here. Pin the output
    // shape exactly so a "harmless cleanup" of the format! string can't
    // silently change what Postgres sees.

    #[test]
    fn audit_sweep_sql_produces_canonical_delete() {
        // Exact-equality (not `contains`) so byte-level format drift is
        // caught immediately.
        assert_eq!(
            audit_sweep_sql("any_table", "5 minutes"),
            "DELETE FROM any_table WHERE created_at < NOW() - INTERVAL '5 minutes'",
        );
    }

    #[test]
    fn audit_sweep_sql_interpolates_table_and_interval_in_order() {
        // Belt-and-braces: assert the table name appears BEFORE the
        // INTERVAL clause, so a `format!` arg swap (table↔interval) won't
        // ship a syntactically-valid but semantically-wrong query.
        let sql = audit_sweep_sql("foo", "1 days");
        let table_pos = sql.find("FROM foo").expect("table after FROM");
        let interval_pos = sql.find("INTERVAL").expect("INTERVAL clause");
        assert!(
            table_pos < interval_pos,
            "table must appear before INTERVAL: {}",
            sql
        );
    }

    #[test]
    fn audit_sweep_sql_holds_canonical_shape_across_combinations() {
        // Property-style test: for every (table, interval) drawn from a
        // matrix of realistic inputs, the output must satisfy four
        // invariants. Catches a "cleaned up the format!()" regression
        // that any single example test might miss because it pinned only
        // one specific combination.
        let tables = [
            "order_idempotency_keys",
            "order_fraud_events",
            "block_history",
            "audit_log", // hypothetical future table
            "t",         // shortest legal name
            "really_long_table_name_for_some_reason",
        ];
        let intervals = [
            "1 hours", "24 hours", "30 days", "90 days", "1 minute", "365 days",
        ];
        for &table in &tables {
            for &interval in &intervals {
                let sql = audit_sweep_sql(table, interval);
                // Invariant 1: starts with the canonical DELETE keyword.
                assert!(
                    sql.starts_with("DELETE FROM "),
                    "({}, {}) must start with DELETE FROM: {}",
                    table,
                    interval,
                    sql,
                );
                // Invariant 2: table name interpolated verbatim immediately
                // after FROM (catches table-name truncation / quoting bugs).
                assert!(
                    sql.contains(&format!("DELETE FROM {} WHERE", table)),
                    "({}, {}) must place table right after FROM: {}",
                    table,
                    interval,
                    sql,
                );
                // Invariant 3: interval literal interpolated verbatim inside
                // single quotes after INTERVAL.
                assert!(
                    sql.contains(&format!("INTERVAL '{}'", interval)),
                    "({}, {}) must interpolate interval in 'quotes': {}",
                    table,
                    interval,
                    sql,
                );
                // Invariant 4: standard time anchor — `NOW()` not CURRENT_TIMESTAMP
                // or anything else. Pin the wire-level choice.
                assert!(
                    sql.contains("created_at < NOW()"),
                    "({}, {}) must compare against NOW(): {}",
                    table,
                    interval,
                    sql,
                );
            }
        }
        // Coverage assertion: matrix size matches expectation so a
        // future shrink to e.g. one input doesn't silently weaken the test.
        assert_eq!(tables.len() * intervals.len(), 36);
    }

    #[test]
    fn idempotency_sweep_uses_correct_interval_literal() {
        let sql = idempotency_sweep_sql(24);
        assert!(sql.contains("DELETE FROM order_idempotency_keys"));
        assert!(sql.contains("INTERVAL '24 hours'"));
    }

    #[test]
    fn idempotency_sweep_accepts_arbitrary_retention() {
        // Cycle ships with 24h, but the builder must work for any value
        // (staging / tests may want a shorter window).
        let sql = idempotency_sweep_sql(1);
        assert!(sql.contains("INTERVAL '1 hours'"));
    }

    use super::fraud_events_sweep_sql;

    #[test]
    fn fraud_sweep_uses_correct_interval_literal() {
        let sql = fraud_events_sweep_sql(30);
        assert!(sql.contains("DELETE FROM order_fraud_events"));
        assert!(sql.contains("INTERVAL '30 days'"));
    }

    #[test]
    fn fraud_sweep_accepts_arbitrary_retention() {
        let sql = fraud_events_sweep_sql(7);
        assert!(sql.contains("INTERVAL '7 days'"));
    }

    use super::block_history_sweep_sql;

    #[test]
    fn block_history_sweep_uses_correct_interval_literal() {
        // Production ships with 90 — assert the literal lands intact so a
        // typo in the format!() arg can't reach prod.
        let sql = block_history_sweep_sql(90);
        assert!(sql.contains("DELETE FROM block_history"));
        assert!(sql.contains("INTERVAL '90 days'"));
    }

    #[test]
    fn block_history_sweep_accepts_arbitrary_retention() {
        // Different from production default, on purpose: catches "retention
        // hardcoded to 90 inside the builder" regressions.
        let sql = block_history_sweep_sql(14);
        assert!(sql.contains("INTERVAL '14 days'"));
    }

    // ── /engage orders panel (cycle #63 / B) ─────────────────────────────

    use super::{format_order_stats, OrderStats24h};

    #[test]
    fn order_stats_format_renders_all_fields() {
        let s = OrderStats24h {
            total_orders: 12,
            revenue: 6_900.0,
            unique_buyers: 7,
            avg_order_value: 575.0,
            pending_total: 2,
        };
        let out = format_order_stats(&s);
        // Each value lands in the rendered text exactly once.
        assert!(out.contains("12"));
        assert!(out.contains("6900"));
        assert!(out.contains("7"));
        assert!(out.contains("575"));
        assert!(out.contains("2"));
        // Headers stay so /engage layout is recognisable across cycles.
        assert!(out.contains("Orders (24h)"));
        assert!(out.contains("Revenue"));
        assert!(out.contains("Pending right now"));
    }

    // ── block_history action constants (cycle #64) ──────────────────────

    use super::{BLOCK_ACTION_AUTO, BLOCK_ACTION_UNBLOCK};

    #[test]
    fn block_action_constants_are_stable() {
        // These strings end up in `block_history.action` as a wire-level
        // enum. Renaming them silently would invalidate every existing row
        // — pin the values here so the contract breaks at compile/test
        // time instead.
        assert_eq!(BLOCK_ACTION_AUTO, "auto_block");
        assert_eq!(BLOCK_ACTION_UNBLOCK, "unblock");
    }

    #[test]
    fn order_stats_format_handles_empty_window() {
        // Cold start / quiet day: zero everything. Should still render
        // cleanly without `inf`/`NaN` (avg division-by-zero guard).
        let s = OrderStats24h::default();
        let out = format_order_stats(&s);
        assert!(!out.contains("inf"));
        assert!(!out.contains("NaN"));
        assert!(out.contains("0"));
    }

    // ── /engage blocks panel (cycle #68) ─────────────────────────────────

    use super::{format_block_stats, BlockStats24h};

    #[test]
    fn block_stats_format_renders_full_panel() {
        let s = BlockStats24h {
            auto_blocks: 3,
            unblocks: 1,
            top_actor_admin: Some("8420420131".into()),
            top_actor_count: 1,
        };
        let out = format_block_stats(&s);
        assert!(out.contains("Blocks (24h)"));
        assert!(out.contains("Auto-blocks") && out.contains("3"));
        assert!(out.contains("Manual unblocks") && out.contains("1"));
        // Admin actor id appears verbatim so admin can spot themselves.
        assert!(out.contains("8420420131"));
        // No `none` short-circuit on a populated window.
        assert!(!out.contains("none"));
    }

    #[test]
    fn block_stats_format_empty_window_says_none() {
        // Quiet day: zero auto-blocks AND zero unblocks → ✅ none. Symmetric
        // with the fraud panel — admin doesn't see a wall of zeros every
        // morning.
        let out = format_block_stats(&BlockStats24h::default());
        assert!(out.contains("Blocks (24h)"));
        assert!(out.contains("none"));
        // Make sure we're not silently leaking 0-counts behind the badge.
        assert!(!out.contains("Auto-blocks"));
    }

    #[test]
    fn block_stats_format_handles_missing_top_actor() {
        // 5 auto-blocks but 0 manual unblocks (no admin actor).
        // Panel must still render — auto_blocks count alone is meaningful.
        let s = BlockStats24h {
            auto_blocks: 5,
            unblocks: 0,
            top_actor_admin: None,
            top_actor_count: 0,
        };
        let out = format_block_stats(&s);
        assert!(out.contains("Blocks (24h)"));
        assert!(out.contains("5"));
        // Em-dash placeholder for missing admin keeps the line balanced.
        assert!(out.contains("—"));
    }

    // ── Auto-block threshold (cycle #60) ─────────────────────────────────

    use super::{should_auto_block_for_fraud, FRAUD_AUTO_BLOCK_THRESHOLD};

    #[test]
    fn auto_block_engages_exactly_at_threshold() {
        // Boundary: at-threshold MUST trigger a block. The /engage panel's
        // "Top offender: 3 events" line then matches the block decision.
        assert!(should_auto_block_for_fraud(FRAUD_AUTO_BLOCK_THRESHOLD));
    }

    #[test]
    fn auto_block_skips_just_below_threshold() {
        // Off-by-one guard for `>=` vs `>`. 2 events should NOT block — that
        // window still allows admin to investigate before user is locked out.
        assert!(!should_auto_block_for_fraud(FRAUD_AUTO_BLOCK_THRESHOLD - 1));
    }

    #[test]
    fn auto_block_engages_well_above_threshold() {
        assert!(should_auto_block_for_fraud(50));
    }

    #[test]
    fn auto_block_skips_zero_and_negative() {
        // Defensive: a corrupted COUNT() could theoretically come back 0 or
        // even negative (i64 overflow / type confusion). Neither must block.
        assert!(!should_auto_block_for_fraud(0));
        assert!(!should_auto_block_for_fraud(-1));
    }

    // ── /unblock parser (cycle #61) ──────────────────────────────────────

    use super::parse_unblock_arg;

    #[test]
    fn unblock_parser_accepts_plain_int() {
        assert_eq!(parse_unblock_arg("1234567890"), Some(1234567890));
    }

    #[test]
    fn unblock_parser_trims_whitespace() {
        // Telegram's command parser keeps leading/trailing whitespace on
        // String args — the parser must canonicalize.
        assert_eq!(parse_unblock_arg("  42  "), Some(42));
    }

    #[test]
    fn unblock_parser_rejects_empty() {
        assert_eq!(parse_unblock_arg(""), None);
        assert_eq!(parse_unblock_arg("   "), None);
    }

    #[test]
    fn unblock_parser_rejects_non_numeric() {
        assert_eq!(parse_unblock_arg("abc"), None);
        assert_eq!(parse_unblock_arg("12abc"), None);
        assert_eq!(parse_unblock_arg("1.5"), None);
    }

    // ── /blocks formatter (cycle #62) ────────────────────────────────────

    use super::{format_blocks_message, BlockedUserRow};

    #[test]
    fn blocks_format_empty_says_none() {
        let s = format_blocks_message(&[], 0);
        assert!(s.contains("Blocked users"));
        assert!(s.contains("none"));
        // Empty list shouldn't show the /unblock hint — there's nothing to act on.
        assert!(!s.contains("/unblock"));
    }

    #[test]
    fn blocks_format_single_user_shows_id_and_code() {
        let rows = vec![BlockedUserRow {
            telegram_id: 12345,
            last_fraud_at: None,
            last_fraud_code: Some("subtotal_mismatch".into()),
        }];
        let s = format_blocks_message(&rows, 1);
        assert!(s.contains("12345"));
        assert!(s.contains("subtotal_mismatch"));
        // Hint at the unblock command — admins forget the syntax.
        assert!(s.contains("/unblock"));
    }

    #[test]
    fn blocks_format_escapes_html_in_code() {
        // Defensive: `code` is server-written today but the format must be
        // safe if a future cycle lets it carry free-text reasons.
        let rows = vec![BlockedUserRow {
            telegram_id: 1,
            last_fraud_at: None,
            last_fraud_code: Some("<script>alert(1)</script>".into()),
        }];
        let s = format_blocks_message(&rows, 1);
        assert!(!s.contains("<script>"));
        assert!(s.contains("&lt;script&gt;"));
    }

    #[test]
    fn blocks_format_shows_truncation_hint_when_more_exist() {
        let rows = vec![BlockedUserRow {
            telegram_id: 1,
            last_fraud_at: None,
            last_fraud_code: None,
        }];
        let s = format_blocks_message(&rows, 75);
        // 75 total, 1 shown → 74 more
        assert!(s.contains("74"));
        assert!(s.contains("more"));
    }

    #[test]
    fn blocks_format_omits_truncation_when_count_matches() {
        let rows = vec![BlockedUserRow {
            telegram_id: 1,
            last_fraud_at: None,
            last_fraud_code: None,
        }];
        let s = format_blocks_message(&rows, 1);
        // When shown == total, no "more" hint.
        assert!(!s.contains("more"));
    }

    #[test]
    fn unblock_parser_rejects_zero_and_negative() {
        // Negative explicitly rejected so a future SQL rewrite like
        // `WHERE telegram_id > $1` can't unblock the whole base by accident
        // when admin types `/unblock -1`.
        assert_eq!(parse_unblock_arg("0"), None);
        assert_eq!(parse_unblock_arg("-42"), None);
    }

    #[test]
    fn test_order_item_serde_roundtrip() {
        let item = OrderItem {
            strain_id: Some("s1".into()),
            strain_name: Some("Indica".into()),
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: 2.5,
            unit_price: None,
            is_set: Some(false),
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
            fulfillment: None,
            bike: None,
        };
        let json = serde_json::to_value(&item).unwrap();
        let back: OrderItem = serde_json::from_value(json).unwrap();
        assert_eq!(back.strain_id, Some("s1".into()));
        assert_eq!(back.quantity, 2.5);
    }

    #[test]
    fn test_order_item_defaults() {
        let item = OrderItem {
            strain_id: None,
            strain_name: None,
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: 1.0,
            unit_price: None,
            is_set: None,
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
            fulfillment: None,
            bike: None,
        };
        let json = serde_json::to_value(&item).unwrap();
        assert!(json.get("strain_id").is_some());
    }

    // ─── Bike lines: rental, sale, deposit form (issues #14 / #15) ─────────

    fn d(y: i32, m: u32, day: u32) -> chrono::NaiveDate {
        chrono::NaiveDate::from_ymd_opt(y, m, day).expect("test date is valid")
    }

    fn rental_item(deal: BikeDeal) -> OrderItem {
        OrderItem {
            strain_id: None,
            strain_name: None,
            accessory_id: None,
            accessory_name: None,
            tea_id: None,
            tea_name: None,
            set_id: None,
            set_name: None,
            quantity: 2.0,
            unit_price: None,
            is_set: None,
            is_accessory: None,
            is_tea: None,
            is_tea_set: None,
            fulfillment: Some("delivery".into()),
            bike: Some(BikeLine {
                bike_key: "nmax-155".into(),
                bike_name: Some("Yamaha NMAX 155".into()),
                deal,
            }),
        }
    }

    #[test]
    fn rental_line_round_trips_with_every_field_the_shop_agreed() {
        let item = rental_item(BikeDeal::BikeRental {
            rental_start: d(2026, 9, 20),
            rental_end: d(2026, 9, 26),
            rate_thb_day: Some(337.0),
            deposit: Some(DepositForm::Money {
                amount: Some(3000.0),
                currency: Some("THB".into()),
                method: Some("cash THB".into()),
            }),
        });
        let json = serde_json::to_value(&item).expect("rental line serialises");
        // D8 vocabulary on the wire: the family key, the unit count, the two
        // dates, and `kind = bike_rental` — the same tag `cart_items` uses.
        assert_eq!(json["bike"]["bike_key"], "nmax-155");
        assert_eq!(json["quantity"], 2.0);
        assert_eq!(json["bike"]["deal"]["kind"], "bike_rental");
        assert_eq!(json["bike"]["deal"]["rental_start"], "2026-09-20");
        assert_eq!(json["bike"]["deal"]["rental_end"], "2026-09-26");
        assert_eq!(json["bike"]["deal"]["deposit"]["form"], "money");

        let back: OrderItem = serde_json::from_value(json).expect("rental line parses back");
        let bike = back.bike.expect("bike half survives the round trip");
        assert_eq!(bike.bike_key, "nmax-155");
        assert_eq!(bike.deal.span_days(), Some(7));
        match bike.deal {
            BikeDeal::BikeRental {
                rate_thb_day,
                deposit,
                ..
            } => {
                assert_eq!(rate_thb_day, Some(337.0));
                let deposit = deposit.expect("deposit survives");
                assert_eq!(deposit.agreed_amount(), Some(3000.0));
                assert!(!deposit.is_passport());
            }
            BikeDeal::BikeSale { .. } => panic!("a rental must not parse back as a sale"),
        }
    }

    #[test]
    fn a_passport_deposit_carries_no_amount_at_all() {
        // The seed's rule is "either money or the passport - never both". The
        // enum is what enforces it: there is no field on `Passport` to put an
        // amount in, so an order cannot claim a held passport AND held cash.
        let json = serde_json::to_value(DepositForm::Passport).expect("passport serialises");
        assert_eq!(json["form"], "passport");
        assert_eq!(json.as_object().map(|o| o.len()), Some(1));
        let back: DepositForm = serde_json::from_value(json).expect("passport parses back");
        assert!(back.is_passport());
        assert_eq!(back.agreed_amount(), None);
    }

    #[test]
    fn an_unagreed_deposit_is_absent_not_zero() {
        // "Not computed yet" must not render as "no deposit owed": the money
        // form with no figure yields None, and so does a NaN written by an
        // older client — never 0.0.
        let blank = DepositForm::Money {
            amount: None,
            currency: None,
            method: None,
        };
        assert_eq!(blank.agreed_amount(), None);
        let nan = DepositForm::Money {
            amount: Some(f64::NAN),
            currency: Some("THB".into()),
            method: None,
        };
        assert_eq!(nan.agreed_amount(), None);
        let zero = DepositForm::Money {
            amount: Some(0.0),
            currency: Some("THB".into()),
            method: None,
        };
        // A deliberate 0 is a fact the shop can state (a waived deposit); it
        // is only the ABSENT figure that must never become one.
        assert_eq!(zero.agreed_amount(), Some(0.0));
    }

    #[test]
    fn an_unquoted_rental_keeps_its_rate_absent() {
        // D11: when the door is silent the line carries no number at all —
        // not an average, not a "from" price, not the pre-discount tariff.
        let item = rental_item(BikeDeal::BikeRental {
            rental_start: d(2026, 9, 20),
            rental_end: d(2026, 9, 20),
            rate_thb_day: None,
            deposit: None,
        });
        let json = serde_json::to_value(&item).expect("serialises");
        assert!(json["bike"]["deal"]["rate_thb_day"].is_null());
        assert!(json["bike"]["deal"]["deposit"].is_null());
        let bike = item.bike.expect("bike half present");
        // Both ends inclusive: one calendar day is one day, not zero.
        assert_eq!(bike.deal.span_days(), Some(1));
    }

    #[test]
    fn sale_line_has_no_dates_and_rental_line_has_no_price() {
        let sale = BikeDeal::BikeSale {
            price_thb: Some(250_000.0),
        };
        assert_eq!(sale.rental_dates(), None);
        assert_eq!(sale.span_days(), None);
        let json = serde_json::to_value(&sale).expect("sale serialises");
        assert_eq!(json["kind"], "bike_sale");
        assert!(json.get("rental_start").is_none());

        let rental = BikeDeal::BikeRental {
            rental_start: d(2026, 12, 1),
            rental_end: d(2026, 12, 31),
            rate_thb_day: None,
            deposit: None,
        };
        assert_eq!(rental.span_days(), Some(31));
        let json = serde_json::to_value(&rental).expect("rental serialises");
        assert!(json.get("price_thb").is_none());
    }

    #[test]
    fn backwards_dates_report_no_span_rather_than_a_negative_one() {
        let backwards = BikeDeal::BikeRental {
            rental_start: d(2026, 9, 26),
            rental_end: d(2026, 9, 20),
            rate_thb_day: None,
            deposit: None,
        };
        assert_eq!(backwards.span_days(), None);
    }

    #[test]
    fn a_legacy_order_item_still_parses_without_a_bike_half() {
        // Orders placed before the rebrand are still in `items` JSONB, and
        // `api/reviews.rs` deserialises them. A missing `bike` key must be
        // None, not a parse error.
        let legacy = serde_json::json!({
            "strain_id": "s1",
            "strain_name": "Indica",
            "quantity": 2.5,
            "is_set": false
        });
        let item: OrderItem = serde_json::from_value(legacy).expect("legacy line parses");
        assert!(item.bike.is_none());
        assert_eq!(item.quantity, 2.5);
    }

    #[test]
    fn finite_money_keeps_absent_absent() {
        assert_eq!(finite_money(None), None);
        assert_eq!(finite_money(Some(f64::NAN)), None);
        assert_eq!(finite_money(Some(f64::INFINITY)), None);
        assert_eq!(finite_money(Some(-1.0)), None);
        assert_eq!(finite_money(Some(449.0)), Some(449.0));
    }
}
