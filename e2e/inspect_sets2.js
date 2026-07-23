const { chromium } = require('playwright-core');

const url = process.argv[2] || 'https://parade-indexes-preventing-sensor.trycloudflare.com/sets?v=' + Date.now();
const outDir = process.argv[3] || '/tmp/sets-inspect2';

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 2,
  });
  const page = await context.newPage();
  const fs = require('fs');
  fs.mkdirSync(outDir, { recursive: true });

  const logs = [];
  page.on('console', msg => logs.push(`[console] ${msg.type()}: ${msg.text()}`));
  page.on('pageerror', err => logs.push(`[pageerror] ${err.message}`));
  page.on('requestfailed', req => logs.push(`[reqfail] ${req.url()} ${req.failure()?.errorText}`));

  console.log('Loading', url);
  await page.goto(url, { waitUntil: 'networkidle', timeout: 60000 });
  await page.waitForTimeout(10000);

  await page.screenshot({ path: `${outDir}/sets-full.png`, fullPage: true });

  // Find cards by the SET badge text
  const cards = await page.$$('span:has-text("📦 SET")');
  console.log('SET badge count:', cards.length);

  const sizes = [];
  for (let i = 0; i < Math.min(cards.length, 6); i++) {
    const card = await cards[i].evaluateHandle(el => {
      let node = el;
      while (node && node !== document.body) {
        const style = node.getAttribute('style') || '';
        if (style.includes('padding-bottom')) return node;
        node = node.parentElement;
      }
      return el.parentElement;
    });
    const box = await card.boundingBox();
    sizes.push({ index: i, w: box?.width, h: box?.height });
    await card.screenshot({ path: `${outDir}/set-card-${i}.png` });
  }
  console.log('Card sizes:', JSON.stringify(sizes, null, 2));

  fs.writeFileSync(`${outDir}/logs.txt`, logs.join('\n'));
  await browser.close();
})();
