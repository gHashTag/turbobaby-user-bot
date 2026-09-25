# Rolling back a TurboBaby deployment

Written 2026-09-24. Migration 087, the deploy window and the Railway CLI's own help were added
2026-09-25, for the deploy of `f5e6b4f` over the live `4a5aa72`. Each rule below cites the file it
comes from. Where this repository records nothing, this runbook says **UNKNOWN** instead of
guessing (§7).

## 0. What a deployment is

* **The service:** `turbobaby-bot` in the Railway project `woody`, environment `production`
  (AGENTS.md §10: `railway link -p woody -e production -s turbobaby-bot`). Its public origin is
  `https://turbobaby-bot-production.up.railway.app`, the canonical `WEB_APP_URL` in
  `src/config.rs` and the default of `e2e/playwright.config.ts`.
* **One deployment is one image built from one uploaded source tree** (`Dockerfile`):
  * the server binary compiled from `src/`, with every migration compiled into it
    (`src/db/mod.rs`, `MIGRATIONS`, one `include_str!` per file);
  * the **committed** `dist/`, copied as is (`COPY dist ./dist`). The image does not run
    Trunk.

  So the server and the Mini App client always travel together. A rollback replaces both.

## 1. Read this first: the Railway project is shared

* **The project also runs `woody-weed-bot`, another shop's live service.** The owner's mandate of
  2026-09-13 is not to run variables, deploy, down or logs against it
  (`loop/LOOP_STATE.md`, invariant 1; AGENTS.md, the after-deploy checklist). The databases pair
  as `woody` ↔ `turbobaby-bot` and `woody_client` ↔ `woody-weed-bot`, and must not be mixed
  (`loop/LOOP_STATE.md`).
* **`--service` does not steer a write.** `railway variables --set … --service <name>` writes to
  whichever service is *linked*. Measured 2026-09-13: this wrote to the wrong service and poisoned
  the other shop's database (AGENTS.md §12). Every Railway write starts with
  `railway link -p woody -e production -s turbobaby-bot` and ends with a check.
* **Changing a variable redeploys the last uploaded source** (AGENTS.md §12; `loop/LOOP_STATE.md`,
  invariant 5). During a rollback, change no variable until the source you want is live.
* **`railway up` from a git worktree silently deploys an earlier source** (AGENTS.md §13). Upload
  only from a clean export of the exact commit, made fresh for this upload (§3B has the commands):
  `git clone -c core.autocrlf=false` checked out at `<sha>`, or
  `git -c core.autocrlf=false archive <sha> | tar -x -C <dir>`. The flag matters on Windows: with
  `core.autocrlf=true` (Git for Windows sets it system-wide) and no `.gitattributes`, the export
  turns every text file to CRLF — `dist/index.html` grows by one byte per line — and the served
  page no longer equals the commit you meant to ship.
* **A clean export holds tracked files only.** So `dist/assets/` is absent: it is in `.gitignore`,
  Trunk fills it on every local build, and `.github/workflows/deploy.yml` deletes it before its own
  upload. Never upload the working tree of a clone someone works in: whatever is uncommitted there,
  `src/` included, is compiled into the image.
* **`railway up` makes a new project when it does not find yours.** The help text of Railway CLI
  5.62.1 (`railway up --help`, read 2026-09-25) says so in three places:
  * not signed in, `up` signs you in (or creates an account) and then chains into creating a
    project and a service and deploying into them;
  * `--new` creates a new project and service "even if one is already linked", and is implied on a
    cold or unauthenticated first run, and for `-y` when nothing is linked. So in a directory
    without a link, `up -y` makes a new project;
  * `-y` accepts all defaults: it skips the sign-in confirmation and the name prompt of the new
    project, the two prompts that would have stopped exactly this.

  So never pass `-y` and never pass `--new`. Sign in with `railway login` as a step of its own. If
  `up` ever offers to create a project, the directory is not linked: cancel and link.
* **The link belongs to a directory.** `railway link` links "the current directory" (its help
  text), so which project and service `up` targets depends on the directory it runs in. A link
  checked in your repository checkout proves nothing about the export directory. So, **in the
  export directory, immediately before the upload**:
  `railway link -p woody -e production -s turbobaby-bot`, then `railway status`, which must name
  project `woody`, environment `production` and service `turbobaby-bot`. Anything else: stop.
