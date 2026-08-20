import { defineConfig } from "@playwright/test";

// Audits every comb field's declared cell count against the official form's
// interior dividers. A capacity error is a promise to the taxpayer about how
// many characters fit, and it is nearly invisible to the pixel metrics: thin
// guides move the complete-page number by a fraction of a percent while being
// plainly wrong on paper. Diagnostic only; its output is a review list.
export default defineConfig({
  testDir: "./visual/tools",
  testMatch: "generate-region-table.spec.ts",
  outputDir: "../../test-results/form-renderer-region-table",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  timeout: 900_000,
  use: {
    baseURL: "http://127.0.0.1:4182",
    browserName: "chromium",
    deviceScaleFactor: 1.5,
    viewport: { width: 900, height: 1400 }
  },
  webServer: {
    command:
      "npm run dev --workspace @ebirforms/form-preview -- --host 127.0.0.1 --port 4182 --strictPort",
    url: "http://127.0.0.1:4182",
    reuseExistingServer: false,
    timeout: 60_000
  }
});
