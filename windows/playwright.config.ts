import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./tests/ui",
  fullyParallel: true,
  // Hosted Windows runners can exhaust browser socket buffers while Vite
  // serves module graphs to concurrent fresh contexts.
  workers: process.env.CI ? 1 : 2,
  timeout: 30_000,
  reporter: "list",
  use: { baseURL: "http://127.0.0.1:1420", viewport: { width: 820, height: 780 }, screenshot: "only-on-failure" },
  webServer: {
    command: "node node_modules/vite/bin/vite.js --host 127.0.0.1 --port 1420",
    url: "http://127.0.0.1:1420/settings.html",
    reuseExistingServer: !process.env.CI,
  },
});
