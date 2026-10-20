import { defineConfig } from 'playwright/test';

export default defineConfig({
  testDir: './tests',
  timeout: 25_000,
  use: { baseURL: 'http://127.0.0.1:1420', browserName: 'chromium', headless: true },
  webServer: {
    command: 'npm run dev -- --host 127.0.0.1',
    url: 'http://127.0.0.1:1420',
    reuseExistingServer: true,
    timeout: 20_000,
  },
});
