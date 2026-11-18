import { defineConfig } from '@playwright/test';
// Native Git process startup on Windows runners is materially slower. Keep
// assertions and one-worker execution intact while allowing reviewed reads to finish.
const windows=process.platform==='win32';
export default defineConfig({
  testDir: './e2e', timeout: windows ? 180000 : 60000, expect: { timeout: windows ? 45000 : 15000 }, fullyParallel: false, workers: 1,
  use: { baseURL: 'http://127.0.0.1:1420', viewport: { width: 1360, height: 900 }, trace: 'retain-on-failure' },
  webServer: { command: 'npm run dev', url: 'http://127.0.0.1:1420', reuseExistingServer: !process.env.CI },
});
