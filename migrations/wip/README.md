# Unfinished migrations

Files here are **not** part of `MIGRATION_SQL` and never run. They live in a
subdirectory on purpose: `migration_files()` in `src/db/mod.rs` reads
`migrations/` non-recursively and keeps only `*.sql`, so a directory entry is
skipped. A half-finished migration sitting in `migrations/` proper turns every
`db::` test red, and the pre-commit hook then blocks *all* commits in the repo —
which is what happened here.

## 077_quest_progress.sql

Server-side quest checkpoint progress (ordered QR scans that survive an app
restart). Was on disk as an untracked `039_quest_progress.sql`, colliding with
the existing `039_sets_columns.sql`; renumbered to the next free slot.

The rest of the change is in a git stash — `stash@{0}: On main: quest-qr-wip` —
which rewrites `src/api/quest.rs` (+105/-45) and adds the `include_str!` line to
`src/db/mod.rs`. It carries **no tests**, and nothing in `src/` references
`user_quest_progress` or `sequence_order` yet, so the table would be an orphan
if wired in as-is.

To finish it:

1. `git stash pop` and re-apply the quest.rs work.
2. Move this file up to `migrations/` — **and renumber it first.** 077 is no longer
   free: the bike catalog migrations took 077 onward (see `DECISIONS.md` D1, forced by
   the gap-free numbering test). Take the next free slot above the highest
   `migrations/*.sql` on disk at the time you promote this.
3. Point the `include_str!` in `src/db/mod.rs` at `077_quest_progress.sql` —
   the stash still names `039_quest_progress.sql`.
4. Cover the ordering gate and the restart-survival claim with tests; extend
   `tests/integration_quest_location.rs`.
5. `cargo test --features backend --bin turbobaby-bot-server -- db::` to confirm
   the wiring, numbering and orphan-table guards all pass.
