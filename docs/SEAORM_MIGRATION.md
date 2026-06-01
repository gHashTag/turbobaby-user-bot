# SeaORM migration strategy

The project carries **two ORMs in parallel**: `deadpool_postgres` +
`tokio_postgres` for raw SQL in `src/db/*.rs`, and `sea_orm` for typed
entities in `src/db/entities/`. MEMORY.md states the preference is
SeaORM; this doc plans the migration.

## Current state (cycle #78 snapshot)

### Raw SQL files (deadpool_postgres + tokio_postgres)

| File | Lines | Surface area | Migration priority |
|------|-------|--------------|--------------------|
| `src/db/users.rs` | 20 | tiny — single `set_user_lang`-style helper | **#1 (smallest, lowest risk)** |
| `src/db/strains.rs` | 252 | CRUD + marketing flags (cycle #11). Has SeaORM entity `entities/strain.rs`. | **#2** |
| `src/db/loyalty.rs` | 182 | bonus_transactions + loyalty_config | #3 |
| `src/db/referrals.rs` | 417 | referral_events + leaderboard + cycle #76 `tx.commit()` warn | #4 |
| `src/db/orders.rs` | 1271 | order creation, fraud events, block_history, audit sweeps. Most complex; defer. | **last** |

Total raw SQL surface: ~2466 lines across 5 files.

### Existing SeaORM entities (no migrations to do — already typed)

`src/db/entities/{garden_config,loyalty_profile,order,quest_place,strain,treasure_hunt,user}.rs`

Of these, only **strain** and **order** have matching raw-SQL siblings
that still write through `tokio_postgres`. The rest are entity-only and
already canonical.

## Migration pattern (one file at a time)

The repeating shape for each raw-SQL function:

```rust
// Before (deadpool_postgres + tokio_postgres):
let client = pool.get().await.context("db pool")?;
let row = client
    .query_opt("SELECT id, name FROM strains WHERE id = $1", &[&id])
    .await?;
let strain = row.map(|r| Strain {
    id: r.try_get("id").unwrap_or_default(),
    name: r.try_get("name").unwrap_or_default(),
});

// After (SeaORM):
use crate::db::entities::strain::{Entity as StrainEntity};
let strain = StrainEntity::find_by_id(id).one(db).await?;
```

Notable wins:
* `try_get(...).unwrap_or_default()` (a known footgun — silently swallows
  type mismatches and column-rename refactors) replaced with strongly
  typed entity columns.
* Transaction boundaries become explicit `db.begin().await?` /
  `tx.commit().await?` with proper error propagation (vs. the cycle #76
  `tx.commit().await.ok()` silent path).
* Connection pool management moves to SeaORM's `DatabaseConnection`
  (which already wraps deadpool internally).

### Migration checklist per file

1. **Sibling entity exists?** If yes, skip to step 3. Otherwise generate
   it from the live schema with `sea-orm-cli generate entity` (point at
   the prod DB or a recent dump).
2. **Add the entity to `src/db/entities/mod.rs`** and re-export.
3. **Pick one function** in the raw-SQL file. Rewrite it using the
   entity's `find`/`insert`/`update` API. Keep the old function signature
   unchanged so callers don't move.
4. **Replace the function body** — old `client.query_opt(...)` →
   entity-method call. Build the result type from the entity's `Model`
   via `Into`/`From` impl.
5. **Run the file's tests.** Most raw-SQL functions are covered; if not,
   that's a deeper cycle (write tests first, then migrate).
6. **Commit the migration** with a `refactor(db)` CC type. One file per
   PR keeps blast radius bounded.

## What NOT to migrate (yet)

* **`src/db/mod.rs`** — pool initialisation. SeaORM `Database::connect`
  replaces the pool builder but keep `pub struct Database { pool: Pool }`
  shape until every consumer is migrated; otherwise the trait boundary
  fragments.
* **Audit-sweep code in `src/db/orders.rs`** (cycles #66 + #67's
  `audit_sweep_sql` helper) — pure builder, not raw query execution.
  Already maintainable.
* **Anything with `RETURNING` clauses driven by Postgres-specific syntax
  that doesn't have a SeaORM equivalent yet.** Document the gap, file an
  issue against `sea-orm`, keep the raw path with `#[allow(dead_code)]`
  + comment until upstream catches up.

## Risk model

* **Per-file blast radius**: 20-1271 lines. Pick small first (`users.rs`)
  to validate the pattern before tackling `orders.rs`.
* **Schema drift**: SeaORM entities can be regenerated from the DB at
  any time, but uncommitted manual changes get clobbered. Treat
  `src/db/entities/*` as generated code — never hand-edit; only modify
  via `sea-orm-cli`.
* **Performance**: SeaORM adds a thin abstraction layer; benchmarks
  in upstream show <5% overhead for the patterns used here (single-row
  reads, simple writes). Not a concern at our load.

## Suggested cycle pacing

| Cycle | File | Expected lines moved | Estimated effort | Status |
|-------|------|----------------------|------------------|--------|
| #79 ✅ | `src/db/mod.rs` (Database user methods) | 60 | small — validated pattern | **done** |
| #80 ✅ | `db/strains.rs` reads (Strain::from_row → From<Model>) + 4 callsites | ~150 | small/medium | **done** |
| #81 ✅ | `api/strains.rs` writes (create + update + delete + toggle + SOTD) | ~140 | medium | **done — file 100% off raw SQL** |
| #82 ✅ | `db/loyalty.rs` cleanup + 2 new entities + `api/happy_hour.rs` migration | ~80 net | small/medium | **done** — see below for scope shift |
| #83 ✅ | `api/loyalty.rs` 2 endpoints: `use_bonus` + `add_bonus` (full SeaORM tx) | ~80 | small/medium | **done** — validated tx pattern |
| #84 ✅ | `db/referrals.rs` full file (6 functions + 1 entity + 6 callsites + tests) | ~290 | medium | **done — file 100% off raw SQL** |
| #85 ✅ | `orders.rs` Part 1 — reads (`Order::from_row` → `From<Model>` + 3 callsites) | ~70 | small/medium | **done** |
| #86 ✅ | `orders.rs` Part 4 (sweeps) + memory capture `seaorm-patterns.md` | ~90 | small | **done** — scope-shifted; Part 2 too big for one cycle |
| #87 ✅ | `orders.rs` Part 2 — `create_order` SeaORM tx (7 stmts incl. advisory locks) | ~140 | large | **done** |
| #88 ✅ | `orders.rs` Part 3 — `update_order_status` (3 branches) + `complete_order_and_update_loyalty` (4-stmt tx) | ~280 | medium/large | **done** |
| #89 (next) | accessory/tea/set entities + price-auth lookups in `create_order` | ~150 | medium | |
| #90 | record_fraud_event + scattered helpers (query_blocked_users, manual_unblock, record_block_history) | ~200 | medium | |
| #84-#86 | `src/db/orders.rs` (split into 3-4 cycles by feature) | 1271 | large | |

Total ≈ 6-8 cycles. Each cycle is independently committable; no big-bang.

### Validated pattern (cycle #79)

The 6 user-related methods on `Database` migrated cleanly:

* **Reads (find_by_id)**: `Entity::find_by_id(pk).one(&self.orm).await?.map(|m| m.field)`. Failure path: convert `Result<Option<Model>>` to whatever the caller's signature expects; for the legacy `Option<String>` return shape, log+drop the Err.
* **Upserts (ON CONFLICT)**: `Entity::insert(am).on_conflict(OnConflict::column(pk).update_columns([...]).to_owned()).exec(&self.orm)`. Build `ActiveModel` with `Set(value)` for written columns and `..Default::default()` for everything else — that lets the DB DEFAULT clause fire on insert path.
* **Updates (no-row tolerant)**: `Entity::update_many().col_expr(col, Expr::value(v)).filter(pk.eq(id)).exec(&self.orm)`. Use `update_many` instead of `update` to silently no-op when the row doesn't exist — matches the raw-SQL `UPDATE ... WHERE id = $1` semantics.

The 60-line migration touched zero callsites — all consumers go through `Database::method(...)` so the implementation swap is invisible.

### Cycle #80 additions

Two more shapes validated:

* **`From<Model> for WireType`**: when the wire JSON shape diverges from the
  entity (RFC3339 strings vs `DateTimeWithTimeZone`, hidden columns,
  `f64` finite-clamps), put the conversion in `impl From<Model> for X`.
  Cleaner than threading `Model` through callers, and centralises the
  wire-shape drift in one place. Migration changes `from_row(&Row)` →
  `From::from(Model)`.
* **Custom ORDER BY**: when the legacy SQL had a `CASE WHEN` priority
  expression, use `sea_orm::sea_query::Expr::cust("...")` and pass it to
  `order_by`. Not as clean as native enum mapping but lets the migration
  proceed without redesigning the sort algorithm.
* **`WHERE id = ANY($1)` arrays**: `Column::Id.is_in(ids)`.

Also obsoleted a defensive comment about SQLSTATE 0A000 cached-plan
issues. SeaORM/sqlx don't share `tokio_postgres`'s prepared-statement
cache shape and aren't subject to that ALTER TYPE bug, so the
per-call statement-marker workaround disappeared cleanly.

### Cycle #81 additions

Three more shapes validated for writes:

* **INSERT via `ActiveModel { col: Set(v), ..Default::default() }`** —
  `Entity::insert(am).exec(&db.orm)`. Unset fields fall back to the DB
  DEFAULT clause; this preserved the prior "raw SQL only listed editable
  columns, let DB fill audit-only ones" semantics.
* **UPDATE many cols + NOT_FOUND**: `update_many().set(active_model).filter(pk.eq(id))`
  returns a `result.rows_affected` u64 that lets us return 404 on
  zero-row UPDATE — the only sane way to detect a missing row through
  `update_many` (the singular `update` would error if the row doesn't
  exist, but the request shape needs the row-count distinction).
* **DELETE**: `Entity::delete_by_id(id).exec(&db.orm)`. No rows-affected
  check — matches the raw `DELETE WHERE id = $1` which never returned
  NOT_FOUND.
* **Datetime conversion at boundaries**: when storing a
  `Option<chrono::DateTime<chrono::Utc>>` into a SeaORM
  `Option<DateTimeWithTimeZone>` column, `.map(|t| t.into())` does the
  type elision via the existing `From` impl.

`api/strains.rs` is now 100% off raw SQL.

### Cycle #82 scope shift

The original plan said "migrate `src/db/loyalty.rs` (182 lines)". On
inspection, the file turned out to be almost entirely *dead* wire-shape
types (`LoyaltyProfile`, `BonusTransaction`, `LoyaltyConfig` — none used
outside the file's own tests). The actual loyalty SQL queries live in
`api/happy_hour.rs`, `db/referrals.rs`, `api/orders.rs`, `api/garden.rs`,
`api/admin.rs` — scattered, not centralised.

Cycle #82 adapted:

1. **Generated two entities**: `entities/loyalty_config.rs` (JSONB
   singleton with `id=1` PK) and `entities/bonus_transaction.rs` (append-
   only ledger). Hand-written from the migration SQL because regenerating
   would clobber neighbouring tweaks; these are simple enough that the
   manual write took 10 lines each.
2. **Migrated `api/happy_hour.rs`**: `SELECT config FROM loyalty_config
   LIMIT 1` → `LoyaltyConfigEntity::find_by_id(1).one(&db.orm)`. The
   JSONB column auto-deserialises to `serde_json::Value`. Single-row
   singletons are the cleanest possible SeaORM pattern.
3. **Cleaned up `db/loyalty.rs`**: dropped the three dead wire-shape
   structs (`LoyaltyProfile`, `BonusTransaction`, `LoyaltyConfig`),
   simplified `calculate_tier` to take its four scalar inputs directly
   (no longer needs the `LoyaltyConfig` indirection). Tests rewritten to
   pass thresholds as constants. File shrank 183 → 122 lines.
4. **Deferred**: `db/referrals.rs::confirm_referral` has a 4-statement
   tokio_postgres transaction that includes a `bonus_transactions`
   INSERT. Migrating just one INSERT would split the transaction
   boundary (regression). The whole `confirm_referral` belongs to cycle
   #83's `referrals.rs` pass, which will use a SeaORM
   `DatabaseConnection::begin().await?` transaction.

Lesson learned: per-file scoping in the migration plan assumes the file
*has* the queries it owns. When the queries are scattered across the
API layer, the cycle should target the queries, not the file. Cycles
#83+ likely face the same shift.

### Cycle #83 — two new patterns

* **Column-expression UPDATE with placeholder values**: when the SQL
  did `bonus_balance + $1` or `GREATEST(0, bonus_balance - $1)`, use
  `sea_orm::sea_query::Expr::cust_with_values("bonus_balance + $1", [v])`
  rather than `Expr::value(v)`. The latter would try to write the value
  *as* the new column contents; `cust_with_values` is the raw-SQL
  escape hatch that lets us reference the existing column on the right
  side of the assignment.
* **Full SeaORM transaction** (`begin → ops → commit`): `state.db.orm
  .begin().await?` returns a `DatabaseTransaction` that implements
  `ConnectionTrait`, so every entity API (`Entity::insert(am).exec(&tx)`,
  `update_many().exec(&tx)`, etc.) works against it identically. **Drop
  auto-rolls back**: no need to write an explicit `.rollback()` arm
  — when the function returns `Err(...)` before `tx.commit()`, the tx
  drops and Postgres rolls back. Only commit on the happy path.

The cycle migrated 2 endpoints in `api/loyalty.rs` (`use_bonus` —
single column-expr update with concurrent-safe `WHERE balance >= $1`
guard; `add_bonus` — 3-statement self-contained transaction). Confirms
the SeaORM tx pattern works for the rest of the loyalty domain and
unblocks cycle #84's referrals migration (which has the bigger
`confirm_referral` tx deferred from #82).

8 of the 10 scattered loyalty queries are still on tokio_postgres
*because they're inside other modules' transactions* (orders.rs create-
order tx, garden.rs reward-use tx, callbacks.rs order-reject refund tx).
Migrating one statement out of those tx would split the boundary —
defer until those whole transactions migrate in cycles #84-#86.

### Cycle #84 — `src/db/referrals.rs` fully migrated

Largest single-cycle migration so far. The whole file (6 public
functions, 5 statements per `confirm_referral` tx, retry loops in
`get_or_create_referral_code`) moved off `Pool` onto
`&sea_orm::DatabaseConnection`. New patterns:

* **`QuerySelect::lock_exclusive()` = `SELECT ... FOR UPDATE`.** The
  race-guard in `confirm_referral` (which is *the* reason this whole
  flow is in a tx) maps cleanly. No extra ceremony.
* **OnConflict with column-expression preserved value** (record_referral
  upsert): `INSERT ... ON CONFLICT DO UPDATE SET referred_by =
  COALESCE(existing, EXCLUDED.referred_by)` → use
  `OnConflict::column(pk).value(col, Expr::cust_with_values("COALESCE(table.col, $1)", [...]))`.
  The standard `update_columns(...)` form would clobber; the
  `.value(col, Expr...)` form is the escape hatch for "compute the new
  value from existing".
* **Mixing typed entity API with raw `Statement` for aggregates.**
  `COUNT(*) FILTER (WHERE ...)` and `GROUP BY ... LEFT JOIN ...
  ORDER BY agg DESC` don't have idiomatic SeaORM builder forms in 1.1.
  Use `Statement::from_sql_and_values(DbBackend::Postgres, sql, [args])`
  + `orm.query_one(stmt)` / `orm.query_all(stmt)`. Same approach as
  `api/loyalty.rs::get_leaderboard` from before — formalised here.
* **Signature change: `&Pool` → `&sea_orm::DatabaseConnection`.** The
  6 callsites updated mechanically: `&db.pool` → `&db.orm` /
  `&state.db.pool` → `&state.db.orm`. No structural caller changes.

Combined commit + tests (cycle #84 added 2 unit-tests pinning the
retry-loop semantics post-migration): `tests/referrals.rs` now has 8
passing (was 6).

`src/db/referrals.rs` is now 100% off raw SQL. The deferred
bonus_transactions INSERT from cycle #82 is part of `confirm_referral`
and was completed here. The `Pool` import was removed entirely from
this file — first DB-module file to be fully `DatabaseConnection`-only.

### Cycle #85 — orders.rs Part 1 (reads)

The biggest file in the migration plan (1271 lines) was split into 4
parts per the table. Part 1 = the read path only — safest first slice,
identical pattern to cycle #80's strain reads.

* `Order::from_row(&Row)` deleted (~40 lines) → `impl From<Model> for
  Order` with the same finite-clamp on f64 numeric columns. The
  `tokio_postgres::Row` import is gone from `src/db/orders.rs`. Write-
  side functions in the same file still use raw `tokio_postgres` —
  those move in Parts 2-4.
* `api/orders.rs::get_orders` (paginated admin): `find().order_by
  (CreatedAt, Desc).limit(N as u64).offset(M as u64).all()`. The
  `::float8` cast in the SQL goes away because sqlx auto-coerces
  NUMERIC↔f64 (lesson from #80).
* `api/orders.rs::get_order` (find_by_id): straightforward
  `find_by_id().one()`.
* `api/orders.rs::get_user_orders`: `find().filter(TelegramId.eq).
  order_by(CreatedAt, Desc).limit(50).all()`.
* The `get_user_orders_seaorm` dead-code wrapper at the top of
  `src/db/orders.rs` (from a prior staging step) is removed — the
  canonical reads now live in `api/orders.rs` per the call-site
  ownership model.

Part 2 (next cycle) tackles the big `create_order` transaction —
loyalty upsert + bonus deduction + idempotency check + order insert +
fraud event + manager attribution + auto-block. That's where the
`SeaORM tx` pattern from #83 will really earn its keep.

### Cycle #86 — Part 4 (sweeps) + memory capture (scope shift)

Originally cycle #86 was planned as Part 2 (`create_order` SeaORM tx).
Reading the function it's 400+ lines with 8 statements, multiple
conditional branches (bonus deduction only if `bonus_used > 0`, fraud
event only on 422, etc.), and `SELECT ... FOR UPDATE` on idempotency
keys. Migrating in a single cycle would have low confidence — the
scope-estimation table in `memory/seaorm-patterns.md` (written this
cycle) flags this exactly as "split into multiple cycles".

So #86 took two smaller pieces instead:

* **B (sweeps).** Migrated `cleanup_old_idempotency_keys`,
  `cleanup_old_fraud_events`, `cleanup_old_block_history`. All three
  used the same `audit_sweep_sql` builder (cycle #67) + raw
  `client.execute`. New pattern (#16 in seaorm-patterns):
  `orm.execute(Statement::from_string(DbBackend::Postgres, sql)).await?
  .rows_affected()`. Signature `&Pool` → `&sea_orm::DatabaseConnection`,
  error type `Box<dyn Error>` → `sea_orm::DbErr`. 3 main.rs callers
  updated mechanically (`db.pool.clone()` → `db.orm.clone()`).
* **C (memory capture).** Wrote `memory/seaorm-patterns.md` with 20
  validated patterns + anti-patterns + scope-estimation table + status
  of every db file. The 8 prior cycles repeatedly mentioned "should
  capture patterns to memory" but never did; #86 finally did the
  knowledge transfer so cycle #87 starts from a documented baseline.

`create_order` migration moves to cycle #87 as a single dedicated
cycle.

### Cycle #87 — `create_order` SeaORM tx (7 statements)

Biggest single tx in the project. The body shrank from 113 raw-SQL lines
to ~155 typed SeaORM lines (slightly bigger source but compile-time-safe
column refs and explicit ActiveModel inserts). Patterns used in
combination (look in `memory/seaorm-patterns.md` for each):
* #13 — tx via `begin().await? / commit().await?` / drop-rollback.
* #11/#12 — `Expr::cust_with_values("GREATEST(0, bonus_balance - $1)", [v])`
  for the column-expr bonus deduction with concurrent-safe guard
  (`Balance.gte(amount)` filter).
* #5 — `ActiveModel { col: Set(v), ..Default::default() }.insert(&tx)`
  for the order row and the idempotency_keys row.
* #16 — raw `Statement::from_sql_and_values` for `pg_advisory_xact_lock(...)`
  (no entity model for session primitives) and the 1-min rate-limit
  `SELECT 1 FROM orders WHERE created_at > NOW() - INTERVAL '1 minute'`
  (small bounded scan, doesn't justify an entity helper).

New entity: `order_idempotency_key` (migrations/029). 4 columns, used
twice in the tx — once for the SELECT replay check, once for the INSERT
that completes the tx.

Pattern that emerged this cycle (added to `memory/seaorm-patterns.md`
as #21 after-the-fact): **Postgres advisory locks via raw `Statement`
inside a SeaORM `DatabaseTransaction`** — pass `&tx` to
`tx.execute(Statement::from_sql_and_values(...))`. The `ConnectionTrait`
impl on `DatabaseTransaction` accepts raw statements identically to
`DatabaseConnection`. Advisory-lock primitives don't have a typed API;
this is the right escape hatch.

Cycle #88 still tackles the `update_order_status` tx (refund-on-reject)
and the fraud_event INSERTs. After that, `db/orders.rs` write-side is
complete.

### Cycle #88 — `update_order_status` (3 branches) + `complete_order_and_update_loyalty`

Last major txs in `orders.rs`. Both migrated in one cycle because the
patterns are now well-validated (cycles #83-#87) and the two flows are
tightly coupled: `update_order_status` delegates to
`complete_order_and_update_loyalty` for the happy path.

**`update_order_status`** (api/orders.rs handler) — three branches:
* `completed` → delegates to `db::orders::complete_order_and_update_loyalty(&db.orm, …)`.
* `rejected` → SeaORM tx: `find_by_id().lock_exclusive().one(&tx)` for
  the row-level FOR UPDATE, conditional bonus refund via
  `OnConflict::do_nothing` + column-expr `update_many` (patterns #9 +
  #11), then status flip via `update_many`.
* anything else → bare `update_many` with `rows_affected == 0 → 404`.

Removed the pattern of reading status outside the tx and then locking
inside — the cycle-#88 version reads once before deciding which branch
to take, and the tx itself FOR UPDATE-locks once it knows. Slight
duplication but clearer flow.

**`complete_order_and_update_loyalty`** (db/orders.rs helper) — 4-stmt
SeaORM tx:
1. `find_by_id(order_id).lock_exclusive().one(&tx)` — race guard.
2. Raw `Statement` for the `SELECT COUNT(*)` of prior completions
   (pattern #15; aggregate with `!=` clause doesn't justify a typed
   helper).
3. `update_many().col_expr(Status, Expr::value("completed"))` — the flip.
4. `LpEntity::insert(am).on_conflict(...value(TotalSpent, Expr::cust_with_values("COALESCE(...) + $1", [...]))...).exec(&tx)` —
   upsert with column-expression preserved value (pattern #10 — first
   used in cycle #84 for `record_referral`).
5. Raw `Statement` for the tier-CASE-WHEN subquery against
   `loyalty_config`.

Signature `&Pool` → `&sea_orm::DatabaseConnection`. Two callers updated:
* `api/orders.rs::update_order_status` (the `completed` branch).
* `bot/callbacks.rs:434` (admin confirm flow).

After this cycle, the only remaining raw-SQL in `db/orders.rs` is the
audit-table helpers (`record_fraud_event`, `record_block_history`,
`query_blocked_users`, `manual_unblock`, `order_stats_24h`,
`block_stats_24h`, `fraud_stats_24h`) — those are stat aggregations and
audit INSERTs that mostly use raw Statement already. Cycle #90 cleans
them up.

## Why this is worth doing

* **Single source of truth** — one ORM, one transaction model, one
  failure mode to reason about.
* **Type safety at the column level** — silenced-by-`unwrap_or_default`
  schema mismatches become compile errors.
* **Tests are cleaner** — entity-based fixtures are smaller than the
  `Row` mocks required for `tokio_postgres`.

But: this is **slow, non-glamorous work** with zero user-visible impact.
Don't prioritise it over user-facing cycles. Run it as background
maintenance, one file per loop, until it's done.
