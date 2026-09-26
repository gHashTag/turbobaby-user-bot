# Orchestrator decisions

Decisions that span more than one subsystem, with the evidence that forced each one.
Recorded here because a worker touching one file cannot make them and must not guess.

Parent epic: #1

---

## D1 — Bike migrations start at 077, contiguous

`migration_numbers_are_sequential_and_unique` (`src/db/mod.rs:1021-1047`) asserts the
numeric prefixes form a **gap-free** run from 001. A reserved gap is therefore not an
option. Highest top-level migration is `076_promo_broadcast.sql`, so bikes take 077 onward.

`migrations/wip/077_quest_progress.sql` already claims 077, but it sits in a
subdirectory and `migration_files()` uses a non-recursive `read_dir`, so it is invisible
to both fitness tests and does not collide today. **It must be renumbered when it is
promoted**, not the bike migrations.

## D2 — Forward migrations only; never edit 001-076

`run_migrations()` backfills `_schema_migrations` for an established database
(`src/db/mod.rs:488-520`), so history is replayed only on fresh installs. Editing 001 or
008 in place would therefore change fresh installs only and leave any established
database holding the cannabis catalog for ever. Forward migrations are correct in both
worlds.

## D3 — Repoint the established-database probe away from `strains`

`src/db/mod.rs:495` reads:

```sql
SELECT to_regclass('public.strains') IS NOT NULL AS est
```

This is the highest-consequence single line in the change. If `strains` is dropped
while this line stands, the probe returns false on a populated production database, the
backfill branch is skipped, the runner concludes the database is FRESH, and it attempts
to apply 001-076 against live data.

Repoint to `public.orders` — created in `001_initial.sql:57`, and orders survive the
rebrand because a rental is still an order.

## D4 — Rename the crate

`woody-weed-bot` is the most visible remaining cannabis string: it is in the binary name.

| from | to |
| --- | --- |
| `woody-weed-bot` (package) | `turbobaby-bot` |
| `woody_weed_bot` (lib) | `turbobaby_bot` |
| `woody-weed-bot-server` (bin) | `turbobaby-bot-server` |

Touches `Cargo.toml`, the doctest at `src/db/macros.rs:14`, and the import root in
~40 `tests/*.rs` files. Mechanical, but atomic — it cannot be split across workers.

## D5 — The garden mechanic is removed, not repointed

