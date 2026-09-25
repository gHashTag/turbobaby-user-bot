# TurboBaby — documentation

TurboBaby is a motorbike **rental** in Kamala, Phuket, run as a Telegram Mini App. The
repository's front page is [`../README.md`](../README.md); the decisions that outrank the code
are in [`../DECISIONS.md`](../DECISIONS.md). This page lists what is in `docs/`.

## The `.t27` contracts

- [`t27-handover.md`](t27-handover.md) — where the contract migration stands, the four gates and how to run them, what is left.
- [`t27-contract-map.md`](t27-contract-map.md) — what each contract under `specs/` owns and what it defers to.
- [`t27-reuse-map.md`](t27-reuse-map.md) — what the published `.t27` corpus already offers, measured 2026-09-12.
- [`t27-semantic-merge-matrix.md`](t27-semantic-merge-matrix.md) — the history of one merge of spec drafts, 2026-09-13.

## Deploy and operations

- [`ROLLBACK.md`](ROLLBACK.md) — rolling a deployment back, and the fail-closed deploy run.
- [`SMOKE_TESTS.md`](SMOKE_TESTS.md) — local write checks of the order endpoint's defences (not the post-deploy check).
- [`BROWSER_SMOKE.md`](BROWSER_SMOKE.md) — does the WebAssembly bundle in `dist/` mount.
- [`SETUP_TEMPLATE.md`](SETUP_TEMPLATE.md) — standing up a new copy of the bot.
- [`grafana-dashboard.json`](grafana-dashboard.json), [`prometheus-alerts.yaml`](prometheus-alerts.yaml), [`prometheus-recording-rules.yaml`](prometheus-recording-rules.yaml) — monitoring.

## Audits and plans

- [`SECURITY_AUDIT.md`](SECURITY_AUDIT.md) — `cargo audit` findings and why each ignored advisory is ignored.
- [`PANIC_AUDIT.md`](PANIC_AUDIT.md) — the `.unwrap()` / `.expect()` baseline.
- [`TRY_GET_AUDIT.md`](TRY_GET_AUDIT.md) — `try_get` with a silent default, classified by risk.
- [`ERROR_UX_AUDIT.md`](ERROR_UX_AUDIT.md) — how each screen reports an API error.
- [`SEAORM_MIGRATION.md`](SEAORM_MIGRATION.md) and [`API_MIGRATION_PLAN.md`](API_MIGRATION_PLAN.md) — the move to SeaORM.

## Interface

- [`DESIGN_SYSTEM.md`](DESIGN_SYSTEM.md) — UI tokens and variants.
- [`event-share-templates.md`](event-share-templates.md) — the deep links already in posts; since 2026-09-26 it holds no template to post.

## History, kept as written

- [`TZ2_MARKETING.md`](TZ2_MARKETING.md) — the status of an earlier marketing-flags task.
- [`reports/`](reports/) — verification reports from earlier work on this codebase, and a competitor survey of Phuket bike rentals ([`reports/competitor-landscape-2026-09.md`](reports/competitor-landscape-2026-09.md)).

These describe the code as it was when they were written and are not updated as it changes.

## Anchor

`φ² + φ⁻² = 3`
