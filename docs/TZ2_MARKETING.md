# ТЗ #2 — marketing flags status

Single-document summary of cycles #130–#137 implementing
**"Управление скидками, акциями и отображением сортов через
админ-панель"**. Useful when:

- A PM needs proof a section of ТЗ #2 is delivered.
- A new contributor needs to extend marketing logic (e.g. add a
  fourth promo flag).
- An ops engineer needs to flip the customer-side display.

Cycle history at the bottom for chronology; the head of the doc is
"as-of-now" reference.

---

## What's implemented

All six ТЗ sections plus the "без участия программиста" requirement
are shipped. Recap:

| Section | Where |
| ------- | ----- |
| §1 Скидки на сорта | `migrations/028` columns + `EditStrainCard` form + `effective_strain_price` math + customer strikethrough render |
| §2 Сорт дня | `migrations/005` columns + dedicated `PUT /api/strains/:id/strain-of-day` + hero block in `menu_screen.rs` |
| §3 Бестселлеры | Per-strain toggle in `EditStrainCard`; bulk via `POST /api/strains/bulk-marketing` + admin checkbox/action-bar UI |
| §4 Новинки | Per-strain + bulk (same endpoint, `is_new_arrival` field); separate `🆕 New Arrivals` hero block |
| §5 Быстрое управление | Per-strain edit card + bulk action bar above strain list + global "hide all badges" admin button |
| §6 Priority order | `api/strains.rs::get_strains` `CASE` — SOTD → New → Best → Sale → Rest |

---

## Schema (migrations 028 + 032)

`migrations/028_strain_marketing_flags.sql` — seven nullable / default-off
columns on `strains`:

```sql
discount_percent  DOUBLE PRECISION NOT NULL DEFAULT 0
sale_price        DOUBLE PRECISION                    -- null = no override
sale_active       BOOLEAN          NOT NULL DEFAULT FALSE
sale_until        TIMESTAMPTZ                         -- null = no expiry
is_best_seller    BOOLEAN          NOT NULL DEFAULT FALSE
is_new_arrival    BOOLEAN          NOT NULL DEFAULT FALSE
new_until         TIMESTAMPTZ
display_order     INTEGER          NOT NULL DEFAULT 0
```

Plus `WHERE`-filtered indexes on each of the three `BOOLEAN` flags so
the priority-sort `CASE` is fast.

`migrations/032_loyalty_config_marketing_badges_hidden.sql` — adds the
admin-controlled global toggle:

```sql
ALTER TABLE loyalty_config
  ADD COLUMN IF NOT EXISTS marketing_badges_hidden BOOLEAN NOT NULL DEFAULT FALSE;
```

---

## API endpoints

