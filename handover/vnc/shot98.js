const { chromium } = require('/home/agent/.npm/_npx/9b9809298fe40d2c/node_modules/playwright-core');
(async () => {
  const b = await chromium.launch({ executablePath: process.argv[3] });
  const p = await b.newPage({ viewport: { width: 1440, height: 900 } });
  await p.goto('http://localhost:6081/vnc.html?autoconnect=1&resize=scale', { waitUntil: 'load' });
  await p.waitForTimeout(4000);
  const clip = process.argv[4] ? (([x, y, w, h]) => ({ x, y, width: w, height: h }))(process.argv[4].split(',').map(Number)) : undefined;
  await p.screenshot({ path: process.argv[2], clip });
  await b.close();
})().catch(e => { console.error(e.message); process.exit(1); });
