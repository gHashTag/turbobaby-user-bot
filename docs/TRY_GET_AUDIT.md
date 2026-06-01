# `try_get + unwrap_or_default` audit (cycle #97)

After the 17-cycle SeaORM migration finished (cycle #96), the codebase
has **93 `try_get::<T>("", "col").unwrap_or_*` callsites**. Each one
silently returns a default if the column is missing, NULL, or of the
wrong type. That's mostly fine — but for `NOT NULL` columns, the
default would mask schema drift (column renamed, type changed, dropped).

This doc inventories the callsites by file, classifies risk, and lays
out the fix plan. Companion to `docs/PANIC_AUDIT.md` (cycle #77, which
covered `.unwrap()` panics).

## Callsite distribution

| File | Callsites |
|------|-----------|
| `src/api/catalog.rs` | 32 |
| `src/api/tech_tree.rs` | 24 |
| `src/api/garden.rs` | 13 |
| `src/api/quest.rs` | 11 |
| `src/api/loyalty.rs` | 7 |
| `src/api/orders.rs` | 3 |
| `src/bot/callbacks.rs` | 2 |
| `src/api/admin.rs` | 1 |

Total: 93 across 8 files.

## Risk classification

Apply this rule to each callsite:

* **🟢 NULL-able OK**: column is `NULL`-able in the schema, and the
  default is a sane "no value" representation (empty string for `name`,
  `false` for `is_blocked`, etc.). Examples: `r.try_get::<Option<String>>("", "first_name").ok().flatten()`.
* **🟡 NOT NULL — silent on type drift**: column is `NOT NULL` so a missing-value would never happen — but a renamed column or type change would silently return the default. Real bug, will surface as "all values look reset" in admin views. Examples: `r.try_get::<i32>("", "stock").unwrap_or(0)` on `accessories.stock NOT NULL DEFAULT 0`.
* **🔴 NOT NULL — semantically dangerous**: as above AND the default value is *plausible business data* (price=0.0, balance=0.0, status="pending"). A schema break would silently zero out prices on the menu, balances in loyalty, statuses in orders. Examples: `r.try_get::<f64>("", "price").unwrap_or(0.0)`.

## Sampled findings

### `api/catalog.rs` (32 callsites)

5 row helpers (accessory_row, accessory_set_row, tea_product_row, tea_set_row, set_row). All read NOT NULL columns from catalog tables.

* `r.try_get::<String>("", "id").unwrap_or_default()` — id is TEXT PRIMARY KEY. 🟡 If row exists, id is non-null. But "" id slips through to JSON which is silently wrong.
* `r.try_get::<String>("", "name").unwrap_or_default()` — NOT NULL. Same shape. 🟡
* `r.try_get::<f64>("", "price").unwrap_or(0.0)` — **NOT NULL DEFAULT 0**. 🔴 Schema drift → all prices read as 0 → menu shows "free items". Already has `if v.is_finite() { v.max(0.0) }` clamp on top, but that doesn't catch "all 0s from a wrong column".
* `r.try_get::<i32>("", "stock").unwrap_or(0)` — **NOT NULL DEFAULT 0**. 🟡 Stock=0 means out-of-stock; default matches missing semantically.
* `r.try_get::<bool>("", "is_available").unwrap_or(false)` — **NOT NULL DEFAULT true**. 🟡 But default of `false` here is conservative (hides item rather than wrongly shows it).
* Array columns: `r.try_get::<Vec<String>>("", "accessories").unwrap_or_default()` — empty vec on drift = empty set in JSON. 🟡

**Recommendation**: keep `unwrap_or_default` for strings/arrays/bools (defaults are safe). Audit price/total_price callsites — those should propagate as `Result<_, StatusCode>` instead.

### `api/tech_tree.rs` (24 callsites)

Same shape: 13-column `tech_nodes` and 7-column `achievements`. All NOT NULL except some Optional in schema (`unlocks`, `features` as text[]).

* All-zero defaults on `xp_required` / `xp_reward` / `estimated_hours` / `priority` — 🟡 NOT NULL, schema drift would zero out skill-tree progression UI. Cosmetic only — not financial.

**Recommendation**: low priority, cosmetic. Don't fix unless skill tree gets fees attached.

### `api/garden.rs` (13 callsites)

Plant + reward state. Critical for game logic.

* `r.try_get::<bool>("", "is_completed").unwrap_or(false)` — NOT NULL. 🟡 Default-false means "still growing" — safe.
* `r.try_get::<i32>("", "water_count").unwrap_or(0)` — NOT NULL. 🟡 Default-0 means "freshly planted" — safe but visible.
* `r.try_get::<i32>("", "discount_percent").unwrap_or(0)` on garden_rewards — NOT NULL. 🟡 Zero discount is a degraded but not catastrophic reward.

**Recommendation**: monitor. Garden is non-financial UX, low-priority.

### `api/loyalty.rs` (7 callsites)

`loyalty_tiers` read + `loyalty_profiles` read.

* `r.try_get::<f64>("", "points_multiplier").unwrap_or(0.0)` — **NOT NULL DEFAULT 1.0** (per migration). 🔴 Schema drift → all multipliers read as 0 → all tier-based discounts wiped.
* `r.try_get::<i32>("", "min_points").unwrap_or(0)` — NOT NULL. 🟡 All tiers default to 0 threshold = everyone is in every tier. 🔴 actually — financial impact.

**Recommendation**: fix. `loyalty_tiers` reads should return `Result<_, StatusCode>` and 500 on parse error.

### `api/orders.rs` (3 callsites)

Price-auth lookups in create_order — accessories / tea / sets prices.

* `r.try_get::<f64>("", "price").unwrap_or(0.0)` — **NOT NULL DEFAULT 0**. 🔴 **CRITICAL.** Schema drift → price=0 → server-side authority says "this item costs 0" → free orders pass validation.

**Recommendation**: **fix this immediately or pin in CI**. Single-row read; propagate error on missing column.

### `api/quest.rs` (11 callsites)

QR scan flow + quest places.

* `r.try_get::<f64>("", "lat").unwrap_or(0.0)` — NOT NULL. 🟡 Map will show point at (0, 0) — visible bug but not financial.
* `r.try_get::<bool>("", "is_final").unwrap_or(false)` — NOT NULL. 🟡 Default-false means QR doesn't trigger "completion" — degraded UX but not exploitable.

**Recommendation**: low priority.

### `api/admin.rs` + `bot/callbacks.rs` (3 callsites)

Admin tooling + referral lookups. Low-volume, admin-visible.

* `r.try_get::<i64>("", "referrer_id").unwrap_or(0)` on referral_events — NOT NULL. 🟡 Zero would prevent notification but is detectable.

## Priority queue for follow-up cycles

* **Cycle #98 (proposed):** Fix the 3 🔴 callsites in `api/orders.rs` price-auth — propagate `?` instead of `unwrap_or(0.0)`. Same fix for `loyalty_tiers.points_multiplier`. ~50 lines.
* **Cycle #99 (lower priority):** Audit the 🟡 callsites where default of `0` / `false` / `""` could be visually misleading in admin views. Add `tracing::warn!` when a NOT NULL read returns the default (proxies as "schema drift detected").
* **Defer:** 🟢 callsites — that's expected NULL-able behaviour. No fix.

## How to validate this audit

Each callsite needs the corresponding schema migration checked. Schema files: `migrations/001_initial.sql` through `migrations/031_block_history.sql`. The `NOT NULL`-vs-`NULL` distinction lives there.

This audit is a snapshot; it should be re-run if migrations add new tables or rename columns. The 93-count tracking metric: re-grep `try_get.*unwrap_or` and compare.
