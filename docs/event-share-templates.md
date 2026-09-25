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

`p_event_{EVENT_ID}`. Since 2026-09-24 such a link opens the bike catalog: `/events` is one of the
compatibility paths (`LEGACY_ROUTE_ALIASES` in `specs/turbobaby/deeplink.t27`).

## Product links

`{PREFIX}_{PRODUCT_ID}`, where `{PREFIX}` is one of the prefixes `Kind::prefix` in
`src/trios/deeplink.rs` returns. Every kind there belongs to the previous shop, its catalogue or its
events, and every one opens the bike catalog today: `destination` in the same file sends each kind
to `/menu`, which is the catalog under its historical route name, or to a compatibility path. There
is no deep link to a single bike yet.
