const { chromium } = require('playwright-core');
(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 });
  const page = await context.newPage();
  const logs = [];
  page.on('console', msg => logs.push(`[${msg.type()}] ${msg.text()}`));
  page.on('pageerror', err => logs.push(`[pageerror] ${err.message}`));
  page.on('requestfailed', req => logs.push(`[reqfail] ${req.url()} ${req.failure()?.errorText}`));

  await page.route('https://telegram.org/js/telegram-web-app.js?v=4', route => route.fulfill({ status: 200, body: '// mocked' }));

  // Mock localStorage with a fake admin token
  await page.addInitScript(() => {
    window.Telegram = {
      WebApp: {
        version: '8.0', platform: 'macos', initData: '', initDataUnsafe: { start_param: '' },
        ready: () => {}, expand: () => {}, setBackgroundColor: () => {}, setHeaderColor: () => {}, disableVerticalSwipes: () => {},
      }
    };
  });

  await page.goto('https://parade-indexes-preventing-sensor.trycloudflare.com/admin?v=' + Date.now(), { waitUntil: 'networkidle', timeout: 60000 });
  await page.waitForTimeout(5000);
  await page.screenshot({ path: '/tmp/test-admin.png', fullPage: false });
  console.log('URL:', page.url());
  console.log('Logs:');
  logs.forEach(l => console.log(l));
  await browser.close();
})();
