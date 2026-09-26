# E2E Tests — Admin Panel (Playwright)

> **Skipped since 2026-09-25, and not run by CI.** `admin.spec.ts` and `upload.spec.ts`
> each call `test.skip(true, STALE_REASON)`: the admin gate is password-only
> (`X-Admin-Token`), so the initData these tests inject reaches the login screen. Their
> tab names were brought in line with `src/ui/screens/admin_screen.rs` on that date.

## Prerequisites

- Node.js ≥ 18
- The HMAC-signed Telegram initData stored at `/tmp/secrets/init_data` (plain text, one line)

## Setup

```bash
cd e2e
npm install
npx playwright install chromium
```

## Run

```bash
npm test
```

Or equivalently:

```bash
npx playwright test
```

## Headed mode (see the browser)

```bash
npm run test:headed
```

## Debug mode (step through tests)

```bash
npm run test:debug
```

## Output

- Screenshots land in `e2e/screenshots/<tab>.png`
- HTML report opens automatically after a run (`playwright-report/index.html`)

## Tabs under test

`Дашборд`, `Заказы`, `Байки`, `Юниты`, `Сервис`, `Сокровища`, `Лояльность`,
`Менеджеры`, `События`, `Рассылка` — the ten tabs `AdminPanel` renders (each label
also carries an emoji). Until 2026-09-25 this list named the old shop's tabs.

Each tab gets its own `test('Tab: <Name>', ...)` which:
1. Opens `https://turbobaby-bot-production.up.railway.app/admin`
2. Injects `window.Telegram.WebApp` with the HMAC-signed `initData` from `/tmp/secrets/init_data`
3. Waits for `.admin-tabs` to appear (WASM load)
4. Clicks the tab
5. Takes a screenshot to `e2e/screenshots/<tab>.png`

## Security note

The `initData` secret is **never logged** — it is read directly from the file system and injected only into the browser context via `addInitScript`. It does not appear in test output or CI logs.
