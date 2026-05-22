# E2E Tests — Admin Panel (Playwright)

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

`Strains`, `Accessories`, `Tea`, `Sets`, `AccessorySets`, `TeaSets`,
`Dashboard`, `Orders`, `Quests`, `Treasures`, `Garden`, `Loyalty`, `Managers`

Each tab gets its own `test('Tab: <Name>', ...)` which:
1. Opens `https://woody-weed-bot-production.up.railway.app/admin`
2. Injects `window.Telegram.WebApp` with the HMAC-signed `initData` from `/tmp/secrets/init_data`
3. Waits for `.admin-tab-bar` to appear (WASM load)
4. Clicks the tab
5. Takes a screenshot to `e2e/screenshots/<tab>.png`

## Security note

The `initData` secret is **never logged** — it is read directly from the file system and injected only into the browser context via `addInitScript`. It does not appear in test output or CI logs.
