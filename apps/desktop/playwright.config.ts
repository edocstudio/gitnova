import { defineConfig } from '@playwright/test';
export default defineConfig({
  testDir: './e2e', timeout: 60000, expect: { timeout: 15000 }, fullyParallel: false, workers: 1,
  use: { baseURL: 'http://127.0.0.1:1420', viewport: { width: 1360, height: 900 }, trace: 'retain-on-failure' },
  webServer: { command: 'npm run dev', url: 'http://127.0.0.1:1420', reuseExistingServer: !process.env.CI },
});
