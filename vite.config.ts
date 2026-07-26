import { defineConfig } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "path";

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => ({
  plugins: [react()],
  resolve: {
    alias: {
      "@tauri-apps/api": path.resolve(__dirname, "node_modules/@tauri-apps/api"),
      ...(mode === 'test' ? {
        // Only the R3F reconciler is mocked under jsdom; real three runs headless
        // (geometry/math classes need no WebGL context).
        "@react-three/fiber": path.resolve(__dirname, "tests/mocks/react-three-fiber-mock.tsx"),
      } : {}),
      "@": path.resolve(__dirname, "./src"),
    },
  },
  // Prevent vite from obscuring rust errors
  clearScreen: false,
  // Tauri expects a fixed port, fail if that port is not available
  server: {
    strictPort: true,
    port: 5173,
  },
  envPrefix: ["VITE_", "TAURI_"],
  build: {
    // Tauri supports es2021
    target: "es2021",
    // don't minify for debug builds.
    //
    // `true` rather than `"esbuild"`: Vite 8 bundles with rolldown/oxc and no longer ships esbuild.
    // Naming it explicitly routes through `transformWithEsbuild`, which is deprecated and now fails
    // with "Cannot find package 'esbuild'" — the build dies in the worker-bundling step for
    // worldCache.ts, which is the only worker entry and so the only place that surfaces it.
    minify: !process.env.TAURI_DEBUG,
    // produce sourcemaps for debug builds
    sourcemap: !!process.env.TAURI_DEBUG,
    rollupOptions: {
      input: {
        main: path.resolve(__dirname, "index.html"),
        landscape: path.resolve(__dirname, "landscape.html"),
      },
    },
  },
  test: {
    globals: true,
    environment: "jsdom",
    include: ["src/**/*.{test,spec}.{ts,tsx}"],
    setupFiles: ["./tests/setup-vitest.ts"],
    // Matches tests/vitest.config.ts, which has carried this since it started rendering <App />.
    //
    // The first `render(<App />)` in a file pays for the whole lazy module graph plus world
    // generation under jsdom; every later render in the same file is ~150ms. That first one measures
    // 4.2s on a dev machine — 84% of vitest's 5s default — and 10.9s on a GitHub ubuntu runner,
    // which is where it started failing. It passed before only because the margin happened to be on
    // the right side of the line, not because it was comfortable.
    //
    // Raising the timeout rather than trimming the test: what makes it slow is App's real startup
    // cost, which is the thing under test. A version fast enough for a 5s budget would be a version
    // that no longer mounts the app.
    testTimeout: 15000,
  },
}));
