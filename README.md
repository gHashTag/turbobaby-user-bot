# woody-weed-bot

Telegram Mini App marketplace. Rust backend (Axum + Teloxide), Dioxus/WASM
frontend, PostgreSQL.

## Setup for new contributors

```sh
# 1. Toolchain (rustup will read rust-toolchain.toml and install the right
#    channel + components + targets the first time you `cd` here)
cd woody-weed-bot

# 2. Git hooks (Lefthook drives pre-commit, pre-push, commit-msg)
brew install lefthook   # macOS — see https://lefthook.dev for other OS
lefthook install
```

That's it. The hooks enforce on every commit:

| Hook         | Check                                                        |
| ------------ | ------------------------------------------------------------ |
| `pre-commit` | block direct commit to `main`; `cargo fmt --check`; `cargo check --features backend`; `cargo check --target wasm32-unknown-unknown`; **`defensive-tests`** (see below) |
| `pre-push`   | block direct push to `refs/heads/main`                       |
| `commit-msg` | enforce Conventional Commits format                          |

If you ever need to bypass a hook (don't — fix the underlying issue):

```sh
LEFTHOOK=0 git commit -m '…'
```

## Quality / security gates

The codebase is defended in depth — when something looks wrong, a check
fails *somewhere* before it hits production. The full stack:

| Layer | Catches | Lives in |
| ----- | ------- | -------- |
| `defensive-tests` (pre-commit, cycles #104/#109) | schema-truth drift (`db::*` triad — manifest / entity columns / orphan tables), metric helper that's declared but unwired (`metrics::metric_wiring`), broken test code | `src/db/mod.rs`, `src/metrics.rs`, `lefthook.yml` |
| `cargo test --features backend` (CI) | every backend test — 517+ unit, validators, pure helpers | `.github/workflows/ci.yml` `check` job |
| `cargo clippy -- -D warnings` (CI, cycle #113) | any new lint on backend or WASM | same job |
| `cargo audit` (CI, cycle #107) | RustSec advisory database — vulnerable transitive deps | `audit` job + `docs/SECURITY_AUDIT.md` |
| `cargo deny check` (CI, cycle #115) | license allowlist, multi-version dups, unknown registries, wildcards | `deny` job + `deny.toml` |
| Server-side abuse guards (cycles #105/#106) | per-IP rate-limit on anonymous orders + failed admin auth | `src/api/rate_limit.rs` |
| Browser smoke test (cycle #114, manual) | WASM panic on init, broken CSS, layout regression | `docs/BROWSER_SMOKE.md` |

If a check feels wrong, **don't disable it** — the cycle history
(#101–#115) records why each one exists.

## Documentation

- [`docs/SEAORM_MIGRATION.md`](docs/SEAORM_MIGRATION.md) — the 17-cycle migration plan + status table
- [`docs/API_MIGRATION_PLAN.md`](docs/API_MIGRATION_PLAN.md) — callsite-by-file plan from cycle #91
- [`docs/PANIC_AUDIT.md`](docs/PANIC_AUDIT.md) — `.unwrap()`/`.expect()` baseline from cycle #77
- [`docs/TRY_GET_AUDIT.md`](docs/TRY_GET_AUDIT.md) — `try_get + unwrap_or` risk classification, cycle #97
- [`docs/ERROR_UX_AUDIT.md`](docs/ERROR_UX_AUDIT.md) — error UX inventory, cycle #73
- [`docs/SECURITY_AUDIT.md`](docs/SECURITY_AUDIT.md) — cargo audit findings + ignore rationale, cycle #107
- [`docs/BROWSER_SMOKE.md`](docs/BROWSER_SMOKE.md) — browser smoke test procedure, cycle #114
- [`docs/SMOKE_TESTS.md`](docs/SMOKE_TESTS.md) — manual smoke checklist (orders, fraud, blocks, etc.)
- [`docs/DESIGN_SYSTEM.md`](docs/DESIGN_SYSTEM.md) — UI tokens / variants
- [`docs/TZ2_MARKETING.md`](docs/TZ2_MARKETING.md) — ТЗ #2 marketing flags status (8 cycles, #130–#138)

## Commit message format

```
type(scope): description
```

Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`,
`ci`, `build`, `perf`, `wip`. Merge/Revert commits are auto-exempted.

Examples:

- `feat(menu): add filter by strain type`
- `fix(orders): close Two Generals window in checkout`
- `refactor(admin): split tab components`

## Day-to-day commands

```sh
# Backend (cargo binary)
cargo run --features backend

# Backend tests
cargo test --features backend

# WASM frontend
cargo check --target wasm32-unknown-unknown --no-default-features --lib

# Format
cargo fmt
```
