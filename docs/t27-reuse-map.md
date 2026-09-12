# What already exists in `.t27`, and what TurboBaby may therefore write

Answers the instruction "study all the specs so we can reuse and not duplicate `.t27`
specifications". Measured 2026-09-12 against the **published** corpus, not a local checkout.

Related: [`DECISIONS.md`](../DECISIONS.md), issues #16-#21 (the six specs), #22 (publication).

---

## Where the corpus actually is

`https://t27.ai/#/specs` serves files out of `gHashTag/trinity` at
`apps/website/public/t27/files/specs/`, but those files are **vendored build output**, not the
source. `public/t27/manifest.json` declares its own provenance:

```json
"generatedFrom": { "repo": "gHashTag/t27", "commit": "1cd2877f9...", "specsOrCompilerDirty": true }
```

So the authority is `gHashTag/t27` and the other scanned worlds; `trinity` holds a copy.

A local `trinity` checkout is the wrong instrument for this question. This machine's checkout
holds **31** `.t27` files and is 256 commits behind `origin/main`, which holds **1075**.
`manifest.json` counts **856** indexed specs across eight repositories. Every number below was
read from `origin/main`, never from the working copy.

| repo | specs | duplicates skipped |
| --- | --- | --- |
| ghashtag/t27 | 575 | - |
| ghashtag/tri-net | 113 | - |
| ghashtag/trinity-fpga | 64 | - |
| ghashtag/trios | 58 | 13 |
| dmitrii-f-t27/trinity-memory | 37 | 0 |
| ghashtag/tt-trinity-corona | 5 | - |
| ghashtag/trinity | 3 | - |
| ghashtag/tt-trinity-gamma | 1 | 48 |

129 further duplicates were skipped in earlier scans. **The catalog already deduplicates by
content hash.** Duplication is therefore not a silent risk to be avoided by hand; it is a
measured, reported quantity. What hand care still buys is not writing a spec that *should*
have been an import.

---

## Does the commerce domain already exist? No.

The 856-spec index (`shared-core.json`) carries a `terms` array and a description per spec. I
probed it for every term the six TurboBaby specs would claim.

**Zero hits, in terms and in descriptions:** `price`, `pricing`, `money`, `currency`, `thb`,
`baht`, `inventory`, `stock`, `rental`, `commerce`, `shop`, `vehicle`, `bike`, `moto`,
`discount`, `tariff`, `availability`, `fleet`, `i18n`.

The terms that *did* hit are all false positives, and it matters that they were checked rather
than assumed:

| term | hits | what it actually is |
| --- | --- | --- |
| `product` | 19 | arithmetic products — GF16, MAC, dot-product, posit16, fp8. No merchandise. |
| `score` | 24 | similarity and reliability scores. No game score. |
| `catalog` | 3 | an FPGA **IP-core** catalog (`specs/fpga/stdlib.t27`), a die-to-die format catalog, a numeric coverage delta. No product catalog. |
| `order` | 4 | ordering/sequence, not a purchase order. |
| `deposit` | 1 | `pool_after_deposit` in a TRI-NET compute-reward pool. Not a rental security deposit. |
| `checkout` | 1 | `git checkout`, in `specs/git/operations.t27`. |

Of 58 spec categories the largest are `tri` (150), `tools` (66), `fpga` (66), `ml` (60),
`crons` (34), `functions` (29), `agents` (28), `skills` (27). There is **no** commerce,
catalog-of-goods, pricing or booking category anywhere in the corpus.

**Conclusion:** the six TurboBaby specs duplicate nothing. This is a new domain for `.t27`, and
the reuse the instruction asks for is available at the level of *form*, not content.

---

## What is reused: the form

Two published specs fix the idiom. `specs/demos/hello_world.t27` — the file linked in the
instruction — is the language tour; `specs/catalog/discovery.t27` is a real production spec
whose contract drives a live workflow. TurboBaby follows both:

1. `// SPDX-License-Identifier: Apache-2.0` as line 1.
2. A `;` header giving the file path and purpose, then a **`WHY (measured <date>)`** paragraph
   citing the number that forced the spec, and where useful a **what this never does**
   paragraph. `discovery.t27` spends 20 lines on this before its first declaration.
