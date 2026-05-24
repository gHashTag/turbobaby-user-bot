import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: '.',
  testMatch: '**/*.spec.ts',
  timeout: 60_000,
  retries: 1,
  reporter: [['list'], ['html', { open: 'never', outputFolder: 'playwright-report' }]],
  use: {
    baseURL: 'https://woody-weed-bot-production.up.railway.app',
    extraHTTPHeaders: {
      'ngrok-skip-browser-warning': 'true',
    },
    screenshot: 'only-on-failure',
    trace: 'on-first-retry',
    navigationTimeout: 30_000,
    // Use locally installed Chromium if available
    launchOptions: {
      executablePath: '/Users/playra/Library/Caches/ms-playwright/chromium-1208/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing',
    },
  },
  projects: [
    {
      name: 'chromium',
      use: {
        ...devices['Desktop Chrome'],
        userAgent:
          'Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36',
      },
    },
  ],
  outputDir: 'test-results',
});
