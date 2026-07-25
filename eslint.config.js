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
      // Existing Tauri IPC payloads use `any` widely; surface it without failing the build.
      '@typescript-eslint/no-explicit-any': 'warn',
    },
  },
);