3. `phi^2 + 1/phi^2 = 3 | TRINITY` closing the header.
4. `module <name>;` — hyphens (`hello-world`) and underscores (`catalog_discovery`) both parse.
5. Identity constants first, as `discovery.t27` does: `pub const KIND : str`,
   `pub const ID : str`, `pub const NAME : str`.
6. Every `const` carries an explicit type; arrays carry an explicit length (`[3]str`).
   The type after `:` is not optional and no width is left to a compiler default.
7. Both test forms are legal and both are used:
   `test "quoted prose" { try std.testing.expectEqual(@as(i8, X), f(..)); }` and
   `test bare_snake_name { assert A == B; }`.
8. `invariant name` followed by an indented `assert EXPR`, no braces and no semicolon — for
   claims about the whole module that no single example can carry.
9. **L3 PURITY**: ASCII only, English identifiers only. This is why `data/fleet_seed.json`
   carries transliterated colour names and no Cyrillic.
10. **L4 TESTABILITY**: every `.t27` must hold at least one `test`, `invariant` or `bench`.
    A spec of bare constants is rejected by the law, and separately proves nothing.

`discovery.t27` also demonstrates the habit worth copying most: it states what the scan
**never** does, and it explains why a cheap check is insufficient — "the compiler accepts any
text as an empty module, so 'it compiles' proves nothing: a licence file named `.t27` compiles
too". That is the same standard `DECISIONS.md` D9 applies to an absent price.

---

## How these specs reach t27.ai (issue #22)

Two surfaces, and they are not equally reachable. This was probed, not assumed.

### `#/specs` — automatic, with one deliberate manual gate

`discovery.t27` *is* the contract for `.github/workflows/t27-world-scan.yml`, which runs on
`cron: '17 3 * * *'` and on `workflow_dispatch`. It reads the contract through the vendored
compiler, scans both owners' entire public inventories, vendors what qualifies, rebuilds
`manifest.json` / `shared-core.json` / `universe-atlas.json`, runs the catalog gates and commits.

This repository qualifies on every published criterion:

| criterion | value | this repo |
| --- | --- | --- |
| `OWNERS` | `["gHashTag", "dmitrii-f-t27"]` | `gHashTag` — yes |
| `PUBLIC_ONLY` | `true` | `visibility: PUBLIC` — yes |
| `SKIP_FORKS` | `true` | `isFork: false`, `parent: null` — yes |
| `MIN_DECLARATIONS` | `1`, in one of up to `PROBE_FILES = 8` | consts + tests + invariants — yes |
| `MAX_FILE_BYTES` | `1048576` | far under — yes |

**`SKIP_FORKS = true` is the load-bearing row.** The instruction said to fork
`woody-weed-bot`. A GitHub fork would have carried `isFork: true` and been excluded from the
catalog for ever — the specs would exist and never appear. This repository is instead a fresh
public repo holding the upstream content and history, so `isFork` is `false` and it qualifies.
That one property is the difference between part 7 of the request working and silently never
working.

So **no wiring code is needed for the spec browser.** The remaining step is the publish gate:
`deploy-site.yml` is manual on purpose — "content is approved before it goes out" — so the
specs land in the committed catalog data automatically and become visible on t27.ai when the
owner runs the deploy. That gate is a decision, not a defect, and it is not mine to pull.

### `#/queen` — blocked, and not by anything in this repository

The board is single-repo by construction. `apps/website/src/pages/Queen.tsx` declares:

```ts
interface BoardResponse { repo: string; columns: ...; cards: BoardCard[]; pulse: ... }
```

One `repo` string, not a list. A second repository's epic cannot appear on that board without
a change to the supervisor's board contract and to the page.

The deployed origin the page falls back to is also not currently serving the board API.
`DEPLOYED_QUEEN = https://trios-agent-server-production.up.railway.app` answers `/health` with
`{"status":"ok","pid":2,"cdpConnected":false,...}` — a CDP/browser service — while
`/api/board`, `/api/status` and `/api/queen/needs-you` all return **404**.

Issue #22 therefore splits: the spec-browser half needs nothing from us, and the Queen-board
half needs a `repo` parameter in the board contract before it is possible at all. Recorded
rather than worked around, because the honest reading is "the board cannot show this yet", not
"we forgot to register the epic".
