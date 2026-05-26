const { chromium } = require('playwright-core');

(async () => {
  const browser = await chromium.connectOverCDP('http://localhost:9223');
  const context = browser.contexts()[0];
  const page = await context.newPage();
  
  await page.goto('http://localhost:8081/admin');
  await page.waitForTimeout(5000);
  
  // Click Upload button to trigger dynamic file input
  const uploadBtn = page.locator('button').filter({ hasText: /🎥 Upload/ }).first();
  await uploadBtn.click();
  await page.waitForTimeout(1000);
  
  // Find the dynamically created input and inject a file
  const input = await page.locator('input[type=file]').first();
  const count = await input.count();
  if (count === 0) {
    console.log('FAIL: No file input found');
    await browser.close();
    process.exit(1);
  }
  
  await input.evaluate((el) => {
    const blob = new Blob(['test'], { type: 'video/mp4' });
    const file = new File([blob], 'test.mp4', { type: 'video/mp4' });
    const dt = new DataTransfer();
    dt.items.add(file);
    el.files = dt.files;
    el.dispatchEvent(new Event('change'));
  });
  
  // Wait for upload + UI update
  await page.waitForTimeout(8000);
  
  const videos = await page.locator('video').count();
  const inputs = await page.locator('input[placeholder="URL видео"]').all();
  const hasUrl = inputs.length > 0 && (await inputs[0].inputValue()).includes('/uploads/');
  
  if (videos > 0 && hasUrl) {
    console.log('PASS: Video preview visible after upload');
  } else {
    console.log('FAIL: Video elements:', videos, 'Has URL:', hasUrl);
    await browser.close();
    process.exit(1);
  }
  
  await browser.close();
})();
