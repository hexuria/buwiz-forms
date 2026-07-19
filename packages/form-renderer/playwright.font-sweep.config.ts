import { defineConfig } from "@playwright/test";

// Font-attribution sweep: re-renders one form under candidate font stacks and
// diffs each candidate against BOTH pinned rasters (chromium gate reference
// and Poppler diagnostic), with region-ranked output per candidate. Purely a
// calibration diagnostic; it never produces promotion evidence.
export default defineConfig({
  testDir: "./visual/tools",
  testMatch: "font-sweep.spec.ts",
  outputDir: "../../test-results/form-renderer-font-sweep",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  timeout: 180_000,
  use: {
    baseURL: "http://127.0.0.1:4176",
    browserName: "chromium",
    deviceScaleFactor: 1.5,
    viewport: { width: 900, height: 1400 }
  },
  webServer: {
    command:
      "npm run dev --workspace @ebirforms/form-preview -- --host 127.0.0.1 --port 4176 --strictPort",
    url: "http://127.0.0.1:4176",
    reuseExistingServer: false,
    timeout: 30_000
  }
});
