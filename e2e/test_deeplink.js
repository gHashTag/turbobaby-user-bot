const { chromium } = require('playwright-core');
(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({ viewport: { width: 390, height: 844 }, deviceScaleFactor: 2 });
  const page = await context.newPage();
  const logs = [];
  page.on('console', msg => logs.push(`[${msg.type()}] ${msg.text()}`));
  page.on('pageerror', err => logs.push(`[pageerror] ${err.message}`));

  // Mock Telegram WebApp with start_param
  await page.addInitScript(() => {
    window.Telegram = {
      WebApp: {
        version: '8.0',
        platform: 'macos',
        initData: '',
        initDataUnsafe: { start_param: 'p_set_b2ec8f6f-acd0-46dd-8130-01fed0fa578d' },
        ready: () => {},
        expand: () => {},
        setBackgroundColor: () => {},
        setHeaderColor: () => {},
        disableVerticalSwipes: () => {},
        openTelegramLink: (url) => { console.log('openTelegramLink', url); window.open(url, '_blank'); },
      }
    };
  });

  // Block the real Telegram SDK from overwriting our mock.
  await page.route('https://telegram.org/js/telegram-web-app.js?v=4', route => route.fulfill({ status: 200, body: '// mocked' }));

  await page.goto('https://parade-indexes-preventing-sensor.trycloudflare.com/?v=' + Date.now(), { waitUntil: 'networkidle', timeout: 60000 });
  await page.waitForTimeout(10000);
  await page.screenshot({ path: '/tmp/test-deeplink.png', fullPage: false });
  console.log('URL:', page.url());
  console.log('Logs:');
  logs.forEach(l => console.log(l));
  await browser.close();
})();
