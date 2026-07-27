import { defineConfig, type Plugin } from "vitest/config";
import react from "@vitejs/plugin-react";
import path from "path";
import fs from "fs";

/**
 * Standalone scratch pages that live in `public/` so the dev server can serve them, but which must
 * not end up inside the shipped desktop binary.
 *
 * `ecosystem.html` is the reason this exists: it pulls three.js and simplex-noise from
 * `cdnjs.cloudflare.com` at runtime, unpinned and unverified. Vite copies `public/` verbatim, so
 * every `tauri build` was packaging a page that fetches and executes remote code. The production CSP
 * (`default-src 'self'`) now blocks that at runtime — but shipping the page at all is the part worth
 * removing, not just neutralising.
 *
 * They stay in `public/` because `tests/challenger_runner.js` navigates to
 * `http://localhost:5173/ecosystem.html` against the dev server, and that workflow is unaffected:
 * this plugin only runs at `closeBundle`, on the build output.
 */
const DEV_ONLY_PUBLIC_PAGES = ["ecosystem.html", "webgl-test.html"];

function excludeDevOnlyPagesFromBundle(): Plugin {
  return {
    name: "anima-exclude-dev-only-public-pages",
    apply: "build",
    closeBundle() {
      for (const page of DEV_ONLY_PUBLIC_PAGES) {
        const target = path.resolve(__dirname, "dist", page);
        if (fs.existsSync(target)) {
          fs.rmSync(target);
        }
      }
    },
  };
}

/**
 * Record which npm packages this build actually put inside `dist/`.
 *
 * # The distinction this exists to make
 *
 * `npm ls --omit=dev --all` answers "what did npm install for production", and the licensing
 * artifacts used it as if it answered "what ships". It does not, and the gap runs both ways:
 *
 *   * `node_modules/` is never shipped — Tauri packages `dist/`, so an installed package whose code
 *     no bundled module imports leaves no bytes in the product. Measured: 36 packages in the
 *     production install closure, 17 with bytes in `dist/`. Attributing the other 19 is
 *     over-claiming.
 *   * a **devDependency** can still contribute runtime code, and `npm ls --omit=dev` cannot see it
 *     by construction. Measured in this build: `vite/modulepreload-polyfill.js`,
 *     `vite/preload-helper.js`, `rolldown/runtime.js` and four `@oxc-project/runtime` helpers are
 *     compiled into the shipped chunks. Three third-party components, none of them a production
 *     dependency, all of them distributed.
 *
 * Neither error is visible by reading `package.json`. The bundler is the only component that knows,
 * so it is the component that reports: this walks the real module graph of the real build.
 *
 * `chunk.modules` rather than `chunk.moduleIds`: the former is what landed *in a chunk*, with a
 * `renderedLength` per module. A module that reached the graph and was then tree-shaken to nothing
 * contributes no bytes and is not distributed, so `renderedLength === 0` is skipped.
 *
 * # Why it writes outside `dist/`
 *
 * The file is evidence about the release, not part of it. It is committed, so
 * `scripts/gen_third_party_licenses.mjs` can read the boundary without anyone running a build, and
 * `npm run check:bundle-closure` fails when a fresh build disagrees with the committed copy.
 *
 * Only `{name, version}` is recorded — never a module path. Paths are absolute and
 * machine-specific, and the file has to be byte-identical on a developer's Windows checkout and on
 * a CI ubuntu runner or the freshness gate reports the machine rather than the dependency graph.
 *
 * The package map is passed in rather than owned, because Vite compiles workers in a **separate**
 * rollup build with its own plugin instances (`worker.plugins` is a factory). Two instances writing
 * to one map means the file is the union of the main graph and every worker graph; two instances
 * each owning a map would mean whichever finished last silently erased the other's findings. The
 * worker entry imports only first-party modules today — but "today" is exactly the assumption this
 * file exists to stop making.
 */
const bundledPackages = new Map<string, BundledPackage>();

type BundledPackage = { name: string; version: string; origin: "node_modules" | "injected" };

/**
 * Virtual ids for code the toolchain injects rather than resolves: rolldown and oxc prefix them
 * with NUL, and there is no `node_modules` path to read a version from. Two shapes occur, and both
 * are real in this build:
 *
 *   `\0@oxc-project+runtime@0.139.0/helpers/esm/typeof.js`  — name and version in the id, `+` for `/`
 *   `\0vite/preload-helper.js`                              — bare package name, version from disk
 *
 * `@oxc-project/runtime` is the case that makes this necessary: it is **not installed at all** — it
 * is neither in `node_modules/` nor in `package-lock.json`, because rolldown carries the helper
 * sources inside its own distribution. Its code ships regardless.
 */
function parseInjectedId(id: string): { name: string; version: string | null } | null {
  const spec = id.replace(/^\0/, "");
  const withVersion = /^(@?[^@/]+(?:\+[^@/]+)?)@(\d[^/]*)\//.exec(spec);
  if (withVersion) return { name: withVersion[1].replace("+", "/"), version: withVersion[2] };
  const bare = /^(@[^/]+\/[^/]+|[^@/][^/]*)\//.exec(spec);
  return bare ? { name: bare[1], version: null } : null;
}