* **Name the target on `up` too:** `railway up --service turbobaby-bot --environment production`.
  That is a second guard, not the first. `--service` did not steer `railway variables --set`
  (above), and whether it steers `up` in a directory linked to another service is not measured
  (§7). The guard that is measured is link plus status, in the same directory.

## 2. How production gets code today

* **A merge to `main` deploys nothing.** `.github/workflows/deploy.yml` skips itself while the
  secret `RAILWAY_TOKEN` and the variable `RAILWAY_SERVICE` are absent (measured 2026-09-13,
  AGENTS.md §10). Production changes only through a manual `railway up`.
* **So "the previous deployment" is not "the previous merge".** Before rolling back, find out what
  is live:
  * `railway deployment list` gives the deployment history and status (AGENTS.md, after-deploy
    checklist);
  * `scripts/postdeploy-smoke.sh` names the commit whose `dist/index.html` is being served.
* **Measured 2026-09-24 01:03 +07 with that script:** production served `dist/index.html` as
  committed in `4a5aa72` (2026-09-17, bundle `c3a91ae8dc97ab9e`). `main` at `f0640f8` carries a
  rebuilt `dist/` (bundle `37c3bcae3fd19d27`). `main` was ahead of production.
* **Measured again 2026-09-25, about 09:40 +07:** production still served bundle
  `c3a91ae8dc97ab9e` (`4a5aa72`), with the five zones from before 087, and `/health` was ok.
  `main` is `f5e6b4f` (bundle `17f356526a369bb9`) and carries migrations 087 and 088, so the next
  deploy crosses 087 (§4).

### What happens while an upload goes live

* **The migrations commit before the new container answers anything.** The `main` function in
  `src/main.rs` calls `db.run_migrations().await?` before `tokio::net::TcpListener::bind`.
  `run_migrations` (`src/db/mod.rs`) logs `run_migrations: applied <file>` for each file it
  applies.
* **Meanwhile the old container keeps serving, on the database the new one has just migrated.**
  Railway moves traffic to the new deployment once it passes the health check (`railway.toml`,
  `healthcheckPath = "/health"`), not when its migrations commit.
* **Across 087 that window answers 500 on zones.** From the log line
  `run_migrations: applied 087_delivery_zones_phuket.sql` until traffic moves, the old `4a5aa72`
  answers the zone paths of §4 with 500. Its logs show `list_delivery_zones:`,
  `create_order: delivery zone lookup failed` and `get_order_status: delivery zone lookup failed`,
  and admins may receive the `5xx Error on prod` alert (`alert_5xx_text` in `src/main.rs`). Expect
  seconds of this: an inference from the boot sequence, not a measurement.
* **If the new container fails its health check after 087, the old one keeps serving, broken, with
  no end.** Railway leaves the previous deployment serving when a new one fails its health check
  (`railway.toml` sets that check: `/health`, 300 s, restart `ON_FAILURE` up to 10 times). The
  first action is to run the same upload again, from the same clean export (§3B), changing
  nothing. 087 is recorded in `_schema_migrations` and is not applied again. 088 runs only if it is
  not yet recorded, and a second run changes nothing
  (`UPDATE events SET is_public = FALSE WHERE is_public = TRUE`). If that does not go live either,
  the owner's repair in §4 gives the old container its zones back without changing code.

## 3. Two ways back

### A. Redeploy the previous deployment

Redeploy the last deployment of `turbobaby-bot` that was good. Pick it from the service's
deployment list (`railway deployment list`, or the Railway dashboard). This repository records no
CLI command for redeploying a chosen older deployment, so this step is done from the dashboard.
Write down the deployment id you chose.

What this keeps: the source that deployment was built from, meaning its server and its `dist/`
together (§0). What it may not keep: see §7.

**After 087 no earlier deployment can serve the zones on its own.** Do A only after the owner's
repair, or roll forward instead (§4, "Rolling back after 087").

