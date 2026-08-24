import { test, expect } from '@playwright/test';
import { existsSync } from 'node:fs';
import { resolve } from 'node:path';

// ---------------------------------------------------------------------------------------
// The real-backend gate.
//
// # Why it is declared conditionally rather than skipped
//
// Browser-scope E2E (`ipc_contract.spec.ts`) is zero-skip: the app is served, the transport is
// deterministic, and anything missing is a failure. Coverage against the *actual* Tauri process is
// a different thing and cannot honestly live in the same run:
//
//   * it needs `cargo build --release --features desktop`, which most contributors will not have;
//   * driving it needs a WebDriver session (`tauri-driver`), because Playwright cannot attach to a
//     Tauri webview over HTTP — the previous suite's five specs spawned the binary and then drove
//     an unrelated Vite page, which connected nothing;
//   * CLAUDE.md records that running the full Bevy/Tauri backend on the development machine has
//     **crashed it**.
//
// A `test.skip()` would put it back in the run as a permanent amber that nobody reads, which is
// how the five fake specs stayed green for so long. So when `ANIMA_E2E_REQUIRE_BACKEND` is unset
// this file declares no test at all — the suite reports zero skips because there is nothing to
// skip — and when it *is* set, the gate is declared and fails closed on every missing
// precondition.
//
// CI sets the flag on the job that has built the binary. Anywhere else it stays an external,
// human gate, named here rather than implied by an empty result.
// ---------------------------------------------------------------------------------------

const REQUIRED = process.env.ANIMA_E2E_REQUIRE_BACKEND === '1';

const BINARY = resolve(
  __dirname,
  `../../src-tauri/target/release/anima-engine${process.platform === 'win32' ? '.exe' : ''}`,
);

if (REQUIRED) {
  test.describe('real Tauri backend', () => {
    test('the release binary that this gate drives exists', () => {
      expect(
        existsSync(BINARY),
        `ANIMA_E2E_REQUIRE_BACKEND=1 but ${BINARY} is missing. Build it with ` +
          `\`cargo build --release --features desktop\`. This is a failure, not a skip: the flag ` +
          `exists so backend coverage cannot go quiet.`,
      ).toBe(true);
    });

    test('a WebDriver session is available to drive it', () => {
      // Stated as an explicit unmet precondition rather than pretended.
      //
      // Driving the real app means `tauri-driver` plus a WebDriver-capable runner, and neither is
      // wired up in this repository. Spawning the binary and pointing Playwright at a Vite page —
      // what the previous specs did — is not a substitute; it drives a different process.
      //
      // So this fails, loudly, with the work named. It is reached only when someone has asked for
      // real-backend coverage, and then an honest "not built yet" is the correct answer.
      expect(
        process.env.ANIMA_E2E_WEBDRIVER_URL,
        'Real-backend E2E needs a WebDriver endpoint (tauri-driver) and none is configured. ' +
          'Set ANIMA_E2E_WEBDRIVER_URL once the harness exists. Until then this gate fails when ' +
          'required, which is the honest state — the previous "live IPC" specs spawned the binary ' +
          'and drove an unrelated page, so they never tested it at all.',
      ).toBeDefined();
    });
  });
}
