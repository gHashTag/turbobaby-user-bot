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
| #80 (next) | `src/db/strains.rs` (read-side first) | ~120 | small/medium | |
| #81 | `src/db/strains.rs` (write-side + marketing flags) | ~130 | medium | |
| #82 | `src/db/loyalty.rs` | 182 | medium | |
| #83 | `src/db/referrals.rs` (split into 2 cycles if needed) | 417 | medium/large | |
| #84-#86 | `src/db/orders.rs` (split into 3-4 cycles by feature) | 1271 | large | |

Total ≈ 6-8 cycles. Each cycle is independently committable; no big-bang.

### Validated pattern (cycle #79)

The 6 user-related methods on `Database` migrated cleanly:

* **Reads (find_by_id)**: `Entity::find_by_id(pk).one(&self.orm).await?.map(|m| m.field)`. Failure path: convert `Result<Option<Model>>` to whatever the caller's signature expects; for the legacy `Option<String>` return shape, log+drop the Err.
* **Upserts (ON CONFLICT)**: `Entity::insert(am).on_conflict(OnConflict::column(pk).update_columns([...]).to_owned()).exec(&self.orm)`. Build `ActiveModel` with `Set(value)` for written columns and `..Default::default()` for everything else — that lets the DB DEFAULT clause fire on insert path.
* **Updates (no-row tolerant)**: `Entity::update_many().col_expr(col, Expr::value(v)).filter(pk.eq(id)).exec(&self.orm)`. Use `update_many` instead of `update` to silently no-op when the row doesn't exist — matches the raw-SQL `UPDATE ... WHERE id = $1` semantics.

The 60-line migration touched zero callsites — all consumers go through `Database::method(...)` so the implementation swap is invisible.

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
