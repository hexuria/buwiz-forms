import { defineConfig } from "@playwright/test";

// Browserless emitter for official-fidelity-v1 component values from already
// captured page rasters. Used to (a) cross-check the TypeScript and Python
// implementations agree exactly, and (b) produce the numbers pinned as
// reviewed baselines in the Rust providers. No dev server, no rendering.
export default defineConfig({
  testDir: "./visual/tools",
  testMatch: "fidelity-baseline.spec.ts",
  outputDir: "../../test-results/form-renderer-fidelity-baseline",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  timeout: 900_000
});
