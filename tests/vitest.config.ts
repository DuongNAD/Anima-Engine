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
      '@react-three/fiber': path.resolve(__dirname, './mocks/react-three-fiber-mock.tsx'),
    },
  },
});
