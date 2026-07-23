const { chromium } = require('playwright-core');

const url = process.argv[2] || 'https://parade-indexes-preventing-sensor.trycloudflare.com/?v=' + Date.now();
const outDir = process.argv[3] || '/tmp/home-sets-inspect2';

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

  await page.screenshot({ path: `${outDir}/home-full.png`, fullPage: true });

  // Locate the packs carousel by the header text "НАБОРЫ" / "Packs"
  const sections = await page.$$('h2');
  console.log('Headers found:', await Promise.all(sections.map(h => h.textContent())));

  // Find the horizontal-scroll container right after the packs header
  const carouselInfo = await page.evaluate(() => {
    const headers = Array.from(document.querySelectorAll('h2'));
    const packHeader = headers.find(h => /Наборы|Packs/i.test(h.textContent || ''));
    if (!packHeader) return { error: 'no pack header' };
    const container = packHeader.parentElement?.nextElementSibling;
    if (!container) return { error: 'no carousel container' };
    const children = Array.from(container.children);
    return {
      containerTag: container.tagName,
      containerStyle: container.getAttribute('style') || '',
      childCount: children.length,
      children: children.map((c, i) => ({
        index: i,
        tag: c.tagName,
        style: c.getAttribute('style') || '',
        rect: c.getBoundingClientRect(),
        innerHTML: c.innerHTML.slice(0, 300),
      })),
    };
  });
  console.log('Carousel info:', JSON.stringify(carouselInfo, null, 2));

  // Screenshot individual visible children
  if (carouselInfo.children) {
    const children = await page.$$('h2');
    const packHeader = await page.$('h2:has-text("НАБОРЫ"), h2:has-text("Packs")');
    if (packHeader) {
      const container = await packHeader.evaluateHandle(el => el.parentElement.nextElementSibling);
      const kids = await container.$$(':scope > *');
      for (let i = 0; i < kids.length; i++) {
        try {
          await kids[i].screenshot({ path: `${outDir}/carousel-child-${i}.png` });
        } catch (e) {
          console.log('screenshot child', i, 'failed', e.message);
        }
      }
    }
  }

  fs.writeFileSync(`${outDir}/logs.txt`, logs.join('\n'));
  await browser.close();
})();
