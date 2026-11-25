const { defineConfig } = require('@playwright/test');

module.exports = defineConfig({
  testDir: '.',
  testMatch: '**/*.spec.cjs',
  outputDir: '../../target/dashboard-playwright/results',
  reporter: 'line',
  workers: 1,
  timeout: 90000,
  use: {
    browserName: 'chromium',
    viewport: { width: 1440, height: 980 },
    launchOptions: process.env.XRAY_CHROMIUM_BINARY
      ? { executablePath: process.env.XRAY_CHROMIUM_BINARY }
      : {},
  },
});