### B. Upload a commit from a clean export (the manual `railway up` reality)

Use this when the good deployment is no longer listed, or when you only know its commit. A forward
deploy is made the same way (AGENTS.md §10).

```sh
DEP=<new empty directory, outside every repository and worktree>
git clone -c core.autocrlf=false --no-checkout https://github.com/gHashTag/turbobaby-user-bot.git "$DEP"
git -C "$DEP" checkout --detach <sha>
git -C "$DEP" rev-parse HEAD                      # the full <sha>
git -C "$DEP" status --porcelain                  # nothing
git -C "$DEP" ls-files --eol | grep -c 'w/crlf'   # 0
cat "$DEP/dist/version.txt"                       # the bundle hash committed at <sha>
bash "$DEP/scripts/predeploy-smoke.sh" --no-build # SMOKE PASS
git -C "$DEP" status --porcelain                  # still nothing
test ! -e "$DEP/dist/assets" && echo absent       # dist/assets is absent (§1)
cd "$DEP"
railway link -p woody -e production -s turbobaby-bot
railway status                                    # woody / production / turbobaby-bot, else stop
railway up --service turbobaby-bot --environment production   # never -y, never --new (§1)
```

* `git clone -c` writes `core.autocrlf=false` into the new clone's own config, so the checkout
  after it keeps LF. A `git -c core.autocrlf=false clone` would not: that flag lasts one command.
* The clone form keeps a `.git/`, which `.railwayignore` leaves out of the upload and which the
  post-deploy smoke needs (§6). The `archive` form of §1 makes an export with no history.
* **`scripts/predeploy-smoke.sh` only with `--no-build`.** Without it, the script rebuilds `dist/`
  with a bare `trunk build --release`. That bundle has none of `scripts/build-frontend.sh`'s
  post-processing: the `?v=` on snippet imports, the `await window.__telegramReady` wait and the
  `__loadWasmWithRetry` wrapper. The smoke would then pass a bundle that is not the committed one,
  and it would be uploaded. The second `status --porcelain` shows the smoke changed no tracked
  file. `dist/assets/` is ignored by git, so `status` cannot show it; the `test` after it can.
* A smoke that cannot run is a FAIL, not a pass: `scripts/cdp_smoke.py` stops with `SMOKE FAIL`
  when the Python module `websocket-client` is missing.

The link is AGENTS.md §10's. The clean directory is AGENTS.md §13's. This builds a new image from
that commit's `Dockerfile`, `src/`, `migrations/` and committed `dist/`. Wait for `SUCCESS` in
`railway deployment list` before doing anything else.

## 4. The schema does not roll back

* **Migrations run at boot, before the server listens.** In `src/main.rs`,
  `db.run_migrations().await?` comes before `tokio::net::TcpListener::bind`. A migration that fails
  ends the process, so `/health` never answers. Railway's health check then fails the deployment
  (`railway.toml`: path `/health`, timeout 300 s, restart `ON_FAILURE` up to 10 times).
* **Migrations are forward-only** (DECISIONS.md D2). `migrations/` holds no down migration.
  `run_migrations` (`src/db/mod.rs`) applies each file once and records its name in
  `_schema_migrations`.
* **Rolling the code back leaves the schema where the newest deployment put it.** An older binary
  walks its own, shorter `MIGRATIONS` list and finds every name already recorded, so it starts on
  the newer schema. Nothing in `run_migrations` refuses a database that holds migrations the binary
  does not know. A rollback is therefore safe only when the older code works on the newer schema.
* **Two migrations here cannot be undone by any rollback.**
  * `migrations/083_drop_cannabis_catalog.sql` drops `strains`, `strain_reviews`,
    `lab_certificates` and the three `garden_*` tables with `CASCADE`. A build older than 083 reads
    those tables. Rolling back across 083 does not bring them or their rows back, and no step in
    this runbook can (see backups in §7).
  * `migrations/087_delivery_zones_phuket.sql` drops `NOT NULL` and the defaults of
    `delivery_zones.eta_min` and `eta_max`, and sets both to NULL on every row, active or not. The
    18 Phuket rows it inserts get NULL too. The minutes the rows held are kept nowhere: the
    migration files 060 and 071 hold only the seeded values, not an admin's later edits. What that
    does to a rollback is the next subsection.
