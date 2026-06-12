import { defineConfig, devices } from '@playwright/test';

export default defineConfig({
  testDir: './',
  timeout: 30 * 1000,
  expect: {
    timeout: 5000,
  },
  reporter: 'list',
  use: {
    headless: true,
  },
  projects: [
    {
      name: 'tauri-e2e',
      use: {
        ...devices['Desktop Chrome'],
      },
    },
  ],
});
