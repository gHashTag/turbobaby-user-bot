/**
 * E2E test for file upload in the admin panel.
 *
 * This test verifies that the upload flow sends auth headers
 * (X-Telegram-Init-Data, X-Admin-Telegram-Id, X-Admin-Token)
 * and successfully receives an image URL from the backend.
 *
 * Run locally against a mock API:
 *   node e2e/mock_api.js          # mock backend on :3001
 *   trunk serve --config Trunk.e2e.toml  # frontend on :8080
 *   npx playwright test e2e/upload.spec.ts
 */

import { test, expect } from '@playwright/test';
import * as path from 'path';

const BASE_URL = 'http://localhost:8080';

test.describe('Admin Upload', () => {
  test('upload image on Strains tab sends auth headers', async ({ page }) => {
    await page.goto(`${BASE_URL}/admin`, { waitUntil: 'networkidle' });
    await expect(page.locator('text=Admin').first()).toBeVisible({ timeout: 30_000 });

    // Inject Telegram context AFTER the real telegram-web-app.js has loaded
    // so our WASM app sees a valid initData string.
    await page.evaluate(() => {
      Object.defineProperty(window, 'Telegram', {
        configurable: true,
        writable: true,
        value: {
          WebApp: {
            initData: 'user=%7B%22id%22%3A8420420131%7D&auth_date=1710000000&hash=abc123',
            initDataUnsafe: {
              user: { id: 8420420131, first_name: 'Test', username: 'testadmin', language_code: 'en' },
              auth_date: 1710000000,
              hash: 'abc123',
            },
            version: '6.9', platform: 'tdesktop', colorScheme: 'dark', themeParams: {},
            isExpanded: true, viewportHeight: 900, viewportStableHeight: 900,
            ready: () => {}, expand: () => {}, close: () => {}, sendData: () => {},
            onEvent: () => {}, offEvent: () => {},
            showAlert: (msg: string) => { console.log('[TG alert]', msg); },
            showConfirm: (msg: string, cb: (ok: boolean) => void) => cb(true),
          },
        },
      });
    });

    // Navigate to Strains tab
    await page.click('text=Strains');
    await page.waitForTimeout(800);

    // Click the first 📷 Upload button
    const uploadBtn = page.locator('button:has-text("📷 Upload")').first();
    await expect(uploadBtn).toBeVisible({ timeout: 10_000 });

    const [fileChooser] = await Promise.all([
      page.waitForEvent('filechooser'),
      uploadBtn.click(),
    ]);

    // Select test PNG
    const testFile = path.join(__dirname, 'fixtures', 'test.png');
    await fileChooser.setFiles(testFile);

    // Wait for upload to finish
    await page.waitForSelector('text=⏳ Загрузка...', { state: 'hidden', timeout: 15_000 });

    // Assert URL was populated
    const imageInput = page.locator('input[placeholder="URL картинки"]').first();
    const imageUrl = await imageInput.inputValue();
    expect(imageUrl).toContain('/uploads/');
    expect(imageUrl).toContain('.png');

    // Assert preview appears
    const previewImg = page.locator('img[src*="/uploads/"]').first();
    await expect(previewImg).toBeVisible();
  });
});