* **Rolling forward again is safe for the tracker.** The newer binary finds its migrations
  recorded and applies only the missing ones.
* **Never empty `_schema_migrations` to "reset" after a rollback.** On an established database with
  an empty tracker, the runner marks only migrations up to `076_promo_broadcast.sql` as applied
  (`LAST_PRE_BIKE_MIGRATION` in `src/db/mod.rs`). It then runs 077 onward again over live data.

### Rolling back after 087

* **An older build cannot read a zone row after 087.** The zone entity's `Model`
  (`src/db/entities/delivery_zone.rs`) declares `eta_min: i32` and `eta_max: i32` in `4a5aa72` and
  in every build before `a18cdd9`, the commit that made both `Option<i32>`. sea-orm 1.1.20 (the
  version in `Cargo.lock`) turns a NULL in a field that is not an `Option` into the error "A null
  value was encountered while decoding" (`TryGetable` for primitive types, sea-orm's
  `executor/query.rs`). On such a build, after 087:
  * `GET /api/delivery/zones` answers 500 to everyone (`list_delivery_zones`,
    `src/api/orders.rs`);
  * `create_order` with the id of an active zone answers 500. With the id of an inactive zone,
    such as the Koh Phangan rows after 087, it answers 422;
  * the status of an order whose zone is active answers 500 (`get_order_status`), the pickup row
    included;
  * the admin zone list and every zone edit answer 500 (`list_delivery_zones_admin` and
    `update_delivery_zone`, `src/api/admin.rs`). The admin screen cannot repair this; only SQL can;
  * its Mini App reads no zone list, so the checkout hides the zone picker. A customer whose Mini
    App saved an active zone (`woody_last_zone_id`) gets a 500 on every order.

  The build still looks healthy. `/health` runs `SELECT 1` (`health_handler`, `src/api/mod.rs`),
  and the boot's `SCHEMA SELF-CHECK` looks only for missing columns. The post-deploy smoke's
  check 3 is the one that fails (§6).
* **No deployment made so far can go back on its own.** Every deployment this repository knows of
  is `4a5aa72` or older (§2, §7). No build from `a18cdd9` on has booted against production: the
  first one to boot applies 087, and on 2026-09-25 `4a5aa72` still served its zone list, which it
  cannot do once 087 has run.
* **No commit between `4a5aa72` and `f5e6b4f` is a way back either.**
  * `a18cdd9` is half a release, and so are the two docs-only commits after it (`e412ecc`,
    `225af5f`). Its server sends the ETA as `null`, but its committed `dist/` is still bundle
    `ea3aedd8e89a916e` (`dist/version.txt`), built at `ff1741f`. That client's `DeliveryZone`
    (`src/ui/api/types.rs`) reads `min_eta_minutes` and `max_eta_minutes` as `u32` and cannot read
    a `null`. §5 forbids exactly this pairing.
  * `ce1709b` is the first commit whose server and committed `dist/` agree (bundle
    `a41a77c967b58518`, `Option` on both sides). Neither it nor any commit after it, `f5e6b4f`
    included, has ever been deployed, so none of them is a known-good state to go back to.
* **So after 087 there are two ways back, and both are the owner's decision.**
  1. **Forward only (preferred: it changes no data).** Upload `f5e6b4f` again, or a commit that
     fixes the fault, from a fresh clean export (§3B).
  2. **`4a5aa72` plus the owner's repair.** Run the repair below first. Then do §3A (redeploy the
     `4a5aa72` deployment) or §3B with `<sha>` = `4a5aa72`, and §6 with `SMOKE_DIST_REF=4a5aa72`.
     A customer whose Mini App saved a Koh Phangan zone id still gets 422 from `create_order` until
     they pick a zone again.

**Capture before a deploy that crosses 087.** The repair needs these, and nothing else keeps them:

* the owner saves the output of these reads outside the export (the first must say exactly
  `woody`, not `woody_client`):

  ```sql
  SELECT current_database();
  SELECT id, name, name_en, fee, min_order, eta_min, eta_max, is_active, sort_order, updated_at
    FROM delivery_zones ORDER BY sort_order, name;
  ```

