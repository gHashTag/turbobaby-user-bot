# Rolling back a TurboBaby deployment

Written 2026-09-24. Each rule below cites the file it comes from. Where this repository records
nothing, this runbook says **UNKNOWN** instead of guessing (§7).

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
  only from a clean tree: `git archive <sha> | tar -x -C <dir>`, or a normal clone.

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

## 3. Two ways back

### A. Redeploy the previous deployment

Redeploy the last deployment of `turbobaby-bot` that was good. Pick it from the service's
deployment list (`railway deployment list`, or the Railway dashboard). This repository records no
CLI command for redeploying a chosen older deployment, so this step is done from the dashboard.
Write down the deployment id you chose.

What this keeps: the source that deployment was built from, meaning its server and its `dist/`
together (§0). What it may not keep: see §7.

### B. Upload the previous commit (the manual `railway up` reality)

Use this when the good deployment is no longer listed, or when you only know its commit:

```sh
git archive <good-sha> | tar -x -C <empty directory>
cd <that directory>
railway link -p woody -e production -s turbobaby-bot
railway up -y -d
```

The command is AGENTS.md §10's. The clean directory is AGENTS.md §13's. This builds a new image
from that commit's `Dockerfile`, `src/`, `migrations/` and committed `dist/`. Wait for `SUCCESS`
before doing anything else.

## 4. The schema does not roll back

* **Migrations run at boot, before the server listens.** In `src/main.rs`,
  `db.run_migrations().await?` (line 451) comes before `TcpListener::bind` (line 1210). A migration
  that fails ends the process, so `/health` never answers. Railway's health check then fails the
  deployment (`railway.toml`: path `/health`, timeout 300 s, restart `ON_FAILURE` up to 10 times).
* **Migrations are forward-only** (DECISIONS.md D2). `migrations/` holds no down migration.
  `run_migrations` (`src/db/mod.rs:600`) applies each file once and records its name in
  `_schema_migrations`.
* **Rolling the code back leaves the schema where the newest deployment put it.** An older binary
  walks its own, shorter `MIGRATIONS` list and finds every name already recorded, so it starts on
  the newer schema. Nothing in `run_migrations` refuses a database that holds migrations the binary
  does not know. A rollback is therefore safe only when the older code works on the newer schema.
* **One migration here cannot be undone by any rollback.**
  `migrations/083_drop_cannabis_catalog.sql` drops `strains`, `strain_reviews`, `lab_certificates`
  and the three `garden_*` tables with `CASCADE`. A build older than 083 reads those tables.
  Rolling back across 083 does not bring them or their rows back, and no step in this runbook can
  (see backups in §7).
* **Rolling forward again is safe for the tracker.** The newer binary finds its migrations
  recorded and applies only the missing ones.
* **Never empty `_schema_migrations` to "reset" after a rollback.** On an established database with
  an empty tracker, the runner marks only migrations up to `076_promo_broadcast.sql` as applied
  (`LAST_PRE_BIKE_MIGRATION`, `src/db/mod.rs:395`). It then runs 077 onward again over live data.

## 5. Hazards that ride on an older build

* **The Telegram menu button.** Every boot points the bot's menu button at the deployment's web app
  URL (`src/main.rs`, the block that calls `set_chat_menu_button`). Builds older than `99e9b06`
  (2026-09-14) fall back to the *other shop's* production URL when `WEB_APP_URL` is unset. Those
  builds also default `BOT_USERNAME` to the other shop's bot (see the comment in `src/config.rs`
  above `canonical_web_app_url`, and DECISIONS.md D19). Rolling back past `99e9b06` with those
  variables unset sends TurboBaby's customers to the other shop's Mini App. Whether they are set on
  the service is UNKNOWN (§7). After linking, a read-only check shows it:
  `railway variables --kv | grep -E '^(WEB_APP_URL|BOT_USERNAME)='`.
* **Never roll back half a release.** The two paths in §3 keep a server and its `dist/` together.
  What must not happen is a partial rollback, for example an older `dist/` uploaded under a newer
  server. `specs/turbobaby/order_presentation.t27` records why. A server that withholds an unusable
  money figure as `null` needs a client that reads an absence; the earlier `dist/` lost whole
  screens on such a figure (`SHIPPING_THIS_SERVER_WITH_THE_COMMITTED_DIST_LOST_SCREENS_BEFORE_THE_REBUILD`).

## 6. After a rollback

1. `railway deployment list`: the deployment you chose is `SUCCESS` (AGENTS.md, after-deploy
   checklist).
2. Run the smoke. It sends GET requests only:

   ```sh
   SMOKE_DIST_REF=<good-sha> scripts/postdeploy-smoke.sh
   ```

   It must end with `0 failed`. It checks five things: `/health` answers with `db` ok,
   `/api/bikes` and `/api/delivery/zones` answer with JSON lists, an unmatched `/api` path answers
   with a JSON 404, and `/` is byte-equal to `dist/index.html` of the commit you rolled back to.
   If `/` differs, the script names the commit whose `dist/index.html` is actually served. That is
   AGENTS.md §13's rule: check that what arrived is what you uploaded, not only `SUCCESS`.
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
* Whether a Railway redeploy of an older deployment runs with that deployment's variables or with
  the current ones.
* How long Railway keeps an older deployment available to redeploy.
* Which deployment id and source commit are live at a given moment. Nothing in the repository
  records it. `railway deployment list` and the smoke's served-bundle lookup are the only readings.
* Whether `WEB_APP_URL` and `BOT_USERNAME` are set on the service (§5).
  `specs/turbobaby/runtime_config.t27` (`DEPLOYMENT_ENV_NOTE`) reaches the same conclusion: which
  variables the running deployment holds cannot be established from this tree.
