import { test } from '@playwright/test';

/**
 * Decide what a missing `anima-engine` release binary means for a spec.
 *
 * # The two kinds of skip in this suite, and why only one of them is legitimate
 *
 * Five specs spawn `src-tauri/target/release/anima-engine` and talk to the real backend. On a
 * machine that has not run `cargo build --release --features desktop`, the spawn fails with ENOENT
 * and the spec skips. That skip is honest — the dependency genuinely is not there — but it was
 * indistinguishable from the *other* skip this suite used to produce, where the specs quietly
 * skipped because they were pointed at the wrong application entirely (see `global-setup.ts`).
 *
 * So the missing-binary case is made explicit and, crucially, **escalatable**: set
 * `ANIMA_E2E_REQUIRE_BACKEND=1` and a missing binary is a hard failure instead. CI should set it,
 * because CI is where "the backend E2E silently stopped running" would otherwise go unnoticed for
 * months.
 *
 * # Why it is not simply always required
 *
 * Spawning that binary runs the full Bevy/Tauri backend, and CLAUDE.md records that doing so on the
 * development machine has **crashed it**. Forcing every local `npm run test:e2e` to boot the real
 * simulator would make the suite something developers stop running, which costs more coverage than
 * it buys. The default therefore stays "skip locally, enforce in CI", with the reason stated rather
 * than implied by a bare `test.skip()`.
 */
export function requireBackendOrSkip(spawnError: unknown, specName: string): boolean {
  if (!spawnError) return true;

  const message = (spawnError as { message?: string })?.message ?? String(spawnError);
  const detail =
    `${specName}: the release binary src-tauri/target/release/anima-engine is not runnable ` +
    `(${message}). Build it with \`cargo build --release --features desktop\`.`;

  if (process.env.ANIMA_E2E_REQUIRE_BACKEND === '1') {
    throw new Error(
      `[e2e] ANIMA_E2E_REQUIRE_BACKEND=1 and ${detail}\n` +
        `Failing rather than skipping: this flag exists so CI cannot lose backend coverage quietly.`,
    );
  }

  console.warn(
    `[e2e SKIP] ${detail}\n` +
      `  This skip is deliberate and narrow. Set ANIMA_E2E_REQUIRE_BACKEND=1 to make it a failure.`,
  );
  test.skip();
  return false;
}
