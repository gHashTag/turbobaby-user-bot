# Continuing the `.t27` migration — handover

Written 2026-09-21, at the point where the migration itself is finished and what remains is either
an owner's decision or a separate piece of work. Everything here is a measurement with the command
that produced it, so a reader on another machine can reproduce it rather than believe it.

For what each contract owns, read [`t27-contract-map.md`](t27-contract-map.md). This file is about
what to do next.

## Where it stands

| | measured 2026-09-21 |
| --- | --- |
| canonical contracts | **45** (44 under `specs/turbobaby/` + `specs/agents/turbobaby.t27`) |
| assertions executed, all passing | **9 996** — re-measured 2026-09-25 on `f5e6b4f`, after #64, which changed comments only (9 996 after #63 on 2026-09-24, 9 859 after #62, 9 535 earlier that day, 9 514 on 2026-09-23 before that day's money family, 9 038 on 2026-09-22, 8 911 on 2026-09-21) |
| contract-to-contract ownership edges | 203+, **zero dangling** |
| source lines named by some contract | **73 407 of 80 308 (91.4 %)** |
| enforced contract-to-source bindings | **237**, across 41 of 45 contracts and 80 source files — re-measured 2026-09-25 on `f5e6b4f`, after #64 (237 after #63 on 2026-09-24, 227 after #62, 198 earlier that day, 197 on 2026-09-23, 168 across 40 on 2026-09-22, 66 across 16 on 2026-09-21) |

