import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./visual",
  outputDir: "../../test-results/form-renderer-1702mx",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  use: {
    baseURL: "http://127.0.0.1:4184",
    browserName: "chromium",
    deviceScaleFactor: 1.5,
    viewport: { width: 900, height: 1400 }
  },
  webServer: {
    command:
      "npm run dev --workspace @ebirforms/form-preview -- --host 127.0.0.1 --port 4184 --strictPort",
    url: "http://127.0.0.1:4184",
    reuseExistingServer: false,
    timeout: 30_000
  }
});
