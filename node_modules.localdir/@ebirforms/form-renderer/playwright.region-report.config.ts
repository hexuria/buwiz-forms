import { defineConfig } from "@playwright/test";

// Browserless replay of the region-ranked diff report from the last captured
// visual run. No webServer and no page rendering: it re-slices the recorded
// actual screenshots against the pinned references in seconds, so calibration
// can re-rank regions without paying for a full browser run.
export default defineConfig({
  testDir: "./visual/tools",
  testMatch: "region-report-replay.spec.ts",
  outputDir: "../../test-results/form-renderer-region-replay",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]]
});
