const { chromium } = require('playwright-core');

(async () => {
  const browser = await chromium.connectOverCDP('http://localhost:9223');
  const context = browser.contexts()[0];
  const page = await context.newPage();
  
  await page.goto('http://localhost:8081/admin');
  await page.waitForTimeout(5000);
  
  // Click upload button
  const uploadBtn = page.locator('button').filter({ hasText: /🎥 Upload/ });
  await uploadBtn.click();
  await page.waitForTimeout(2000);
  
  // Simulate file upload via evaluate (mock the file selection)
  await page.evaluate(async () => {
    const formData = new FormData();
    const blob = new Blob(['test video'], { type: 'video/mp4' });
    formData.append('file', blob, 'test.mp4');
    const resp = await fetch('http://localhost:8081/api/upload', {
      method: 'POST',
      body: formData,
      headers: { 'X-Telegram-Init-Data': 'test', 'X-Admin-Telegram-Id': '123', 'X-Admin-Token': 'test' }
    });
    const data = await resp.json();
    
    // Find the VideoUpload component and set its video_url Signal directly
    // This simulates what upload_video() would do
    window.__test_video_url = data.url;
    return data;
  });
  
  await page.waitForTimeout(2000);
  
  // Now check if video preview is visible
  const videos = await page.locator('video').count();
  console.log('Video elements:', videos);
  
  // Check Test Set button
  const testBtn = page.locator('button').filter({ hasText: /Test Set/ });
  await testBtn.evaluate(el => el.click());
  await page.waitForTimeout(1000);
  
  const videos2 = await page.locator('video').count();
  console.log('Video elements after Test Set:', videos2);
  const src = await page.locator('video').getAttribute('src').catch(() => 'none');
  console.log('Video src:', src);
  
  await browser.close();
})();
