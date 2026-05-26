const { chromium } = require('playwright-core');

(async () => {
  const browser = await chromium.connectOverCDP('http://localhost:9223');
  const context = browser.contexts()[0];
  const page = await context.newPage();
  
  await page.goto('http://localhost:8081/admin');
  await page.waitForTimeout(5000);
  
  // Find the first video_url input and set its value via JS
  // This simulates what VideoUpload component should do after upload
  const hasVideoBefore = await page.evaluate(() => document.querySelectorAll('video').length);
  console.log('Videos before:', hasVideoBefore);
  
  // Simulate upload by finding the first input with placeholder "URL видео" and typing
  const inputs = await page.locator('input[placeholder="URL видео"]').all();
  console.log('Video URL inputs found:', inputs.length);
  
  if (inputs.length > 0) {
    await inputs[0].fill('/uploads/test-video.mp4');
    await page.waitForTimeout(500);
    const hasVideoAfter = await page.evaluate(() => document.querySelectorAll('video').length);
    console.log('Videos after typing URL:', hasVideoAfter);
  }
  
  await browser.close();
})();
