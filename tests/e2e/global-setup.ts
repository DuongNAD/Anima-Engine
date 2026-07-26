import { chromium } from '@playwright/test';

// Playwright's `webServer.url` probe only waits for the port to answer, and Vite answers as soon as
// it is listening — before it has transformed anything. The first real navigation is what pays for
// compiling the app, and with the Landscape/three chunks in the graph that is well over the 5 s
// timeout the specs pass to `page.goto`. Every worker then raced that same cold compile and skipped
// itself with "failed to connect", which reads exactly like a missing dev server.
//
// One warm-up navigation before the suite starts moves that cost out of the timed window, so a skip
// afterwards means what it says.
export default async function globalSetup() {
  const url = 'http://localhost:5173';
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    await page.goto(url, { waitUntil: 'load', timeout: 120_000 });
    // The landscape entry is a second Vite input and a separate module graph; warm it too.
    await page.goto(`${url}/landscape.html`, { waitUntil: 'load', timeout: 120_000 });
  } catch (err) {
    // Not fatal: the specs each handle an unreachable server themselves. Surface it so a genuinely
    // broken dev server is visible in the log rather than showing up only as a wall of skips.
    console.warn(`[e2e global-setup] warm-up navigation failed: ${err}`);
  } finally {
    await browser.close();
  }
}
