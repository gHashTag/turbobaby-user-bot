const { chromium } = require('playwright');
(async () => {
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 390, height: 844 } });
  const url = process.argv[2] || 'http://127.0.0.1:8080/sets';
  await page.goto(url, { waitUntil: 'networkidle', timeout: 120000 });
  await page.waitForTimeout(8000);
  const cards = await page.locator('div[style*="padding-bottom:135%"]').all();
  console.log('Found wrapper cards:', cards.length);
  for (let i = 0; i < cards.length; i++) {
    const box = await cards[i].boundingBox();
    const style = await cards[i].evaluate(el => {
      const cs = window.getComputedStyle(el);
      return { height: cs.height, width: cs.width, paddingBottom: cs.paddingBottom, position: cs.position };
    });
    console.log(`Card ${i}: box=`, box, 'style=', style);
  }
  await page.screenshot({ path: '/tmp/sets_inspected.png', fullPage: true });
  await browser.close();
})();
