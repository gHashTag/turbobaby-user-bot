const { chromium } = require('playwright-core');

const url = process.argv[2] || 'https://parade-indexes-preventing-sensor.trycloudflare.com/?v=' + Date.now();
const outDir = process.argv[3] || '/tmp/home-sets-inspect';

(async () => {
  const browser = await chromium.launch();
  const context = await browser.newContext({
    viewport: { width: 390, height: 844 },
    deviceScaleFactor: 2,
  });
  const page = await context.newPage();
  const fs = require('fs');
  fs.mkdirSync(outDir, { recursive: true });

  // Capture console + page errors
  const logs = [];
  page.on('console', msg => logs.push(`[console] ${msg.type()}: ${msg.text()}`));
  page.on('pageerror', err => logs.push(`[pageerror] ${err.message}`));
  page.on('requestfailed', req => logs.push(`[reqfail] ${req.url()} ${req.failure()?.errorText}`));

  console.log('Loading', url);
  await page.goto(url, { waitUntil: 'networkidle', timeout: 60000 });

  // Wait for WASM to initialize (preloader hidden or fallback timeout)
  await page.waitForTimeout(8000);

  // Take full screenshot
  await page.screenshot({ path: `${outDir}/home-full.png`, fullPage: true });
  console.log('Screenshot saved:', `${outDir}/home-full.png`);

  // Try to find home pack cards
  const cards = await page.$$('a[href*="sets"] div[style*="padding-bottom"]');
  console.log('Found home pack cards:', cards.length);

  const sizes = [];
  for (let i = 0; i < cards.length; i++) {
    const box = await cards[i].boundingBox();
    if (!box) continue;
    const parent = await cards[i].evaluateHandle(el => el.parentElement);
    const parentBox = await parent.boundingBox();
    sizes.push({
      index: i,
      outer: parentBox ? { w: parentBox.width, h: parentBox.height } : null,
      inner: { w: box.width, h: box.height },
    });
    await parent.screenshot({ path: `${outDir}/card-${i}.png` });
  }
  console.log('Card sizes:', JSON.stringify(sizes, null, 2));

  // Inspect image styles inside cards
  const imageInfo = await page.evaluate(() => {
    return Array.from(document.querySelectorAll('img[alt]')).map(img => ({
      src: (img.src || '').split('/').pop(),
      alt: img.alt,
      style: img.getAttribute('style') || '',
      width: img.clientWidth,
      height: img.clientHeight,
      naturalWidth: img.naturalWidth,
      naturalHeight: img.naturalHeight,
    }));
  });
  console.log('Images:', JSON.stringify(imageInfo, null, 2));

  fs.writeFileSync(`${outDir}/logs.txt`, logs.join('\n'));
  await browser.close();
})();
