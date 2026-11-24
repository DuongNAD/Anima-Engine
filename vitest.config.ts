import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: [path.resolve(__dirname, 'tests/setup-vitest.ts')],
    include: [
      'src/__tests__/**/*.{test,spec}.{ts,tsx}',
      'tests/frontend/**/*.{test,spec}.{ts,tsx}',
    ],
    exclude: ['**/node_modules/**', '**/dist/**', 'tests/e2e/**'],
    testTimeout: 15000,
  },
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'),
      'react': path.resolve(__dirname, 'node_modules/react'),
      'react-dom': path.resolve(__dirname, 'node_modules/react-dom'),
      '@tauri-apps/api': path.resolve(__dirname, 'node_modules/@tauri-apps/api'),
      'three': path.resolve(__dirname, 'node_modules/three'),
    },
  },
});
