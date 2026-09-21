# turbobaby-bot

[![CI](https://github.com/gHashTag/turbobaby-user-bot/actions/workflows/ci.yml/badge.svg)](https://github.com/gHashTag/turbobaby-user-bot/actions/workflows/ci.yml)

TurboBaby — motorbike **rental and sales** in Kamala, Phuket, as a Telegram Mini
App. Rust backend (Axum + Teloxide), Dioxus/WASM frontend, PostgreSQL.

The tree is a full-history derivative of
[`gHashTag/woody-weed-bot`](https://github.com/gHashTag/woody-weed-bot), so
anything still speaking of weed, strains, garden or `woody-weed` is rebrand
debt rather than a feature. It is intentionally not marked as a formal GitHub
fork: the published t27 world scanner skips repositories with `isFork: true`,
which would make this repository's `.t27` contracts invisible. Add the lineage
remote to a fresh clone with:

```sh
git remote add upstream git@github.com:gHashTag/woody-weed-bot.git
```

## Where the truth lives

Four documents outrank the code, and all four outrank an issue's prose:

| file | what it settles |
| ---- | --------------- |
| [`DECISIONS.md`](DECISIONS.md) | the cross-subsystem decisions D1–D20 — price authority, nullable money, the CLICK 125 exclusion, the PII prohibition list |
| [`data/fleet_seed.json`](data/fleet_seed.json) | every published number, with the source that published it. De-identified; the raw sheets never enter this repo |
| [`specs/`](specs/) | the `.t27` contracts the two language runtimes are checked against — **45 tracked specs**, indexed by [`docs/t27-contract-map.md`](docs/t27-contract-map.md), including [`specs/agents/turbobaby.t27`](specs/agents/turbobaby.t27). To pick the work up: [`docs/t27-handover.md`](docs/t27-handover.md) |
| [`.claude/agents/turbobaby-bot.md`](.claude/agents/turbobaby-bot.md) | the **owner agent** of this repository: its scope, its tools, and the four rules it can never violate |

## Setup for new contributors

```sh
# 1. Toolchain (rustup will read rust-toolchain.toml and install the right
#    channel + components + targets the first time you `cd` here)
cd turbobaby-user-bot

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
| `cargo test --features backend` (CI) | every backend test — unit, validators, pure helpers | `.github/workflows/ci.yml` `check` job |
| `cargo clippy -- -D warnings` (CI, cycle #113) | any new lint on backend or WASM | same job |
| `cargo audit` (CI, cycle #107) | RustSec advisory database — vulnerable transitive deps | `audit` job + `docs/SECURITY_AUDIT.md` |
| `cargo deny check` (CI, cycle #115) | license allowlist, multi-version dups, unknown registries, wildcards | `deny` job + `deny.toml` |
| Server-side abuse guards (cycles #105/#106) | per-IP rate-limit on anonymous orders + failed admin auth | `src/api/rate_limit.rs` |
| `verify_fleet_seed.py` (CI) | a price, deposit or unit count edited in the seed migration but not in the provenance file — or the reverse | `seed` job + `scripts/verify_fleet_seed.py` |
| Browser smoke test (cycle #114, manual) | WASM panic on init, broken CSS, layout regression | `docs/BROWSER_SMOKE.md` |

If a check feels wrong, **don't disable it** — the cycle history
(#101–#115) records why each one exists.

## Deploy

Railway, from `main`, via
[`.github/workflows/deploy.yml`](.github/workflows/deploy.yml). `Dockerfile`
builds the musl server binary (`turbobaby-bot-server`) and copies the committed
`dist/` bundle; [`railway.toml`](railway.toml) pins the healthcheck to `/health`
(which pings the DB and returns 503 when it is unreachable) and the Singapore
region, because the customer base is in Thailand.

**The deploy target does not exist yet.** This repository has no Actions secrets
and no Actions variables (measured 2026-09-13), and the inherited workflow
pushed to a Railway service named after the cannabis-era bot using a token that
is not valid here — every run failed in ~11 s with `Invalid RAILWAY_TOKEN`.
Renaming the service string in the workflow would only have swapped one
non-existent target for another, so the job now reads its target from
configuration and skips with a `NO DEPLOY HAPPENED` warning until a human
creates both of these:

| what | where | why it can't be created from CI |
| ---- | ----- | ------------------------------- |
| `RAILWAY_TOKEN` — secret | Settings → Secrets and variables → Actions → Secrets | tokens are issued in the Railway dashboard |
| `RAILWAY_SERVICE` — variable | Settings → Secrets and variables → Actions → Variables | the service is created (or renamed) in the Railway dashboard |

No secret is committed, and no deploy target is invented.

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
- [`.claude/agents/turbobaby-bot.md`](.claude/agents/turbobaby-bot.md) — the `turbobaby-bot` agent definition: repository scope, the D14 PII prohibition, D11 price provenance, laws L1/L3/L4 and `Closes #N` discipline (issue #23)

## Commit message format

```
type(scope): description
```

Allowed types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`,
`ci`, `build`, `perf`, `wip`. Merge/Revert commits are auto-exempted.

Examples:

- `feat(catalog): filter bikes by class and displacement`
- `fix(orders): close Two Generals window in checkout`
- `refactor(admin): split tab components`

Every merge names the issue it closes (`Closes #N`) — law L1, restated in the
agent definition.

## Day-to-day commands

```sh
# Backend (cargo binary — the crate declares two bins, so name the server one)
cargo run --features backend --bin turbobaby-bot-server

# Backend tests
cargo test --features backend

# WASM frontend
cargo check --target wasm32-unknown-unknown --no-default-features --lib

# Format
cargo fmt
```