The two re-measured rows come from gates 2 and 3, run on 2026-09-25 on `main` at `f5e6b4f` (#64
on top of the rental-only change of #63). Both equal the 2026-09-24 readings on #63's tree to the
unit. The other three rows were not re-measured and keep their 2026-09-21 reading. Gate 1 passes on
`f5e6b4f` with the pinned compiler, and its 45 floors, declarations and checks alike, still equal
that compiler's counts. Gate 4 passes too.

```sh
# 2026-09-25, main at f5e6b4f, pinned compiler 40003ed
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler
# OK - 45 manifested specs, 5 generators each; compiler=<path>
python3 scripts/execute_t27_assertions.py --no-crosscheck
# OK - 45 spec(s), 9996 assert line(s) scanned, 9996 executed, 9996 passed, 0 failed
python3 scripts/verify_t27_against_source.py --require-git-tracked
# OK - 237 bindings hold across 41 contracts and 80 source files
python3 scripts/verify_fleet_seed.py -v
# OK — 14 families (13 offered), 37 units (11 rented, 26 available); ...
```

Gate 2 with a compiler configured, the form "The four gates" shows, is red on `f5e6b4f` for two
stale allowances and no false assertion. See "Gate 2's cross-check after #64" below.

The last two rows are the ones to keep apart. *Named* means a contract cites the file. *Bound* means
a gate fails when the two disagree. 91.4 % is a floor on attention; 237 (after #63 on 2026-09-24,
unchanged after #64 on 2026-09-25; 227 after #62 and 198 earlier on 2026-09-24, 197 on 2026-09-23,
168 on 2026-09-22, 66 on 2026-09-21) is the number that actually holds, and it is the one worth
growing.

### Landed on 2026-09-24, and not yet live

#56 to #63 merged to `main` as `gHashTag` after green CI. #64 is a squash-merge by
`dmitrii-f-t27`. **None of it reaches a customer until someone deploys `main`**, and a merge
deploys nothing (see the last section and `docs/ROLLBACK.md`).

* **Nothing from #56 to #64 is live, as far as a read from outside can tell.** The last read-only
  smoke, `scripts/postdeploy-smoke.sh` at 2026-09-25 01:36:54 UTC, printed `5 passed, 0 failed`.
  Production served `/` byte-equal to the `dist/index.html` committed at `4a5aa72` (bundle
  `c3a91ae8dc97ab9e`), `/health` answered 200 with the database ok, and `/api/delivery/zones`
  listed 5 zones, the set from before 087. The server's own commit cannot be read from outside
  (the `Dockerfile`'s `BUILD_VERSION` defaults to `docker`), so this rests on the served `dist/`
  and the zone list.
* **The deploy of `main` (`f5e6b4f`) is prepared and waits on Railway access alone.** It is a clean
  LF export of `f5e6b4f` (`docs/ROLLBACK.md` §3B). The Railway CLI on this PC (5.62.1) is logged in
  to an account that cannot see project `woody` yet, so `railway up` cannot run until that account
  is invited to the project.
* **That deploy crosses migration 087, and nothing built before `a18cdd9` can serve zones after
  it.** Read `docs/ROLLBACK.md` §4, "Rolling back after 087", before deploying: after 087 the way
  back is forward, or `4a5aa72` plus the owner's repair.

| PR | what changed |
| --- | --- |
| #56 | order status, cancellation and money families; the leaderboard no longer shows a player's Telegram ID as a name (`public_display_name`, on read and on write); `dist/` rebuilt |
| #57 | e2e and monitoring pointed at TurboBaby's own origin |
| #58 | `scripts/postdeploy-smoke.sh` (GET only) and `docs/ROLLBACK.md` |
| #59 | CLICK 125 redirects to NMAX 155 only; no seeded unit count shown as availability; admin Add reports success only on a 2xx |
| #60 | the owner's eighteen Phuket delivery zones (migration 087; Airport 690), no ETA published anywhere, the Koh Phangan rows deactivated; `dist/` bundle `a41a77c967b58518` |
| #61 | no PromptPay QR under one satang (422); a rental deposit must equal the family's published figure, per unit |
| #62 | spec hygiene: brain citations re-read, one owner per restated fact (the migration census is schema-provenance's), ride handling bound to `ride.js`, a negative control for the seed gate, a cannabis-era vocabulary guard, gate-1 floors equal to the measurement |
| #63 | rental only, Phuket only (owner, 2026-09-24): buy-out and events retired from every customer surface, migration 088 hides any public event, no row deleted; `dist/` bundle `17f356526a369bb9` |
| #64 | by `dmitrii-f-t27`, merged 2026-09-24 16:28 UTC as `f5e6b4f`: seven specs made to compile under the current t27 compiler (its message names `gHashTag/t27` master `a103752`, not the CI pin). Comments only: 33 lines in 7 specs, each `;` comment turned into `//` or `; --`, no declaration, assertion or constant touched. No Docker image input changed: `git diff 5b2800a f5e6b4f` touches `specs/` alone, which the image does not copy. What it did change in gate 2 is under "Gate 2's cross-check after #64" |

```sh
python D:/t27work/coverage_2109.py .     # the naming measurement, if you keep the helper
```

## The four gates, and how to run them

```sh
# 1. structure: module, unique ID, declaration floors, typecheck, five generators.
#    Needs the pinned compiler; see below.
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler -v

# 2. behaviour: parses and EXECUTES every assertion in the corpus. Stdlib only, ~1 s.
python3 scripts/execute_t27_assertions.py -v            # add --no-crosscheck without a compiler
#    With a compiler it exits 1 on f5e6b4f: two stale pins ("Gate 2's cross-check after #64").

# 3. drift: contract constants against the Rust and SQL they constrain.
python3 scripts/verify_t27_against_source.py --require-git-tracked -v

# 4. seed provenance (pre-existing).
python3 scripts/verify_fleet_seed.py -v
```

Gates 2 and 3 also run from `cargo test --features backend` via `tests/t27_gates_run.rs`, because
that is a job CI already has. **Their own workflow jobs are written and unlanded**: adding them edits
`.github/workflows/ci.yml`, and the token used here has no `workflow` scope. One command from the
repository owner unblocks it:

```sh
gh auth refresh -h github.com -s workflow
```

What CI runs today, read from `.github/workflows/ci.yml` at `f5e6b4f`: gate 1 in the job "T27
Canonical Contracts", gate 4 in its own job ("Fleet Seed Provenance"), and gates 2 and 3 only
through `tests/t27_gates_run.rs`. That file passes `--no-crosscheck` to gate 2 on purpose, so **CI
never runs gate 2's compiler cross-check**. The next subsection is what that costs.

### Gate 2's cross-check after #64

Measured 2026-09-25 on `f5e6b4f` with the pinned compiler, `T27C` set and no `--no-crosscheck`:
every assertion runs and passes (9996 of 9996), 9667 declaration names and function bodies agree
with t27c, 4 pinned front-end disagreements are printed, and the gate **exits 1**:

```
execute_t27_assertions: FAIL - 2 structural problem(s):
  - specs/turbobaby/commerce.t27: pinned front-end disagreement 'commerce_checkout_decision' no longer disagrees with t27c (...) -- re-measure and remove the KNOWN_FRONTEND_DISAGREEMENTS entry
  - specs/turbobaby/deposit_tiers.t27: pinned front-end disagreement 'refund_decision' no longer disagrees with t27c (...) -- re-measure and remove the KNOWN_FRONTEND_DISAGREEMENTS entry
```

* **Why.** `KNOWN_FRONTEND_DISAGREEMENTS` in `scripts/execute_t27_assertions.py` lists each
  function whose body the pinned compiler reads differently from the gate, and the gate fails when
  a listed one stops disagreeing. #64 turned the `;` comments inside `commerce_checkout_decision`
  and `refund_decision` into `//`, and t27c no longer truncates those two bodies (next section).
* **Before #64 it was green.** The same command on `5b2800a`'s text of the seven specs exits 0,
  with the same counts and the same 4 pins printed.
* **Two pins are still real:** `families_on_tier` and `published_rows_at_amount` in
  `deposit_tiers.t27`, the `while (c) : (step)` class.
* **Not fixed here, and not a deploy blocker.** Remove the two stale entries and the script
  header's "four functions" in one change. Nothing in it reaches the Docker image. Until then,
  read the assertions with `--no-crosscheck` and treat any cross-check line besides those two as
  new.

### Negative controls, 2026-09-25

Run on a separate clone at `f5e6b4f`, one plant at a time, each restored before the next. The
clone ended clean (`git status --porcelain --ignored` empty), and gates 3 and 2 re-ran green at
237 bindings and 9996 of 9996. Every plant turned its gate red (exit 1), and every message names
the fault:

| plant | gate | what it printed |
| --- | --- | --- |
| source drift: `DAILY_GAME_STARS_CAP` in `src/trios/stars_cap.rs`, 300 → 3000 | 3 | `star_award.PER_SOURCE_WINDOW_CAP ~ stars_cap.rs DAILY_GAME_STARS_CAP: contract=300 source=3000`, with both files and the binding's why |
| contract drift: `PER_SOURCE_WINDOW_CAP` in `star_award.t27`, 300 → 301 | 3 | the same binding, `contract=301 source=300` |
| the same contract drift | 2, `--no-crosscheck` | 5 of 9996 false: one assert in each of four `star_award.t27` tests, and the invariant `the_window_bound_is_per_source_and_not_per_player` (`REACHABLE_WINDOW_TOTAL_STARS == PER_SOURCE_WINDOW_CAP * 4`, 1200 vs 1204) |
| the `star_award.PER_SOURCE_WINDOW_CAP` row deleted from `BINDINGS` | 3 | `the binding table holds 236 entries; the floor is 237 (DECISIONS.md D16)` |
| `window_remaining(270) == 30` flipped to `!= 30` in the test `the_window_remainder_saturates_at_both_ends` | 2, `--no-crosscheck` | `9995 passed, 1 failed`, with the file, the test and both sides of the comparison |

The deleted row is caught only by the `MIN_BINDINGS` floor, and the message cannot say which row
went. From reading the gate, not from a plant: its only check on the table's size is
`len(BINDINGS) < MIN_BINDINGS`, so a row deleted in the same change that lowers `MIN_BINDINGS` to
236 stays green. Only a reviewer reading that diff catches it.

### Building the pinned compiler

CI pins `gHashTag/t27` at `40003ed1379c8a417e13e45843de35088b88f8c0`. Two traps cost an hour here:

1. `.trinity/seals/` holds filenames with `:` and `"`, which **cannot exist on NTFS**. Check out only
   what the build needs: `git checkout <ref> -- Cargo.toml Cargo.lock bootstrap bindings/javascript cli gen`.
2. The build verifies a frozen hash of `bootstrap/src/compiler.rs`. With `core.autocrlf=true` the
   checkout rewrites line endings and the hash fails. Set `core.autocrlf=false` **before** checking out.

Then `cargo build --locked --release -p t27c`.

**Still the pin on `f5e6b4f`, and it still handles every spec (2026-09-25).**

* **CI.** The job "T27 Canonical Contracts" (`ubuntu-latest`) checks out `gHashTag/t27` at
  `40003ed1379c8a417e13e45843de35088b88f8c0` into `.t27-toolchain`, sets up Rust with
  `dtolnay/rust-toolchain@stable`, runs
  `cargo build --locked --release --manifest-path .t27-toolchain/Cargo.toml -p t27c`, and runs
  gate 1 only, with `T27C=.t27-toolchain/target/release/t27c`.
* **The build behind the 2026-09-25 readings** is a Windows build of the same commit, made on
  2026-09-20. `t27c version` prints `t27c 0.1.0`, backends Zig, Verilog, C and Rust, and
  compiler LOC 29252. Every source file in its checkout is byte-equal to `40003ed`, apart from the
  absent `bindings/python`, which the workspace `Cargo.toml` excludes. The binary was not rebuilt
  for this reading, so being built from those sources is likely, not proven. CI and this build
  share the commit, not the toolchain: CI takes whatever Rust stable is current, on Linux.
* **With it, gate 1 parses, typechecks and generates all 45 specs on `f5e6b4f`**, the seven #64
  touched included, and it did the same on `5b2800a`. #64's message says those seven did not parse
  for the Queen board, and that it was verified with a compiler built from `gHashTag/t27` master
  `a103752`, not with this pin. The build steps above are not stale.

## Three defects in that compiler — filed, unfixed, and load-bearing

[gHashTag/t27#4530](https://github.com/gHashTag/t27/issues/4530), with minimal reproductions:

* a function body is **silently truncated at the first `;` comment inside it**. `gen-c` emits
  `commerce_checkout_decision` as `{ /* TODO: implement */ }` — all nine gates gone — while
  `typecheck` answers `ok, 0 errors, 0 warnings`;
* a `while (cond) : (step) { }` loop **and everything after it** is dropped;
* `packed struct` parses into two nodes, one with an empty name.

*Corrected 2026-09-25:* the first defect no longer reaches the corpus at the two places it was
pinned. #64 turned the `;` comments inside `commerce_checkout_decision` (commerce) and
`refund_decision` (deposit-tiers) into `//`. `gen-c` from the same pinned compiler now emits
`commerce_checkout_decision` with its real body, all nine gates in place. On `5b2800a`'s text it
still emits the `{ /* TODO: implement */ }` stub, so the defect is in the compiler and is
unchanged. The second defect still reaches two functions, `families_on_tier` and
`published_rows_at_amount` in `deposit_tiers.t27`. Apart from those two, gate 2's cross-check
finds no function body that it and t27c read differently.

**Never write `while (c) : (step)` in a contract.** Two agents did; the assertion gate's cross-check
caught both. Use `while (c) { ... i += @as(u8, 1); }`.

Also structural, and the reason gate 2 exists at all: `parse --json` returns `TestBlock` and
`InvariantBlock` nodes with **empty children**. The compiler discards every test and invariant body,
so no consumer of its AST can see an assertion.

## What is actually left

### 1. Seven defects the last two contracts exposed — the highest-value work

All measured, all pinned in `order_presentation.t27`, none fixed. In rough order of what a customer
loses:

1. **An unreadable status is drawn as a finished delivery.** `status_stepper.rs` resolves an
   unrecognised status to the index *after* the last step and fills every segment at or below the
   current one. "We cannot read your order's state" renders as "delivered".
2. **A total nobody could compute is shown as a price.** One of five money figures renders a dash;
   the other four go through a formatter that takes a plain `f64` and whose sanitiser maps
   non-finite and negative to `0.0`. D9 obeyed on the line, broken on the order, four lines apart.
3. **A refused cancellation is indistinguishable from a successful one.** The response is bound to
   `_resp` and discarded; the dialog closes on both paths.
4. **Two readings of case over one status string**, across fifteen readers: six lower-case before
   deciding, nine compare raw. A status in another case keeps its label and loses its filter chip,
   its cancel button, its reorder button and its place in the pipeline.
5. **Four answers to one unrecognised status in a single render**, one of which prints the raw
   server token as a step label.
6. **Four shapes of absence with no rule between them**: dash, zero, omitted row, and a lost screen
   (a payload missing one total fails deserialization and the customer is told the order was not
   found).
7. **`i18n` declares eight status keys, not nine.** Two of the nine server names have no key at all.

**Status family, 2026-09-22: defects 1, 4 and 5 are fixed, and 7 is contained.** The list above
is the 2026-09-21 record and is kept as it was.

* **One reading.** `src/trios/order_status_view.rs` is now the only code that reads the status
  string. The orders list, the order detail screen and the stepper all get their answers from it.
  It compiles on the host, so its eleven unit tests run under `cargo test`. It reads the string
  exactly, as the server does: the whitelist, the terminal guard and the pending-only cancel check
  in `src/api/orders.rs` all compare raw.
* **Defect 1.** A status the reading cannot understand is drawn as one flat grey bar with nothing
  filled. It is no longer drawn as a finished delivery.
* **Defect 4.** The fifteen readers are now one. A status spelled in another case is read as
  unknown by the label, the chip, both buttons and the bar alike.
* **Defect 5.** No step label is ever the server's own word. The fix found something the
  2026-09-21 reading missed: the raw token was not reserved for unknown names. Every cancelled and
  every rejected card printed it too, in both locales.
* **Defect 7.** The nine-to-eight collapse is now a single match. Gate 3 binds the collapse to the
  vocabulary and binds the label table to its arms. Which labels a customer sees is still an owner
  decision: questions A and B in section 3.
* **The census outside the boundary was wrong.** `profile_screen.rs` holds the same lower-casing
  block under renamed functions. `success_screen.rs` has its own label-only match. The bot's push
  to the customer is a fourth surface. `order_presentation.t27` corrects all three, with dates.
* **Where the pipeline lives.** The display pipeline the stepper walks moved into the module, so a
  host test can pin its content to `order_status.t27`. That contract now cites it there. Its admin
  citations, one line stale since 2c21df1, were re-pointed in the same edit.
* **The witness.** `tests/order_presentation_wiring.rs` guards the three UI files as text,
  because they compile only for wasm32.

On this change alone: 144 more assertions and 10 more bindings, and gate 3 now covers
17 contracts.

**Cancel family, 2026-09-22: defect 3 is fixed.** The list above stays as the 2026-09-21 record.

* **The answer is read.** `order_cancel_answer` in `src/trios/api_errors.rs` sorts it into four
  kinds: cancelled (2xx), refused (any other status outside 5xx), a server failure (5xx) and no
  answer. Those reach the owner's three outcomes: done, not done and unknown. It compiles on the
  host, and its unit tests run under `cargo test`. The screen calls it from `confirm_cancel`.
* **The dialog closes only on a 2xx** (AGENTS.md lesson 4). Once the server confirms, the offer is
  withdrawn for the rest of the view. The dialog has exactly two closes, the verdict's and the
  customer's own "no". The witness lists every write to it across the card and the sending task,
  and gate 3 counts the closes and where each one stands.
* **Every answer triggers a status re-read.** The card calls `on_cancel_answered`. The screen owns
  the status resource, so it drops the stale reading and restarts the resource (AGENTS.md lesson 5).
  This also happens past the one-hour poll.
* **No answer, or a 5xx, means the outcome is unknown.** Confirm stays shut until a fresh reading
  shows the order still pending. This follows the owner's receipt and cash-desk rule: check before
  repeating. A 5xx is held too. The handler answers one when its own commit fails, and a proxy in
  front of the server can answer one after the commit has landed. What the gate reads comes from
  the screen's own resource, and the witness and the site table pin that input.
* **The answer is shown above the pending gate**, so a 409 that removes the dialog is still shown.
  Only existing keys are used, and no copy was invented.
* **The witness.** `tests/order_cancel_wiring.rs` guards the screen as text. `order_presentation.t27`
  and `client_errors.t27` record the repair, with dates. The sentence choices live in
  client-errors, which counts the cancellation as its third surface. Gate 3 counts every non-test
  call to the general mapper in its own module (1) and every trio module that calls it (1).

On this change alone: 180 more assertions and 8 more bindings. The status family above adds 144
assertions and 10 bindings, and the duplication resolved in section 2 adds 26 and 5. Together with
the 2026-09-21 table that makes
8 911 + 144 + 26 + 180 = 9 261 assertions and 66 + 10 + 5 + 8 = 89 bindings, which were the
numbers the gates printed until the money family below.

**Money family, 2026-09-22: defects 2 and 6 are fixed on the two screens. D9 is still not obeyed
end to end.** The list above stays as the 2026-09-21 record.

* **One rule, host-tested.** `order_money_text` and `order_total_text` in `src/trios/pricing.rs`
  decide every order figure, over `measured_money` (finite and not negative; a zero is kept).
  `published_money` is now written on top of that filter plus the refusal of zero, so "unusable"
  is defined once (D15). The detail card and the list both call the rule, and neither screen
  calls `format_baht` any more.
* **One shape of absence, the dash.** A missing, null, NaN, infinite or negative figure renders as
  the same `bike_card::DASH` the line total already used. No sentence goes with it: that is
  owner question E below.
* **Zeros are taken from the owners.** A zero total stays a zero, because order-money floors its
  identity at zero (an order the discounts paid in full). A zero bonus or star count omits its
  row. A zero subtotal is a dash, because it is a sum of line prices and the line already calls a
  zero price absent.
* **Defect 6's lost screen is gone.** Every figure is an `Option` carrying `#[serde(default)]`.
  The detail flattens the rule's `OrderMoney` block, and the list's `total` is an `Option`. The
  2026-09-21 reading missed the list: there, one order without a total lost **every** order.
  What still loses a screen is recorded: a bare non-money field (id, items, status, created_at,
  quantity) and a figure of another JSON type.
* **What the fix does NOT do, and why D9 stays false.** Both order endpoints send the row through
  `Order::from` (`src/db/orders.rs`). It replaces an unusable stored figure with `0.0` and clamps
  stars at zero, and JSON cannot carry NaN. So `sanitize_money` was never the zero a customer
  saw. For every payload the shipped server sends, only one thing renders differently: a zero
  subtotal, now a dash. A substituted zero total still prints as a price, and a substituted zero
  discount still omits its row. Closing that means the server sending null. That in turn needs
  every other reader (home, profile, `types::Order`, the admin order) to read an absence first.
  It is cross-contract work, not an owner question, and the server was not changed here.
* **The witness.** `tests/order_money_wiring.rs` guards both screens as text, checks that every
  money site the contract cites lands on its code, and trips if the server's `finite_money`
  stops being the same predicate as `measured_money`. `order_presentation.t27` records the repair
  with dates. Commerce is its ninth neighbour, because question F touches commerce's ledger.
  `pricing_honesty.t27` carries a dated scope note.

On this change alone: 126 more assertions and 6 more bindings. The bindings are the two
`format_baht` counts, the two bare-figure counts, the detail's row comparisons, and the
checkout's bike-line keys, each a zero with a witness. The running totals are
8 911 + 144 + 26 + 180 + 126 = 9 387 assertions and 66 + 10 + 5 + 8 + 6 = 95 bindings, which are
the numbers the gates print.
`MIN_BINDINGS` and the gate-3 header counts were deliberately left for the merge.

**Money family, 2026-09-23: defect 2 is closed on the wire, in the source tree. Not merged, not
shipped.** The 2026-09-22 paragraph above stays as its record.

* **The server withholds instead of substituting.** `Order::from` (`src/db/orders.rs`) sends
  `null` for a stored subtotal, bonus, or total that is not finite and non-negative. It does the
  same for a negative star count. The same response names each withheld field under
  `money_withheld`, with the reason `stored_value_not_finite_non_negative`. The log line keeps the
  order id and the value. The old comment was wrong twice: the wire field is not the `NOT NULL`
  column, and `serde_json` writes a NaN as `null`.
* **Every other reader reads an absence first**, so a `null` loses no screen. Home and profile
  print their total through `order_total_text`. The admin card, badges and CSV go through
  `measured_money` in helpers appended at the end of `admin_screen.rs`, so no cited line moved.
  `types::Order` is an `Option`; it has no caller today.
* **Proof.** A new unit test, `an_unusable_stored_figure_is_never_shown_as_a_price` in
  `src/db/orders.rs`, takes a stored row through the real conversion, the client's own
  `OrderMoney` (plain and flattened), and the rule every order screen calls. It was red before
  the change: a stored NaN total printed as `฿0`. It is green after.
* **What is still false, on purpose.** `D9_IS_OBEYED_BY_EVERY_FIGURE_ON_THESE_SCREENS` stays
  `false`, now for two reasons. The first is owner question F: a bike-only order's finite zero
  still prints as a price. The second is the release, which is the blocker below.
* **Release blocker.** The committed `dist/` (4a5aa72, 2026-09-17) still holds bare numbers in
  every reader. Shipping this server before `dist/` is rebuilt would turn one withheld figure
  into a lost orders list, detail, home widget, profile history and admin page. That is worse
  than the zero it removes. The contract records it
  (`SHIPPING_THIS_SERVER_WITH_THE_COMMITTED_DIST_LOSES_SCREENS`); nothing gates it.
* **cargo is here.** `cargo` 1.98.1 lives in `%USERPROFILE%\.cargo\bin`; it is only missing from
  the agent shell's PATH (`PATH="$HOME/.cargo/bin:$PATH"`). Build with `-j 2`. The default 12
  jobs ran this 16 GB machine out of commit memory ("Файл подкачки слишком мал"), and the
  follow-on errors look like broken code, but they are not.

**Landing, 2026-09-24.** Everything above existed on one PC only, uncommitted. It lands in two PRs.

* **Binding groups** (gate 3 from 66 to 168 bindings, specs and scripts only):
  [#55](https://github.com/gHashTag/turbobaby-user-bot/pull/55), merged.
* **The status, cancel and money families, the person-naming resolution, and a rebuilt `dist/`**,
  in one change, so that the wire fix never reaches `main` without a client that reads it.
  The release blocker above is closed by that rebuild: `scripts/build-frontend.sh` built `dist/`
  from this tree and `scripts/predeploy-smoke.sh --no-build` loaded it in headless Chrome.
  `order_presentation.t27` records the new facts and keeps the old ones under
  `_BEFORE_THE_REBUILD` names. D9 stays false for owner question F alone.
* **The build script was macOS-only.** `sed -i '' -E` reads `''` as the script under GNU sed (Linux,
  Git Bash). A `sed_in_place` helper now does both. Checked by stripping `?v=` from the committed
  `dist/index.html` and re-running the expression: byte-equal.
* **The frontend toolchain is here too**: `trunk` 0.21.14 and `wasm-bindgen` 0.2.122 via
  `cargo install --locked`. Build with `CARGO_BUILD_JOBS=2` for the same memory reason as above.
* **Not landed: the gate jobs in `.github/workflows/ci.yml`.** Pushing a workflow file needs the
  `workflow` token scope, which neither account on this PC has (`gh auth refresh -s workflow`).

**The public ride leaderboard published every rider's Telegram id, 2026-09-24.** The ride screen
sent `"Player {tid}"` as the display name and the anonymous `GET /api/game/high-scores` returned it
verbatim; the live `dist/` (4a5aa72) carries the same line. Now `public_display_name` in
`src/api/game.rs` masks a blank name, a name holding the player's own id, or a run of six digits.
It masks on the read, so rows already stored are covered without a write to the live table, and on
the write. The screen sends no name at all. Guarded by the unit test
`a_name_carrying_the_players_telegram_id_is_never_published`, by `tests/leaderboard_anonymity_wiring.rs`,
and by a gate-3 binding (`game_score.IDENTIFIER_DIGIT_RUN`); `game_score.t27` records the correction.
Stored rows still hold the ids. The endpoint no longer shows them, but deleting them is a live write
for the owner.

### 2. One duplication, resolved 2026-09-22

`person_naming.t27` restated the attendee-handle bound that `turbobaby/events-booking` owns
(`HANDLE_MIN_DECLARED`, `HANDLE_MIN_ENFORCED`, `HANDLE_MAX_DECLARED`, `HANDLE_MAX_ENFORCED`,
`handle_is_accepted`, `handle_bounds_agree`) in four constants and three functions of its own,
beside an unbound copy of that module's line count.

**Corrected 2026-09-22:** this section used to say the restatement was made "without naming that
owner". That was inaccurate. The file did name the owner, in `HANDLE_BOUND_IS_ALREADY_OWNED_BY`,
but outside its edge block, with `REUSE_EDGE_COUNT` at 4, and it kept the copy on purpose under
`THIS_FILE_RESTATES_THE_HANDLE_BOUND = true`. Naming the edge alone would have left two
present-tense copies of one bound.

What changed:

* **The copy is gone and the edge is named.** `person_naming.t27` declares
  `REUSES_EVENTS_BOOKING_ID` and `EVENTS_BOOKING_SUPPLIES`, and takes the owner's verdict as two
  booleans in `tap_is_promised_but_cannot_resolve`. Every dropped name is listed in
  `RESTATEMENT_DROPPED_ON_RESOLUTION` or `SUPERSEDED_BY_THE_EDGE`, so a grep for an old name lands
  on the correction.
* **Three facts that only the copy held moved to the owner**, renamed to what was measured:
  `HANDLE_GAP_EXAMPLES_IN_TESTS`, `HANDLE_OVER_MAX_EXAMPLES_IN_TESTS` and `ATTENDEE_SOURCE_TESTS`,
  with `length_is_in_the_handle_gap` naming the gap between the two minima. The old "one length
  example" was one *over*-length example; length zero is tested too, but the emptiness check
  refuses it before any length is compared.
* **The absence is pinned outside the corpus.** The flag in the contract is a self-report: gate 2
  reads one file at a time and cannot see that a declaration is missing.
  `tests/person_naming_handle_edge_wiring.rs` names every dropped declaration itself and fails if
  one returns, or if a new name appears in a family a copy was written in (`ATTENDEE_HANDLE_*`,
  `attendee_handle_*`, `SIBLING_MODULE_*`) or in the owner's `ATTENDEE_SOURCE_*`. It also fails if
  the owner's ID is held anywhere but the edge, or if a name the consumer takes from the owner is
  not declared there. A copy under a name in a family nobody has used yet still passes.
* **Gate 3 gains five bindings**, tying `HANDLE_MIN_DECLARED`, `HANDLE_MAX_DECLARED`,
  `HANDLE_MAX_ENFORCED`, `ATTENDEE_SOURCE_TESTS` and `ATTENDEE_SOURCE_LENGTH_COMPARISONS` to
  `src/trios/attendees.rs`. `HANDLE_MIN_ENFORCED` stays unbound: the floor it records is an
  emptiness check with no numeral in it. The fifth binding counts the module's ordering
  comparisons of a length, whatever their direction; there is 1, the upper check. It was added
  after a checker showed that the first four could not see a fix to the floor made in place
  (below).
* **Gate 1's declaration floor for `person_naming.t27` went down**, from 239 to 235. `git log -p`
  on the manifest finds no earlier lowering. DECISIONS.md D16 keeps a floor so that a gate losing
  sight of what it checks goes red. These declarations were removed on purpose and are listed by
  name.

On this change alone: 26 more assertions and 5 more bindings than the 2026-09-21 table above.

The code defect is not touched. `src/trios/attendees.rs` still admits a handle shorter than its
own comment allows and builds a dead `t.me` link from it. Nothing in `person_naming.t27` reads the
bound's values any more, so that fix has one contract to correct, `events_booking.t27`. There it
flips `HANDLE_MIN_ENFORCED`, `HANDLE_GAP_EXAMPLES_IN_TESTS` and the invariant
`the_handle_lower_bound_is_declared_and_not_enforced`.

Whether gate 3 says so depends on the shape of the fix, because `HANDLE_MIN_ENFORCED` itself is
bound by nothing. An edit that changes the module's line count or its `#[test]` count turns
`ATTENDEE_SOURCE_LINES` or `ATTENDEE_SOURCE_TESTS` red. A fix made in place can change neither:
the checker's plant rewrote the emptiness check at `src/trios/attendees.rs:36` as
`if handle.len() < 5 {`, kept 357 lines and 16 tests, and gate 3 without the fifth binding stayed
green. With it, that plant and six other shapes turn red: the comparison reversed,
`chars().count()`, a range `contains` over a length, a second comparison beside the upper one, and
a `>= N` or `> N` filter in `attendee_link`. A floor written as `len().lt(&N)` or
`get(N..).is_none()` still turns nothing red, and the contract has to be found by hand.

### 3. Decisions that belong to the owner, not to a contract

None of these is unblocked by more `.t27`:

* **A. Does `completed` stay an alias of `delivered`?** Now checkable rather than theoretical: nine
  server names, eight i18n keys, five hand-written disjunctions holding the filter together, and a
  sixth surface that reaches the right picture only because the other five caught it first.
  *Corrected 2026-09-22:* the five disjunctions and the accident are gone. The pair is one arm of
  `arm_of`, and `completed` reaches the full bar through that arm. The alias still ships exactly
  as before: same label, chip, step, bar and reorder button. The write path stores only
  `completed`, so a separate label would relabel every finished order. Splitting the arm is a
  one-line change, and it needs the owner's ruling plus an approved ru/en pair.
* **B. Should a rejected order be labelled apart from a cancelled one?** The bot push already
  separates them. The order screens fold `rejected` into Cancelled (label, chip and red bar). Since
  2026-09-22 they no longer print the raw word `rejected`, and that word was the only on-screen
  difference between the two. Nothing in the repository publishes a label for `rejected`: the
  push sentence carries a refund claim these screens cannot check, and the short string at
  `src/locales.rs:184`/`:273` is the admin's callback toast. Add either key together with A, because both move the
  same `i18n.rs` citations.
* **C. Is the one answer to an unreadable status the right one?** The shipped default: the Unknown
  label, the grey of no information, one flat grey bar, no step label, the Active chip (so the
  order is never filtered out of sight), and no cancel or reorder button. It also depends on
  reading the status exactly, as the server does. That has a cost: a row stored in another case
  now reads Unknown on the list and detail screens, while the home and profile screens
  (customer-surface's) still fold case and show a label.
* **D. What does a customer read when a cancellation is refused?** Since 2026-09-22 a 409 (the
  order has already left pending) shows the generic `T_API_ERR_UNKNOWN`, whose "try again later"
  is false here. That is recorded as a named refusal in `client_errors.t27`
  (`CONFLICTED_CANCELLATION_HAS_A_PUBLISHED_SENTENCE = false`). Nothing publishes the sentence.
  The brain has no Mini App node. It does publish principles for the bot: three outcomes, check
  before repeating, and a refusal names its object. It also has one narrower precedent: in the
  mileage-lowering flow, one specific wording was banned because it tells the person nothing. The
  sentence has to describe the attempt, not the order. Whether it offers a next step, and through
  which channel, is part of the question. One existing candidate is `T_BIKE_ASK_MANAGER`. The bike
  card shows it only when the Mini App can read the bot's username, and it links to the bot's chat.
  Adding the key moves every `i18n.rs` citation below it, so land it together with A and B.
* **E. Which sentence, if any, stands beside a dashed order figure?** Since 2026-09-22 an absent
  total, subtotal or discount on the order screens is a bare dash. D9 asks for the dash and
  nothing more. The one published sentence for a missing price, `T_BIKE_PRICE_ON_REQUEST`, is
  D11's, for a rate the door did not quote, and it is worded about a bike model.
  `T_BIKE_QUOTE_NOTE` is about a quote too. Neither was reused under a placed order, and no copy
  was invented. `order_presentation.t27` records the refusal as
  `ORDER_FIGURE_DASH_HAS_A_PUBLISHED_SENTENCE = false`, and a text guard refuses both keys on the
  two screens. An answer needs an owner-approved ru/en pair. Adding a key moves every `i18n.rs`
  citation below it, so land it together with A, B and D.
* **F. What should an order holding a bike line show as its total?** The stored subtotal and
  total leave the rental out (`src/api/orders.rs:495-503`), so a bike-only order stores 0 and 0.
  Today its subtotal is a dash and its total prints `฿0`. The choices are a dash for the whole
  order, or the catalog part with owner-supplied qualifier copy. No shipped client submits a bike
  line; gate 3 binds that, so this is reachability, not traffic. A design that inferred bike
  lines from missing catalog ids was dropped. It went past the recorded defects, it would have
  shipped this answer by default, and it read an empty id differently from the server.
* **Which legacy surface retires next?** `legacy_retirement.t27` measures three distinct states —
  data hidden, route alive, table dropped with readers kept by an allowlist. Choosing what moves is
  a product call (issue #2).
* **The monthly price coefficients.** The formula is pinned (`base × season × term step`); the
  monthly table lives in a manager's sheet this side may only read.
* **A capacity or eviction rule for the ETag map.** Growth is reported, not bounded, because nothing
  publishes a bound.

**Decided on 2026-09-24**, and no longer open: TurboBaby's delivery zones. The owner chose the
sheet's sixteen zones at the sheet's prices, the airport at 690, and no minutes shown. That is
DECISIONS.md D19 addendum and `delivery_terms.t27`, recorded in the brain as decision 24.09.2026-1.
Also decided that day: **rental only, Phuket only.** Bike sales and events leave every customer
surface, and referrals, loyalty and the ride game stay (#63, DECISIONS.md, `legacy_retirement.t27`).
The questions that ruling left open are listed in #63.

**Opened on 2026-09-24** by the changes above. Each one waits on the owner and is named in its PR:

* **G. Deposit per bike or per line?** A multi-bike rental line is held to the per-bike published
  deposit (#61). A deposit a manager lowered for such a rental has no field to reach an order, a
  USD/EUR figure is refused on the order path, and a refused deposit writes no fraud event.
* **H. The airport zone's name.** The row is "Аэропорт" / "Airport". The brain says the airport
  itself is not served, only the hotels next to it (`AIRPORT_ITSELF_IS_NOT_A_DELIVERY_DESTINATION`).
  Also open: whether the sheet's out-of-belt price (1490) and the 17:30 cut-off should be modelled,
  and what checkout says when the zone list is empty.
* **I. Catalog copy (#59).** The detail screen still says "all bikes of this model are busy right
  now" from a seed snapshot when the seeded count is 0. The unmounted `bike_card.rs` still
  hard-codes a count. Admin Add inserts its row before the 2xx. The brain's `knowledge_base` still
  offers PCX 150 / ADV 150 for CLICK 125.
* **J. Vocabulary and ownership (#62).** Should the nmax-155 rate be owned by pricing-honesty (as
  landed) or by rental-terms? Is a named copy bound by gate 3 accepted in place of an import the
  compiler lacks? New copy is needed for `T_MENU_DESC` / `T_MENU_NO_RESULTS`, and the strain-of-day
  carousel is waiting to be retired.
* **Outside the contracts, and blocking what customers see:**
  * the price door's source and Bridge access. The door is still a stub, so every rate is quoted
    by a human;
  * the booking capacity rule and booking depth;
  * a deploy of `main`, which needs a Railway token or a manual `railway up`. On 2026-09-25 the
    manual route for `f5e6b4f` is prepared and waits only on access to project `woody` (see
    "Landed on 2026-09-24, and not yet live"). It crosses 087: `docs/ROLLBACK.md` §4;
  * the `woody` database backup policy (`docs/ROLLBACK.md` §7);
  * whether `WEB_APP_URL` / `BOT_USERNAME` are set on the service.

### 4. The axis worth growing

**237 bindings over 224 distinct constants** — a reading of 2026-09-24 after #63, 224 of 5 564
declared constants (4.0 %), and the same on 2026-09-25 after #64 (5 564 re-counted on both trees). Earlier on 2026-09-24: 227 over 214 after #62, 198 over 190. On 2026-09-23: 197 over 189. On 2026-09-22 it was 168 bindings over 164 of 4 997 declared constants (3.3 %); on
2026-09-21, 66 bindings of roughly 4 200. *Corrected 2026-09-24:* 4 997 is the gate-3 header's
count — lines that start with `pub const ` at column zero — and it reproduces exactly on the
2026-09-22 tree. The same count reads 5 310 today, which makes 3.6 %. The command below counts every
`const`, indented or not public, so it reads higher (5 333) and is not comparable with 4 997:

```sh
grep -E '^\s+"(spec|const)":' scripts/verify_t27_against_source.py \
  | awk '/"spec":/{s=$2} /"const":/{print s" "$2}' | sort -u | wc -l                 # 224 (190 before #59)
cat specs/turbobaby/*.t27 specs/agents/turbobaby.t27 | grep -cE '^\s*(pub\s+)?const\s'   # 5587 (5333 before #59)
```
 Every new binding is a fact that can no longer
drift silently. `scripts/verify_t27_against_source.py` is table-driven: adding one is a few lines,
and the table says how at the top. A contract with no binding describes the code; a contract with
one constrains it.

## Where the answers live

Several refusals fell on 2026-09-21 because the owner's knowledge base — "the brain" — already
published the rule. It is reachable only through the trusted writer, which holds the bridge secrets
itself:

```python
import brain_writer
brain_writer.list_brain()                 # the registry: 39 keys -> file ids
brain_writer.read_text(name="business_rules")
brain_writer.append(text, name=..., anchor=..., place="after")   # additive, backed up, read back
```

`business_rules` (96 kB), `knowledge_base` (69 kB) and `booking_flow` carried the delivery ladder,
the price formula and the cached-number rule. **Read the source, not a summary of it**: a brief
handed to one agent said "Paklok 390" from the dialog table, and the owner's decision of 2026-09-06
sets it to 490 and cancels the older row for that district alone. Publishing the brief's number
would have shipped a superseded price as current.

Contracts cite the brain as `brain:<node>:<line>` and say plainly that **no gate resolves those
citations** — unlike a `src/` citation, which gate 3 can check.

## A prompt for the next session

> Continue the TurboBaby `.t27` migration in `gHashTag/turbobaby-user-bot`. Read
> `docs/t27-handover.md` and `docs/t27-contract-map.md` first; they carry the measurements and the
> traps. Build the pinned compiler as the handover describes, then run all four gates and quote the
> numbers before changing anything — if they do not reproduce, say so instead of proceeding.
> Gate 2 with a compiler configured exits 1 on `f5e6b4f` for two stale pins, and that is known
> (see "Gate 2's cross-check after #64"); any other cross-check line is new.
>
> The work, in order: fix the seven defects listed under "What is actually left", each with a test
> that is red before the fix and with the contract that records the defect corrected in the same
> change; then grow `scripts/verify_t27_against_source.py` beyond its 237 bindings (re-measured
> 2026-09-25 after #64, the same as after #63 on 2026-09-24; 227 after #62, 198 earlier that day,
> 197 on 2026-09-23, 168 on 2026-09-22, 66 on 2026-09-21), because a contract nothing binds only
> describes the code.
>
> Three rules this corpus is built on, and they are not style: never invent a number — where the
> repository publishes nothing, declare the refusal and say what is missing; never restate what
> another contract owns — name its ID and consume its decision; and never cite a sibling contract by
> line number, because 72 such citations went stale here in a single day. When a fix makes a
> contract's recorded fact false, that contract is part of the fix.

## Things that are true of this repository and cost time to learn

* **The test suite assumed a POSIX checkout.** Until 2026-09-21, `cargo test --features backend` on
  Windows stopped at the first binary and never reached the other forty-nine — measured 1 of 50
  binaries running, against 50 of 50 and 2 162 tests afterwards. CI was green throughout, because CI
  was the only host anyone had measured. If a guard that reads the tree as text starts failing
  locally and passing in CI, suspect line endings or path separators before logic.
* **A merge to `main` does not deploy.** `deploy.yml` triggers on push but skips itself while
  `secret RAILWAY_TOKEN` and `variable RAILWAY_SERVICE` are absent, and this repository has zero of
  each. `main` is an integration branch today, not a shop window. Re-measure before relying on it.
* **A rebase on Windows writes CRLF into the files it touches.** With Git for Windows'
  system-wide `core.autocrlf=true`, the files a rebase rewrites come out CRLF, while files it leaves
  alone keep whatever they had. Guards that look for `"\n}\n"` then fail locally and pass in CI.
  Measured 2026-09-24: `tests/wire_absence_wiring.rs` failed 3 tests after the #60 rebase and
  passed once the same bytes were LF. Rebase with `git -c core.autocrlf=false`.
* **Branches that all cite the same lines conflict by meaning, not only by text.** Four branches
  on 2026-09-24 each re-pointed `file:line` citations and each moved lines. After every merge, check
  each citation against the tree it was written for and the line it names now. Re-cite by
  declaration, test or function name where you can, because a name does not drift.
