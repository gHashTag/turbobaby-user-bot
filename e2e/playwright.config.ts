import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: '**/*.spec.ts',
  timeout: 60_000,
  retries: 1,
  reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report' }]],
  use: {
    baseURL: 'https://woody-weed-bot-production.up.railway.app',
    // Telegram WebApp requires these headers to pass initData auth
    extraHTTPHeaders: {
      'ngrok-skip-browser-warning': 'true',
    },
    screenshot: 'only-on-failure',
    trace: 'on-first-retry',
    // Some tabs may load WASM which is slow
    navigationTimeout: 30_000,
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        // Telegram WebApp JS needs a desktop user agent
        userAgent:
          'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36',
      },
    },
  ],
  outputDir: 'test-results',
});
