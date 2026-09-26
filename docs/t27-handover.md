# Continuing the `.t27` migration — handover

Written 2026-09-21, at the point where the migration itself is finished and what remains is either
an owner's decision or a separate piece of work. Everything here is a measurement with the command
that produced it, so a reader on another machine can reproduce it rather than believe it.

For what each contract owns, read [`t27-contract-map.md`](t27-contract-map.md). This file is about
what to do next.

## Where it stands

| | measured 2026-09-21 |
| --- | --- |
| canonical contracts | **45** (44 under `specs/turbobaby/` + `specs/agents/turbobaby.t27`); **46** on `t27/round5-server` and on the integration branch `t27/round5`, with `referral_credit.t27` (2026-09-26) |
| assertions executed, all passing | **11 048** on the integration branch `t27/round5` at `b1d900e` (2026-09-26, the review fix of round 5; see "Round 5, review fix" below), **11 033** on the integration branch `t27/round5` at `992077f` (2026-09-26, R1–R3 integrated; see "Round 5, integrated" below), **11 009** on the server lane `t27/round5-server` at `872698b` (2026-09-26, R1–R3; see "Round 5, server lane" below), **10 758** on `t27/round4` at `168892d` after its review (2026-09-26; see "Round 4, review fixes" below; bindings still 256), **10 754** on the integration branch `t27/round4` at `2f801fe` after the owner's answers of 2026-09-26 (see "Round 4, integrated" below), **10 581** on the integration branch `t27/owner-answers-2509` at `41315b2` after the owner's second list of 2026-09-25 and the operator's shop label (2026-09-26; see "The owner's second list of 2026-09-25 and the operator's shop label" below), **10 273** on the integration branch `t27/owner-answers-2509` after the three fixes on top of `69fd4fa` (2026-09-25; see "Three fixes on top of `69fd4fa`" below), **10 246** at `69fd4fa` (the owner's answers of that day; not pushed, not merged) — **9 996** re-measured 2026-09-25 on `f5e6b4f`, after #64, which changed comments only (9 996 after #63 on 2026-09-24, 9 859 after #62, 9 535 earlier that day, 9 514 on 2026-09-23 before that day's money family, 9 038 on 2026-09-22, 8 911 on 2026-09-21) |
| contract-to-contract ownership edges | 203+, **zero dangling** |
| source lines named by some contract | **73 407 of 80 308 (91.4 %)** |
| enforced contract-to-source bindings | **286** on the integration branch `t27/round5` at `b1d900e` (2026-09-26; +2 the recorder guard, the review fix of round 5), across 42 of 46 contracts and 88 source files — **284** on the integration branch `t27/round5` at `992077f` (2026-09-26; +2 the Mini App's rate, the client lane added none), across 42 of 46 contracts and 88 source files — **282** on the server lane `t27/round5-server` at `872698b` (2026-09-26; −3 +29, R1–R3), across 42 of 46 contracts and 87 source files — **256** on the integration branch `t27/round4` at `2f801fe` (2026-09-26; +7 the server lane of round 4, +1 its client lane), across 41 of 45 contracts and 84 source files — **248** on the integration branch `t27/owner-answers-2509` at `41315b2` (2026-09-26; +2 answer 1, +4 answer 3, +2 the kept cart, +1 the held notification kinds), across 41 of 45 contracts and 83 source files — **239** on the same branch on 2026-09-25 (the two new rows bind the CLICK 125 redirect to the seed), across the same 41 of 45 contracts and 80 source files — **237**, across 41 of 45 contracts and 80 source files, re-measured 2026-09-25 on `f5e6b4f`, after #64 (237 after #63 on 2026-09-24, 227 after #62, 198 earlier that day, 197 on 2026-09-23, 168 across 40 on 2026-09-22, 66 across 16 on 2026-09-21) |

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

Gate 2 with a compiler configured, the form "The four gates" shows, was red on `f5e6b4f` for two
stale allowances and no false assertion; the change that removed them made it green again. See
"Gate 2's cross-check after #64" below.

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
* **The deploy of `main` (`f5e6b4f`) is prepared and waits on Railway access alone.** A clean LF
  export of `f5e6b4f` was made with the earlier, line-by-line form of `docs/ROLLBACK.md` §3B. §3B
  is now one fail-closed run that makes its own clone, checks it and uploads it, so the runbook's
  route is that block with `SHA=f5e6b4f7f7d725707d2112a72250a4bff22e50fa` and a new `DEP`. It does
  not reuse the prepared export. The Railway CLI on this PC (5.62.1) is logged in to an
  account that cannot see project `woody` yet, so `railway up` cannot run until that account is
  invited to the project.
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

### The owner's answers of 2026-09-25, integrated and not yet live

The owner answered a numbered list in chat on 2026-09-25. Six item branches, each cut from `main`
at `0526429` (#65 and #66 on top of `f5e6b4f`), were merged in this order into the local branch
`t27/owner-answers-2509` with `git merge --no-ff`, followed by one `dist/` rebuild. **Nothing is
pushed and nothing is deployed**: a customer sees none of it until the branch reaches `main` and
`main` is deployed (see "Landed on 2026-09-24, and not yet live"). The merge commits, in order:
`d5dfbad` (quests), `34f4ec6` (NMAX), `146fc4c` (cancel), `db57c95` (client half of 12), `6b41186`
(server half of 12) and `9838b9b` (drafts). Then `74ed8a2` rebuilt `dist/`.

| answer | branch | what landed | what was left, and why |
| --- | --- | --- | --- |
| 6, «нет» | none | Nothing to change: the zone keeps the name «Аэропорт» | The rest of question H (the out-of-belt price, the 17:30 cut-off, the empty-list sentence) was not asked |
| 7, «да» | `t27/quests-rental` | Seven old non-rental client paths (`/accessories`, `/tea`, `/game`, `/treasure-hunt`, `/ar-hunt`, `/location-quest`, `/tech-tree`) stay declared and render the catalog. The quest opened from Profile no longer shows «Минимальная покупка: 300 бат»; the key stays translated. The ride game, referrals and loyalty stay. Recorded in `legacy_retirement.t27`, `deeplink.t27` (6 compatibility paths to 13) and the DECISIONS.md D19 addendum; guarded by `tests/rental_only_wiring.rs` | `/tech-tree` is repointed on this repository's reading, since the answer does not name it (`OWNER_ANSWER_NAMES_THE_TECH_TREE = false`), so the owner can overrule it. The legacy HTTP routers behind the seven paths still answer, because the answer did not cover them |
| 8, «Нет» | none | Nothing to change: the rental-term discount percentages stay on the bike card | — |
| 9, undecided | none | `T_BIKE_UNITS_EMPTY` was not touched | Still undecided; question I stays open for it |
| 10, NMAX 155 «пока» | `t27/offer-nmax` | The seed's `offer_instead` for CLICK 125 is `["nmax-155"]`, marked provisional with a dated source. `availability.t27` holds `CLICK_125_REDIRECTS` and `CLICK_125_REDIRECTS_ARE_PROVISIONAL`, and gate 3 binds both to the seed (the 2 new bindings). The seed gate refuses a redirect to a family with no available unit. DECISIONS.md has a D12 amendment | No customer sentence changed: the screen already offered NMAX 155 alone. The brain's `knowledge_base` still names PCX 150 / ADV 150, and that text is the operator's to change |
| 11, delegated | `t27/cancel-refused-copy` | A refused cancellation (409, the order has left pending) shows `T_ORDER_DETAIL_CANCEL_REFUSED` in ru and en instead of "try again later". Recorded as `CONFLICTED_CANCELLATION_DECISION` in `client_errors.t27`. Every `i18n.rs` line citation below the new key was re-pinned. Later the same day the operator reworded it under the same delegation, same key and same lines: «Отменить этот заказ в приложении уже нельзя. Напишите менеджеру.» / "This order can no longer be cancelled in the app. Please message the manager." (`CONFLICTED_CANCELLATION_REWORDED_AT`) | One limit is named in the contract and is the owner's to change with the words: the order card has no link to the manager. The other limit named that day was CLOSED by the rewording on 2026-09-25: the first wording said the order was already being handled, which is false when the shop rejected the order or an earlier attempt with an unknown outcome cancelled it |
| 12, client half | `t27/no-cannabis-ui` | 14 cannabis-era keys deleted from `src/trios/i18n.rs`, each line replaced by a comment so that no line citation moved. 5 arms cut to their non-cannabis half and 2 aligned with their other-language twin. 128 files of the old shop's media left `assets/` (13 stay). Served comments in `index.html` and `styles/` cleaned. Guarded by `tests/no_cannabis_client_wiring.rs` | The 20+ age gate stays, because item 12 did not answer that question. The `woody_*` storage keys keep their prefix, because renaming one discards saved drafts |
| 12, server half | `t27/no-cannabis-server` | The strain-of-day buttons answer with the rental menu. `GET /api/loyalty/tiers` serves no perks and omits the old shop's tier. The public tech-tree reads answer empty. Share cards are built only from rows the catalog shows. The promo sales report withholds names from the dropped table. The docs describe TurboBaby only. Guarded by `tests/server_text_vocabulary_wiring.rs` | A loyalty profile that still stores the old tier key is served that key; re-keying it is a data change for the owner. Old migrations, `docs/reports/` and git history are kept (D2) |
| 13 onwards, delegated | `t27/event-drafts-guard` | Publish refuses every stored promo draft: the two event kinds, the retired shop's kinds and any kind nobody classified. Nothing is deleted, and a refused row stays an unstamped draft. Recorded as `PUBLISH_REFUSAL_*` in `promo_broadcast.t27`; guarded by `tests/promo_publish_wiring.rs` | By the same delegation, the admin archive tabs and the statuses stay and were not changed. The admin HTTP broadcast sends admin-typed text, not a draft, and is unchanged |

**Conflicts, and how they were resolved.** Each conflict kept both sides.

* The DECISIONS.md addenda were all appended at the end. They are kept in merge order.
* **Key count.** The cancel key (+1) and the deletion (−14) landed on the same day. On the merged
  `src/trios/i18n.rs` the count is 552 declarations and a naive grep finds 557 (565 + 1 − 14 and
  570 + 1 − 14). `locale_policy.t27` names the +1 as `KEYS_ADDED_2026_09_25`, recorded by
  client-errors.
* **Line numbers.** Every hunk between the cancel tip's `i18n.rs` and the merged file is a
  same-size replacement, so the cancel branch's re-pinned citations still hold. The same goes for
  `src/promo.rs`: its lines 1 to 1090 are byte-identical to the item-13 tip, so no citation into
  it moved.
* **`tests/legacy_vocabulary_wiring.rs`.** The three survivors the two halves removed are all
  gone, each with its dated reason.
* **A false "Open" bullet.** The server half's DECISIONS.md entry said the old media under
  `assets/` was still served. It now says the client half closed that the same day.
* **Floors.** Gate-1 floors were re-measured after the merges: `legacy_retirement` 254/57 and
  `locale_policy` 136/35. All 45 floors equal the pinned compiler's counts.

**`dist/`.** It was rebuilt once with `scripts/build-frontend.sh` (trunk 0.21.14, wasm-bindgen
0.2.122): bundle `17f356526a369bb9` → `c7cbc40d63a3b4f0`. The new wasm holds the refused-cancellation
sentence in both languages, 0 × "PCX 150", 2 × "NMAX 155", and 0 × the strain menu's title.
That sentence is the FIRST wording; `cafe215` reworded it, and this bundle has not been rebuilt
since (see "Three fixes on top of `69fd4fa`" below).
`scripts/predeploy-smoke.sh --no-build` on the committed `dist/` printed `SMOKE PASS` (app
mounted, console clean), and `git status --porcelain` stayed empty afterwards.

```sh
# 2026-09-25, t27/owner-answers-2509 after the six merges and the dist rebuild, pinned compiler 40003ed
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler
# OK - 45 manifested specs, 5 generators each (all 45 floors equal the measurement)
python3 scripts/execute_t27_assertions.py            # WITH the compiler cross-check
# OK - 45 spec(s), 10246 assert line(s) scanned, 10246 executed, 10246 passed, 0 failed;
#      9845 declaration name(s) and function bodies agreed with t27c; 2 pinned front-end disagreement(s)
python3 scripts/verify_t27_against_source.py --require-git-tracked
# OK - 239 bindings hold across 41 contracts and 80 source files
python3 scripts/verify_fleet_seed.py -v
# OK — 14 families (13 offered), 37 units (11 rented, 26 available); ...; redirects click-125 -> nmax-155 ...
```

**Rust.** `cargo fmt -- --check` was clean. Both clippy forms passed with `-D warnings`: backend
(`--bin turbobaby-bot-server`, and without `--bin`) and `--target wasm32-unknown-unknown --lib`.
`cargo test --features backend` passed 2 416 tests with 0 failed and 133 ignored, doc tests
included.

That test count was taken in two parts, because of the shared target dir
(`D:/turbobaby-bike-bot/target`). Six test executables there have a PDB written at 2026-09-25
17:11 that the linker rejects as corrupt (LNK1285): `ui_safety`, `integration_promo_agent`,
`promo_copy_wiring`, `fleet_pii_boundary`, `referrals` and `integration_share_source`. D: had about
2.3 GB free, and each integration-test PDB is about 430 MB. Nothing in the target dir was deleted.
Those six were linked with `cargo rustc … -- -C link-arg=/PDB:<temp>` and run from the worktree
root, and the other 66 binaries ran under `cargo test`. The six corrupt PDBs are still there;
whoever owns that dir can delete them.

**DB-backed tests.** These ran on a fresh private PostgreSQL 18.0 at 127.0.0.1:55433 (`initdb -A
trust -U postgres`), with `DATABASE_URL=postgres://postgres@127.0.0.1:55433/<fresh db>` and
`-- --include-ignored`. `https_reaches_telegram` was left out, because it calls `api.telegram.org`.

* **Run serially** (`--test-threads=1`, one fresh database), 2 488 passed and 13 failed. A clean
  export of `0526429`, run the same way on its own fresh database, fails the same 13. It also fails
  2 tests that need a `.git`, which the export does not have.
* **The 13 are not caused by this change.** The tests and migrations are byte-identical to the
  base. Eleven read tables that migration 083 dropped: `strains` in create_order ×3, marketing and
  strain_of_day ×5, and `garden_rewards` in use_reward ×2. Two need an available accessory, which
  085 hid (cart_merge ×2).
* **`integration_promo_agent`**, run from its relinked executable, fails 4 of 7 on the same
  dropped `strains`.
* **Run in parallel**, which is the default, integration_events, events_same_day, client_errors
  and invitee_names fail differently from run to run on base and on this branch alike. Their tests
  share rows. All four pass on both trees when run serially on a fresh database.

### Three fixes on top of `69fd4fa`, 2026-09-25, and what they still owe

Three commits on `t27/owner-answers-2509`, not pushed. Dates are UTC; the commits carry the
working machine's +07:00 clock, which had already turned to 2026-09-26.

* **`cafe215`, item 11 reworded (operator, under the owner's delegation).** The 409 sentence is
  now «Отменить этот заказ в приложении уже нельзя. Напишите менеджеру.» / "This order can no
  longer be cancelled in the app. Please message the manager." It is true of every 409:
  `cancel_order` answers 409 for any stored status that is not pending, including an order the
  shop rejected and one already cancelled. The first named limit is CLOSED; the missing link to
  the manager on the order card stays open (bullet D below). The key keeps its name, and both
  `i18n.rs` rows were replaced on their own lines (1157 and 1757), so no citation moved.
  client_errors floor 248 → 253.
* **`cead405`, the bot's /start line.** `welcome_feature1` read «Каталог: от Click 125 до X-ADV
  750», but CLICK 125 is offered to nobody. Measured from `data/fleet_seed.json`: 13 offered
  families (the 7 `price_list_only` ones are a separate block). The smallest is 155 cc, a tie
  between `nmax-155` (449 THB) and `xsr-155` (590 THB); `nmax-155` wins on the lower rate. The
  largest is `xadv-750`, alone at 750 cc. The line is now «Каталог: от NMAX 155 до X-ADV 750» /
  "Catalog: from NMAX 155 to X-ADV 750". It is recorded in `availability.t27`
  (`START_CATALOG_RANGE_*`, floor 120/34 → 129/35) and guarded by
  `tests/catalog_honesty_wiring.rs`, which derives both ends from the seed.
* **`99b6716`, no cannabis branding in the dev and ops files (owner ruling #12).** Changed: the
  `run.sh`/`dev.sh` banners, the `.env.template` header, the alert names `WoodyWeedBot*` →
  `TurboBaby*`, the dashboard title, the `export-assets.sh` archive prefix (`turbobaby-assets-`),
  the four design pages at the root, the lefthook example, and the stale e2e suite. The e2e tab
  list now matches `AdminPanel`, and both spec files are skipped with a dated reason (the admin
  gate is password-only). `tests/legacy_vocabulary_wiring.rs` guards the 13 files
  (`DEV_OPS_FILES`). `observability.t27` gained a CORRECTED note, because that test now reads the
  three unguarded monitoring files (for words, not metric names). Left in place on purpose, none
  of them seen by a customer: the `wwb:` recording-rule prefix, the HMAC key literal in
  `src/api/auth.rs`, the storage keys `woody_last_*` and `wwb_admin_token`, the DOM event
  `woody:telegram-ready`, the warnings that name the other shop's service `woody-weed-bot` and the
  Railway project `woody`, `buildWoody` in `assets/game`, the strain-of-day comment in
  `lefthook.yml` (it names a test file), and all historical records. **Still carrying cannabis
  content, outside that task's list and not changed:** `screenshots/` (the generated UI-kit
  pages, `generate-pages.sh`, `e2e-pipeline.sh` and `e2e-report.md`) and `docs/DESIGN_SYSTEM.md`
  (a Chip example with Sativa/Indica and "OG Kush — Legendary"). Under the ruling they are the next
  cleanup. *Cleaned on 2026-09-26* by `t27/no-cannabis-docs`, merged as `313c365` (see the next
  section).

**Readings on `99b6716`**, pinned compiler 40003ed:

```sh
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler
# OK - 45 manifested specs, 5 generators each (floors equal the measurement: availability 129/35,
#      client_errors 253/58, observability 164/40, order_presentation 570/73)
python3 scripts/execute_t27_assertions.py            # WITH the compiler cross-check
# OK - 45 spec(s), 10273 assert line(s) scanned, 10273 executed, 10273 passed, 0 failed;
#      9859 declaration name(s) and function bodies agreed with t27c; 2 pinned front-end disagreement(s)
python3 scripts/verify_t27_against_source.py --require-git-tracked
# OK - 239 bindings hold across 41 contracts and 80 source files
python3 scripts/verify_fleet_seed.py -v
# OK — 14 families (13 offered), 37 units (11 rented, 26 available); ...
```

`cargo fmt --check` was clean. All 27 changed files are `i/lf w/lf`.

**CLEARED on 2026-09-26.** Every owed step below was done on the integration branch after the
second list's merges, and bundle `f81e946a9d640a39` carries the reworded sentence. The readings
are in the next section. What follows is kept as the record of 2026-09-25.

**Not done: drive D: is full.** On 2026-09-25, `df -h /d` read 222G in size with 17M available
at 18:37 UTC and 13M at 18:50 UTC, still falling; a review about an hour earlier had read 22M.
The shared target (`D:/turbobaby-bike-bot/target`) and this worktree are both on D:. That
review's run of `cargo test --features backend -j 2` failed with os error 112 (not enough space
on the disk) before the lib compiled. Agents may not delete anything outside their worktree, so
freeing space is for the operator or the owner. Until that happens:

* **`dist/` was not rebuilt.** The committed bundle is still `c7cbc40d63a3b4f0`, and its wasm
  still carries the FIRST wording of the 409 sentence. `grep -c -a -F` on
  `dist/turbobaby-bot-c7cbc40d63a3b4f0_bg.wasm` found «Этот заказ уже в работе» once, "already
  being handled and can't be cancelled" once, and each new sentence zero times. **A deploy of this
  branch as it stands would ship the old sentence.** The /start line is server code, and the
  dev/ops files are not in the bundle.
* **No clippy and no full `cargo test --features backend` ran on the three commits.** One reading
  was taken. `tests/catalog_honesty_wiring.rs` and `tests/legacy_vocabulary_wiring.rs` use only
  `std` and `serde_json`, so each was compiled on its own with `rustc 1.98.1 --edition 2021 --test`,
  linked read-only against the shared target's `serde_json` rlib, written to a scratch directory
  on C:, and run: 8 passed and 8 passed, 0 failed, the two new tests included. The extended host
  test in `src/trios/api_errors.rs` has **not** run.
* **What is owed, in order, once D: has room.** The PDB note above still applies.
  1. `cargo fmt --check`.
  2. Both clippy forms with `-D warnings`: backend (`--bin turbobaby-bot-server`) and
     `--target wasm32-unknown-unknown --lib`.
  3. `cargo test --features backend -j 2`, expecting 0 failed.
  4. `scripts/build-frontend.sh`, run once. Check that `dist/version.txt.br` is absent and that
     `dist/assets` is not staged.
  5. Grep the new wasm: both new sentences present, both old ones absent.
  6. Commit as `build(dist): … (bundle c7cbc40d63a3b4f0 -> <new>)`.
  7. `scripts/predeploy-smoke.sh --no-build` must print SMOKE PASS, with `git status` empty
     afterwards.
  8. Record the readings here.

### The owner's second list of 2026-09-25 and the operator's shop label, integrated 2026-09-26, and not yet live

The owner answered a second numbered list in chat on 2026-09-25 and confirmed its numbering the
same day. Six branches, each cut from `t27/owner-answers-2509` at `69fd4fa`, were merged on
2026-09-26 into that branch at `efc2aae`, in this order. Each merge used
`git -c core.autocrlf=false merge --no-ff` with diff3 conflict style. Two follow-up commits
reconciled the branches and applied an operator decision, one fixed a unit test, and one rebuilt
`dist/`. **Nothing is pushed and nothing is deployed.**

| commit | what |
| --- | --- |
| `fbdd469` | merge `t27/no-units-empty` (`eaf4426`), answer 5 |
| `77ac756` | merge `t27/no-age-gate` (`aa8d779`), answer 1 |
| `0ef7717` | merge `t27/legacy-lines-hidden` (`12aaf00`), answer 3 |
| `d064830` | merge `t27/legacy-cart-hidden` (`9c4a6eb`), the kept cart |
| `1aca8c4` | merge `t27/legacy-notifications-held` (`a992251`), the held notification kinds |
| `313c365` | merge `t27/no-cannabis-docs` (`a8872c9`), docs and demo pages |
| `a612624` | one rule for a kept cart (the reconciliation of answer 3 with the kept cart) |
| `6d8a2e3` | the operator's decision: an order of the previous shop shows no shop label |
| `45792a7` | the shop-label unit test made to compile (a closure's lifetime; the lib test target only) |
| `41315b2` | `dist/` rebuilt, bundle `c7cbc40d63a3b4f0` → `f81e946a9d640a39` |

| answer | what landed | what was left, and why |
| --- | --- | --- |
| 1, «Пока убираем» | The 20+ gate is removed for now, on the client and on the server. The checkout loses the box «Мне исполнилось 20+», its notice, the trust line «🛡️ Проверка возраста (20+)», the order button's age blocker and the submit handler's refusal. The request no longer sends `age_confirmed`. `POST /api/orders` no longer answers 422 when the field is absent or false. It still accepts the field and stores it as sent. Five keys were deleted, each line replaced by a comment line. Recorded in `checkout_contact.t27` (`AGE_BLOCKER_REMOVED_AT`, `AGE_BLOCKER_REMOVAL_IS_PROVISIONAL`), `legacy_retirement.t27` (`AGE_GATE_ANSWER_*`) and `client_errors.t27`. Gate 3 gained 2 bindings | The answer is provisional («Пока»). The `woody_last_age_confirmed` key in customers' browsers is neither read nor cleared (D19). Stored orders keep what they recorded |
| 2 | Nothing: the ride game's bird is undecided | Untouched |
| 3, «Все канабисное аналировать» | Read by the operator as: analyse all the cannabis-era content and take it out of customers' sight without deleting stored data. **Server** (`src/trios/legacy_view.rs`): a customer's own order list and detail serve a line of the old catalogue as «Позиция прежнего каталога» / "Item from the previous catalogue", with its quantity and unit price exactly as stored, and serve no id. The two reads withhold the old shop's name. The bonus history serves only the four sentences this repository writes, and serves a garden-era row with an empty type. **Client**: the order screens print the neutral name in the reader's language, and a garden row carries «Бонус». **The kept cart** (`t27/legacy-cart-hidden`): the cart API, the abandoned-cart reminder and the Mini App serve no line of a retired kind; the rows stay stored. **Held notifications** (`t27/legacy-notifications-held`): the queue never delivers a row of a retired or unknown kind (`friend_watered` included), and the garden's message left `src/locales.rs`. **Docs** (`t27/no-cannabis-docs`): `docs/DESIGN_SYSTEM.md`, `docs/event-share-templates.md` and the `screenshots/` demo pages carry no cannabis content, and every demo render is re-taken and classified (`tests/legacy_vocabulary_wiring.rs`). Gate 3 gained 4 + 2 + 1 bindings | Analysed and left, each with its reason in `legacy_retirement.t27`: the loyalty profile's stored tier key, the star history, the reads behind retired surfaces, and the referral, quest-scan and game reads. The admin reads are the archive and are unchanged. No row, no migration |
| 4 | Unavailable models were marked in the brain (done outside this repository) | — |
| 5, «Наверное» | The detail's line «Сейчас все байки этой модели заняты.» is gone, and its key `bike.units.empty` is deleted. Recorded in `availability.t27` (`UNITS_EMPTY_LINE_*`) | The Book control's reason under the same zero, `T_BIKE_BOOK_BLOCKED_NO_UNITS` («Все байки этой модели заняты»), was not named by the answer and stays (question I; answered 2026-09-26, see "Round 4, integrated") |
| operator, 2026-09-26 | An order of the previous shop is never shown as TurboBaby's. A withheld shop is served as the empty shop (`WITHHELD_SHOP`), and both order screens read it through `shown_shop`. The list prints the date alone, and the detail leaves out its labelled delivery row. An order with TurboBaby's stored shop, and one naming no shop, print exactly what they printed before. Recorded in `order_presentation.t27` (`RETIRED_SHOP_DECIDED_AT`, `RETIRED_SHOP_LABEL_SHOWN`, `RETIRED_SHOP_SERVED_AS`; `RETIRED_SHOP_FALLBACK_NOTE` closed), guarded by `tests/legacy_view_wiring.rs` and a unit test in `legacy_view.rs` | A bundle cached from before prints an empty value for such an order. That is not TurboBaby's name either |

**Conflicts, and how they were resolved.** Every side was kept.

* **Floors (gate 1).** Every floor was re-measured with the pinned compiler after each merge and
  each commit (`verify_t27_specs.py -v`, compared entry by entry with `SPEC_MANIFEST`). All 45
  equal the measurement on `41315b2`. The ones that moved: `availability` 129/35 and 132/36 →
  141/37; `client_errors` 253 and 249 → 254/58; `locale_policy` → 140/35, then 141/35, then 144/36;
  `legacy_retirement` 266/59 and 277/59 → 289/61, then 299/63 after the reconciliation;
  `cart_persistence` 266/52 → 276/54; `order_presentation` 598/76 → 606/77.
* **Bindings (gate 3).** The table keeps every row from every branch, and `MIN_BINDINGS` equals
  the measured table size after each merge: 241 → 245 → 247 → 248. That is 239 + 2 (answer 1)
  + 4 (answer 3) + 2 (kept cart) + 1 (held kinds), over 41 contracts and 83 source files.
* **Key count (`locale_policy.t27`).** Each branch measured without the others. On the merged
  `src/trios/i18n.rs`, 565 + 2 − 14 − 1 − 5 = **547** declarations, and the naive grep reads 570
  + 2 − 14 − 1 − 5 = **552**. The +2 are the cancel key and `T_ORDER_LINE_PREVIOUS_CATALOGUE`, the
  −14 is ruling #12, the −1 is answer 5 and the −5 is answer 1. Every branch's CORRECTED
  paragraph is kept. Two MERGED notes record the re-measurement, and the key-count test subtracts
  every deletion. The five non-declaration lines did not move.
* **DECISIONS.md.** The addenda are appended in merge order: answer 5, answer 1, answer 3, the
  kept cart. Then come the reconciliation and the shop-label decision. The answer-1 entry says the
  merged key count.
* **`legacy_retirement.t27`** keeps the answer-1 and answer-3 sections and both closing
  invariants. This file keeps question I as answer 5's branch wrote it, now marked merged.
* **(a) The cart, `a612624`.** Answer 3 served a retired cart line under the neutral name with no
  picture, and guarded a device cart the same way. The kept cart serves no such line at all.
  Merged, both ran, and the second was a no-op on the rental rows the first let through. **One
  rule, the kept cart's.** The neutral-name path is removed from `cart_model_to_resp`, from
  `legacy_view.rs` (`LIVE_CART_KIND`, `cart_line_is_retired`, `cart_line_name`,
  `cart_line_image`, `shown_cart_line_name`) and from the cart, checkout and `CartItemComponent`,
  byte for byte back to before answer 3. The one way into the Mini App's cart that did not ask
  `cart_kind_is_served`, `Cart::add_item` (reorders and unmounted screens), now asks it. So no
  screen can be handed a line that would need a neutral name, and
  `a_cart_line_is_held_to_the_kept_cart_rule_and_never_renamed` holds that. The contracts agree:
  `cart_persistence.t27` has a section "Reconciled with the owner's answer 3", and
  `legacy_retirement.t27` records answer 3's client guards as 4 → 2 and its left reads as 6 → 4.
  The abandoned-cart reminder was closed by the kept cart; answer 3's reason had missed a cart
  kept from before 085. The garden's notification arm was closed by the held kinds.

**`dist/`.** It was rebuilt once with `scripts/build-frontend.sh` (trunk 0.21.14, wasm-bindgen
0.2.122), bundle `c7cbc40d63a3b4f0` → `f81e946a9d640a39`, with no `dist/version.txt.br` and
`dist/assets` not committed. Readings in the new wasm (`grep -c -a -F`):

* the new 409 sentence «Отменить этот заказ в приложении уже нельзя. Напишите менеджеру.» 1, and
  its English 1;
* «Этот заказ уже в работе» 0, and "already being handled and can't be cancelled" 0;
* «Мне исполнилось 20+» 0, and «Сейчас все байки этой модели заняты» 0;
* «Позиция прежнего каталога» 1.

The client vocabulary of `tests/no_cannabis_client_wiring.rs` finds nothing in the js. In the wasm
and `index.html` it finds only internal identifiers, the same count as in the previous bundle: the
DOM events `woody:mainbutton`, `woody:contact` and `woody:telegram-ready`, and the compatibility
path `/sommelier`. `scripts/predeploy-smoke.sh --no-build` printed `SMOKE PASS` (app mounted,
console clean), and `git status --porcelain` stayed empty afterwards.

```sh
# 2026-09-26, t27/owner-answers-2509 at 41315b2, pinned compiler 40003ed
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler
# OK - 45 manifested specs, 5 generators each (all 45 floors equal the measurement)
python3 scripts/execute_t27_assertions.py            # WITH the compiler cross-check
# OK - 45 spec(s), 10581 assert line(s) scanned, 10581 executed, 10581 passed, 0 failed;
#      10109 declaration name(s) and function bodies agreed with t27c; 2 pinned front-end disagreement(s)
python3 scripts/verify_t27_against_source.py --require-git-tracked
# OK - 248 bindings hold across 41 contracts and 83 source files
python3 scripts/verify_fleet_seed.py -v
# OK — 14 families (13 offered), 37 units (11 rented, 26 available); ...; redirects click-125 -> nmax-155 ...
```

**Rust.** `cargo fmt -- --check` was clean. All three clippy forms passed with `-D warnings`:
`--features backend --bin turbobaby-bot-server`, `--features backend`, and
`--target wasm32-unknown-unknown --lib`. `cargo test --features backend -j 2` gave 75 result lines:
2 498 passed, 0 failed and 135 ignored, doc tests included. No test PDB was rejected this time,
because `D:/t27work/.cargo/config.toml` now builds dev and test with `debug = 0`. The shared target
was `D:/turbobaby-bike-bot/target`, and every cargo and trunk run was the only one, with `-j 2`.
Drive D: went from 41.3 GB free to 38.3 GB over the whole session, and C: read 8.0 to 8.1 GB throughout.

**DB-backed tests.** These ran on a fresh private PostgreSQL 18.0: `initdb -A trust -U postgres -E
UTF8 --locale=C`, data dir `D:/t27work/pgdata-final-2509`, listening on 127.0.0.1:55434 only, with
one fresh database. `HTTPS_PROXY`, `HTTP_PROXY`, `ALL_PROXY` and `TELOXIDE_PROXY` were set to
`http://127.0.0.1:9`, so nothing left the machine. The run was
`cargo test --features backend -j 2 --no-fail-fast -- --include-ignored --test-threads=1`,
skipping the two tests of `tests/https_reaches_telegram.rs`. It gave 75 result lines: 2 614
passed, 17 failed and 0 ignored.

The 17 are the known stale fixtures of the base, and nothing else failed:

* `strains`, dropped by 083: create_order ×3, marketing ×1, strain_of_day ×5 and promo_agent ×4;
* `garden_rewards`, dropped by 083: use_reward ×2;
* an available accessory, hidden by 085: cart_merge ×2.

That is the 13 of the serial run of 2026-09-25 plus promo_agent's 4, which that run took from a
relinked executable. `create_order_does_not_require_age_confirmation` (answer 1) passed against
the database. The server was stopped with `pg_ctl stop -m fast`, and the data dir is left in
place.

**What is left.**

* Push, review and merge of `t27/owner-answers-2509`, then a deploy of `main`. Nothing here is
  live. The deploy crosses 087 (`docs/ROLLBACK.md` §4).
* Answer 1 is provisional. The owner may bring the box back; git history holds all of it.
* Answer 2, the ride game's bird, is undecided and untouched.
* Question I: the Book control's reason `T_BIKE_BOOK_BLOCKED_NO_UNITS` under a seeded zero.
  *Answered on 2026-09-26* («Разрешить бронь, наличие уточнит менеджер»); see "Round 4, integrated".
* Answer 3's four analysed-and-left reads, with their reasons in `legacy_retirement.t27`: the loyalty
  tier key, the star history, the retired surfaces' reads, and the referral, quest-scan and game
  reads. The legacy HTTP routers behind the seven repointed paths still answer.
* The cart's write gate, `parse_kind`, still names the old kinds. None of them can write a row
  today, and closing the gate is a separate change (`cart_persistence.t27`).
* The order card still has no link to the manager (bullet D).

### Round 4, server lane: the owner's answers of 2026-09-26 and the stale fixtures, not yet live

Branch `t27/round4-server`, cut from `main` at `405f30e` (#67). Five commits, one per item, and one
for the docs; **nothing is pushed and nothing is deployed**. `dist/` was not rebuilt: every change
is server code, and no client file changed. The owner's words are quoted verbatim in DECISIONS.md
(the D19 addendum of 2026-09-26) and in the Rust modules; the `.t27` files stay ASCII.

| commit | item | what landed | recorded in |
| --- | --- | --- | --- |
| `f3ffc98` | A1, answer «А зачем это вообще там?» | A customer never sees an order of the previous shop: one that names that shop, or holds no bike line, is not listed (the list pages past it, so it takes none of the 50 places), is answered 404 by `get_order_details`, `get_order_status` and `cancel_order` on the owner check's own line, and is not counted in the profile's `orders_count` (the SQL no longer joins orders). A rental beside an old line stays shown, masked. Admin reads unchanged. Every cited line of `src/api/orders.rs` stayed except the list's cap (2502 → 2503, re-pinned) | `order_presentation.t27` `PREVIOUS_SHOP_ORDER_*`, `legacy_retirement.t27` `OWNER_ORDERS_ANSWER_*` |
| `261a0a9` | A2, answer «Зачем они вообще нужны мне?» | `/uploads` serves only a name a bike's `image_url` references exactly (`/uploads/<name>`), through a gate in front of the same directory service on the same line of `src/main.rs`; everything else answers 404. Nothing deleted. The bucket: not served by this server, and its previous-shop keys cannot be told apart by code (one prefix `uploads/` since `436f56c`), so it **needs a production listing** | `upload_media.t27` `OWNER_MEDIA_ANSWER_*`, `LOCAL_READ_*`, `OBJECT_STORE_*` |
| `784680e` | A3, critic note 1 | The 24-hour reminder joins public events only; a hidden event is never reminded. Cancellation and refunds unchanged. `REMINDER_KEPT_FOR_SEATS_ALREADY_HELD` reversed by the operator under answer 3 | `events_booking.t27` `REMINDER_REVERSED_*` |
| `0f35061` | A4, critic note 5 | New welcome credits say "Welcome bonus from a friend's invite" (the garden's words dropped; none of the four existing sentences fits). The bonus history serves it; older rows stay withheld. *Serving it was reversed on review the same day: the words are the lane's, not the owner's, so the sentence is stored and not served (see "Round 4, review fixes" below)* | `legacy_retirement.t27` `WELCOME_CREDIT_SENTENCE`, `WELCOME_SENTENCE_*` |
| `04f75e9` | A5, critic note 6 | The 17 stale DB-backed tests: create_order ×3 and promo_agent ×4 rewritten on a `bike_rental` line of `nmax-155` and on `bikes` rows; cart_merge ×2 hold the cart race (the merge gate names no rental kind, so the summing moved to an in-crate DB test of the write step); marketing ×1, strain_of_day ×5 → 2 and use_reward ×2 hold the retirement | the test files' headers |

**Gates**, on the docs commit, pinned compiler `40003ed`:

```sh
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler
# OK - 45 manifested specs, 5 generators each (all 45 floors equal the measurement)
python3 scripts/execute_t27_assertions.py            # WITH the compiler cross-check
# OK - 45 spec(s), 10679 assert line(s) scanned, 10679 executed, 10679 passed, 0 failed;
#      10186 declaration name(s) and function bodies agreed with t27c; 2 pinned front-end disagreement(s)
python3 scripts/verify_t27_against_source.py --require-git-tracked
# OK - 255 bindings hold across 41 contracts and 83 source files
python3 scripts/verify_fleet_seed.py -v
# OK — 14 families (13 offered), 37 units (11 rented, 26 available); ...
```

Floors that moved: `order_presentation` 606/77 → 625/80, `legacy_retirement` 299/63 → 320/67,
`upload_media` 253/50 → 275/53, `events_booking` 326/85 → 338/87. Gate 3 gained 7 bindings (248 →
255): the list's cap and the three by-id guards, the `/uploads` nest and the bikes lookup, the
reminder's filtered join, and the welcome sentence in its writer and in its rule; each was planted
red once by hand and restored. `f3ffc98` left the `order_presentation` floor one below the
measurement (624 against 625); `261a0a9` closed it.

**Rust.** `cargo fmt -- --check` clean. The three clippy forms passed with `-D warnings`
(`--features backend --bin turbobaby-bot-server`, `--features backend`, and
`--target wasm32-unknown-unknown --lib`). `cargo test --features backend -j 2 --no-fail-fast`: 77
result lines, 2 510 passed, 0 failed, 141 ignored.

**DB-backed.** A fresh private PostgreSQL 18.0 (`initdb -A trust -U postgres -E UTF8 --locale=C`),
data dir `D:/t27work/pgdata-r4a-2509`, listening on 127.0.0.1:55435 only, a fresh database per run,
`HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`/`TELOXIDE_PROXY` = `http://127.0.0.1:9`. The six stale
targets on base `405f30e`'s tests: 17 failed (the known 17). After the change, the whole suite,
`cargo test --features backend -j 2 --no-fail-fast -- --include-ignored --test-threads=1` skipping
the two tests of `tests/https_reaches_telegram.rs`: 77 result lines, **2 649 passed, 0 failed**.
The new DB tests: `tests/integration_previous_shop_orders.rs`, and in-crate
`only_a_file_a_bike_references_is_served` (upload.rs),
`the_reminder_skips_a_hidden_event_and_reminds_a_public_one` (events.rs, against a Telegram stand-in on 127.0.0.1; with the filter removed
it fails, as it should), `a_new_welcome_credit_names_no_garden_and_is_served` (referrals.rs; `…_and_is_withheld` since the review) and
`concurrent_upserts_of_one_rental_line_sum_their_quantities` (cart.rs).

**What is left.**

* The bucket's previous-shop objects: a production listing of `uploads/` keys against the keys the
  rental rows reference, then a bucket read-policy change. Deleting any file is the owner's act.
* `POST /api/cart/merge` with a line of kind `strain` still reaches the dropped table and answers
  500 (the known D9 survivor in `tests/retired_table_wiring.rs`), and no merge can write a rental
  line until `parse_kind` admits `bike_rental` (`cart_persistence.t27`).
* A message the bot sends to a customer when an admin changes the status of a previous-shop order
  is the admin's act and was left; so were the referral panel's `has_ordered` flag (a friend's
  orders, counted whole) and `db.get_strains_of_day`, which no code calls now.
* The client lane's items of the same round (the Book control under a seeded zero, "Unknown" on
  bike lines, the unused strings in the wasm) are not in this branch. *Merged beside it on
  2026-09-26:* see the next section.

### Round 4, integrated: the owner's answers of 2026-09-26, not yet live

Branch `t27/round4`, cut from `main` at `405f30e` (#67). It merges `t27/round4-server` (`dbdb640`,
the section above) and then `t27/round4-client` (`f1f8517`), both with `--no-ff`, rebuilds `dist/`
once and adds this section. **Nothing is pushed and nothing is deployed.** No row is deleted or
rewritten, no migration is added, and no file is removed from a server volume or a bucket.

| commit | what |
| --- | --- |
| `581323e` | merge of `t27/round4-server`, no conflict |
| `d1ddf3d` | merge of `t27/round4-client`; five files conflicted, each resolved to both sides (below) |
| `2f801fe` | `dist/` rebuilt, bundle `f81e946a9d640a39` → `7143e607a7409c70` |
| `ca5ce6f` | the test helper the fixture rewrite left unused, removed |
| the next commit | this section, and the question I notes above and below marked answered |

The owner answered three questions on 2026-09-26. The words are quoted verbatim in DECISIONS.md
(the entries of that date) and in the Rust modules; the `.t27` files stay ASCII. The rulings in
force stay: rental only, Phuket only, nothing deleted (2026-09-24); «всё что касается канабиса
нигде не должно быть» and «Все канабисное аналировать» (2026-09-25).

| question | the owner's words | read as | what changed | recorded in |
| --- | --- | --- | --- | --- |
| Q1, the previous shop's order lines shown as «Позиция прежнего каталога» | «А зачем это вообще там?» | The operator's reading: a customer must not see the previous shop's orders at all | Server, `f3ffc98`. The rule is `customer_sees_order` (`src/trios/legacy_view.rs`): an order naming the previous shop, or holding no bike line, is not shown. It is asked through `Order::shown_to_customer` (`src/db/orders.rs`) by the list, which pages past such an order so it takes none of the 50 places; by the detail, the status and the customer's own cancel, which answer 404 exactly as for a missing order; and by the profile's `orders_count` (`Order::shown_count`). An order with a rental line beside a line of the old catalogue is this shop's and stays shown with that line masked, which is now the only case the neutral name is served for, so that key stays in the bundle. The admin reads are the archive and are unchanged | `order_presentation.t27` `PREVIOUS_SHOP_ORDER_*`, `legacy_retirement.t27` `OWNER_ORDERS_ANSWER_*` |
| Q2 (question I), the Book control disabled with «Все байки этой модели заняты» under the zero the 2026-09-12 seed counts | «Разрешить бронь, наличие уточнит менеджер» | As worded. Removing the «Наличие не подтверждено» arm too is the contract's reading (a manager confirms availability for every family) | Client, `8c85cb3`. `book_block` (`src/ui/screens/catalog_screen.rs`) reads no unit count; `BookBlock` loses `NoUnitsFree` and `UnknownAvailability`, and both keys are deleted. What still disables Book: a closed family (D12), no published rate (D11), a screen with no booking handler. The unmounted `bike_card.rs` no longer needs a free unit to add to cart and prints the manager line instead of the count. `create_order` never refused on the count (`check_bike_lines` only logs a shortfall) and is unchanged, now guarded | `availability.t27` (`NO_UNITS_BOOK_REASON_*`, `UNKNOWN_AVAILABILITY_*`, `customer_may_book`, `UNKNOWN_AVAILABILITY_REMOVAL_IS_THIS_FILES_READING`), `locale_policy.t27` (547 → 545), `tests/catalog_honesty_wiring.rs` |
| Q3, the previous shop's media still reachable by a direct link | «Зачем они вообще нужны мне?» | The operator's reading: stop serving them. Deleting the files is the owner's own irreversible act and is not done here | Server, `261a0a9`. `/uploads`, on the same nest line of `src/main.rs`, serves a name only when some bike's `image_url` is exactly `/uploads/<name>`; everything else answers 404. The upload's local branch still writes there when no object store is configured, which is why the route stays. The bucket is not served by this server, and both shops wrote the one prefix `uploads/`, so telling the previous shop's objects apart needs a production listing, and stopping them needs a bucket policy change. Neither was done | `upload_media.t27` `OWNER_MEDIA_ANSWER_*`, `LOCAL_READ_*`, `OBJECT_STORE_*` |

The critic's notes on `405f30e`:

| note | what changed | commit | recorded in |
| --- | --- | --- | --- |
| 1, the 24-hour event reminder mailed hidden events' stored titles and venues | It now joins public events only. `REMINDER_KEPT_FOR_SEATS_ALREADY_HELD` (operator, 2026-09-25) is reversed by the operator under «Все канабисное аналировать». Cancellation and the Stars refund of a held seat are unchanged | `784680e` | `events_booking.t27` `REMINDER_REVERSED_AT`, `REMINDER_FLAG_FILTERED_JOINS` |
| 2, cannabis-era strings the public wasm still carried | Nineteen keys deleted, each line replaced by a comment line: the garden's bonus label («Награда из сада» / "Garden reward"), «Хранение» and «Зажигалка» with their chips, the strain card's certificate link (`T_MODAL_CERTIFICATE`, beyond the critic's list), and fifteen keys of the unmounted shop game's farm («Посадить», «Собрать», «Урожай!» and the rest). The farm zone left `src/ui/game/shop_game.rs`. The ride game is untouched (answer 2) | `fb5913b` | `legacy_retirement.t27` `LEFTOVERS_2026_09_26_*`, `locale_policy.t27` (545 → 526), `tests/no_cannabis_client_wiring.rs` |
| 3, `/uploads` served `/data/uploads` | Q3 above | `261a0a9` | `upload_media.t27` |
| 4, bike lines printed "Unknown" | `bike_line_name` (`src/trios/order_line.rs`, host-compiled): the bike's stored name when it has text, else its family key; nothing looked up, nothing invented. Used on the order list, the order detail and, beyond the two screens the critic named, the profile's recent orders. The keyless fallback stays on its cited lines | `f1f8517` | `order_presentation.t27` `BIKE_LINE_NAME_*`, `tests/order_presentation_wiring.rs` |
| 5, a new welcome credit said "…a friend's garden invite" | New rows store "Welcome bonus from a friend's invite". *Since the review: not served, because no owner worded it; every welcome row shows the generic label and the amount, and the wording is a question for the owner* (first version: the bonus history served it) | `0f35061`, review below | `legacy_retirement.t27` `WELCOME_CREDIT_SENTENCE`, `WELCOME_SENTENCE_*` |
| 6, the 17 DB-backed tests failing on stale fixtures | Rewritten on a `bike_rental` line of `nmax-155` and on `bikes` rows, or as tests of the retirement | `04f75e9` | the test files' headers |

**Readings, each easy to revert.** Q1 and Q3 are the operator's readings of the owner's questions.
Q1 treats an order with no bike line as the previous shop's, the same fail-closed reading as the
rules before it. Removing the «Наличие не подтверждено» Book reason goes one step past Q2's words.
The `bike_card.rs` change is to a component no screen mounts.

**Conflicts, each resolved to both sides** (`d1ddf3d`; the server merge, `581323e`, had none):

* **DECISIONS.md.** The server's addendum first, then the client's two entries (question I, then
  the leftovers). The server addendum's closing sentence now points at the entry that follows
  instead of saying the Book question is "not recorded here"; it keeps its two lines.
* **`legacy_retirement.t27`, `order_presentation.t27`.** Each side's block, test and invariant kept,
  the server's first. No name is declared twice.
* **Floors (gate 1).** `legacy_retirement` 320/67 (server) and 317/65 (client) merge to the measured
  **338/69** (299/63 + 21/4 + 18/2); `order_presentation` 625/80 and 622/79 to the measured
  **641/82** (606/77 + 19/3 + 16/2). All 45 floors equal the pinned compiler's counts.
* **Bindings (gate 3).** Every comment paragraph kept; `MIN_BINDINGS` = **256**, the measured table
  size (248 + 7 server + 1 client), across 41 contracts and 84 source files.
* **Key count.** The server lane declared and deleted no key, so the client's count stands:
  565 + 2 − 14 − 1 − 5 − 2 − 19 = **526** declarations, and the naive grep reads **531**
  (measured on the merged `src/trios/i18n.rs`).
* No `file:line` citation of one lane points into a file the other lane changed (checked on both
  diffs).

**Gates**, on `2f801fe` and again, with the same numbers, on `ca5ce6f` and on the commit that adds
this section; pinned compiler `40003ed`:

```sh
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler
# OK - 45 manifested specs, 5 generators each (all 45 floors equal the measurement)
python3 scripts/execute_t27_assertions.py            # WITH the compiler cross-check
# OK - 45 spec(s), 10754 assert line(s) scanned, 10754 executed, 10754 passed, 0 failed;
#      10239 declaration name(s) and function bodies agreed with t27c; 2 pinned front-end disagreement(s)
python3 scripts/verify_t27_against_source.py --require-git-tracked
# OK - 256 bindings hold across 41 contracts and 84 source files
python3 scripts/verify_fleet_seed.py -v
# OK — 14 families (13 offered), 37 units (11 rented, 26 available); ...
```

**Rust.** `cargo fmt -- --check` clean. The three clippy forms passed with `-D warnings`
(`--features backend --bin turbobaby-bot-server`, `--features backend`, and
`--target wasm32-unknown-unknown --lib`). `cargo test --features backend -j 2`: 77 result lines,
2 526 passed, 0 failed, 141 ignored, doc tests included.

**`dist/`**, `2f801fe`. Built once with `scripts/build-frontend.sh` (trunk 0.21.14, wasm-bindgen
0.2.122, `CARGO_BUILD_JOBS=2`, the shared target) from `d1ddf3d`: bundle `f81e946a9d640a39` →
`7143e607a7409c70`, no `dist/version.txt.br`, `dist/assets/` still ignored. Read inside the new wasm
(`grep -c -a -F`): «Награда из сада», "Garden reward", «Зажигалка», «Хранение», «Посадить»,
«Собрать», «Урожай!», «Все байки этой модели заняты» and «Наличие не подтверждено» **0 each** (1 each
in `f81e946a9d640a39`); «Позиция прежнего каталога» 1, kept for a mixed order. The client vocabulary
of `tests/no_cannabis_client_wiring.rs`, with its farm words, finds nothing in the js and, in the
wasm and `index.html`, only the internal identifiers the previous bundle had too (the DOM events
`woody:mainbutton`, `woody:contact`, `woody:telegram-ready` and the compatibility path
`/sommelier`); the previous bundle's farm hits are gone. `scripts/predeploy-smoke.sh --no-build`:
**SMOKE PASS** (the app mounted, no uncaught JS error), and `git status` was empty after it.

**DB-backed**, on `2f801fe`. A fresh private PostgreSQL 18.0 (`initdb -A trust -U postgres -E UTF8
--locale=C`), data dir `D:/t27work/pgdata-r4-2509`, listening on 127.0.0.1:55436 only, one fresh
database, `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`/`TELOXIDE_PROXY` = `http://127.0.0.1:9`. The
whole suite, `cargo test --features backend -j 2 --no-fail-fast -- --include-ignored
--test-threads=1` skipping the two tests of `tests/https_reaches_telegram.rs`: 77 result lines,
**2 665 passed, 0 failed, 0 ignored** (2 filtered out). That is the 2 526 + 141 of the run without
a database, less the two skipped, so the critic's 17 stale fixtures now pass with everything else.
The server was stopped with `pg_ctl stop -m fast`, and the data dir is left in place.

The suite's one compiler warning was new in round 4: `04f75e9` left `rand_suffix` in
`tests/integration_marketing.rs` with no caller. `ca5ce6f` removes it; that binary, re-run on the
same database, gave 3 passed and no warning.

**What is left.**

* Push, review and merge of `t27/round4`, then a deploy of `main`. Nothing here is live, and the
  committed `dist/` is what a deploy would serve.
* The bucket's previous-shop objects: a production listing of the `uploads/` keys against the keys
  the rental rows reference, then a bucket read-policy change. Deleting any file is the owner's act.
* `POST /api/cart/merge` with a line of kind `strain` still reaches the dropped table and answers
  500 (the D9 survivor in `tests/retired_table_wiring.rs`); no merge can write a rental line until
  `parse_kind` admits `bike_rental` (`cart_persistence.t27`).
* Left by the server lane, each with its reason in `order_presentation.t27` or the section above:
  the bot's status message to a customer when an admin changes a previous-shop order, the referral
  panel's `has_ordered` flag, and `db.get_strains_of_day`, which no code calls now.
* `catalog_api.t27`'s cursor census (185/183) is a dated reading; 182 was measured at `405f30e`.
* Answer 2, the ride game's bird, is undecided and untouched. The order card still has no link to
  the manager (bullet D).

### Round 4, review fixes, 2026-09-26, not yet live

Two demands of the review of `ff822e3`, each verified against the tree before it was applied, on
the same branch `t27/round4`. **Nothing is pushed and nothing is deployed.** No row is deleted or
rewritten, no migration is added, no file is removed from a volume or a bucket.

| commit | what |
| --- | --- |
| `ac8a10e` | stale `file:line` citations re-pinned in place, each with its old number and the date |
| `168892d` | the welcome credit's new sentence is stored and NOT served, until the owner words it |
| the next commit | this section, and the round-4 rows above marked |

**1. Citations the round moved and did not re-pin.** `04f75e9` grew the `get_strains_of_day`
survivor's reason in `tests/retired_table_wiring.rs` by one line, so everything below its line 75
moved down by one. The review named seven stale citations into that file and one stale survivor
reason. Each was checked: the cited line was read at `405f30e` and at `ff822e3`. All eight were
right, and all were re-pinned:

* `bot_surface.t27`: the survivor entry `:66-75` → `:66-76` (the prose and `LEGACY_SURVIVOR_ENTRY`);
* `legacy_retirement.t27`: the allowlist `:49-82` → `:49-83` (two sites), the reason floor's
  assert `:499` → `:500`;
* `schema_provenance.t27`: the fifth shallow walk `:97` → `:98` (the prose and
  `SHALLOW_WALK_OUTSIDE_THE_RUNNER_MODULE`; no assertion compares the string);
* `tests/legacy_vocabulary_wiring.rs`: the `src/db/mod.rs` survivor said an ignored test still
  calls `get_strains_of_day`. It now says what `retired_table_wiring.rs` says: no test caller
  since 2026-09-26.

The review's method compared `path:N` citations only. A second pass also followed bare `:N`
continuations after a named path, over every tracked file that cites into a file the branch
changed. It found four more that the round had moved:

* `legacy_retirement.t27`: the allowlist's three tests `:450, :476, :495` → `:451, :477, :497`
  (the last one was already a line short at `405f30e`), and `TABLE_POSITION` `:303` → `:304`.
* `scripts/verify_t27_against_source.py`: the `UNIT_ROLLUP_QUERY_ROW_CAP` binding's "why" cited
  `catalog_api.t27:397-401`, and this round's own RE-POINTED note moved that span to `:401-405`.
* `legacy_retirement.t27`: the shop game's rsx root, `shop_game.rs:679`, is at `:596` since
  `fb5913b` removed the farm zone.
* `bot_surface.t27:65`: `:66-75` → `:66-76`.

Two dated re-readings got a dated note: `bot_surface.t27` `(:49-82)` of 2026-09-21, and
`legacy_retirement.t27`'s refutation "closes at 82". The citing lines this branch wrote itself
were also checked, 30 of them, each against the cited file as it stood at the citing commit (found
by `git blame`). None moved. Still stale, and already stale at `405f30e`: `schema_provenance.t27`'s
citations into `scripts/verify_t27_specs.py` (`:257`, `:313-322`, `:326-330`, `:397-408`, `:418`,
`:175-184`, `:28-287`, `:310-311`) and `deposit_tiers.t27:14` (`:119-121`). The manifest grows with
every floor comment. That is its own piece of work and is not changed here. No line was added to
any contract or to the manifest. No floor, binding or assertion count moved in `ac8a10e`.

**2. The welcome credit's sentence.** The review was right. `0f35061` put `referral_welcome` into
`DESCRIBED_TX_TYPES` (4 → 5) with an exact-match arm, so the bonus history served "Welcome bonus
from a friend's invite" in both locales. `profile_screen.rs` prints `tx.description` under the
label. Before that commit a welcome row showed only the generic label. The critic's note 5 gave
no wording, and no owner answer or existing key supplies that sentence. The lane trimmed it
itself, and DECISIONS.md credited it to the operator. Now:

* the writer is unchanged: a new row still stores the trimmed sentence, with no garden words;
* the read withholds it: `DESCRIBED_TX_TYPES` is four again, and there is no welcome arm. Every
  welcome row, old or new, reaches a customer as the generic label (`T_PROFILE_BONUS_OTHER`) and
  the amount, as before the round;
* `legacy_retirement.t27`: `OWNER_ANSWER_3_DESCRIBED_TX_TYPE_COUNT` is 4 again,
  `WELCOME_SENTENCE_IS_SERVED` = false, `_WORDED_BY_THE_OWNER` = false, `_CHOSEN_BY` = "lane",
  and `_AWAITS_THE_OWNER` = true. The floor moved from 338/69 to the measured 341/69;
* gate 3: the second welcome binding was re-pointed from "served sentence" to "withheld
  sentence". Both welcome bindings were planted RED once by hand and restored. There are still 256
  rows;
* DECISIONS.md records it as the lane's interim choice, with an **open question for the owner:
  what should an invited customer's welcome bonus say in the bonus history?**

`dist/` was not rebuilt. The bonus-history rule is server-only: `customer_bonus_description` is
called from `src/api/loyalty.rs` alone. The committed wasm holds neither "friend's invite" nor
"Referral bonus for new user" (`grep -c -a -F`, 0 and 0), and the client half of
`legacy_view.rs` (`shown_line_name`, `shown_shop`) did not change. The bundle stays
`7143e607a7409c70`, so the smoke test of `2f801fe` still covers what a deploy would serve.

**3. The critic's (a), "shown to customers or served by the server".** It found no cannabis-era
text, and there was nothing to fix. The one borderline item is the `/joke` topic "a motorbike
sommelier pretending to be fancy" (`JOKE_STYLES` in `src/ai.rs`). That is a prompt, not shown
copy, and the word is not cannabis-related. The critic left it to the operator, and it is left
here too.

**Gates** on `168892d`, pinned compiler `40003ed`:

```sh
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler
# OK - 45 manifested specs, 5 generators each (all 45 floors equal the measurement)
python3 scripts/execute_t27_assertions.py            # WITH the compiler cross-check
# OK - 45 spec(s), 10758 assert line(s) scanned, 10758 executed, 10758 passed, 0 failed;
#      10242 declaration name(s) and function bodies agreed with t27c; 2 pinned front-end disagreement(s)
python3 scripts/verify_t27_against_source.py --require-git-tracked
# OK - 256 bindings hold across 41 contracts and 84 source files
python3 scripts/verify_fleet_seed.py -v
# OK — 14 families (13 offered), 37 units (11 rented, 26 available); ...
```

**Rust.** `cargo fmt -- --check` is clean. The three clippy forms pass with `-D warnings`.
`cargo test --features backend -j 2 --lib --test legacy_view_wiring --test
legacy_vocabulary_wiring --test promo_report_names_wiring --test retired_table_wiring --test
t27_gates_run` gave lib 1 072 passed, 7 ignored, then 13, 12, 8, 9 and 3 passed, and 0 failed. The
full suite was not re-run: no other test reads what changed. The DB-backed
`chain_tests` of `src/db/referrals.rs` ran on the private PostgreSQL 18.0 of the section above
(`D:/t27work/pgdata-r4-2509`, 127.0.0.1:55436 only, a fresh database, proxies at
`http://127.0.0.1:9`): 3 passed, including the renamed
`a_new_welcome_credit_names_no_garden_and_is_withheld`. The stored row was read back. The server
was stopped with `pg_ctl stop -m fast`.

**Still open**, from the critic's pass. Each one needs a production read or an owner's call, and no
contract settles any of them:

* `GET /api/loyalty/leaderboard` has no login: `get_leaderboard` in `src/api/loyalty.rs` takes
  only the state, and `src/api/mod.rs` adds no auth layer. It returns the top 20's first names,
  total spend and raw tier. `legacy_retirement.t27` left the raw tier because the admin screen is
  its only reader. That reason does not cover a public route. This is a privacy question as well
  as a naming one. *Answered by the owner on 2026-09-26 (R1, «Только для админа»): admin only.
  See "Round 5, server lane" below.*
* `GET /api/quest-places`, `GET /api/treasure-hunts` and `GET /api/loyalty/config` serve stored
  rows or the stored config as they are. `GET /api/loyalty/tiers` hides only a tier named "woody".
  *Answered by the owner on 2026-09-26 (R2, «Закрыть для клиентов»): the three answer customers
  like a missing route. See "Round 5, server lane" below.*
* The bot's profile as Telegram stores it (commands, description, short description). The code
  sets only the menu button.
* The bucket's previous-shop objects, as above.

### Round 5, server lane: R1–R3, the owner's answers of 2026-09-26, not yet live

Branch `t27/round5-server`, cut from `main` at `e39221e` (#68). The shared core S0 is its first
commit, and the client lane `t27/round5-client` branches from that same commit. **Nothing is pushed
and nothing is deployed.** `dist/` was not rebuilt: the Mini App's half is the client lane's, and
the integration rebuilds once. The owner's words are quoted verbatim in DECISIONS.md («R1–R3, the
owner's answers of 2026-09-26»), with the 25 operator decisions and the admin-facing wording listed
for rewording; the `.t27` files stay ASCII.

| commit | item | what landed | recorded in |
| --- | --- | --- | --- |
| `b3f9653` | S0 | `src/trios/referral_credit.rs`: the 10% rule (`credit_for_rental`, floor, net of what was applied), the available and shown balance, the redeem's `applied_redemption`, `order_holds_a_rental`, the four not-creditable reasons in their order, and every wire type both lanes share. 8 unit tests | `referral_credit.t27` |
| `5279b37` | R1, R2 | `get_leaderboard` calls `check_admin` first (401, or 429). `get_quest_places`, `get_treasure_hunts` and `get_loyalty_config` call `admin_or_missing_route` first: without admin proof, the fallback's own 404 body from one builder, `missing_route`. OpenAPI documents both | `loyalty_ledger.t27` (`LEADERBOARD_GATE_SITES`, `CLOSED_READ_ROUTE_COUNT`), `legacy_retirement.t27` (`OWNER_ANSWER_R1_*`, `_R2_*`, `R2_*`) |
| `9c64767` | R3, what stopped | The points to the inviter, the milestone ladder and award, the welcome credit, their two queue producers and their renderers are deleted; `confirm_referral_edge_in` keeps the edge confirmation without money. `/milestones` serves empty offers and `/api/loyalty/:telegram_id` drops `referral_bonus`, so a cached bundle shows neither. Queued `friend_ordered` and `milestone` rows are held, not deleted | `referral_program.t27` (the history marked "at e39221e"), `notification_queue.t27` (`HELD_KINDS_SINCE_R3`) |
| `2cd985c` | R3, what started | Migration 089 (three tables, CREATE only), `src/db/referral_credit.rs` (`record_rental`, `reverse_rental`, `reverse_rental_for_order`, `resolve_request`, `credit_summary`, `overview`; raw SQL, fail-loud reads, lock order order row → person → referral rows), the six routes of `src/api/referral_credit.rs`, the reversal inside the bot's `RejectOrder` (same transaction, fail-closed), `/refstats`' «Реферальный баланс» line, the rule sentence in the bot's two hints, `SENSITIVE_FNS` +3 | `referral_credit.t27`, `order_status.t27` |
| `872698b` | contracts | `referral_credit.t27` (new, 128 declarations, 21 checks) and every re-measured count; gate 1's manifest; gate 3's 29 new rows and 3 removed; every citation the round moved, re-pinned with its old line and the date; the contract map's row | the contracts |
| the next commit | docs | DECISIONS.md's entry and this section | — |

`5279b37`, `9c64767` and `2cd985c` carry code whose contract counts and gate rows land in
`872698b`, so the four gates are green from `872698b` on, not on each commit before it. The
commits were cut from the finished tree, so each was exported (`git archive`) and checked alone:
`cargo check --features backend --all-targets` passes on `5279b37`, `9c64767` and `2cd985c` with
no warning from this crate. Their tests were run only on the finished tree.

**Gates**, on `872698b`, pinned compiler `40003ed`:

```sh
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler
# OK - 46 manifested specs, 5 generators each (every floor equals the measurement)
python3 scripts/execute_t27_assertions.py            # WITH the compiler cross-check
# OK - 46 spec(s), 11009 assert line(s) scanned, 11009 executed, 11009 passed, 0 failed;
#      10493 declaration name(s) and function bodies agreed with t27c; 2 pinned front-end disagreement(s)
python3 scripts/verify_t27_against_source.py --require-git-tracked
# OK - 282 bindings hold across 42 contracts and 87 source files
python3 scripts/verify_fleet_seed.py -v
# OK — 14 families (13 offered), 37 units (11 rented, 26 available); ...
```

Floors that moved (declarations/checks): `api_surface` 186/42, `catalog_write` 159/44,
`legacy_retirement` 360/73, `locale_policy` 149/36, `loyalty_ledger` 219/49, `notification_queue`
316/55, `observability` 165/40, `order_status` 170/32, `pricing_honesty` 122/40,
`referral_program` 289/62, `request_identity` 161/30, `runtime_config` 210/54,
`schema_provenance` 241/51, and the new `referral_credit` 128/21. Gate 3: 256 − 3 + 29 = 282
(`MIN_BINDINGS` 282). **Three rows were removed in the same change that moved `MIN_BINDINGS`**,
which the floor alone cannot catch, so they are named here: `referral_program.MILESTONE_RUNG_COUNT`
against the ladder and against its defaults in `src/db/referrals.rs` (both deleted), and
`legacy_retirement.WELCOME_CREDIT_SENTENCE` against its writer (deleted; the `legacy_view.rs`
withheld-sentence row stays). Each of the 29 new rows was planted red once (`--source-override`,
or a mirror tree for the `tree_*` rows) and restored.

**Rust.** `cargo fmt -- --check` is clean. The three clippy forms pass with `-D warnings`
(`--features backend --bin turbobaby-bot-server`, `--features backend`, and `--target
wasm32-unknown-unknown --lib`). `cargo test --features backend -j 2 --no-fail-fast` on `872698b`: 80
result lines, **2 570 passed, 0 failed**, 161 ignored.

**DB-backed.** A fresh private PostgreSQL 18.0 (`initdb -A trust -U postgres -E UTF8 --locale=C`),
data dir `D:/t27work/pgdata-r5a-2509`, listening on 127.0.0.1:55437 only, a fresh database per run,
`HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`/`TELOXIDE_PROXY` = `http://127.0.0.1:9`. The whole suite on
`872698b`, `cargo test --features backend -j 2 --no-fail-fast -- --include-ignored --test-threads=1`
skipping the two tests of `tests/https_reaches_telegram.rs`: 80 result lines, **2 729 passed, 0
failed**, 2 filtered out. The new DB tests: the 16 of `tests/integration_referral_credit.rs`, the 2
of `tests/integration_closed_reads.rs`, and in-crate
`the_bot_reject_path_reverses_inside_its_transaction` (`src/db/referral_credit.rs`) and
`a_confirmed_referral_credits_nobody` (`src/db/referrals.rs`, the renamed chain test). The server
was stopped with `pg_ctl stop -m fast`; the data dir is left in place.

**Citations.** Edits were kept line-preserving where a cited line sat below them (a removed line
became a comment line) and new declarations were appended at the ends of files, so most citations
did not move. 177 that did were re-pinned in place as `:new (:old until 2026-09-26)`; citations of
code that no longer exists name it "at e39221e". Not re-pinned, deliberately: DECISIONS.md's older
entries (D8 cites `OrderItem` in `src/db/orders.rs` at 502-525; it is at 432-455 now) and
migrations, which are frozen. Still stale, and stale before this round: `schema_provenance.t27`'s
citations into `scripts/verify_t27_specs.py` (the manifest grows with every floor comment).

**What is left.**

* **The integration** (`t27/round5`): merge this branch, then `t27/round5-client`. Both lanes edited
  `catalog_write.t27`, `locale_policy.t27`, `runtime_config.t27` and their floors in
  `scripts/verify_t27_specs.py`, so expect conflicts there and re-measure rather than pick a side.
  Then gate rows 30–31 (the i18n subtitle's percent, RU and EN), `MIN_BINDINGS` to the measurement
  (284 expected), the client paragraph of DECISIONS.md, the one `dist/` rebuild, the wasm greps and
  the smoke test. `tests/ui_endpoints_exist.rs` is red on the client branch alone, by design, until
  this branch is merged. *Done on 2026-09-26: see "Round 5, integrated" below.*
* **Nothing is live** until `main` is deployed; migration 089 runs on the next start.
* **The owner's open questions** (DECISIONS.md): the gross or net base of the 10%, the public
  `/api/referrals/leaderboard`, and the admin-facing wording.
* The two locale strings that announced the stopped credits (`referral_friend_ordered`,
  `referral_milestone_bonus` in `src/locales.rs`) are read by nobody and were left, like
  `REFERRAL_WELCOME_BONUS` and the `loyalty_config` keys `referral_bonus` and `milestone_bonus_N`.

### Round 5, integrated: R1–R3 of 2026-09-26, not yet live

Branch `t27/round5`, cut from `main` at `e39221e` (#68). It merges `t27/round5-server` (`a57c3ab`,
the section above) and then `t27/round5-client` (`62ead98`), both with `--no-ff`, pins the API's
shape on both sides, rebuilds `dist/` once and adds this section. **Nothing is pushed and nothing
is deployed.** No row is deleted or rewritten; migration 089 only creates, and it runs on the next
start of a deployed `main`. The owner's words are quoted verbatim in DECISIONS.md, in the entry
«R1–R3, the owner's answers of 2026-09-26» (server) and the one after it, «R3, the Mini App's half
and the integration» (client lane and integration).

| commit | what |
| --- | --- |
| `2ece6ce` | merge of `t27/round5-server`, no conflict |
| `bbdbaca` | merge of `t27/round5-client`; one file conflicted, resolved to both sides (below) |
| `992077f` | the API's one shape pinned on both sides, gate rows 30–31, `referral_credit.t27`'s integration block, DECISIONS.md's client and integration entry |
| `fa03a3b` | `dist/` rebuilt, bundle `7143e607a7409c70` → `4fa1b87a15ac065a` |
| the next commit | this section, the "Where it stands" rows, and the contract map's `referral_credit.t27` row (515 → 565 lines) |

**The client lane** (`593a499`, `62ead98`, both on the server lane's S0 `b3f9653`). The referrals
page shows «Реферальный баланс» through `shown_balance` and `format_baht`, with «Списать в счёт
аренды» and «Запросить выплату» posting the shared `OpenRequestBody` and re-reading the balance
after every request; the milestone ladder, the «Бонус» card, the friend-facing share text and the
empty friends panel are gone; the subtitle and the profile's referral card print the rule sentence.
The admin screen's Loyalty tab gets «🤝 Рефералы» and `ReferralCreditPanel`, appended at the end
of `src/ui/screens/admin_screen.rs`. Seven i18n keys retired and four declared, line for line: 526
→ 523 declarations, naive grep 531 → 528 (measured on the merged `src/trios/i18n.rs`).

**The conflict** (`bbdbaca`). Only the gate-1 manifest: both lanes raised two floors by one or more
declarations each, for different declarations.

* `catalog_write.t27`: the server's `NEXT_LARGEST_SOURCE_LINES_BEFORE_2026_09_26` (159/44) and the
  client's `ADMIN_SCREEN_LINES_BEFORE_2026_09_26` (159/44) merge to the measured **160/44**. Both
  readings hold together on the merged tree: `src/ui/screens/admin_screen.rs` is 6 627 lines (the
  largest file), `src/main.rs` 4 603 (the second).
* `locale_policy.t27`: the server's `MIGRATION_FILES_SEARCHED_BEFORE_089` (149/36) and the client's
  five R3 key-count declarations (155/36) merge to the measured **156/36**.

`catalog_write.t27`, `locale_policy.t27` and `runtime_config.t27` themselves merged without a
textual conflict, each side's block kept. All 46 floors equal the pinned compiler's counts.

**Citations across the lanes.** Every `path:N` citation in the merged tree, bare `:N` continuations
after a named path included, that points into a file either lane changed was checked against the
version its citing line was written against (the lane whose tip first carries the line, or both
lanes' tips when the line is older than both), line by line through a diff of that version against
the merged file. No citation either lane added points into a file the other lane changed and moved.
The only moved citations are `schema_provenance.t27`'s into `scripts/verify_t27_specs.py` (lines
599, 603, 665, 680, 689, 973), stale since before this round and now further out by this round's
floor comments; left as they are, as the sections above say.

**The API's one shape** (`992077f`). Both lanes compile against the wire types of
`src/trios/referral_credit.rs`, so a field cannot drift between them. What no compiler compares, and
what `tests/ui_endpoints_exist.rs` (every fetched path is served) does not see, is now pinned by
`tests/referral_credit_api_shape_wiring.rs`, 7 tests over one table of the six routes:

* the server registers exactly the six (path, verb, handler) in `routes()`;
* each handler answers from the ledger function whose return type is the table's shared answer
  type (`ReferralCredit`, `OpenRequestResponse`, `AdminOverview`, `RecordRentalResponse`,
  `ReverseResponse`, `ResolveResponse`);
* each hand-written body parser reads exactly the shared body's fields (`parse_record_body` against
  `RecordRentalBody`, and so on);
* each screen builds each route's URL, sends it through a helper of the route's verb
  (`fetch_text_authed_full` and the page's `fetch_credit`; `post_json_authed`; `fetch_text_admin`;
  the panel's `referral_admin_post` over `post_json_admin_full`), sends the shared body and parses
  the shared answer, and builds no other credit path;
* the request kinds the page sends are `REQUEST_KINDS`, and the resolve actions the panel sends are
  the ones `parse_resolve_body` admits.

Its parsers are pinned by their own assertions first. Negative control, each planted in the tree,
run against the built test binary and restored from a copy: a client path (`/reverse` →
`/reversal`), a parsed type (the page parsing `ReferralCredit` from the request's answer), the
overview posted instead of fetched, a server verb (`post(post_rental)` → `get`), a field name the
parser reads (`note` → `notes`), an answer type (`reverse_rental` returning `ResolveResponse`), a
resolve action (`declined` → `decline`) and a swapped body (`ReverseBody` sent to the request
route): **8 of 8 red**, each in the test that names it. The in-crate test
`the_bodies_the_screens_serialize_are_what_the_parsers_read` (`src/api/referral_credit.rs`) runs
each shared body, serialized as a screen serializes it, through its parser and gets it back
unchanged. `referral_credit.t27` records both witnesses, names the seven retired keys (the client
lane left their names to it) and the ninth customer sentence the client lane needed and left out;
its floor 128/21 → **137/22**, measured.

**Gates**, on `992077f` and again, with the same numbers, on the commit that adds this section
(`fa03a3b` between them changes `dist/` only, which no gate reads); pinned compiler `40003ed`:

```sh
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler
# OK - 46 manifested specs, 5 generators each (every floor equals the measurement)
python3 scripts/execute_t27_assertions.py            # WITH the compiler cross-check
# OK - 46 spec(s), 11033 assert line(s) scanned, 11033 executed, 11033 passed, 0 failed;
#      10510 declaration name(s) and function bodies agreed with t27c; 2 pinned front-end disagreement(s)
python3 scripts/verify_t27_against_source.py --require-git-tracked
# OK - 284 bindings hold across 42 contracts and 88 source files
python3 scripts/verify_fleet_seed.py -v
# OK — 14 families (13 offered), 37 units (11 rented, 26 available); ...
```

Gate 3: 282 + 2 = **284** (`MIN_BINDINGS` 284, the measured table size; no row removed). The two
rows bind `referral_credit.CREDIT_PERCENT` to the Mini App's rule sentence, `T_REFERRAL_SUBTITLE`
in `src/trios/i18n.rs`, RU and EN, one arm each. Each was planted red once (the arm's rate set to 15
through `--source-override`: `contract=10 source=15`) and restored. `src/trios/i18n.rs` is the 88th
source file.

**Rust.** `cargo fmt -- --check` clean. The three clippy forms pass with `-D warnings`
(`--features backend --bin turbobaby-bot-server`, `--features backend`, and `--target
wasm32-unknown-unknown --lib`). `cargo test --features backend -j 2 --no-fail-fast` on `992077f`: 82
result lines, **2 594 passed, 0 failed**, 161 ignored, doc tests included;
`tests/ui_endpoints_exist.rs` is green (3 passed), `tests/referral_credit_api_shape_wiring.rs` 7,
`tests/referral_credit_client_wiring.rs` 11, `tests/referral_credit_wiring.rs` 11.

**`dist/`**, `fa03a3b`. Built once with `scripts/build-frontend.sh` (trunk 0.21.14, wasm-bindgen
0.2.122, `CARGO_BUILD_JOBS=2`, the shared target) from `992077f`: bundle `7143e607a7409c70` →
`4fa1b87a15ac065a`, no `dist/version.txt.br`, `dist/assets/` still ignored. Read inside the new wasm
(`grep -c -a -F`, old → new): «Присоединяйся к TurboBaby и получай бонусы!» 1 → 0, «Рубежи друзей»
1 → 0, "Friend milestones" 1 → 0, «Получайте» 1 → 0; the old subtitles, the empty-panel sentences,
«Получайте {0} за друга», "Earn {0} per referral" and the `/milestones` path 1 → 0 (the English
subtitle 2 → 0); «Реферальный баланс» 0 → 1, "Referral balance" 0 → 1, the two buttons, the payout
sentence and the rule sentence 1 each in RU and EN, `/api/referral-credit/me/` 2 (the read and the
request), `/api/admin/referral-credit/overview` 1. `scripts/predeploy-smoke.sh --no-build`: **SMOKE
PASS** (the app mounted, no uncaught JS error), and `git status` was empty after it.

**DB-backed**, on `fa03a3b`. A fresh private PostgreSQL 18.0 (`initdb -A trust -U postgres -E UTF8
--locale=C`), a new data dir `D:/t27work/pgdata-r5-2509`, listening on 127.0.0.1:55438 only, one
fresh database, `HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`/`TELOXIDE_PROXY` = `http://127.0.0.1:9`. The
whole suite, `cargo test --features backend -j 2 --no-fail-fast -- --include-ignored
--test-threads=1` skipping the two tests of `tests/https_reaches_telegram.rs`: 82 result lines,
**2 753 passed, 0 failed, 0 ignored** (2 filtered out), no compiler warning. That is the 2 594 + 161
of the run without a database, less the two skipped; among them the 16 of
`tests/integration_referral_credit.rs`, the 2 of `tests/integration_closed_reads.rs`,
`the_bot_reject_path_reverses_inside_its_transaction` and `a_confirmed_referral_credits_nobody`. The
server was stopped with `pg_ctl stop -m fast`, and the data dir is left in place.

**What is left.**

* Push, review and merge of `t27/round5`, then a deploy of `main`. Nothing here is live; migration
  089 runs on the next start, and the committed `dist/` is what a deploy would serve.
* **The owner's open questions** (DECISIONS.md): the 10% on the net charge (as built) or the gross;
  the public `/api/referrals/leaderboard`, which the referrals page's top list still reads and whose
  rows print the frozen `total_bonus_earned` as «{1} заработано»; the admin-facing wording both
  lanes wrote.
* Customer copy needed and not given, left out (nine sentences, listed in DECISIONS.md and in
  `referral_credit.t27`).
* The locale strings, config keys and env variable of the stopped credits stay, read by nobody, as
  the section above lists.

### Round 5, review fix, 2026-09-26: the recorder guard and the password token, not yet live

One demand of the review of `t27/round5` at `1419dc1`, verified against the tree before it was
applied, on the same branch. **Nothing is pushed and nothing is deployed.** No migration, no row
deleted or rewritten, no client file touched, so `dist/` was not rebuilt (bundle
`4fa1b87a15ac065a` unchanged) and the smoke test of `fa03a3b` still covers what a deploy serves.

| commit | what |
| --- | --- |
| `b1d900e` | the guard in code, the contract's qualification, gate 1's floor, gate 3's two rows, two tests |
| the next commit | DECISIONS.md's entry, this section, the "Where it stands" rows, the contract map's row (565 → 616 lines) |

**The demand, verified.** `check_admin` (`src/api/auth.rs`) answers `Ok(0)` for a valid
`X-Admin-Token`, including when initData is missing, fails the strict check or names a non-admin;
`post_rental` passed that 0 on; `record_rental` refused only `admin_id != 0 && admin_id == inviter`.
`referral_credit.t27` said `RECORDER_MAY_BE_THE_INVITER = false` with no qualifier, and neither it
nor DECISIONS.md named the password path. The only test of the refusal (`refusals`) used initData.
Measured by planting the old guard back and running the new test on a fresh private database: the
password record of the admin's friend answered **200**, and the admin's ledger held one accrual of
100 THB on a record with `recorded_by = 0`.

**The fix.** `post_rental` hands the ledger `state.config.admin_ids`, and `record_rental` refuses
through `recorder_is_the_inviter` (`src/db/referral_credit.rs`): a named recorder when he is the
inviter, a password record when the inviter is on the admin list. Same 422 `recorder_is_inviter`,
nothing written. Still open and said so in the contract and in DECISIONS.md (owner's call): a person
who knows the password and is not on `ADMIN_IDS`, and `recorded_by = 0` naming nobody; also an admin
marking his own payout paid.

* `referral_credit.t27`: both identities and the open gap in prose, eight declarations,
  `recorder_refused`, the test `the_recorder_guard_reads_both_admin_identities` and the invariant
  `no_admin_credits_himself_under_either_identity`; floor 137/22 → **148/24**, measured.
* Gate 3, 284 → **286** rows (`MIN_BINDINGS` 286): `RECORDER_GUARD_ADMIN_LIST_READS` (the one
  `admin_ids.contains(&inviter)` in the ledger, with a witness pinning the guard to the record's
  creditable arm) and `RECORDER_GUARD_ADMIN_LIST_HANDED_TO_THE_RECORD` (the one call in
  `post_rental` that passes `&state.config.admin_ids`). Planted RED through `--source-override`, one
  copy each: the read replaced by `false` (`contract=1 source=0`), the old guard restored (the
  witness matched nothing), the list replaced by `&[]` (`contract=1 source=0`).
* Tests: in-crate `the_recorder_guard_reads_both_admin_identities` (no database) and
  `a_password_record_of_a_listed_admins_friend_is_refused` in `tests/integration_referral_credit.rs`:
  the token alone, initData signed with another bot's token beside the token, and a linked completed
  order each answer 422 and leave no record, no ledger row and the edge pending; a named admin
  records another inviter's friend under his own id; the password records an outsider's friend as
  0. Against the planted old guard it is **red** at its first assertion (`left: (200, Null)`), and
  `refusals` stays green, which is why the old suite never saw the gap.

**Gates** on `b1d900e`, pinned compiler `40003ed`:

```sh
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler
# OK - 46 manifested specs, 5 generators each (every floor equals the measurement)
python3 scripts/execute_t27_assertions.py            # WITH the compiler cross-check
# OK - 46 spec(s), 11048 assert line(s) scanned, 11048 executed, 11048 passed, 0 failed;
#      10522 declaration name(s) and function bodies agreed with t27c; 2 pinned front-end disagreement(s)
python3 scripts/verify_t27_against_source.py --require-git-tracked
# OK - 286 bindings hold across 42 contracts and 88 source files
python3 scripts/verify_fleet_seed.py -v
# OK — 14 families (13 offered), 37 units (11 rented, 26 available); ...
```

**Rust.** `cargo fmt -- --check` clean. Clippy with `-D warnings` passes on `--features backend
--bin turbobaby-bot-server` and `--features backend`; the wasm form was not re-run, because
`src/db/` and `src/api/` are not compiled for wasm32 and no client file changed. `cargo test
--features backend -j 2 --no-fail-fast`: 82 result lines, **2 596 passed, 0 failed**, 162 ignored
(2 594 + the new unit test in the lib and the bin; 161 + the new integration test).

**DB-backed.** A fresh private PostgreSQL 18.0 (`initdb -A trust -U postgres -E UTF8 --locale=C`),
a new data dir `D:/t27work/pgdata-r5f-2509`, listening on 127.0.0.1:55439 only, a fresh database,
`HTTPS_PROXY`/`HTTP_PROXY`/`ALL_PROXY`/`TELOXIDE_PROXY` = `http://127.0.0.1:9`. The whole suite,
`cargo test --features backend -j 2 --no-fail-fast -- --include-ignored --test-threads=1` skipping
the two tests of `tests/https_reaches_telegram.rs`: 82 result lines, **2 756 passed, 0 failed, 0
ignored** (2 filtered out), no compiler warning from this crate; the 17 of
`tests/integration_referral_credit.rs` among them. Read back afterwards: two edges from the admin id
42, both still pending, no record of either friend, no ledger row of 42, one record by 42 (the named
case). The negative control ran on a second fresh database of the same server; with the fixed file
copied back, `tests/integration_referral_credit.rs` ran green on a third (17 passed). The server was
stopped with `pg_ctl stop -m fast`, and the data dir is left in place.

```sh
# 1. structure: module, unique ID, declaration floors, typecheck, five generators.
#    Needs the pinned compiler; see below.
T27C=<path>/t27c python3 scripts/verify_t27_specs.py --require-compiler -v

# 2. behaviour: parses and EXECUTES every assertion in the corpus. Stdlib only, ~1 s.
python3 scripts/execute_t27_assertions.py -v            # add --no-crosscheck without a compiler
#    With a compiler it prints 2 pinned disagreements and exits 0 ("Gate 2's cross-check after #64").

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
* **Fixed on 2026-09-25, in the change after this measurement.** The two stale entries left
  `KNOWN_FRONTEND_DISAGREEMENTS`, with a dated note in their place, and the script header no
  longer says "four". The same command then printed the two real pins and
  `OK - 45 spec(s), 9996 assert line(s) scanned, 9996 executed, 9996 passed, 0 failed; 9667
  declaration name(s) and function bodies agreed with t27c; 2 pinned front-end disagreement(s)`,
  exit 0. Nothing in it reaches the Docker image. The t27c defect behind the `;` class is not
  fixed; no function body in the corpus carries a `;` comment any more, so nothing reaches it.

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
* **D. What does a customer read when a cancellation is refused?** *Decided 2026-09-25, by
  delegation.* The owner answered item 11 of that day's list with "I don't understand what this
  is, think it over" (translated), which hands the wording over and approves no text. Since then a
  409 (the order has already left pending) shows its own key, `T_ORDER_DETAIL_CANCEL_REFUSED`, in
  ru and en. The first wording said the order was already being handled; later the same day the
  operator reworded it under the same delegation to «Отменить этот заказ в приложении уже нельзя.
  Напишите менеджеру.» / "This order can no longer be cancelled in the app. Please message the
  manager." From 2026-09-22 until then it showed the generic `T_API_ERR_UNKNOWN`, whose "try again
  later" was false here. `client_errors.t27` records the decision (`CONFLICTED_CANCELLATION_DECISION`,
  `CONFLICTED_CANCELLATION_REWORDED_AT`). The first wording left two limits open; the first of them
  was CLOSED by the rewording on 2026-09-25. That wording described the order, not the attempt, so
  its first clause was false when the order left pending because the shop rejected it, or because
  an earlier attempt of the customer's own, whose outcome was unknown, did cancel it. The new
  sentence says only what the app can no longer do, which holds for every 409
  (`CONFLICTED_CANCELLATION_SENTENCE_IS_TRUE_OF_EVERY_CONFLICT`), and the host tests in
  `src/trios/api_errors.rs` now refuse any claim about what the shop is doing with the order. The
  second limit stays open and is the owner's to change with the words: the order card has no link
  to the manager; `T_BIKE_ASK_MANAGER` is still the one existing candidate. The rewording replaced
  the two `i18n.rs` rows on their own lines, so no citation moved.
  The key moved every `i18n.rs` line citation below it, and all of them were re-pinned on the
  same branch. The first pass missed one, in `legacy_retirement.t27`, because its path wrapped
  across two comment lines and a same-line search for `src/trios/i18n.rs:NNN` cannot see it; a
  follow-up commit re-pinned it and unwrapped the path. Search for every `i18n.rs` mention, not
  only the path with its line number, when a key moves the file.
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
  and what checkout says when the zone list is empty. *Answered on 2026-09-25 (answer 6, «нет»):*
  the name stays «Аэропорт», so nothing was changed. The rest of H was not asked and stays open.
* **I. Catalog copy (#59).** The detail screen still says "all bikes of this model are busy right
  now" from a seed snapshot when the seeded count is 0. The unmounted `bike_card.rs` still
  hard-codes a count. Admin Add inserts its row before the 2xx. The brain's `knowledge_base` still
  offers PCX 150 / ADV 150 for CLICK 125. *Decided on 2026-09-25:* for now, NMAX 155 alone is
  offered instead of CLICK 125 (DECISIONS.md, D12 amendment of that date; the seed,
  `availability.t27`, gate 3 and the seed gate carry it). The brain's text is the operator's to
  bring in line. *Still open after 2026-09-25:* the owner's answer 9 did not locate the "all bikes
  of this model are busy" line (`T_BIKE_UNITS_EMPTY`), so it was left untouched. Answer 8 kept the
  rental-term discount percentages on the bike card. *Decided later on 2026-09-25, on branch
  `t27/no-units-empty` (merged 2026-09-26 as `fbdd469`):* answer 5 of the owner's second list, «Наверное»
  ("probably"), removed that line and deleted its key (DECISIONS.md, the entry of that date;
  `availability.t27` `UNITS_EMPTY_LINE_*`). *Still open:* the Book control's reason under the same
  zero, `T_BIKE_BOOK_BLOCKED_NO_UNITS` ("Every bike of this model is taken"), which says nearly the
  same thing from the same seeded count and which the answer did not name. *Answered on
  2026-09-26:* «Разрешить бронь, наличие уточнит менеджер» ("Allow booking; the manager will confirm
  availability"). The Book control reads no unit count, both of its count reasons and their keys are
  gone, and the unmounted `bike_card.rs` no longer hard-codes a count (`availability.t27`; see
  "Round 4, integrated" above). Round 4 did not touch Admin Add or the brain's `knowledge_base`.
* **J. Vocabulary and ownership (#62).** Should the nmax-155 rate be owned by pricing-honesty (as
  landed) or by rental-terms? Is a named copy bound by gate 3 accepted in place of an import the
  compiler lacks? New copy is needed for `T_MENU_DESC` / `T_MENU_NO_RESULTS`. The strain-of-day
  carousel was retired on 2026-09-25 (owner: nothing cannabis-related anywhere; DECISIONS.md, the
  D19 addendum of that date).
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

**239 bindings over 226 distinct constants** on the integration branch `t27/owner-answers-2509`
(2026-09-25, the owner's answers of that day; not merged): 226 of 5 698 top-level `pub const`
declarations (4.0 %, the gate-3 header's count); the first command below prints 226 and the second
5 721 there. On `main`:
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
> Gate 2 with a compiler configured prints two pinned disagreements and exits 0 (see "Gate 2's
> cross-check after #64"); any other cross-check line is new.
>
> `t27/owner-answers-2509` (at `41315b2` on 2026-09-26) carries both of the owner's lists of
> 2026-09-25 and the operator's shop-label decision. It is verified end to end: four gates,
> clippy, `cargo test`, the DB-backed run, `dist/` and the smoke (see "The owner's second list of
> 2026-09-25 and the operator's shop label"). *It was merged to `main` as #67 (`405f30e`).*
> `t27/round4` (2026-09-26) carries the owner's answers of 2026-09-26 and the critic's six notes
> on top of `405f30e`, verified end to end the same way (see "Round 4, integrated"). It is not
> pushed. Push it for review before anything else builds on it.
>
> The work, in order: fix the seven defects listed under "What is actually left", each with a test
> that is red before the fix and with the contract that records the defect corrected in the same
> change; then grow `scripts/verify_t27_against_source.py` beyond its 248 bindings on `main` at
> `405f30e` (256 on the unmerged integration branch `t27/round4` on 2026-09-26; before #67, 237 on
> `main`, 248 on `t27/owner-answers-2509` at `41315b2` on 2026-09-26, 239 on
> 2026-09-25; 237 re-measured
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
