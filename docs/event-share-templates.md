# Share templates for Telegram posts

*Rewritten 2026-09-26.* This file used to give snippets for posting events and products to the bot
channel or the broadcast list. Neither may be posted any more: the bot rents bikes, in Phuket only
(DECISIONS.md, "D19 addendum — rental only, Phuket only, decided 2026-09-24"), and nothing of the
previous shop's catalogue may appear anywhere (the owner's answer 12 of 2026-09-25, recorded as
`OWNER_RULING_2026_09_25_AT` in `specs/turbobaby/legacy_retirement.t27`). What follows describes the
links that are already in chats, so that nobody takes one for a live template.

## The shape of a link

The Mini App (`src/ui/share.rs`) and the server's share card (`deep_link` in `src/api/share.rs`)
mint `https://t.me/{BOT_USERNAME}?start={PAYLOAD}`. Posts made from this file's old snippets carry
`?startapp=` instead, which Telegram opens as the bot chat unless the bot has a Main Mini App
configured (the comment on `deep_link` says why the app moved off it). Either way the link survives
native Telegram forwarding.

## Event links

`p_event_{EVENT_ID}`. The event is one of the kinds `Kind::prefix` in `src/trios/deeplink.rs`
names, so such a link is read and lands like a product link, below.

## Product links

`{PREFIX}_{PRODUCT_ID}`, where `{PREFIX}` is one of the prefixes `Kind::prefix` in
`src/trios/deeplink.rs` returns. Every kind there belongs to the previous shop, its catalogue or its
events. There is no deep link to a single bike yet.

## Where an old link lands

A `?start=` link opens the bot chat, where the `/start` arm of `handle_command`
(`src/bot/commands.rs`) answers a payload `is_miniapp_start_payload` (`src/bot/mod.rs`) accepts with
a Mini App button that carries it; the app gets the payload when the customer taps that button. An
old `?startapp=` link reaches the app only when the bot has a Main Mini App configured; otherwise it
opens the bot chat and nothing more.

When the app has the payload, `HomeScreen` (`src/ui/screens/home_screen.rs`) sends the customer
where `ProductKind::route` in `src/ui/share.rs` says. That sends every kind, events included, to
`/menu`, the bike catalog under its historical route name, and has done so since 2026-09-16, the
date the function's own comment gives. `SHIPPED_LANDING_SITE` in `specs/turbobaby/deeplink.t27`
names it.

`destination` in `src/trios/deeplink.rs` is a different table, with a screen per kind, and it does
not decide where a link lands: only its own test module calls it
(`DESTINATION_TABLE_HAS_A_PRODUCTION_CALLER = false` in the same contract).
