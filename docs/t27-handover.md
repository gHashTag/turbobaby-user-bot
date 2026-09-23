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
| assertions executed, all passing | **9 038** — re-measured 2026-09-22 (was 8 911) |
| contract-to-contract ownership edges | 203+, **zero dangling** |
| source lines named by some contract | **73 407 of 80 308 (91.4 %)** |
| enforced contract-to-source bindings | **168**, across 40 of 45 contracts — re-measured 2026-09-22 (was 66 across 16 of 45) |

The last two rows are the ones to keep apart. *Named* means a contract cites the file. *Bound* means
a gate fails when the two disagree. 91.4 % is a floor on attention; 168 (2026-09-22; 66 the day
before) is the number that actually holds, and it is the one worth growing.

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

### Building the pinned compiler

CI pins `gHashTag/t27` at `40003ed1379c8a417e13e45843de35088b88f8c0`. Two traps cost an hour here:

1. `.trinity/seals/` holds filenames with `:` and `"`, which **cannot exist on NTFS**. Check out only
   what the build needs: `git checkout <ref> -- Cargo.toml Cargo.lock bootstrap bindings/javascript cli gen`.
2. The build verifies a frozen hash of `bootstrap/src/compiler.rs`. With `core.autocrlf=true` the
   checkout rewrites line endings and the hash fails. Set `core.autocrlf=false` **before** checking out.

Then `cargo build --locked --release -p t27c`.

## Three defects in that compiler — filed, unfixed, and load-bearing

[gHashTag/t27#4530](https://github.com/gHashTag/t27/issues/4530), with minimal reproductions:

* a function body is **silently truncated at the first `;` comment inside it**. `gen-c` emits
  `commerce_checkout_decision` as `{ /* TODO: implement */ }` — all nine gates gone — while
  `typecheck` answers `ok, 0 errors, 0 warnings`;
* a `while (cond) : (step) { }` loop **and everything after it** is dropped;
* `packed struct` parses into two nodes, one with an empty name.

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

### 2. One duplication, reported and unresolved

`person_naming.t27` restates the attendee-handle rule that `turbobaby/events-booking` already owns,
without naming that owner. Either name the edge or drop the restatement.

### 3. Decisions that belong to the owner, not to a contract

None of these is unblocked by more `.t27`:

* **Does `completed` stay an alias of `delivered`?** Now checkable rather than theoretical: nine
  server names, eight i18n keys, five hand-written disjunctions holding the filter together, and a
  sixth surface that reaches the right picture only because the other five caught it first.
* **Which legacy surface retires next?** `legacy_retirement.t27` measures three distinct states —
  data hidden, route alive, table dropped with readers kept by an allowlist. Choosing what moves is
  a product call (issue #2).
* **The monthly price coefficients.** The formula is pinned (`base × season × term step`); the
  monthly table lives in a manager's sheet this side may only read.
* **A capacity or eviction rule for the ETag map.** Growth is reported, not bounded, because nothing
  publishes a bound.

### 4. The axis worth growing

**168 bindings over 164 of 4 997 declared constants (3.3 %)** — a reading of 2026-09-22; on
2026-09-21 it was 66 bindings of roughly 4 200. Every new binding is a fact that can no longer
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
>
> The work, in order: fix the seven defects listed under "What is actually left", each with a test
> that is red before the fix and with the contract that records the defect corrected in the same
> change; then grow `scripts/verify_t27_against_source.py` beyond its 168 bindings (re-measured
> 2026-09-22; 66 on 2026-09-21), because a contract nothing binds only describes the code.
>
> Three rules this corpus is built on, and they are not style: never invent a number — where the
> repository publishes nothing, declare the refusal and say what is missing; never restate what
> another contract owns — name its ID and consume its decision; and never cite a sibling contract by
> line number, because 72 such citations went stale here in a single day. When a fix makes a
> contract's recorded fact false, that contract is part of the fix.

## Two things that are true of this repository and cost time to learn

* **The test suite assumed a POSIX checkout.** Until 2026-09-21, `cargo test --features backend` on
  Windows stopped at the first binary and never reached the other forty-nine — measured 1 of 50
  binaries running, against 50 of 50 and 2 162 tests afterwards. CI was green throughout, because CI
  was the only host anyone had measured. If a guard that reads the tree as text starts failing
  locally and passing in CI, suspect line endings or path separators before logic.
* **A merge to `main` does not deploy.** `deploy.yml` triggers on push but skips itself while
  `secret RAILWAY_TOKEN` and `variable RAILWAY_SERVICE` are absent, and this repository has zero of
  each. `main` is an integration branch today, not a shop window. Re-measure before relying on it.
