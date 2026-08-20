import { defineConfig } from "@playwright/test";

// Response-curve sweeps for the three provisional official-fidelity-v1
// constants (criterion section 8.3). Browserless; reads captured rasters.
export default defineConfig({
  testDir: "./visual/tools",
  testMatch: "fidelity-constant-sweep.spec.ts",
  outputDir: "../../test-results/form-renderer-constant-sweep",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  timeout: 900_000
});
