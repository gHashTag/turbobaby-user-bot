const { chromium } = require('playwright');

(async () => {
  const browser = await chromium.launch({
    executablePath: '/Users/playra/Library/Caches/ms-playwright/chromium-1208/chrome-mac-arm64/Google Chrome for Testing.app/Contents/MacOS/Google Chrome for Testing',
    headless: true
  });
  const page = await browser.newPage();
  
  page.on('console', msg => console.log(`[${msg.type()}] ${msg.text()}`));
  page.on('pageerror', err => console.log(`[PAGE ERROR] ${err.message}`));
  
  await page.goto('https://woody-weed-bot-production.up.railway.app/admin', { waitUntil: 'domcontentloaded', timeout: 30000 });
  await page.waitForTimeout(15000);
  
  const bodyText = await page.evaluate(() => document.body.innerText);
  console.log('Body text length:', bodyText.length);
  console.log('Body text:', bodyText.substring(0, 300));
  
  await page.screenshot({ path: 'check_deploy.png', fullPage: true });
  console.log('Screenshot saved');
  
  await browser.close();
})();
