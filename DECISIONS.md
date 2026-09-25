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
