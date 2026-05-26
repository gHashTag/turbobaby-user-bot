const { chromium } = require('playwright-core');

(async () => {
  const browser = await chromium.connectOverCDP('http://localhost:9223');
  const context = browser.contexts()[0];
  const page = await context.newPage();
  
  page.on('request', req => {
    if (req.url().includes('upload')) console.log('REQ:', req.method(), req.url());
  });
  page.on('response', res => {
    if (res.url().includes('upload')) console.log('RES:', res.status(), res.url());
  });
  page.on('requestfailed', req => {
    if (req.url().includes('upload')) console.log('FAIL:', req.url(), req.failure().errorText);
  });
  
  await page.goto('http://localhost:8081/admin');
  await page.waitForTimeout(5000);
  
  // Find and click video upload button
  const buttons = await page.locator('button').allInnerTexts();
  console.log('Buttons:', buttons);
  
  // Click the video upload button (🎥 Upload)
  const videoUploadBtn = page.locator('button').filter({ hasText: /Upload/ }).nth(1);
  await videoUploadBtn.click();
  await page.waitForTimeout(2000);
  
  // Upload a file using filechooser
  const [fileChooser] = await Promise.all([
    page.waitForEvent('filechooser'),
    videoUploadBtn.click()
  ]);
  
  await fileChooser.setFiles('/Users/playra/woody-weed-bot/assets/favicon.svg');
  await page.waitForTimeout(5000);
  
  await browser.close();
})();
