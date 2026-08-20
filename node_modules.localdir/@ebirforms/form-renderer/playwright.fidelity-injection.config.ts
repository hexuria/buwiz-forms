import { defineConfig } from "@playwright/test";

// The criterion's own regression test. Seeds known defects into the render and
// proves official-fidelity-v1 detects each one. A criterion that cannot catch
// a deliberately injected defect is worse than no criterion, so this suite
// gates the criterion itself and must be run whenever its constants,
// primitives, or cell construction change.
export default defineConfig({
  testDir: "./visual/tools",
  testMatch: "fidelity-injection.spec.ts",
  outputDir: "../../test-results/form-renderer-fidelity-injection",
  fullyParallel: false,
  retries: 0,
  workers: 1,
  reporter: [["list"]],
  timeout: 900_000,
  use: {
    baseURL: "http://127.0.0.1:4178",
    browserName: "chromium",
    deviceScaleFactor: 1.5,
    viewport: { width: 900, height: 1400 }
  },
  webServer: {
    command:
      "npm run dev --workspace @ebirforms/form-preview -- --host 127.0.0.1 --port 4178 --strictPort",
    url: "http://127.0.0.1:4178",
    reuseExistingServer: false,
    timeout: 60_000
  }
});
