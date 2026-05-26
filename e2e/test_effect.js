const { chromium } = require('playwright-core');

(async () => {
  const browser = await chromium.connectOverCDP('http://localhost:9223');
  const context = browser.contexts()[0];
  const page = await context.newPage();
  
  await page.goto('http://localhost:8081/admin');
  await page.waitForTimeout(5000);
  
  const logs = [];
  page.on('console', msg => logs.push(msg.text()));
  
  // Type into video URL input directly to test effect
  const input = page.locator('input[placeholder="URL видео"]').first();
  await input.fill('/uploads/effect-test.mp4');
  await page.waitForTimeout(1000);
  
  const effectLogs = logs.filter(l => l.includes('VideoUpload'));
  console.log('Effect logs after typing:');
  effectLogs.forEach(l => console.log(' ', l));
  
  await browser.close();
})();