`src/trios/garden.rs`, the six-catalog EXISTS gate at `src/db/orders.rs:379-386`, and
7 integration tests exist to plant a virtual seed when someone buys cannabis. A
motorbike rental has no botanical analogue, and the user's replacement game is the biker
ride game (#12-#15). Drop the tables in a forward migration, delete the Rust and its
tests. Migrations 004/026/036/037/041 stay on disk untouched per D2.

## D6 — `lab_certificates` becomes `bike_service_records`

This one has a real counterpart rather than a forced analogy. The fleet register tracks
oil and gear service, ABS and air filter; the ops sheet tracks `current_km`,
`last_service_km`, `interval_km`, `next_km` and an `overdue` status per unit.

**Admin-only.** Service state is not surfaced in the public catalog: two units currently
read `overdue`, and a public "serviced" badge we cannot keep accurate is exactly the kind
of number the data-honesty rule forbids.

## D7 — `fulfillment`, two l's

`OrderItem` at `src/db/orders.rs:502-525` already spells it `fulfillment`, pinned by
tests at 1667 and 1692. Matching the shipped spelling avoids a serde split. The spec's
`fulfilment` is overruled by the code.

## D8 — Cart rental lines

`migrations/059_cart_tables.sql:24` declares `UNIQUE (cart_id, kind, catalog_id)`, which
would stop a customer renting two bikes of the same family for different dates.

- `kind` = `bike_rental`
- `catalog_id` = the **family** key, not a unit — a customer books a model, the shop
  assigns the unit
- `quantity` = how many units of that family
- `rental_start` / `rental_end` on the line
- the constraint widens to `UNIQUE (cart_id, kind, catalog_id, rental_start, rental_end)`

### D8 amendment — two things the decision above missed

Recorded because D8 as first written would have shipped a defect.

**1. There is a second constraint, and it is unnamed.** `059_cart_tables.sql:16` declares
`kind TEXT NOT NULL CHECK (kind IN ('strain', 'set', 'accessory', 'tea'))` **inline**, so its
name is whatever Postgres generated. `bike_rental` violates it and every add-to-cart would fail
at the database with a `23514`. It must be dropped by looking the constraint up by the column
it constrains — a guessed name aborts the migration on any database ever patched by hand — and
the four cannabis kinds must stay in the replacement list, because `ADD CONSTRAINT` validates
existing rows and would abort on a live cart still holding one.

**2. Widening the UNIQUE constraint silently un-does what 059 was written to prevent.**
Postgres treats NULLs in a UNIQUE constraint as **distinct**, so once the dates join the key,
`(cart, 'accessory', id, NULL, NULL)` no longer conflicts with itself and every legacy kind can
be added to a cart twice. The widened constraint therefore needs a companion partial unique
index on `(cart_id, kind, catalog_id) WHERE rental_start IS NULL AND rental_end IS NULL`.
`UNIQUE NULLS NOT DISTINCT` expresses this in one clause but requires Postgres 15 and nothing
in this repository pins the server version, so the portable form is used.

Both are implemented in `migrations/081_cart_rental_lines.sql`.

## D9 — Money fields are nullable, and absent stays absent

Three separate constructs in this tree turn an unknown number into a confident zero:

| construct | location | what it does |
| --- | --- | --- |
| the `clamp` closure | `src/db/strains.rs:123-129, 165-171` | NaN/Inf becomes `0.0` — a price of 0 reads as FREE |
| `NOT NULL DEFAULT 0` | `migrations/008_strains_full_seed.sql:21` | an unpriced row is priced at zero |
| `try_get_warn!` | `src/db/macros.rs:28-44` | a renamed column yields the default and only a `tracing::warn` |

`rate_day_thb`, `deposit_thb` and `monthly_low_season_thb` are therefore **NULLABLE**,
mapped as `Option<f64>` filtered with `.is_finite()`, and routed through **none** of the
three constructs above. Absent renders as a dash — never 0, never an average, never a
"from" price.

`try_get_warn!` is the single most dangerous construct during this rename: it is
fail-open by design, so a half-migrated column renders as `0` / `false` / `""` with a log
line as the only evidence. Grep every call site before renaming any column it reads.

### A fourth construct, found while widening the cart

`059_cart_tables.sql:19` declares `unit_price DOUBLE PRECISION NOT NULL CHECK (unit_price >= 0)`.
A cart line **cannot** hold an absent price: the column rejects NULL, and the only value that
satisfies the constraint without inventing a number is `0`, which renders as free.

The resolution is not a schema change. A family with no published rate is **not addable to a
cart** — the call refuses, and the catalog says a human quotes this price (D11). That is the
honest behaviour anyway: a machine whose price nobody has set is a machine we cannot yet take
money for. CLICK 125 is the live instance, and it is already closed to new rentals by D12.

What this forbids is the tempting shortcut of inserting `0.0` to get past the constraint and
"fixing up the display later". The zero would then be a real row in a real cart.

## D10 — `bikes` and `bike_units` join `CRITICAL_COLUMNS`

`CRITICAL_COLUMNS` (`src/db/mod.rs:340-353`) currently guards only `accessory_sets`,
`tea_sets` and `sets`. Without an entry, a bike table missing a column produces no
startup warning at all and the first symptom is a `try_get_warn!` default served to a
customer.

## D11 — Price authority: the door, never the file

The owner's ruling of 2026-09-12. Full text and the arithmetic that reconciles the three
competing price sets is in [`data/fleet_seed.json`](data/fleet_seed.json).

- The client-facing price is the number the **door** (the owner's live sheet) returns. It
  is never computed from a file when the door can be asked.
- **If the door is silent the bot must not compute.** It says a human quotes this price
  and emits no number at all — not exact, not "from", not an average, not a range.
  Invention and silence are forbidden equally.
- Every priced answer logs any divergence of >= 1 baht between door and file, and the
  logging never blocks or delays the answer.
- One measured door call took 21.4 s, so the timeout path is the common path, not the
  edge case.

The published tariff in the seed is **pre**-class-discount (scooters -25%, motorcycles
-15%). Rendering it without applying the discount overstates every scooter by 33%.

## D12 — CLICK 125 is not offered

Owner, 2026-09-12: not rented for now. `KB_faq` says the same independently and redirects
the request to PCX 150 / ADV 150 / NMAX 155. One unit is still out on a contract that
predates the decision; the family is closed to **new** rentals and that existing rental
is not cancelled.

## D13 — Two locales: ru, en. No Thai

`KB_faq`: clients are handled only by Russian-speaking managers; the Thai team does not
deal with clients. Russian is primary.

## D14 — What must never enter this repository

The internal sources carry, and the public repo must never contain in any form:

- renter names, Telegram handles, phone numbers
- outstanding debts
- physical key codes
- TAX and insurance dates
- per-unit purchase cost, and any sale price derived from it
- plate numbers
- the free-text personal notes in the rental-history sheet

Only aggregate, de-identified signal is used: published tariffs, deposit tiers, term
patterns, unit counts, model years, colours, and the scooter/motorcycle class split.

## D15 — One implementation of the price arithmetic, and it is shared

Found by compiling, not by reading: `cargo check --features backend` reports
`unused import: bikes::*` and then 14 more never-used items from `src/db/bikes.rs`. The
694-line typed data-access layer for the bike domain is reached by **nothing**.
`src/api/bikes.rs` — the code that actually answers customers — issues its own raw
`Statement`s and carries its own `publishable_money` / `publishable_fraction`.

Both are correct. That is precisely the problem: D9 and D11 are now asserted in three
separate places, each with its own tests and its own chance to drift.

| copy | location | what it holds |
| --- | --- | --- |
| server, typed | `src/db/bikes.rs` | `round_half_up_baht`, `apply_class_discount`, `usable_discount`, `discount_for_class` — all unreachable |
| server, HTTP | `src/api/bikes.rs` | `publishable_money`, `publishable_fraction`; deliberately computes **no** client rate (D11) |
| client, WASM | `src/ui/screens/catalog_screen.rs` | `discount_fraction` plus its own half-up client rate |

The rule, and it binds every worker touching a price:

- The arithmetic — class discount, term discount, half-up rounding to the baht — lives
  **once**, in `src/trios/pricing.rs`. That module is already the shared, dependency-free
  home both the Axum server and the Dioxus/WASM client compile against; `src/db/bikes.rs`
  cannot be it, because it pulls in `sea_orm` and the UI cannot link that.
- The **honesty filters** (absent stays absent; NaN, infinity and negatives become absent,
  never `0.0`) live there too, as one pair of functions, not three.
- A `.t27` spec pins the arithmetic, so the two language runtimes are checked against one
  written contract rather than against each other.

Why this is worth a decision rather than a cleanup ticket: the recurring defect in this
kind of tree is the hand-copied computation. Three copies of a *price* is the worst place
to have it — D11 makes price provenance the most sensitive rule in the repository, and a
drift between the number the API serves and the number the catalog screen renders is
invisible to every test that only ever exercises one of them.

## D16 — A gate whose input can reach zero must pin a floor

`src/api/mod.rs::route_wiring_tests::every_routes_fn_is_declared_and_merged` was added on
**2026-06-02** to catch "added a routes file but forgot to wire it". It finds its subjects
by matching the source text `pub fn routes(`.

On **2026-06-06**, `baf77cc` ("refactor(api): tighten 59 pub items to pub(crate)") rewrote
every one of those declarations to `pub(crate) fn routes(`. The needle matched nothing from
that day on. `modules_with_pub_fn_routes()` returned an empty vector, both assertions ran
against empty vectors, and the test passed — for three months, while checking nothing.
Measured today: **0 of 19** route modules were visible to it.

The gate had been made blind by exactly the bug class it existed to catch, and nothing
reported it, because a passing test and a vacuous test look identical from the outside.

Two fixes, both landed:

1. The recogniser accepts any visibility on the declaration.
2. A companion test, `routes_fns_are_found`, asserts the subject list holds at least 10
   modules. The floor is well below the real 19 so that deleting a module is not a failure;
   only *losing the ability to see modules* is.

The general rule for this repository: **every source-walking gate must assert that it found
something.** `orphan_table_tests` and `entity_wiring_tests` already do (`assert!(!
tables.is_empty())`, `assert!(!entities.is_empty())`) — this one did not, and it is the one
that broke. A count is not a decision, but a count of zero is always a bug.

---

## D17 — Where an issue and the seed disagree, the seed wins, and the disagreement is recorded

The six spec issues (#16–#21) were written before `data/fleet_seed.json` was reconciled.
Three of them state a premise the seed refutes. Writing the spec the issue asked for would
have published a false claim in a file whose whole purpose is to be citable, so in each
case the spec was written against the measurement and the issue's figure was recorded
inside the spec's `WHY` paragraph rather than quietly dropped.

**#18 asked for a function that cannot exist.** It specifies
`deposit_for(engine_cc, class)` and tiers of "3,000 / 5,000 / 10,000–20,000". Measured:

| family | class | `displacement_cc` | `deposit_thb` |
| --- | --- | --- | --- |
| `xmax-300` | scooter | 300 | 5000 |
| `xmax-300-new` | scooter | 300 | **7000** |

Two families agreeing on both arguments and disagreeing on the result. A function of
`(class, displacement_cc)` cannot return two values, so the signature the issue asks for
is not implementable against this fleet — not difficult, *impossible*. The real distinct
deposits are 3,000 / 5,000 / 7,000 / 15,000 / 20,000 / 25,000; **10,000 never appears.**
The deposit is therefore modelled as a per-family published attribute with a closed set of
legal tier values, and the totality invariant runs over the family-key domain — every
offered family maps to exactly one tier — not over a displacement band that provably does
not exist. Also, the field is `displacement_cc`; `engine_cc` appears nowhere in the seed.

**#21's count is off by four.** It says "five of thirteen families have no measured rate".
Exactly **one** of fourteen in-stock families lacks a rate — `click-125`, whose
`base_rate_thb_day` is null and whose `offered` is `false`, so it is not bookable either.
The seed's own `totals.families_without_published_rate` is `1`. The larger and more
interesting absence is the **mirror** defect the issue did not mention: **seven**
price-list-only families carry a published rate and **zero** stock. Both failure modes are
real and they point opposite ways — stock without a rate (1), a rate without stock (7) —
and the spec that exists to forbid "an absent measurement rendered as a confident number"
is the wrong place to carry an unreproducible number of its own.

**#17's bands are not disjoint.** The published term discounts are ranges, not multipliers:
week `[0.06, 0.15]`, two weeks `[0.15, 0.25]`, month `[0.35, 0.5]`. Consecutive bands
**touch at 0.15**, so "a longer term carries a strictly larger discount" is false exactly
at that edge. Monotonicity is stated on a well-defined representative of each band instead,
and the touching edge is named in the header so a later reader does not file it as a bug in
the tariff. The term ladder itself checks out: `observed_terms_days` is
`[7, 30, 90, 120, 150, 180]`, which is the issue's "7 days, 1, 3, 4, 5, 6 months".

The rule: **an issue is a brief, not a measurement.** A spec may not inherit a number from
the prose that commissioned it. Where they differ the spec cites the seed, states the
issue's figure, and says which one was reproducible — because the alternative is a
published number whose only provenance is that someone typed it into a ticket, which is
the defect [D9](#d9) and [D11](#d11) exist to forbid, arriving through the front door.

## D18 — The market is a declared profile, not ambient Thailand

The owner's standing instruction (2026-09-13) is that this agent runs on **any market,
country and currency**. Measured before this decision, the deployment's market was ambient
in the code rather than declared anywhere: the money formatter hardcoded the baht glyph,
the comma group separator and whole-baht display (`format_baht`), the i18n templates baked
the glyph into customer-facing sentences, the shop timezone was a hardcoded +7 in five
files (measured 2026-09-13: `api/happy_hour.rs`, `api/share.rs`, `promo.rs`, `api/orders.rs`,
`ui/screens/events_screen.rs`), and the checkout phone default was +66. No file stated a
market; several stated facts of one. Under that shape, a second market could only arrive
as a second fork of the formatter — exactly the duplication [D15](#d15) removed for price
arithmetic.

A market is therefore a **profile**: thirteen named fields (`country_code` through
`default_calling_code`), owned as follows —

- the **contract** (field set, bounds, refusals) is `specs/turbobaby/market_profile.t27`
  (`turbobaby/market`);
- the **deployment instance** is the `market` block in `data/fleet_seed.json`, and
  `scripts/verify_fleet_seed.py` fails if the block disagrees with the spec's `TH_*`
  consts, carries a field outside the contract, or violates a bound;
- the **formatter instance** is `MarketMoneyFormat` / `THB_MARKET` in
  `src/trios/pricing.rs`; `format_baht` is its named shortcut so no call site names a
  currency.

Three copies is one more than [D15](#d15) would like, and each copy is pinned to the
others: the Rust tests pin the formatter to the THB profile, the seed verifier pins the
block to the spec, and the spec's own tests run every rule over a **second, proof profile**
(EUR — suffix symbol, two minor digits, swapped separators, DST-true) that exists solely so
a contract shaped like one currency cannot pass as a contract for all of them. No shop
exists at the proof profile; every value in it is a public ISO/IANA fact.

Two refusals ride with the profile. A market may *display* fewer minor digits than its
ISO exponent (Thailand: satang exist, shop prices are whole baht) but never more —
invented precision is a lie in the other direction. And no exchange rate enters this
repository without a source ([D11](#d11)): the deposit's foreign-currency equivalents are
agreed by a human at the door, so the spec's rate table is empty and its length is pinned.
What remains hardcoded (the glyph inside i18n templates, the +7 literals in five files, the
+66 default) is unwired debt tracked in #35, not a second market's problem: the profile it
must read now exists.

## D19 — The Woody WeedPecker heritage is hidden, not deleted

Owner mandate, 2026-09-14: «из WOODY идут события рассылки, а надо чтобы у каждого
своя!! сейчас все путается!», «раздели и почини всё», «замени все названия woody наша
turbobaby». This database forked from the woody bot carrying its live catalogue, and
two shops ran through one bot: 30 Woody WeedPecker events in the calendar (DJ sets,
cannabis sommelier nights) with the promo sweep drafting posts about them and the
24-hour reminder loop mailing customers; 70 accessories, 30 teas and 8 sets on sale;
the menu button and share links pointing at the other bot (`WEB_APP_URL` unset fell
back to the woody production URL; `BOT_USERNAME` defaulted to `Woody_WeedPecker_bot`).

Migration **085** flips visibility flags — `events.is_public = FALSE`,
`*.is_available = FALSE` — and rebrands the pickup zone. No row is deleted and no
table is dropped: old orders and bookings still resolve their item names, and the
separation is reversible by flipping the flags back. The screens and endpoints stay
so a future TurboBaby event (a ride day, a community meet) starts from a clean
public slate rather than from a code resurrection.

What this decision does *not* settle: the fate of the legacy screens (`/sets`,
`/tea`, `/sommelier`, `/accessories`, `/garden`, the games) and the Phangan delivery
zones (Thong Sala, Haad Rin, …) in a shop that README places in Kamala, Phuket.
Those are product calls; the data waits, hidden, until the owner makes them.

The string rebrand that rides with it (`Woody Weed` → `TurboBaby` in payment QR
text, pickup names, share/referral texts, profile contacts, i18n) is cosmetic and
total: no customer-visible surface keeps the old shop's name. Internal keys keep
their names — `woody_last_*` localStorage entries hold customers' saved checkout
drafts, and renaming the key would silently discard their data, which is a worse
bug than an invisible legacy word.

## D20 — The logo is the owner's own channel avatar

Owner mandate, 2026-09-14: «ты логотип найди и замени на реальные данные» (with the
«Актуальная Система Учета» Drive folder as the pointer). The folder holds no logo
file — it is sheets, agreements and passports — and the Wix site turbophuket.com has
a text-only header (its sole uploaded media is the og:image hero photo). The one
brand image the owner actually publishes is the avatar of his manager channel
t.me/turbophuket («TURBOBABY manager аренда мото байки»), fetched at its full
320×320 and installed as `assets/logo.jpg` (the bot itself has no avatar set).

The cannabis-era trust block on the home screen (GACP certification, medical-use
warning, 20+ badge) was retargeted to verifiable rental facts: legal entity and
location, Thai helmet law, license requirement. No invented product claims
(insurance, deposit terms) — those stay the owner's to make. The cannabis shop
address in checkout (Koh Phangan pickup) became TurboBaby / Kamala, matching README
and the migration 085 zone rename.


## D12 amendment (2026-09-24) — the CLICK 125 redirect names only the fleet

Kept at the end of this file rather than under D12, so that none of the line citations into
this file (several contracts cite it by line) moves.

The redirect as recorded in D12 names two machines the shop does not have. PCX 150 and
ADV 150 are tariff rows with zero units (`data/fleet_seed.json`, `price_list_only`), and the
owner's later rules forbid naming a model outside the fleet "not even as a replacement"
(brain `knowledge_base`, the «нет в парке» note of 05.07.2026 and the client-bot rules of
06.07.2026; `business_rules`, «Клиентам называть только модели реального парка»). The
customer screens showed all three until this date. They now offer NMAX 155 alone:
`specs/turbobaby/availability.t27` records the shown list as `CLICK_125_REDIRECT_LABELS_SHOWN`,
gate 3 binds it to the screen's constant, and `tests/catalog_honesty_wiring.rs` checks every
label against the seed. The brain itself still carries the three-name redirect in two places
(`knowledge_base`, the CLICK line of the tariff notes and of the «что НЕ сдаём» list); that is
the owner's text to change, not this repository's.

## D19 addendum — the delivery zones, decided 2026-09-24

D19 left the Phangan delivery zones waiting for the owner, who decided them in chat on
2026-09-24. Migration **087** deactivates the four Koh Phangan rows — deactivated, not
deleted, for the reason 071 gave: placed orders reference `delivery_zone_id` — and inserts
TurboBaby's eighteen Phuket zones at the owner's fees: the sixteen of the owner's live
delivery sheet plus Kathu and Nai Harn from the brain's delivery rule. No zone carries an
ETA, because none is published; the shop promises a window. The pickup row stays at fee 0.
The table, its sources and the two rules it does not model (the sheet's out-of-belt price
and the 17:30 cut-off) are in `specs/turbobaby/delivery_terms.t27`. The legacy screens are
still open.

## D19 addendum — rental only, Phuket only, decided 2026-09-24

Owner decision in chat, 2026-09-24, verbatim: «Ареда [sic] только пхукет» — rental only, Phuket
only. Earlier the same week the owner kept the referral programme, loyalty and the ride game, and
put events and bike sales on the retirement plan. So a customer may only rent a bike, and only in
Phuket. Kept at the end of this file, like the two entries above, so that no line citation into it
moves.

This reverses two earlier choices and says so rather than editing them. D19 kept the event screens
and endpoints for "a future TurboBaby event" on a clean public slate; they no longer wait for one.
The customer buy-out is not D19's: issue #11 asked for sales, and migration 086 added the
`for_sale` flag the bike detail's sale block read; it is withdrawn from customers too. Nothing is
deleted — a placed order may carry a sale line and a booking may name an event — so the retirement
follows this repository's precedent: data kept, surfaces changed, the change named.

* **Events.** `/events`, `/events/:id` and `/my-bookings` stay declared and render the catalog,
  as `/sets` and `/sommelier` do; the events screens stay compiled and unmounted. The admin API
  can no longer store an event as public (`EVENTS_PUBLISHABLE = false` in `src/api/events.rs`),
  and migration **088** hides, in 085's shape, any row still public — one made public between 085
  and this change, which nobody has read production to rule out. Every customer read and the promo
  sweeper's event scans filter that flag, so once 088 has run the calendar answers empty; until
  it runs, such a row would still be served. The event share card answers not found, as the
  retired strain kind does. Kept: the customer's bookings list and self-cancel over HTTP and the
  admin cancel (the two Stars refund paths), the guest list, the 24-hour reminder to a seat
  already held, the admin Events tab, and every event row; none of these filters the flag.
* **Bike sales.** The bike detail overlay renders no buy-out block. `GET /api/bikes` and
  `GET /api/bikes/:key` serve `for_sale: false` and `sale_price_thb: null` whatever the row
  holds; the admin list keeps the stored values and no row is written. `POST /api/orders`
  refuses a new `bike_sale` line with 422; placed sale orders still read. The admin sale
  inputs stay, as the admin tabs of the Woody catalogue stayed under 085.
* **Copy.** The catalog subtitle loses its buying half, word for word: «Аренда и выкуп на
  Пхукете» → «Аренда на Пхукете», "Rent or buy on Phuket" → "Rent on Phuket". The served API
  description, the README and the agent card lose "and sales" the same way. No sentence was
  written new.
* **Phuket.** No customer sentence names another place: `tests/rental_only_wiring.rs` reads the
  Mini App's translations, the bot's locale file and the screens' own strings. The Koh Phangan
  zones were deactivated by 087, but a returning customer's checkout could still submit the zone
  id it had saved before, which the server refuses with 422. The checkout now sends a saved zone
  only while the served list holds it (`served_zone_id` in `src/trios/store.rs`); otherwise it
  sends none, as a customer who never chose does.
* **Open, for the owner.** `GET /api/loyalty/tiers` is public and serves each tier's stored perks;
  the seed (`migrations/009_loyalty_tiers_seed.sql`) gives the top tier «VIP мероприятия», an
  events promise, and other tiers perks from the old shop. Only the admin screen renders the list,
  and production may hold other values. Dropping the events perk would be a deletion, not new copy,
  and waits for the owner. So does whether "rental only" also retires the legacy paths that sell
  goods (`/accessories`, `/tea`), which `legacy_retirement.t27` records as open.

The contracts carry the details: `specs/turbobaby/legacy_retirement.t27` records the ruling and
classifies each surface on its own inputs (events: hidden data under an unreachable surface;
bike sales: no row hidden, the offer masked on read — neither a completed retirement),
`events_booking.t27` owns what the events surface still answers and describes 088,
`catalog_api.t27`, `commerce.t27` and `deeplink.t27` correct the facts the ruling changed,
`schema_provenance.t27` carries the migration census 088 moved, and `tests/rental_only_wiring.rs`
guards the source.

## D19 addendum — the legacy paths, decided 2026-09-25

The owner answered a numbered list of questions in chat on 2026-09-25; answer 7, verbatim: «да».
It settles the question the entry above left open. Kept at the end of this file, like the entries
above, so that no line citation into it moves.

* **Repointed.** The previous shop's client paths that are not a rental — its goods
  (`/accessories`, `/tea`), its game (`/game`) and its hunts (`/treasure-hunt`, `/ar-hunt`,
  `/location-quest`) — stay declared and render the catalog, as `/sets` and the events paths do.
  `/tech-tree`, a development tech tree, is repointed with them: the answer does not name it, and
  counting it among the old non-rental addresses is this repository's reading, recorded as such in
  `legacy_retirement.t27` so the owner can overrule it. Their screens stay compiled and unmounted;
  no row is touched and no table dropped.
* **Kept.** The ride game (`/ride`), the referral programme and loyalty. The quest a customer opens
  from the profile (`/quest/:id`) stays, without the line «Минимальная покупка: 300 бат» — the
  previous shop's minimum purchase. The key stays translated and no screen renders it. No sentence
  was written new; a line was removed.
* **Not decided.** The HTTP routes behind the repointed paths — the legacy catalog, quest and
  tech-tree routers — still answer; the quest router is also what the kept quest scans through.

`specs/turbobaby/legacy_retirement.t27` records the answer path by path, `deeplink.t27` the
compatibility set it grew (six paths to thirteen), and `tests/rental_only_wiring.rs` guards the
source.

## D12 amendment (2026-09-25) — NMAX 155 is offered instead of CLICK 125, for now

Kept at the end of this file, like the amendment of 2026-09-24, so that no line citation into it
moves.

The owner answered the redirect question on 2026-09-25, in answer 10 of a numbered list, verbatim:
«Пока предлагаем нмакс 155 пока» — for now, NMAX 155 is what is offered instead of CLICK 125, and
not PCX 150 or ADV 150. The answer says «пока» twice, so the decision is recorded as provisional:
a later answer may change it, and the repository says so wherever it records the list.

* **The seed.** `data/fleet_seed.json`, `pricing_policy.not_offered.click-125.offer_instead`, held
  `["pcx-150", "adv-150", "nmax-155"]`, the KB_faq redirect D12 quotes, and holds `["nmax-155"]`.
  Beside it: `offer_instead_source` names the dated source `sources.owner_decision_2026_09_25`,
  `offer_instead_provisional` is `true`, and the old list is kept as
  `offer_instead_before_2026_09_25`. The file kept its 350 lines, so no line citation into it
  moved; `order_money.t27` re-measures its size (20161 bytes).
* **The contract.** `specs/turbobaby/availability.t27` `CLICK_125_REDIRECTS` holds the decision,
  with `CLICK_125_REDIRECTS_ARE_PROVISIONAL` and `CLICK_125_REDIRECTS_DECIDED_ON`; the three-name
  list is kept as `CLICK_125_REDIRECTS_BEFORE_2026_09_25`, so the correction of 2026-09-24 still
  executes against it. Gate 3 binds the list and the provisional flag to the seed.
* **What a customer sees does not change.** The screens' source has named NMAX 155 alone since the
  amendment of 2026-09-24 (`CLICK_125_ALTERNATIVES` in `src/ui/screens/catalog_screen.rs`); the
  decision confirms that list. `tests/catalog_honesty_wiring.rs` now also checks the screen's
  label against the seed's `offer_instead`. No customer sentence was written or changed.
* **The seed gate.** `scripts/verify_fleet_seed.py` now reads `pricing_policy.not_offered`: every
  key a closed family offers must be an offered family with a unit the SQL seeds as available,
  never a price-list-only one, and the list must name a `sources` entry with a date.
  `tests/fleet_seed_negative_control.rs` plants the pre-2026-09-25 list and a missing source,
  and sees the gate go red on each.

The brain's `knowledge_base` still carries the three-name redirect in the places the amendment of
2026-09-24 names; that text is the operator's to change, not this repository's.

## D19 addendum — nothing cannabis-related anywhere, decided 2026-09-25

Owner decision, 2026-09-25, answer 12 of a numbered list, verbatim: «всё что касается канабиса нигде
не должно быть» — nothing cannabis-related may appear anywhere. Kept at the end of this file, like
the entries above, so that no line citation into it moves. This entry is the server half: the bot,
the HTTP API and the docs. The Mini App's own copy is a separate change.

Nothing is deleted from the database, no table is dropped and no migration is added. Every change
stops SERVING or SENDING something, in code.

* **Bot.** The strain-of-day buttons still under old messages answer with the rental menu, the one
  `/menu` sends, instead of a strain card with a THC line and a per-gram price, and read no table.
  `/admin` loses the line listing the old catalogue's tabs, `/factpost` its leaf, and the bot's
  locale file the carousel's heading and arrows and the old unit of sale.
* **API.** `GET /api/loyalty/tiers` serves every `perks` list empty and omits the tier named after
  the old shop; that closes the perks question the 2026-09-24 addendum left open. The public
  tech-tree reads (`/api/tech-tree/nodes`, `/nodes/:id`, `/achievements`) answer an empty roadmap,
  not found and an empty badge list, because the stored roadmap and badges are the old shop's. The
  share card builds only from a catalogue row the public catalog shows, so a row 085 hid answers
  not found. The served OpenAPI document is held to a wider list of retired words.
* **Promo report** (added by review the same day). The owners' sales report — the bot's `/promo`
  and `GET /api/admin/promo-report` — prints «(название скрыто)» where a published post was about
  the product table migration 083 dropped: a row of that table's kind, or a bestseller whose stored
  link opens that table's card or that has no link to tell. A kind no live post writes is printed
  empty. The rows stay and still count in the totals; `promo_posts` is not written. Whether an old
  draft may still be SENT is the Publish button's question, answered by the same day's item-13
  decision in `specs/turbobaby/promo_broadcast.t27`, not by this entry.
* **Docs.** `docs/README.md` is a TurboBaby index. `README.md`, the owner agent card,
  `docs/DESIGN_SYSTEM.md` and `docs/SETUP_TEMPLATE.md` no longer describe the product as the old
  shop.
* **Guard.** `tests/server_text_vocabulary_wiring.rs` reads every string literal the server can
  send (test code excluded) and the two front pages against the vocabulary. The few survivors are
  listed with reasons: wire names of the retired catalogue kind, the file names of migrations 083
  and 085, the old URLs `src/config.rs` refuses, and the key admin tokens are signed with.
* **Kept, as records or as names.** Old migrations (D2), the history in this file, `docs/reports/`
  and the commit history. Internal names that are never shown: the `strain` wire kind, the
  `woody_*` localStorage keys (D19), the Railway project and service names the deploy runbook
  checks. The reader of the dropped `strains` table stays compiled for one ignored test.
* **Open.** The old catalogue's media under `assets/` (photos named after strains, the
  paraphernalia, the garden sprites) is still served by path; removing it is a deletion this change
  did not make. *Closed the same day by the client half of the ruling, which landed with this entry
  in one integration:* 128 files of the old shop's media left `assets/` and 13 stay (git history
  keeps the removed ones; `legacy_retirement.t27`, `CANNABIS_RULING_SERVED_FILES_REMOVED`). A loyalty profile that still stores the old shop's tier key is served that key by
  the profile and leaderboard reads; re-keying it is a data change for the owner.

The contracts carry the details: `specs/turbobaby/legacy_retirement.t27` records the ruling
(`OWNER_RULING_2026_09_25_AT`) and classifies the surfaces, the promo report's rule among them
(`PROMO_REPORT_NAME_*`, held to the code by `tests/promo_report_names_wiring.rs`),
`bot_surface.t27` the carousel (`CAROUSEL_RETIRED_AT`), `locale_policy.t27` the locale struct's
four removed fields.

## The bike detail's «all taken» line, removed 2026-09-25

Kept at the end of this file, like the entries above, so that no line citation into it moves.

The owner answered a second numbered list in chat on 2026-09-25, items 1 to 5; the owner confirmed
that numbering. Item 5 was the line the bike detail screen printed under a zero unit count,
«Сейчас все байки этой модели заняты.» / "Every bike of this model is out right now."
(`T_BIKE_UNITS_EMPTY`). That zero is the fleet seed of 2026-09-12 as an admin last edited it, not
live occupancy. Answer 9 of the first list that day had not located the line, so it was left
untouched then. Answer 5, verbatim: «Наверное» ("probably"), read as: remove it. The answer hedges,
so the removal is recorded as hedged, and a later answer may restore the line.

* **Removed.** The line, from the availability block of `src/ui/screens/bike_detail.rs`, and the
  key `bike.units.empty` with both of its sentences, from `src/trios/i18n.rs`, deleted outright as
  ruling #12's fourteen keys were: each deleted line became a comment line, so no line citation
  into that file moved. Nothing was written in its place. «Наличие уточняет менеджер»
  (`T_BIKE_AVAILABILITY_UNKNOWN`) stays and now stands alone in that block.
* **Kept, and open.** Under the same zero the Book control is disabled with the reason «Все байки
  этой модели заняты» / "Every bike of this model is taken" (`T_BIKE_BOOK_BLOCKED_NO_UNITS`). It
  reads the same seeded count and says nearly the same thing, but item 5 did not name it, and a
  disabled control must name its reason (issue #9), so changing it needs an owner-approved
  sentence. The file may still rule a family out of booking (`FILE_MAY_RULE_OUT`).

`specs/turbobaby/availability.t27` records the answer (`UNITS_EMPTY_LINE_*`, and the open reason
as `NO_UNITS_BOOK_REASON_*`), `locale_policy.t27` the key count (552 to 551), and
`tests/catalog_honesty_wiring.rs` guards the source.

## D19 addendum — the 20+ age gate, removed for now, decided 2026-09-25

The owner answered a second numbered list in chat on 2026-09-25, and confirmed its numbering
the same day: «Тут набираться от 1 до 5 правильная. Просто двойка задвоилась.» Answer 1,
verbatim: «Пока убираем». It answers whether the checkout keeps its 20+ age gate, which this
repository records as heritage of the fork rather than a rental decision
(`specs/turbobaby/checkout_contact.t27`, `AGE_BLOCKER_IS_INHERITED_HERITAGE`). «Пока» makes the
decision provisional: a later answer may bring the box back, and git history holds all of it.
Kept at the end of this file, like the entries above, so that no line citation into it moves.

* **Client.** The checkbox «Мне исполнилось 20+» and the notice under it, the trust line
  «🛡️ Проверка возраста (20+)», the order button's age blocker and the submit handler's age
  refusal are gone from the checkout, and the order request no longer carries `age_confirmed`.
  Five keys were deleted from `src/trios/i18n.rs` — those four and the sentence for a body code
  (`age_not_confirmed`) the server never sent — each line replaced by a comment line, so that
  no line citation moved. No sentence was written in their place.
* **Server.** `POST /api/orders` no longer refuses an order whose `age_confirmed` is absent or
  false; it answered 422. It still accepts the field, because a Mini App bundle cached before
  the change sends it `true`, and stores it as sent in the orders column (migration 057,
  default false). No migration was added and no row is written or deleted.
* **Kept.** A saved checkout draft that carries the field still restores, and the
  `woody_last_age_confirmed` key in customers' browsers is neither read nor cleared (D19).
  Every stored order keeps what it recorded.

`specs/turbobaby/checkout_contact.t27` records the decision beside
`AGE_BLOCKER_IS_INHERITED_HERITAGE` (`AGE_BLOCKER_REMOVED_AT`, `AGE_BLOCKER_REMOVAL_IS_PROVISIONAL`),
`legacy_retirement.t27` the answer to the question it owned (`AGE_GATE_ANSWER_*`),
`locale_policy.t27` the key count (552 to 547) and `client_errors.t27` the unmapped body code.
Gate 3 binds the button's blocker count to the code and holds the orders module to no
comparison of the field; `tests/integration_create_order.rs` holds the endpoint to it.
Merged on 2026-09-26 with answer 5 above, whose key also left the file, the count reads 546.

## D19 addendum — stored content out of customers' sight, decided 2026-09-25

The owner answered a second numbered list in chat on 2026-09-25; answer 3, verbatim: «Все
канабисное аналировать». The operator read it as: analyse all of it and take it out of customers'
sight, without deleting or rewriting stored data. Kept at the end of this file, like the entries
above, so that no line citation into it moves. Every change stops SERVING or PRINTING something,
in code; no row is written and no migration is added.

* **Masked on the server**, so a bundle cached before the change is served the neutral text too.
  A customer's own order list and detail (`get_user_orders` and `get_order_details`, through
  `Order::for_customer`) serve a line of the old catalogue — any line that is not a bike line — as
  «Позиция прежнего каталога» ("Item from the previous catalogue" for an English reader, the
  operator's wording under the answer) with its quantity and unit price exactly as stored, and
  nothing else: no id, so no reorder can rebuild the old item. The same two reads withhold an
  order's shop when it names the previous shop. The server cart serves a line of a retired kind
  under the neutral name and with no picture. The bonus history serves a stored description only
  when it is one of the four sentences this repository writes, and a garden-era row with an empty
  type, so every bundle labels it with the existing generic label («Бонус» / "Bonus").
* **Guarded on the client**, where it builds the text itself: the order screens print the neutral
  name in the reader's language, the profile labels a garden row with the generic label, and a
  cart kept on the device prints the neutral name and draws no stored picture on the cart and the
  checkout.
* **Open.** A withheld shop reaches the screens' fallback for an absent shop, which names
  TurboBaby, so an order the previous shop took reads as placed with this one. No neutral shop
  wording was given; that line is the owner's to reword.
* **Analysed and left**, each with its reason in the contract: the stored loyalty tier key, the star
  history, the reads behind retired surfaces, the abandoned-cart reminder, the referral, quest-scan
  and game reads, and the referral worker's garden arm.
* **Unchanged.** The admin reads, which are the archive; every row; every migration. One key was
  declared (`T_ORDER_LINE_PREVIOUS_CATALOGUE`) and none deleted.

`specs/turbobaby/order_presentation.t27` (`RETIRED_LINE_*`) owns the order line's name and the
shop, `legacy_retirement.t27` (`OWNER_ANSWER_3_*`) records the answer and the analysis,
`src/trios/legacy_view.rs` holds the rules and `tests/legacy_view_wiring.rs` holds the handlers and
screens to them.

## D8/D19 addendum — a cart kept from the previous shop is stored and not served, 2026-09-26

Kept at the end of this file, like the entries above, so that no line citation into it moves.
The rulings this rests on, verbatim: rental only, 2026-09-24, «Ареда [sic] только пхукет»;
answer 12 of 2026-09-25, «всё что касается канабиса нигде не должно быть»; and answer 3 of the
second list the same day, «Все канабисное аналировать» [sic], which the operator reads as:
analyse all of it and take it out of customers' sight without deleting stored data. Nothing is
deleted (2026-09-24).

A customer who kept a cart from the previous shop still had its lines in three places, and each
handed them back with names and pictures: the server's `cart_items` rows (every JSON answer of the
cart API), the abandoned-cart reminder (a Telegram message naming them), and the Mini App's own
saved cart on the device (localStorage `wwb_cart` and Telegram CloudStorage `wwb_cart_cloud`, read
at start-up). D8's amendment kept the four old kinds in `cart_items`' CHECK so that such rows
would not abort migration 081; the rows stay, and are now not served.

* **One rule, not a new list.** A cart serves a line only of a kind in
  `trios::pricing::SERVED_CART_KINDS` = `["bike_rental"]`, asked through
  `trios::pricing::cart_kind_is_served`. That name is the tag of the only deal the checkout admits
  since 2026-09-24 (`BikeDeal::BikeRental`; a new `bike_sale` line is refused, #63), and the kind
  D8 gave the cart. A test in `src/db/orders.rs` holds the list to the enum's tag.
* **Cart API.** `cart_model_to_resp` builds every answer (get, add, merge) from `served_rows`, so a
  hidden row is neither listed nor summed. PATCH and DELETE of one line by id answer 404 for a
  hidden row, as for a missing one. `clear_cart` deletes only the served lines. The write gate,
  `parse_kind`, is unchanged; no row of the old kinds can be written today (083, 085).
* **Reminder.** The inner join that picks due carts and the query that names their lines both
  filter on the served kinds. A hidden line is not summed or named, and a cart holding nothing
  else is never due: no message, and its counter is not spent.
* **Mini App.** Both reads of a saved cart pass it through `Cart::without_retired_lines`, and
  `CartItem::from_server` asks the same predicate before it classifies a kind. No `CartItemType`
  variant is a rental line, so every line of a saved cart is dropped today. The client does
  rewrite its OWN storage without them: the existing persist effect saves the cart it shows to
  `wwb_cart` and `wwb_cart_cloud`. That is the customer's device state, not stored shop data, and
  no write site was added.
* **Not changed.** No row is deleted or rewritten, no table dropped, no migration added, no
  sentence written: a kept cart reads as the empty cart the screens already render. Stored order
  lines and the reorder button are another change's.

`specs/turbobaby/cart_persistence.t27` records the rule (`SERVED_CART_KINDS` and the section dated
2026-09-26); gate 3 binds its list to the code and counts the reminder's filtered reads;
`tests/legacy_cart_hidden_wiring.rs` guards the three readers.

## D19 addendum — answer 3 and the kept-cart and held-notification changes reconciled, 2026-09-26

Kept at the end of this file, like the entries above, so that no line citation into it moves.
Four branches of 2026-09-25 and 2026-09-26 were merged on 2026-09-26 into `t27/owner-answers-2509`:
answer 3's (stored content out of customers' sight), the kept-cart change and the held
notification kinds above them. Answer 3's entry above says three things the other two changed.
The contracts now say one thing:

* **The cart.** Answer 3 served a cart line of a retired kind under the neutral name with no
  picture, and a cart kept on the device printed the same name. The kept-cart change serves no
  such line at all. Its rule was kept, and the neutral-name cart path was removed from the cart API,
  from `src/trios/legacy_view.rs` and from the cart, checkout and `CartItemComponent`. The Mini App's
  `Cart::add_item` now asks the same `cart_kind_is_served` as the two other ways into its cart, so
  a reorder or an unmounted screen cannot put such a line there either, and no screen is handed
  one. `specs/turbobaby/cart_persistence.t27` records it (`RECONCILED_WITH_ANSWER_3_AT`), and
  `legacy_retirement.t27` points to it (`OWNER_ANSWER_3_CART_RULE_OWNER_ID`).
* **The reminder.** Answer 3 left it, reasoning that no cart line of a retired kind can be
  written. A cart kept from before 083 and 085 still holds such lines. The kept-cart change closed
  it: the reminder reads served lines only.
* **The garden's notification arm.** Answer 3 left it. The held-kinds change closed it: such a
  queued row is held unsent (`notification_queue.t27`, `HELD_KINDS_DECIDED_AT`).

No row was written or deleted, no migration was added and no sentence was written.

## D19 addendum — an order of the previous shop shows no shop label, decided 2026-09-26

Kept at the end of this file, like the entries above, so that no line citation into it moves.
Answer 3's entry above left one item open: a withheld shop reached the order screens' fallback
for an order with no shop, and that fallback reads «TurboBaby». So an order the previous shop took
read as placed with TurboBaby, in the list and on the detail's labelled delivery row. The operator
decided on 2026-09-26, announced it to the owner, and the owner did not object: an order of the
previous shop must not be shown as TurboBaby's, and such an order shows no shop label at all.
No wording was needed.

* **Served.** The two customer reads serve a withheld shop as an empty shop, and no longer as no
  shop (`customer_shop_id`, `WITHHELD_SHOP` in `src/trios/legacy_view.rs`). The row keeps what it
  stored, and the admin reads are unchanged.
* **Shown.** Both order screens read the served shop through `shown_shop`. For the empty shop the
  list prints the date alone, and the detail leaves out its labelled delivery row. An order with
  TurboBaby's stored shop still prints it. An order naming no shop still prints the fallback. Both
  are exactly as before. A bundle cached from before prints an empty value for such an order,
  which is not TurboBaby's name either. The Mini App's checkout never sends an empty shop.

`specs/turbobaby/order_presentation.t27` records it (`RETIRED_SHOP_DECIDED_AT`,
`RETIRED_SHOP_LABEL_SHOWN`, `RETIRED_SHOP_SERVED_AS`; `RETIRED_SHOP_FALLBACK_NOTE` is closed).
`tests/legacy_view_wiring.rs` holds the screens to it, and the rules' module has the unit test.

## D19 addendum — what answer 3 still left in sight, the owner's answers of 2026-09-26 (server)

Kept at the end of this file, like the entries above, so that no line citation into it moves.
Questions about what answer 3 (2026-09-25) still left in customers' sight were put to the owner on
2026-09-26. The rulings in force stay: rental only, Phuket only, nothing deleted (2026-09-24);
«всё что касается канабиса нигде не должно быть» and «Все канабисное аналировать» (2026-09-25).
Every change below stops SERVING or SENDING something, or changes what a NEW row says; no row is
deleted or rewritten, no migration is added, and no file is removed from any volume or bucket.

* **The previous shop's orders.** Asked about the lines shown as «Позиция прежнего каталога», the
  owner answered «А зачем это вообще там?». The operator reads it as: a customer must not see the
  previous shop's orders at all. An order naming the previous shop, or holding no bike line, is
  not listed (it takes none of the list's 50 places), is answered 404 by the detail, the status
  and the customer's own cancel exactly like a missing order, and is not counted in the profile's
  order count. An order holding a rental line beside a line of the old catalogue is this shop's
  and stays shown with that line masked, which is now the only case the neutral name is served
  for. The admin reads are the archive and are unchanged. `order_presentation.t27`
  (`PREVIOUS_SHOP_ORDER_*`); the rule is `customer_sees_order` in `src/trios/legacy_view.rs`.
* **The previous shop's media.** Asked about media still reachable by a direct link, the owner
  answered «Зачем они вообще нужны мне?». The operator reads it as: stop serving them; deleting the
  files is the owner's own irreversible act and is not done here. The upload's local branch still
  writes to `/data/uploads` whenever no object store is configured, so `/uploads/<name>` stays and
  serves a name only when a bike's stored picture is exactly `/uploads/<name>`; everything else
  answers 404. The bucket is not this server's to gate (its objects are fetched from the bucket's
  own address), and both shops' code wrote the one key prefix `uploads/`, so telling the previous
  shop's objects apart needs a production listing and stopping them a bucket policy change: the
  owner's or the operator's, not done here. `upload_media.t27` (`OWNER_MEDIA_ANSWER_*`,
  `LOCAL_READ_*`, `OBJECT_STORE_*`).
* **The 24-hour event reminder** (operator, under «Все канабисное аналировать»). It was kept for
  seats already held (2026-09-25). Every event is hidden since 088 and each is the previous shop's,
  so it mailed their stored titles and venues. It now reminds public events only; cancellation and
  the Stars refund of a held seat are unchanged. `events_booking.t27` (`REMINDER_REVERSED_AT`).
* **The welcome credit's sentence** (critic note 5, under the rulings of 2026-09-25). A new
  `referral_welcome` row said "Welcome bonus from a friend's garden invite". None of the four
  sentences already written fits the invitee's row, so a new row now STORES the same sentence with
  the garden's words dropped: "Welcome bonus from a friend's invite". Those words are the lane's
  interim choice, not an owner's or an operator's decision, and this repository invents no
  customer copy, so the sentence is NOT served: the bonus history withholds the description of
  every welcome row, old or new, and a customer sees the generic label and the amount, as before
  this round. **Open question for the owner:** what should an invited customer's welcome bonus say
  in the bonus history? Until the owner words it, nothing is shown there. A first version of this
  round served the sentence and recorded it as the operator's; the review of the round reversed
  that on 2026-09-26. `legacy_retirement.t27` (`WELCOME_CREDIT_SENTENCE`, `WELCOME_SENTENCE_*`).

The second question of that day (the Book control under a seeded zero) is the client's and is
recorded in the entry that follows. `legacy_retirement.t27` keeps the day's answers together (`OWNER_ANSWERS_2026_09_26_AT`).

## The Book control under a seeded zero: booking allowed, decided 2026-09-26 (question I)

Kept at the end of this file, like the entries above, so that no line citation into it moves.

The entry on the «all taken» line above left one thing open (question I): under the same zero unit
count, a count seeded on 2026-09-12 and changed only by an admin, the Book control on the bike
detail stayed disabled with the reason «Все байки этой модели заняты» / "Every bike of this model
is taken" (`T_BIKE_BOOK_BLOCKED_NO_UNITS`). The owner answered on 2026-09-26, verbatim: «Разрешить
бронь, наличие уточнит менеджер» ("Allow booking; the manager will confirm availability").

* **Removed.** The arm of `book_block` (`src/ui/screens/catalog_screen.rs`) that disabled the
  control under a zero, and the arm that disabled it when no count was served, «Наличие не
  подтверждено» / "Availability not confirmed" (`T_BIKE_BOOK_BLOCKED_UNKNOWN_AVAILABILITY`). The
  answer names the zero. Removing the second arm is the operator's reading of it: a manager confirms
  availability for every family, so "not confirmed" is no reason to refuse one. Both keys were
  deleted from `src/trios/i18n.rs` the way the earlier ones were, each line replaced by a comment
  line. The unmounted card component (`src/ui/components/bike_card.rs`) stopped gating its
  add-to-cart on the count and stopped printing it; it prints the manager line.
* **Unchanged.** The server never refused an order on the count: `check_bike_lines` in
  `src/api/orders.rs` logs a shortfall for staff and accepts the order. What still disables the Book
  control reads no count: a closed family (D12), no published rate (D11), and a screen with no
  booking handler. «Наличие уточняет менеджер» stays on every card and detail, and no sentence was
  written. The admin screen keeps its count. The CLICK 125 redirect still names only a family with a
  unit at base, because that decides what is offered instead, not whether a customer may book.

`specs/turbobaby/availability.t27` records the answer (`NO_UNITS_BOOK_REASON_*`,
`UNKNOWN_AVAILABILITY_*`, `customer_may_book`), `locale_policy.t27` the key count (547 to 545), and
`tests/catalog_honesty_wiring.rs` holds the client, the card and `create_order` to it.

## D19 addendum — cannabis-era copy the bundle still carried, retired 2026-09-26

Kept at the end of this file, like the entries above, so that no line citation into it moves.
A review of `405f30e` found copy of the previous shop in the public Mini App bundle that no mounted
screen printed: every arm of `src/trios/i18n.rs` ships in the wasm, used or not. Under the owner's
rulings of 2026-09-25, «всё что касается канабиса нигде не должно быть» (#12) and «Все канабисное
аналировать» (answer 3), the operator had it retired on 2026-09-26.

* **Deleted, nineteen keys**, each line replaced by a comment line so that no line citation moved:
  the garden's bonus label «Награда из сада» / "Garden reward"; the accessories screen's last two
  paraphernalia categories «Хранение» / «Зажигалка» with their chips; the strain card's
  lab-certificate link «📄 Сертификат»; and fifteen keys of the unmounted shop game's farm («Посадить»,
  «Собрать», «Урожай!» and the rest). The dead code that used them went too: the farm zone of
  `src/ui/game/shop_game.rs` and two chips of `src/ui/screens/accessories_screen.rs`. Git keeps it.
* **Kept.** The ride game (answer 2). Both unmounted screens stay compiled and exported, as answer 7
  left them. Generic words (clothing, souvenir, the cafe and grill game, the old sort labels) carry
  nothing cannabis-related. The tea fold list's CBD label is not in the bundle and hides that word
  rather than showing it.
* **Unchanged.** Every row, every migration. No sentence was written.

`specs/turbobaby/legacy_retirement.t27` records it (`LEFTOVERS_2026_09_26_*`), `locale_policy.t27`
the key count (545 to 526), and `tests/no_cannabis_client_wiring.rs` keeps the keys, their copy and
the farm's words out of the tables and out of every string literal under `src/ui`.

## R1–R3, the owner's answers of 2026-09-26

Kept at the end of this file, like the entries above, so that no line citation into it moves.
Three questions left open by round 4 were put to the owner on 2026-09-26. He answered, verbatim:

* **R1**, `GET /api/loyalty/leaderboard`, which had no login and served the top 20's first names,
  total spend and raw tier: «Только для админа» ("Only for the admin").
* **R2**, `GET /api/quest-places`, `GET /api/treasure-hunts` and `GET /api/loyalty/config`, which
  served stored rows and the stored config as they are: «Закрыть для клиентов» ("Close it to
  customers").
* **R3**, the referral programme: «Должно начисляться исключительно за то кто арендовал 10% скидка»,
  «Пригласивший и может забрать скидкой за аренду или деньгами», and, of the referral points, the
  milestone ladder and the welcome credit, «Убрать, только скидка 10%»; the 10% is taken «с каждой
  аренды друга». In short: the inviter gets 10% of each rental an invited friend completes, and may
  take it off a rental or as money; the points go.

The rulings in force stay: rental only, Phuket only, nothing deleted (2026-09-24). No row is deleted
or rewritten. Migration 089 (`089_referral_credit.sql`) only creates three tables and their indexes.

**What changed.**

* **R1.** `get_leaderboard` in `src/api/loyalty.rs` calls `check_admin` as its first statement. A
  non-admin gets its 401, or 429 when rate-limited, like every admin route. The admin screen, the
  only reader, is unchanged.
* **R2.** The three GETs call `admin_or_missing_route` (`src/api/mod.rs`) first. A caller without
  admin proof gets the missing-route answer: status 404 with the same body `api_not_found` builds
  for an unmatched path, from one shared builder, `missing_route`; a rate-limited caller gets the
  same 404. Admins are still served. A failed attempt still counts toward the admin limiter. The routes stay registered, because unregistering
  a GET would answer 405 and so say the path exists. The writes were already admin-only.
* **R3, what stopped.** The referral points to the inviter on a friend's first completed order, the
  milestone ladder and its award, and the welcome credit to the invitee. Their code is deleted from
  `src/db/referrals.rs` and from `complete_order_and_update_loyalty` (`src/db/orders.rs`), and so
  are the two queue producers that announced them. Edge confirmation stays, with no money: pending
  becomes confirmed and `referral_count` goes up by one on the friend's first completed order
  (`confirm_referral_edge`), and also on a creditable recorded rental.
* **R3, what started.** A separate THB ledger, `referral_ledger`. A manager records a completed
  rental of an invited friend (`POST /api/admin/referral-credit/rentals`), and the direct inviter is
  credited 10% of it, rounded down to whole baht. The inviter can ask for the balance off a rental
  («Списать в счёт аренды») or paid out («Запросить выплату»). A manager resolves each request by
  hand. The code is `src/db/referral_credit.rs`, `src/api/referral_credit.rs` and the shared
  arithmetic `src/trios/referral_credit.rs`. The contract is
  `specs/turbobaby/referral_credit.t27`.

**The operator's decisions**, recorded with the answer they implement. Each can be revisited by the
owner:

1. Referral money is a new THB ledger (`referral_ledger`), separate from
   `loyalty_profiles.bonus_balance`. Its balance is the sum of its rows; no balance column is
   stored. Holds are derived from open requests. Why: points cannot pay a rental (`bonus_used` is
   at most the subtotal, which is 0), a payout from `bonus_balance` would pay out cashback, and a
   stored balance can drift, which is the defect `loyalty_ledger.t27` measures.
2. Credit is born only when a manager records a completed rental, through
   `POST /api/admin/referral-credit/rentals`. Nothing credits automatically when an order completes.
   Why: no order carries a rental amount (`check_full_subtotal` prices bike lines at 0, D11), and
   no shipped client creates rental orders.
3. The base is the rental charge the manager took, in whole THB, without deposit and delivery, net
   of any referral balance applied to that same rental. The credit is floor((R − applied) × 10 / 100).
   Why: the credit never exceeds 10% of the cash actually received. This is the conservative
   reading; the owner may prefer the gross charge (open question 1 below).
4. Rounding is down to whole baht, with integer THB (BIGINT) everywhere. A rental whose credit
   rounds to 0 is still recorded, with no ledger row.
5. Every recorded rental credits: no cap, no expiry, no minimum payout, no fee and no cooldown. An
   extension paid separately may be recorded as its own rental. Why: «с каждой аренды друга», and
   no number may be invented.
6. Only the direct inviter is credited (one level); the friend gets nothing. The edge is read from
   `referral_events` (first-touch, any status), never from `loyalty_profiles.referred_by`.
7. Not creditable: a self edge (including the backfill of migration 007); an edge created after the
   linked order; an existing customer (`first_purchase_at`, or an earlier recorded rental, before
   the edge); and a recording admin who is the inviter (422). Why: `/start ref_` records an edge
   with no new-user check, so old customers could otherwise be attributed.
8. No retroactive credit. A record linked to an order created before migration 089 was applied
   (`_schema_migrations.applied_at`) is refused with 409. By manager policy, offline rentals from
   before the deploy are not to be recorded.
9. «Списать в счёт аренды» is a customer request that holds the whole available balance. At the
   customer's next recorded rental the manager's record applies min(hold, rental, balance)
   automatically; the manager may decline the request instead. It is not a checkout field this
   round. Why: no checkout can carry a rental line, and a hold inside `create_order` would edit the
   most sensitive money path for a feature no one can reach.
10. «Запросить выплату» holds the whole available balance. The manager pays by hand and marks it
    paid, or declines. There are no partial payouts, and "paid" is refused if the ledger is below
    the amount. Nothing moves money automatically.
11. At most one open request per person (a partial unique index). A second tap of the same kind
    returns the open one (200, `already_open`); a request of the other kind gets 409.
12. Clawback is automatic only in the bot's `RejectOrder`, the one path that can un-complete an
    order. It runs in the same transaction, fail-closed, and is a no-op when there is no live record.
    Every other refund or correction is the admin's whole reversal; a correction is a reversal plus
    a new record. Rows are appended or marked, never deleted.
13. A reversal may make a balance negative. The customer is shown 0 (no approved copy explains a
    debt). It is never collected, and it blocks requests until later credits net it out.
14. Customer routes use `check_owner`, with its lenient fallback, plus `check_not_blocked`. There is
    no strict HMAC gate. Why: a request moves no money, and the reconstructed-initData cohort relies
    on the fallback. A blocked customer's credits are still recorded.
15. An admin record carries a required `idempotency_key` in the JSON body, compared with the payload
    (409 on reuse with a different payload). Why: the existing `post_json_admin_full` helper sends no
    custom header, and a new helper would move `webapp_bridge` counts.
16. Advisory locks use the two-int4 key space (`hashtext('referral_credit')`, `hashtext(tid)`). Each
    transaction takes at most one person lock. The global order is: the order row, then the person
    lock, then the referral rows.
17. R1: a non-admin gets `check_admin`'s 401 (429 when rate-limited), like every admin route. R2:
    only the three GETs answer non-admins with the byte-identical `api_not_found` body, through one
    shared builder. Admins are still served, failed attempts count toward the admin limiter, and the
    routes stay registered.
18. The stopped credits' code is deleted (the points to the inviter, the milestones and the welcome
    credit). Edge confirmation is kept without money, on the first completed order and on a
    creditable recorded rental.
19. Queued `friend_ordered` and `milestone` notification rows are held, not delivered, because
    their producers are removed. Why: `notification_queue.t27`'s rule and
    `tests/notification_drain_wiring.rs` keep producers, renderers and both kind lists equal (the
    `friend_watered` precedent). The queue drains within a tick, so few rows or none are affected.
    No queued row is deleted.
20. `REFERRAL_WELCOME_BONUS` stays parsed into `Config` and is read by nobody (the
    `Config::delivery_zones` precedent). The `loyalty_config` keys `referral_bonus` and
    `milestone_bonus_N` stay stored and inert, and `validate_loyalty_config_body` still requires
    `referral_bonus`.
21. Server switches for cached bundles: `/api/referrals/me/:telegram_id/milestones` serves empty
    `thresholds` and `bonuses`, and `/api/loyalty/:telegram_id` omits `config.referral_bonus`. The old ladder and the old
    per-friend line vanish before the client redeploys.
22. Customer copy uses approved wording only. The rule sentence «10% с каждой аренды приглашённого
    друга» / "10% of every rental your invited friend completes" goes on the bot's `/invite` hint and
    on the friend-joined notice's hint (`src/locales.rs`, `referral_share_hint` and
    `referral_invite_progress_hint`), and, in the client lane, on the referrals subtitle and the
    profile's former per-friend line. `/refstats` shows «Реферальный баланс» / "Referral balance"
    (`referral_bonus_earned`) with the shown balance, and leaves the line out when the balance
    cannot be read. The friend-facing share text is dropped, the empty invitees panel is hidden, and
    a redeem in progress shows "✅ " plus the button's own label (client lane).
23. The customer response carries the balance, the hold, the available amount and the open request
    only: no entries, no friend ids, no order ids.
24. The admin view is a third Loyalty sub-tab, «Рефералы» (client lane). Admin-facing labels and the
    English Telegram notices to admins are written by the lane and listed below for rewording.
25. `/api/referrals/leaderboard` is left unchanged, although it is public, publishes `telegram_id`
    and shows frozen point totals: R1 names only `/api/loyalty/leaderboard` (open question 2).

**Admin-facing wording the server lane wrote**, not the owner's, listed for rewording:

* The notice to admins when a customer opens a request (`request_notice` in
  `src/api/referral_credit.rs`, sent after the commit): the head «💸 Referral payout request» or
  «🏍 Referral balance to apply to a rental», then the customer's first name, @username and
  `id <telegram_id>`, the amount (`format_baht`), `#R<request id>`, and «Admin → Лояльность →
  Рефералы».
* The note stored on the reversal the bot's reject makes: "order rejected in the bot"
  (`BOT_REJECT_NOTE` in `src/db/referral_credit.rs`).
* The refusal codes the admin routes answer with, machine words rather than sentences:
  `order_not_found`, `order_of_another_customer`, `order_has_no_rental_line`,
  `order_not_completed`, `order_before_program`, `order_already_recorded`,
  `idempotency_key_reused`, `recorder_is_inviter`, `no_referral_effect` (with the reason
  `no_edge`, `self_edge`, `edge_after_order` or `existing_customer`), `request_open`,
  `nothing_available`, `rental_not_found`, `request_not_found`, `request_not_open`,
  `redeem_cannot_be_paid`, `balance_below_request`, and the body validators' `invalid_customer`,
  `invalid_rental_amount`, `invalid_order_id`, `invalid_note`, `invalid_idempotency_key`,
  `invalid_kind` and `invalid_action`.
* The OpenAPI descriptions of the six new routes and of the R1/R2 responses (`src/api/openapi.rs`).

**Left as it is.** The `bonus_transactions` rows and `referral_milestones` rows the stopped credits
wrote, and every balance they moved. The two locale strings that announced them
(`referral_friend_ordered`, `referral_milestone_bonus` in `src/locales.rs`) are read by nobody. The
earlier open question about the welcome credit's sentence (the entry "what answer 3 still left in
sight") now concerns old rows only: no welcome row is written since R3, and old ones stay withheld.

**Open questions for the owner.**

1. Is the 10% taken on the rental charge net of the referral balance applied to it (as built), or
   on the gross charge?
2. `/api/referrals/leaderboard` is public and publishes `telegram_id` and frozen point totals.
   Close it like R1, or leave it?
3. The admin-facing wording above: keep it, or reword it?

`specs/turbobaby/referral_credit.t27` owns the credit. `referral_program.t27` records what stopped,
`loyalty_ledger.t27` R1, `legacy_retirement.t27` the answers of R1 and R2
(`OWNER_ANSWER_R1_*`, `OWNER_ANSWER_R2_*`), `notification_queue.t27` the held kinds, and
`order_status.t27` the reject's reversal. The tests are `tests/referral_credit_wiring.rs`,
`tests/integration_referral_credit.rs` and `tests/integration_closed_reads.rs`.