| Method | Path | Purpose | Cycle |
| ------ | ---- | ------- | ----- |
| `GET`  | `/api/strains` | List strains. Marketing flags are masked off when **either** the env override or the DB toggle is on, and `include_hidden=1` is not passed (admin path bypasses the mask). | #133-B + #136 |
| `PUT`  | `/api/strains/:id` | Update one strain — accepts all 7 marketing fields as an authoritative snapshot. | (pre-ТЗ #2) |
| `PUT`  | `/api/strains/:id/strain-of-day` | Dedicated SOTD endpoint, preserves `strain_of_day_set_at` audit column. | (pre-ТЗ #2) |
| `POST` | `/api/strains/bulk-marketing` | Bulk toggle. Body: `{ ids: [...], is_best_seller: bool?, is_new_arrival: bool?, sale_active: bool? }`. Each flag is `Option<bool>` — `None` leaves the field untouched. Validator: 1..=500 ids, 1..=200 chars each, at least one flag must be set. | #133-C |
| `GET`  | `/api/admin/marketing-display` | Returns `{ hidden, db_hidden, env_override }`. | #136 |
| `PUT`  | `/api/admin/marketing-display` | Sets `loyalty_config.marketing_badges_hidden`. Falls back to insert if id=1 doesn't exist. Invalidates the strains ETag cache. | #136 |

---

## Toggle state — two inputs

`api/strains.rs::marketing_badges_hidden(state).await` OR-combines:

1. **`HIDE_MARKETING_BADGES=1` env var** — set in Railway / `.env`.
   Restart needed to change. Use as an ops kill switch
   independent of DB state.
2. **`loyalty_config.marketing_badges_hidden` (BOOLEAN)** — admin
   self-service via the StrainsTab toggle button. No restart.

Either being `true` hides the customer-facing Sale / Best Seller /
New Arrival badges. **SOTD is never hidden** — it's the headline
daily feature, not a promo.

---

## Customer render paths

Two implementations, in sync via `crate::trios::pricing::effective_strain_price`:

- `src/ui/screens/menu_screen.rs::render_strain_card` — the
  production path used by the actual menu screen. Hero blocks for
  SOTD + New Arrivals.
- `src/ui/components/strain_card.rs::StrainCard` — generic component
  (used today only via `StrainGrid`), upgraded in #131 to render all
  four badges + strikethrough sale-price. CSS classes in
  `styles/main.css` (#132).

The CSS regression test (`src/main.rs::css_class_consistency_tests`,
cycle #133-A) ensures any `class:` literal in JSX has a matching CSS
rule — the next "added class, forgot CSS" gap fails CI before merge.

---

## Admin UI flows

`StrainsTab` in `src/ui/screens/admin_screen.rs`:

- **Per-strain**: edit any strain → `EditStrainCard` expands inline
  with all six toggles (SOTD, Sale, Best Seller, New Arrival, plus
  `display_order` and the two `*_until` datetime pickers).
- **Bulk**: select strains via checkbox column → action bar appears
  with six buttons (ON+OFF × three flags) + a "Clear" link.
- **Global hide toggle**: small button under the "Страйны (N)"
  header — fetches current state on mount, flips via PUT, optimistic
  UI update.

Strict input validation for the **treasure hunt** lat/lon fields
(unrelated module, surfaced during this work) shipped in #137 via
`crate::trios::validation::parse_finite_float_in_range`.

---

## Known limits / deferred

* **No dedicated "quick management" dashboard screen.** ТЗ §5 implies
  a separate page. The action bar above the strain list provides the
  same functional surface (apply multiple flags from one click), but
  the visual "centralised dashboard" framing is missing. Low impact
  since the bar covers the actual workflow.
* **Bulk endpoint doesn't toggle `display_order` or `sale_until`.**
  Only the three booleans. ТЗ §3 §4 strictly speak of "одновременно
  несколько сортов" with boolean state, so this is in spec.
* **No bulk for accessories / tea / sets.** Catalog tables have
  `discount_percent` and `is_deal_of_day` only — no Best / New /
  Sale model. ТЗ #2 is strain-specific; deferred until a similar TZ
  for catalog.
* **No `cargo test` integration test for `/api/strains/bulk-marketing`.**
  The validator has seven unit tests (#133-C); the full POST-DB-GET
  round-trip is covered by manual smoke only. Add when other DB-bound
  integration tests get a runner (currently `tests/referrals.rs` uses
  `#[ignore]`).

---

## Cycle history

| # | Output |
| - | ------ |
| #130 | Audit of original ТЗ #2 — surfaced 6 gaps |
| #131 | `StrainCard` renders all four badges + strikethrough price |
| #132 | CSS rules for `.new-badge` / `.best-badge` / `.sale-badge` / strikethrough |
| #133-A | CSS-class-vs-CSS-rule regression test (`every_jsx_class_literal_has_a_css_rule`) |
| #133-B | `HIDE_MARKETING_BADGES` env var + `mask_marketing_flags` helper + `parse_bool_env` |
| #133-C | `POST /api/strains/bulk-marketing` endpoint + validator + 7 unit tests |
| #134 | Admin checkbox column + bulk-action bar (ON buttons) |
| #135 | Bulk OFF buttons — symmetric `ON`/`OFF` × `{BEST, NEW, SALE}` |
| #136 | Migration 032 + DB toggle + `PUT /api/admin/marketing-display` + admin UI button |
| #137 | `parse_finite_float_in_range` validator — treasure-hunt lat/lon strict parse (adjacent fix) |
| #138 | this document |

15 cumulative new tests across the chain. 569 backend tests pass as
of cycle #137.
