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
