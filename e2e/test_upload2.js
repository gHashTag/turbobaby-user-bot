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
  
  const result = await page.evaluate(async () => {
    const formData = new FormData();
    const blob = new Blob(['test video content'], { type: 'video/mp4' });
    formData.append('file', blob, 'test.mp4');
    try {
      const resp = await fetch('http://localhost:8081/api/upload', {
        method: 'POST',
        body: formData,
        headers: {
          'X-Telegram-Init-Data': 'test',
          'X-Admin-Telegram-Id': '123',
          'X-Admin-Token': 'test-token'
        }
      });
      const data = await resp.json();
      return {status: resp.status, data};
    } catch(e) {
      return {error: e.message};
    }
  });
  
  console.log('Result:', JSON.stringify(result));
  
  await browser.close();
})();
