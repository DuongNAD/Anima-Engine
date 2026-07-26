import { chromium } from '@playwright/test';

/**
 * Markers that identify the served app as Anima and not something else.
 *
 * `<title>Anima Engine</title>` comes from `index.html`; `/src/main.tsx` is Anima's module entry.
 * Both are cheap, stable, and present in the served HTML before any React render, so the check does
 * not depend on the app booting successfully — only on it being the right app.
 */
const IDENTITY_MARKERS = ['<title>Anima Engine</title>', '/src/main.tsx'];

/**
 * Global setup: prove the server is Anima, then warm the module graph.
 *
 * # Why the identity check exists
 *
 * The old config hard-coded port 5173 and set `reuseExistingServer: !process.env.CI`, so any Vite
 * server already listening was adopted as the app under test. On 2026-07-27, 5173 on this machine
 * was held by a dev server belonging to `E:\Project\LIVA` — a different project entirely. The suite
 * would have navigated fine, found none of Anima's DOM, and each spec would have converted the
 * mismatch into `test.skip()`. Skips report as "not a failure", so a run that exercised a completely
 * different application looks like a healthy one.
 *
 * This function is what makes that impossible: a wrong app **throws**, failing the run before any
 * spec gets the chance to soften it into a skip.
 *
 * # Why the warm-up also exists
 *
 * Playwright's `webServer.url` probe only waits for the port to answer, and Vite answers as soon as
 * it is listening — before it has transformed anything. The first real navigation pays for compiling
 * the app, and with the Landscape/three chunks in the graph that is well over the 5 s timeout the
 * specs pass to `page.goto`. Every worker then raced that same cold compile and skipped itself with
 * "failed to connect", which reads exactly like a missing dev server. Doing it once here moves that
 * cost out of the timed window, so a skip afterwards means what it says.
 */
export default async function globalSetup() {
  const port = Number(process.env.ANIMA_E2E_PORT ?? 5177);
  const url = `http://127.0.0.1:${port}`;

  // ---- identity, before anything else --------------------------------------------------------
  let html: string;
  try {
    const res = await fetch(url, { redirect: 'follow' });
    if (!res.ok) {
      throw new Error(`HTTP ${res.status} ${res.statusText}`);
    }
    html = await res.text();
  } catch (err) {
    throw new Error(
      `[e2e] could not reach the dev server at ${url}: ${err}\n` +
        `Playwright starts its own server (reuseExistingServer: false), so this means the server ` +
        `failed to boot rather than that one is missing. Check the webServer output above. Set ` +
        `ANIMA_E2E_PORT to move off ${port} if it is occupied.`,
    );
  }

  const missing = IDENTITY_MARKERS.filter((m) => !html.includes(m));
  if (missing.length > 0) {
    const title = /<title>([^<]*)<\/title>/i.exec(html)?.[1] ?? '(no <title>)';
    throw new Error(
      `[e2e] the server at ${url} is NOT serving Anima Engine.\n` +
        `  page title: ${title}\n` +
        `  missing marker(s): ${missing.join(', ')}\n\n` +
        `This is a hard failure on purpose. The suite previously hard-coded port 5173 with ` +
        `reuseExistingServer enabled, and on 2026-07-27 that port was held by an unrelated ` +
        `project's Vite server — every spec would have "passed" by skipping against the wrong app. ` +
        `Do not convert this into a skip.`,
    );
  }

  // ---- warm-up -------------------------------------------------------------------------------
  const browser = await chromium.launch();
  try {
    const page = await browser.newPage();
    await page.goto(url, { waitUntil: 'load', timeout: 120_000 });
    // The landscape entry is a second Vite input and a separate module graph; warm it too.
    await page.goto(`${url}/landscape.html`, { waitUntil: 'load', timeout: 120_000 });
  } catch (err) {
    // Not fatal: identity is already proven and each spec handles a slow page itself. Surface it so
    // a genuinely broken app is visible in the log rather than only as a wall of skips.
    console.warn(`[e2e global-setup] warm-up navigation failed: ${err}`);
  } finally {
    await browser.close();
  }

  console.log(`[e2e global-setup] verified Anima Engine at ${url}`);
}