* `SMOKE_OUT=<dir outside the export>/pre SMOKE_DIST_REF=4a5aa72 scripts/postdeploy-smoke.sh`
  saves the served zone list, active rows only, as `zones.json`, and its check 5 confirms that
  production still serves `4a5aa72` (§6);
* any `pg_dump` finishes before the upload starts. 087's `ALTER TABLE` waits for it, the runner
  sets no `lock_timeout` (`src/db/mod.rs`), and both the new container's boot and the old
  container's zone reads queue behind that `ALTER`.

**The owner's emergency repair.** SQL, not a migration: nothing goes into `migrations/` or
`_schema_migrations`. The owner runs it against `woody` and nowhere else (§1). It makes every row
readable by an `i32` build again. It must cover every row, active or not, because the admin list
reads them all:

```sql
-- One statement per row. <id>: the row's id. <min>, <max>: whole minutes, <max> >= <min>, never 0.
UPDATE delivery_zones
   SET eta_min = COALESCE(eta_min, <min>),
       eta_max = COALESCE(eta_max, <max>)
 WHERE id = '<id>';

-- Done when this says 0:
SELECT count(*) FROM delivery_zones WHERE eta_min IS NULL OR eta_max IS NULL;
```

* Rows that existed before the deploy take `<min>` and `<max>` from the capture above.
* The 18 zones 087 inserted get minutes the owner chooses. Never 0: the older build would promise
  every customer a delivery in zero minutes.
* Know what it does. The older build publishes these minutes to customers, and the owner decided on
  2026-09-24 that no delivery time is published (087's header, point 2). It is an emergency measure
  the owner chooses, not a default.

**Undo it before rolling forward again.** 087 and 088 are recorded and never run again. The newer
build would serve whatever the repair wrote: `served_eta` (`src/delivery.rs`) publishes any stored
pair as real minutes. It would also serve whatever `4a5aa72` published meanwhile. `4a5aa72` stores
30 and 60 minutes on a zone an admin creates without them (`create_delivery_zone`), and makes public
an event created without `is_public` (`validate_event_request`, `src/api/events.rs`). So the owner
runs this just before the upload:

```sql
UPDATE delivery_zones SET eta_min = NULL, eta_max = NULL
 WHERE eta_min IS NOT NULL OR eta_max IS NOT NULL;
SELECT id, title FROM events WHERE is_public;     -- anything listed was published meanwhile
UPDATE events SET is_public = FALSE WHERE is_public = TRUE;
SELECT id, name, name_en, fee, is_active, sort_order FROM delivery_zones ORDER BY sort_order, name;
```

The two `UPDATE`s repeat what 087 and 088 did. The last `SELECT` shows the zones an admin created
or edited in the meantime, for the owner to review.

**088 needs no undo for a rollback.** `4a5aa72` runs with every event hidden, which is the state
085 already left (`migrations/085_unpublish_woody_catalog.sql`). 088 only hides again whatever was
published after 085.

## 5. Hazards that ride on an older build

* **The Telegram menu button.** Every boot points the bot's menu button at the deployment's web app
  URL (`src/main.rs`, the block that calls `set_chat_menu_button`). Builds older than `99e9b06`
  (2026-09-14) fall back to the *other shop's* production URL when `WEB_APP_URL` is unset. Those
  builds also default `BOT_USERNAME` to the other shop's bot (see the comment in `src/config.rs`
  above `canonical_web_app_url`, and DECISIONS.md D19). Rolling back past `99e9b06` with those
  variables unset sends TurboBaby's customers to the other shop's Mini App. Whether they are set on
  the service is UNKNOWN (§7); the last recorded state is "unset before 2026-09-14" (DECISIONS.md
  D19; the comment above `canonical_web_app_url` in `src/config.rs`). After linking, a read-only
  check shows it:
  `railway variables --kv | grep -E '^(WEB_APP_URL|BOT_USERNAME)='`.
