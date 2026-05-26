const { chromium } = require('playwright-core');

(async () => {
  const browser = await chromium.connectOverCDP('http://localhost:9223');
  const context = browser.contexts()[0];
  const page = await context.newPage();
  
  // Collect console messages
  const logs = [];
  page.on('console', msg => logs.push(msg.text()));
  page.on('pageerror', err => logs.push('ERROR: ' + err.message));
  
  await page.goto('http://localhost:8081/menu');
  await page.waitForTimeout(6000);
  
  const wsErrors = logs.filter(l => l.includes('_dioxus') || l.includes('WebSocket'));
  console.log('WebSocket errors:', wsErrors.length);
  wsErrors.forEach(e => console.log('  -', e.substring(0, 100)));
  
  const playButtons = (await page.locator('button').allInnerTexts()).filter(t => t.includes('▶'));
  console.log('Play buttons:', playButtons.length);
  
  await browser.close();
})();
