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

That's it. The hooks now enforce on every commit:

| Hook         | Check                                                        |
| ------------ | ------------------------------------------------------------ |
| `pre-commit` | block direct commit to `main`; `cargo fmt --check`; `cargo check --features backend`; `cargo check --target wasm32-unknown-unknown` |
| `pre-push`   | block direct push to `refs/heads/main`                       |
| `commit-msg` | enforce Conventional Commits format                          |

If you ever need to bypass a hook (don't — fix the underlying issue):

```sh
LEFTHOOK=0 git commit -m '…'
```

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
