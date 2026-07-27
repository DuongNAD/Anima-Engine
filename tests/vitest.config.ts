import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: [path.resolve(__dirname, './setup-vitest.ts')],
    include: ['frontend/**/*.{test,spec}.{ts,tsx}'],
    exclude: ['**/node_modules/**', '**/dist/**'],
    testTimeout: 15000,
    // Bounded on purpose, and this is the single highest-value line in this file.
    //
    // Every file here pays for a full `render(<App />)` — the whole lazy module graph plus world
    // generation, under jsdom. That is CPU-bound and memory-hungry, so letting Vitest default to
    // one worker per core makes 26 files fight for the machine, each one's 15 s budget ticking
    // while it is descheduled.
    //
    // The failure that produces is **misleading, not merely slow**: a render that never completes
    // surfaces as `Unable to find an element with the text …` — an assertion-shaped message with a
    // real DOM dump attached — so it reads as a broken expectation rather than a starved one.
    // Measured on this machine (i5-14600KF, 2026-07-27) with an unrelated project's Vitest run
    // occupying a core and 1.4 GB:
    //
    //   default workers  → 28 failed | 215 passed | 1 skipped, 26 files, wall 40.3 s
    //   maxWorkers: 4    →  0 failed | 243 passed | 1 skipped, 26 files, wall 47.5 s
    //
    // Same commit, same assertions, minutes apart. Nothing was relaxed to get there: all 243
    // assertions still run. It trades ~7 s of wall clock for a suite whose red means red. If this
    // is ever raised, the thing to check is not the timeout — it is how many `render(<App />)`
    // calls are running at once.
    maxWorkers: 4,
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, '../src'),
      'react': path.resolve(__dirname, '../node_modules/react'),
      'react-dom': path.resolve(__dirname, '../node_modules/react-dom'),
      '@tauri-apps/api': path.resolve(__dirname, '../node_modules/@tauri-apps/api'),
      // Both, and they need each other. `tests/` is its own npm package with its own
      // node_modules, which is why everything above is pinned to the root copy; `three` is pinned
      // for the same reason. Only the R3F reconciler is mocked — real three runs headless — and a
      // mocked reconciler talking to a *second* copy of three would be worse than either alone.
      'three': path.resolve(__dirname, '../node_modules/three'),
      '@react-three/fiber': path.resolve(__dirname, './mocks/react-three-fiber-mock.ts'),
    },
  },
});
