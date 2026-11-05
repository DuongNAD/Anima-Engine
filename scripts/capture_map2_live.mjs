import { chromium } from 'playwright';

async function capture() {
  const browser = await chromium.launch({
    headless: true,
    args: ['--use-gl=angle', '--use-angle=d3d11', '--enable-webgl', '--ignore-gpu-blocklist'],
  });
  const context = await browser.newContext({ viewport: { width: 1280, height: 720 } });
  const page = await context.newPage();

  page.on('console', msg => console.log('BROWSER LOG:', msg.type(), msg.text()));
  page.on('pageerror', err => console.error('BROWSER ERROR:', err));

  console.log('Navigating to http://localhost:5173/map2.html ...');
  await page.goto('http://localhost:5173/map2.html');
  await page.waitForSelector('canvas');
  await page.waitForTimeout(5000);

  // 1. Initial Orbit view
  await page.screenshot({ path: 'map2_orbit.png' });
  console.log('Saved map2_orbit.png');

  // 2. High Overview angle (drag mouse to rotate)
  await page.mouse.move(640, 360);
  await page.mouse.down();
  await page.mouse.move(640, 180, { steps: 15 });
  await page.mouse.up();
  await page.waitForTimeout(1000);
  await page.screenshot({ path: 'map2_high_angle.png' });
  console.log('Saved map2_high_angle.png');

  await browser.close();
}

capture().catch(err => {
  console.error('Capture failed:', err);
  process.exit(1);
});
