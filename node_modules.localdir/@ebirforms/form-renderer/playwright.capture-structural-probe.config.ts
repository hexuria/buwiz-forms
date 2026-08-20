import { defineConfig } from "@playwright/test";

// Capture-only companion to playwright.structural-defects.config.ts. Writes
// the blanked page rasters and a DOM geometry dump so the offset-distribution
// and global-shift analyses can run offline against fixed artifacts, instead
// of re-driving a browser for every variation. Diagnostic only; never
// promotion evidence. Uses its own port so it can run alongside the others.
export default defineConfig({
  testDir: "./visual/tools",
  testMatch: "capture-structural-probe.spec.ts",
  outputDir: "../../test-results/form-renderer-structural-probe",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  timeout: 900_000,
  use: {
    baseURL: "http://127.0.0.1:4181",
    browserName: "chromium",
    deviceScaleFactor: 1.5,
    viewport: { width: 900, height: 1400 }
  },
  webServer: {
    command:
      "npm run dev --workspace @ebirforms/form-preview -- --host 127.0.0.1 --port 4181 --strictPort",
    url: "http://127.0.0.1:4181",
    reuseExistingServer: false,
    timeout: 60_000
  }
});
