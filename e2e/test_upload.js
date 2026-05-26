const { chromium } = require('playwright-core');

(async () => {
  const browser = await chromium.connectOverCDP('http://localhost:9223');
  const context = browser.contexts()[0];
  const page = await context.newPage();
  
  // Intercept and log all requests
  page.on('request', req => {
    if (req.url().includes('upload')) {
      console.log('REQUEST:', req.method(), req.url());
      console.log('  Headers:', JSON.stringify(req.headers()));
    }
  });
  page.on('response', res => {
    if (res.url().includes('upload')) {
      console.log('RESPONSE:', res.status(), res.url());
    }
  });
  page.on('requestfailed', req => {
    if (req.url().includes('upload')) {
      console.log('FAILED:', req.url(), req.failure().errorText);
    }
  });
  
  await page.goto('http://localhost:8081/admin');
  await page.waitForTimeout(5000);
  
  // Try to trigger upload via console — simulate what admin panel does
  await page.evaluate(async () => {
    const formData = new FormData();
    const blob = new Blob(['test'], { type: 'video/mp4' });
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
      console.log('UPLOAD RESULT:', JSON.stringify(data));
      return data;
    } catch(e) {
      console.error('UPLOAD ERROR:', e.message);
      return {error: e.message};
    }
  });
  
  await page.waitForTimeout(3000);
  
  // Get console logs
  const logs = await page.evaluate(() => {
    return window.__console_logs || [];
  });
  
  await browser.close();
})();
