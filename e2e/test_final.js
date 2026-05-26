const { chromium } = require('playwright-core');

(async () => {
  const browser = await chromium.connectOverCDP('http://localhost:9223');
  const context = browser.contexts()[0];
  const page = await context.newPage();
  
  await page.goto('http://localhost:8081/admin');
  await page.waitForTimeout(5000);
  
  const logs = [];
  page.on('console', msg => logs.push(msg.text()));
  
  // Click Upload and simulate file
  const uploadBtn = page.locator('button').filter({ hasText: /🎥 Upload/ });
  await uploadBtn.click();
  await page.waitForTimeout(2000);
  
  await page.evaluate(() => {
    const input = document.createElement('input');
    input.type = 'file';
    input.accept = 'video/*';
    input.onchange = async (e) => {
      const file = e.target.files[0];
      if (!file) return;
      const formData = new FormData();
      formData.append('file', file);
      const resp = await fetch('/api/upload', {
        method: 'POST',
        body: formData,
        headers: { 'X-Telegram-Init-Data': 'test', 'X-Admin-Telegram-Id': '123', 'X-Admin-Token': 'test' }
      });
      const data = await resp.json();
      console.log('[TEST] Upload result:', JSON.stringify(data));
    };
    const blob = new Blob(['test'], { type: 'video/mp4' });
    const file = new File([blob], 'test.mp4', { type: 'video/mp4' });
    const dt = new DataTransfer();
    dt.items.add(file);
    input.files = dt.files;
    input.dispatchEvent(new Event('change'));
  });
  
  await page.waitForTimeout(5000);
  
  const videos = await page.locator('video').count();
  console.log('Video elements:', videos);
  
  const src = await page.locator('video').getAttribute('src').catch(() => 'none');
  console.log('Video src:', src);
  
  const relevant = logs.filter(l => l.includes('VideoUpload') || l.includes('TEST'));
  relevant.forEach(l => console.log('LOG:', l));
  
  await browser.close();
})();
