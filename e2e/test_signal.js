const { chromium } = require('playwright-core');

(async () => {
  const browser = await chromium.connectOverCDP('http://localhost:9223');
  const context = browser.contexts()[0];
  const page = await context.newPage();
  
  await page.goto('http://localhost:8081/admin');
  await page.waitForTimeout(5000);
  
  // Click Test Set button
  const testBtn = page.locator('button').filter({ hasText: /Test Set/ });
  await testBtn.evaluate(el => el.click());
  await page.waitForTimeout(1000);
  
  // Check if video element exists
  const hasVideo = await page.locator('video').count();
  console.log('Video elements after Test Set:', hasVideo);
  
  // Check video src
  const src = await page.locator('video').getAttribute('src').catch(() => 'none');
  console.log('Video src:', src);
  
  await browser.close();
})();
