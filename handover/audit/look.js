const { chromium } = require('/home/agent/.npm/_npx/9b9809298fe40d2c/node_modules/playwright-core');
(async () => {
  const b = await chromium.launch({ executablePath: '/home/agent/.cache/ms-playwright/chromium-1134/chrome-linux/chrome' });
  const p = await b.newPage({ viewport: { width: +process.argv[4] || 1100, height: 900 } });
  const html = require('fs').readFileSync(process.argv[2], 'utf8');
  await p.setContent('<!doctype html><meta charset="utf-8"><meta name="viewport" content="width=device-width">' + html, { waitUntil: 'load' });
  await p.waitForTimeout(800);
  await p.screenshot({ path: process.argv[3], fullPage: false });
  await b.close();
})();