* **Never roll back half a release.** The two paths in §3 keep a server and its `dist/` together.
  What must not happen is a partial rollback, for example an older `dist/` uploaded under a newer
  server. `specs/turbobaby/order_presentation.t27` records why. A server that withholds an unusable
  money figure as `null` needs a client that reads an absence; the earlier `dist/` lost whole
  screens on such a figure (`SHIPPING_THIS_SERVER_WITH_THE_COMMITTED_DIST_LOST_SCREENS_BEFORE_THE_REBUILD`).
  A commit can be half a release on its own: `a18cdd9` is one (§4).

## 6. After a rollback

1. `railway deployment list`: the deployment you chose is `SUCCESS` (AGENTS.md, after-deploy
   checklist).
2. Run the smoke. It sends GET requests only:

   ```sh
   SMOKE_DIST_REF=<good-sha> SMOKE_OUT=<new directory outside the export> scripts/postdeploy-smoke.sh
   ```

   It must end with `0 failed`. It checks five things: `/health` answers with `db` ok,
   `/api/bikes` and `/api/delivery/zones` answer with JSON lists, an unmatched `/api` path answers
   with a JSON 404, and `/` is byte-equal to `dist/index.html` of the commit you rolled back to.
   If `/` differs, the script names the commit whose `dist/index.html` is actually served. That is
   AGENTS.md §13's rule: check that what arrived is what you uploaded, not only `SUCCESS`.
   * Run it from a clone that has `<good-sha>`. Check 5 reads `dist/index.html` from the git
     history of the tree the script sits in, and an `archive` export has none.
   * `SMOKE_OUT` goes outside the export, so the export stays clean. Give each run a new
     directory: the script overwrites its files.
   * After a rollback across 087, check 3 (`/api/delivery/zones`) is the one that shows whether
     the owner's repair (§4) worked. `/health` is green either way.
3. `railway logs -s turbobaby-bot`: zero `InvalidToken`, and no `SCHEMA SELF-CHECK` line (AGENTS.md,
   after-deploy checklist). The server writes `SCHEMA SELF-CHECK` at boot when the live schema lacks
   a column the code reads (`src/main.rs`, right after `run_migrations`).
4. Tell users to close the Mini App fully and open it again. A normal reload can keep the old
   bundle (AGENTS.md §7).
5. Run nothing against `woody-weed-bot` at any step (§1).

## 7. UNKNOWN: not recorded anywhere in this repository

* **The database backup policy.** Nothing here records a backup, snapshot or point-in-time-restore
  procedure for the `woody` Postgres database. Searched 2026-09-24: the only matches for "backup"
  concern assets (`BACKUP_ASSETS` in `src/config.rs`, `scripts/export-assets.sh`). Until the owner
  records a policy, a destructive migration (§4) is permanent.
* Whether `woody` and `woody_client` live in one Postgres instance. If they do, restoring that
  instance's volume would roll back the other shop's database too; a logical dump of `woody` alone
  would not.
* Whether a Railway redeploy of an older deployment runs with that deployment's variables or with
  the current ones.
* How long Railway keeps an older deployment available to redeploy.
* How long the old and the new container overlap during a deploy (§2).
* Whether `railway up --service` steers an upload when the directory is linked to another service
  (§1). It has been measured only for `railway variables --set`.
* Which deployment id and source commit are live at a given moment. Nothing in the repository
  records the current one. `loop/LOOP_STATE.md` records two HISTORICAL deployments as `SUCCESS`,
  `003e355f` and `7c990c78` (source `ce1bd26`, 2026-09-14 14:55 +07). Both predate `99e9b06`, so
  they carry the menu-button hazard of §5: do not pick them as a rollback target without checking
  `WEB_APP_URL` and `BOT_USERNAME` first. Both also predate `a18cdd9`, so after 087 they cannot
  read a zone either (§4). `railway deployment list` and the smoke's served-bundle lookup are the
  only current readings.
* Whether `WEB_APP_URL` and `BOT_USERNAME` are set on the service (§5).
  `specs/turbobaby/runtime_config.t27` (`DEPLOYMENT_ENV_NOTE`) reaches the same conclusion: which
  variables the running deployment holds cannot be established from this tree.
