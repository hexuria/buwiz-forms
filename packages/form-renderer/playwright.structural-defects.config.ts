import { defineConfig } from "@playwright/test";

// Localizes structural defects to a bounding box and an offset, so a finding
// like "1130px unmatched" becomes "these three rules sit 2 device px low".
// Text is neutralized on both sides, because text is unwinnable against a
// font-substituted reference and would otherwise dominate the structural
// stratum. Diagnostic only; never promotion evidence.
export default defineConfig({
  testDir: "./visual/tools",
  testMatch: "structural-defects.spec.ts",
  outputDir: "../../test-results/form-renderer-structural-defects",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  timeout: 900_000,
  use: {
    baseURL: "http://127.0.0.1:4179",
    browserName: "chromium",
    deviceScaleFactor: 1.5,
    viewport: { width: 900, height: 1400 }
  },
  webServer: {
    command:
      "npm run dev --workspace @ebirforms/form-preview -- --host 127.0.0.1 --port 4179 --strictPort",
    url: "http://127.0.0.1:4179",
    reuseExistingServer: false,
    timeout: 60_000
  }
});
