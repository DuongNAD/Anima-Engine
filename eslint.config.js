import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import reactHooks from 'eslint-plugin-react-hooks';
import reactRefresh from 'eslint-plugin-react-refresh';
import globals from 'globals';

export default tseslint.config(
  {
    ignores: [
      'dist',
      'src-tauri',
      // One-off CommonJS codegen script at the repo root, not part of the TS/browser app this
      // config lints. Came in with the `main` merge and trips `no-require-imports` otherwise.
      'update_vegetation.js',
      'tests/node_modules',
      'map-prototype',
      'rabbit-standalone',
      '**/*.d.ts',
      // Vendored agent-scratch workspaces — not source.
      '.agents/**',
      '$.agents/**',
      '**/.agents/**',
      // Ad-hoc Playwright visual-verification harnesses: standalone scripts that
      // run browser-injected code (page.evaluate references page globals), so static
      // linting reports false positives. Not part of the maintained test suite.
      '**/challenger_runner.js',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  {
    rules: {
      // Intentional ignore-catches (best-effort fetches) are idiomatic here.
      'no-empty': ['error', { allowEmptyCatch: true }],
      // `declare namespace JSX { ... }` is the only way to augment R3F intrinsics.
      '@typescript-eslint/no-namespace': ['error', { allowDeclarations: true }],
    },
  },
  {
    // Plain JS scripts (config + ad-hoc test runners). These mix Node with
    // browser-injected code (Playwright page.evaluate), so allow both global sets.
    files: ['**/*.{js,cjs,mjs}'],
    languageOptions: {
      globals: { ...globals.node, ...globals.browser },
      sourceType: 'commonjs',
    },
  },
  {
    // .mjs is always an ES module, never CommonJS. Such a file may import a Node builtin
    // whose name is also a global (`import process from 'node:process'`); that is the
    // explicit form, not a redeclaration, so builtinGlobals is off for these files only.
    files: ['**/*.mjs'],
    languageOptions: {
      sourceType: 'module',
    },
    rules: {
      'no-redeclare': ['error', { builtinGlobals: false }],
    },
  },
  {
    files: ['**/*.{ts,tsx}'],
    languageOptions: {
      ecmaVersion: 2022,
      globals: { ...globals.browser },
    },
    plugins: {
      'react-hooks': reactHooks,
      'react-refresh': reactRefresh,
    },
    rules: {
      ...reactHooks.configs.recommended.rules,
      'react-refresh/only-export-components': ['warn', { allowConstantExport: true }],
      // tsc already enforces unused vars; let underscore-prefixed args through.
      '@typescript-eslint/no-unused-vars': ['warn', { argsIgnorePattern: '^_' }],
      // `any` is a warning, not an error — but there are none left. See below.
      '@typescript-eslint/no-explicit-any': 'warn',

      // The React Compiler rules, new in eslint-plugin-react-hooks 7 and errors by default.
      //
      // They arrived with the eslint 9 -> 10 upgrade, which was taken to clear a high-severity
      // brace-expansion advisory reachable only through eslint's pinned minimatch. Turning them on
      // as errors would have failed `npm run lint` on 53 pre-existing findings across the R3F and
      // Pixi components, none of them introduced by that upgrade — so the security fix would have
      // been gated behind an unrelated refactor. They were set to `warn` instead, which put them
      // under scripts/eslint_ratchet.mjs: the count could not grow, and lowering it was ordinary
      // work rather than a prerequisite.
      //
      // That backlog is now empty. Every finding from these four rules, and every `any` and unused
      // binding above, has been resolved in the code — the frame loops read `state.scene` and
      // `state.camera` rather than closing over render values, the remaining imperative three.js
      // writes are named operations that take their target as a parameter, and the six
      // `eslint-disable-next-line` directives are gone. Three of the findings were live defects.
      //
      // The severities stay as they are and the ratchet baseline is 0, which enforces the same
      // thing from the other side: a warning now means "this commit introduced it".
      'react-hooks/immutability': 'warn',
      'react-hooks/purity': 'warn',
      'react-hooks/refs': 'warn',
      'react-hooks/set-state-in-effect': 'warn',
    },
  },
);
