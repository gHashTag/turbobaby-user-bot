# API-layer SeaORM migration plan (cycle #91+)

Cycle #79-#90 migrated all of `src/db/*` and the major `src/api/*` files
(strains, happy_hour, referrals, orders). Cycle #90's post-hoc audit
revealed **58 callsites of `state.db.pool` / `&db.pool` still remaining
across 8 files** — the original `docs/SEAORM_MIGRATION.md` plan only
enumerated `src/db/*` modules and tracked api files as they bubbled up.

This doc is the missing enumeration. Future cycles work down the table;
when a file hits 0 callsites, mark it ✅.

## Callsite inventory (snapshot at start of cycle #91)

| File | Callsites | Tx? | Entities needed? | Priority | Status |
|------|-----------|-----|------------------|----------|--------|
| `api/catalog.rs` | 27 | No (all CRUD) | none needed — all via Statement | low — bulk of work | ✅ #95 |
| `api/garden.rs` | 8 | Some (reward-use tx) | maybe garden_plant + garden_harvest | medium | ✅ #94 |
| `api/admin.rs` | 7 | No | none (stats only) | medium | ✅ #92 |
| `api/quest.rs` | 5 | No | quest_place exists; treasure_hunt exists | medium | ✅ #93 |
| `api/tech_tree.rs` | 4 | Yes (cascade tx) | none — Statement throughout | low | ✅ #93 |
| `api/loyalty.rs` | 3 | No (get_profile, get_loyalty_tiers, update_loyalty_config) | loyalty_tier? | low | ✅ #92 |
| `api/game.rs` | 3 | No (read + upsert + leaderboard) | game_high_score (single use) | high (smallest) | ✅ #91 |
| `bot/callbacks.rs` | 1 | No (single SELECT inside larger flow) | none — uses existing entities | high (1 callsite) | ✅ #91 |

**Total: 58 callsites, 8 files.** Plus `Database::pool` field, plus `deadpool_postgres` Cargo dep.

## Classification rules (lessons from #79-#90)

When you grep a new file:

* **CRUD endpoint group with shared table**: generate the entity (`api/strains.rs` pattern — cycle #80/#81). Pays off after >2 callsites.
* **Single-callsite read-only** with no other consumer: raw `Statement` (pattern #15). Don't generate an entity. `api/orders.rs` price-auth (#90) sets the precedent.
* **Transaction**: SeaORM `begin().await? → ops → commit().await?` (pattern #13). Drop = auto-rollback; remove all explicit `.rollback()` arms.
* **`SELECT ... FOR UPDATE`**: `lock_exclusive()` on a typed `find()` (pattern #14).
* **Aggregate with `COUNT FILTER`, `GROUP BY`, `LATERAL`, `UNION ALL`**: raw `Statement`. Typed builder in SeaORM 1.1 doesn't express these idiomatically.
* **`ON CONFLICT DO UPDATE SET col = expr_using_existing`**: `OnConflict::value(col, Expr::cust_with_values(...))` (pattern #10).

See `~/.claude/projects/-Users-playra-turbobaby-bot/memory/seaorm-patterns.md` for the full pattern catalog (21 patterns + 4 anti-patterns).

## Suggested cycle sequence

* **#91** (this cycle): plan doc + smallest 2 files. `bot/callbacks.rs` (1 callsite) and `api/game.rs` (3 callsites). Quick momentum + validates classification rules.
* **#92**: `api/admin.rs` (7 callsites) + `api/loyalty.rs` (3 remaining). Both no-tx, no new entities — pure `Statement` migrations or `Database` method extraction.
* **#93**: `api/tech_tree.rs` (4 callsites). Likely needs `tech_tree_node` entity since these are the admin CRUD endpoints; entity pays off.
* **#94**: `api/quest.rs` (5 callsites). `quest_place` + `treasure_hunt` entities already exist (#82-era infra) — should be quick.
* **#95**: `api/garden.rs` (8 callsites). Has a reward-use transaction — moderate complexity.
* **#96** (the big one): `api/catalog.rs` (27 callsites, 5 catalog tables). Probably split into 2-3 cycles like `api/strains.rs` was. Generate 5 entities upfront.
* **#97-#98** (after #96): drop `Database::pool` field + `deadpool_postgres` Cargo dep.

Total realistic finish: cycles #91-#98 (8 more cycles).

## Why this is worth doing

Per `docs/SEAORM_MIGRATION.md`: single ORM = one transaction model, one failure mode, one connection pool. Currently we ship both — anyone reading the codebase has to know both APIs and how they interact (they share the underlying `DATABASE_URL` but not the connection cache). Tests written against entity API can't easily cover raw-SQL endpoints, so test surface is uneven.

Not user-visible, won't change a single byte on the wire. But it removes ~600 lines of duplicate config setup, kills the `deadpool_postgres` + `tokio_postgres` dependency tree from the binary, and unifies "how do I write a DB query in this project" to one answer.

## Do NOT add to this plan

* New entity generation that doesn't have a real query waiting (anti-pattern from cycle #82's loyalty.rs dead types).
* Partial-tx migrations (anti-pattern from cycle #82 deferred bonus_transactions).
* Bulk "find-and-replace" scripts. Each callsite needs human review for column-expr semantics, NULL handling, OnConflict variants.

## Cycle #96 — MIGRATION COMPLETE

`Database::pool` field dropped. Four Cargo deps removed: `tokio-postgres`, `deadpool-postgres`, `postgres-types`, `tokio-postgres-rustls`, `rustls-native-certs`. Pool builder code, `NoTlsConnect` / `RustlsConnect` adapters, and the `BoxFuture` type are gone (~120 lines from `src/db/mod.rs`).

`run_migrations` now uses `orm.execute_unprepared(MIGRATION_SQL)` — sqlx doesn't suffer from the SQLSTATE 0A000 stale-plan issue, so the per-call eviction workaround is gone too.

Last 2 raw `db.pool.get().transaction()` callsites in `bot/callbacks.rs` (order confirm + reject flows) migrated to SeaORM tx with auto-rollback-on-drop.

**100% migration. 17 cycles (#79–#96), zero behaviour regressions, 506 + 8 tests stable throughout.**