function readVersion(manifestPath: string): string | null {
  if (!fs.existsSync(manifestPath)) return null;
  const parsed: unknown = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
  if (typeof parsed !== "object" || parsed === null || !("version" in parsed)) return null;
  return typeof parsed.version === "string" ? parsed.version : null;
}

function emitBundleClosure(packages: Map<string, BundledPackage>): Plugin {
  const root = path.resolve(__dirname).replace(/\\/g, "/");
  const unresolved = new Set<string>();

  const add = (pkg: BundledPackage): void => {
    // Keyed by name AND version. A single-name key silently collapses two versions of the same
    // package into whichever the bundler happened to emit first, and both versions ship.
    packages.set(`${pkg.name}@${pkg.version}`, pkg);
  };

  const record = (moduleId: string): void => {
    // Rolldown emits Windows separators on Windows.
    const id = moduleId.replace(/\\/g, "/");
    const marker = "/node_modules/";
    const at = id.lastIndexOf(marker);

    if (at === -1) {
      if (id.startsWith(root)) return; // first-party source, covered by LICENSE
      const injected = parseInjectedId(id);
      const version =
        injected?.version ??
        (injected && readVersion(path.join(root, "node_modules", ...injected.name.split("/"), "package.json")));
      if (injected && version) add({ name: injected.name, version, origin: "injected" });
      // Shipped code that could not be resolved to a component is recorded, never dropped: the
      // gate below refuses to pass while any remains. Only machine-independent virtual ids reach
      // here, so this stays byte-stable across checkouts.
      else unresolved.add(id);
      return;
    }

    const prefix = id.slice(0, at + marker.length);
    const segments = id.slice(at + marker.length).split("/");
    // `@scope/name` is two segments; everything else is one.
    const name = segments[0].startsWith("@") ? `${segments[0]}/${segments[1]}` : segments[0];
    if (!name) return;
    const version = readVersion(path.join(prefix, ...name.split("/"), "package.json"));
    if (version) add({ name, version, origin: "node_modules" });
    else unresolved.add(`${name} (no readable package.json under ${marker})`);
  };

  return {
    name: "anima-emit-bundle-closure",
    apply: "build",
    generateBundle(_options, bundle) {
      for (const chunk of Object.values(bundle)) {
        if (chunk.type !== "chunk") continue;
        for (const [moduleId, rendered] of Object.entries(chunk.modules)) {
          if (rendered.renderedLength > 0) record(moduleId);
        }
      }
    },
    closeBundle() {
      const dir = path.resolve(__dirname, "licensing");
      fs.mkdirSync(dir, { recursive: true });
      // Byte comparison, not `localeCompare`: locale collation orders punctuation by rules that
      // differ from a byte sort, so `@scope/x` would land in a different place than any consumer
      // sorting the obvious way — the same trap `scripts/gen_sbom.mjs` documents.
      const byte = (a: string, b: string): number => (a < b ? -1 : a > b ? 1 : 0);
      const body = {
        $comment:
          "Generated by `vite build` (plugin: anima-emit-bundle-closure). Do not edit by hand. " +
          "This is the npm distribution boundary: packages with rendered bytes in dist/. " +
          "`origin: injected` is toolchain code compiled into the output rather than resolved from " +
          "node_modules — it is distributed even though it is not a production dependency. " +
          "Regenerate with `npm run build`; verify with `npm run check:bundle-closure`.",
        packages: [...packages.values()].sort(
          (a, b) => byte(a.name, b.name) || byte(a.version, b.version),
        ),
        unresolved: [...unresolved].sort(byte),
      };
      fs.writeFileSync(path.join(dir, "bundle-closure.json"), `${JSON.stringify(body, null, 2)}\n`);
    },
  };
}

// https://vitejs.dev/config/
export default defineConfig(({ mode }) => ({
  plugins: [react(), excludeDevOnlyPagesFromBundle(), emitBundleClosure(bundledPackages)],
  // Only `plugins`. Vite compiles workers in a separate rollup build that does not inherit the main
  // plugin list, so the closure recorder has to be registered here too or a third-party module
  // reachable only from `worldGen.worker.ts` would ship unattributed. Nothing else is set: adding
  // `format` here would change the emitted worker bundle, which is a build change and not a
  // licensing one.
  worker: {
    plugins: () => [emitBundleClosure(bundledPackages)],
  },
  resolve: {
    alias: {
      "@tauri-apps/api": path.resolve(__dirname, "node_modules/@tauri-apps/api"),
      ...(mode === 'test' ? {
        // Only the R3F reconciler is mocked under jsdom; real three runs headless
        // (geometry/math classes need no WebGL context).
        "@react-three/fiber": path.resolve(__dirname, "tests/mocks/react-three-fiber-mock.ts"),
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
