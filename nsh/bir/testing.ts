// Helpers for tests that reach the door. Tests only: a screen never imports this file.
import { Door } from "ebirforms/native";
import { create_taxpayer, list_taxpayers, screen_forms_index } from "./functions";
import type { Taxpayer } from "./functions";

/** The demo taxpayer of the repo (the only one tests and screenshots may name): Juana Santos, TIN 000-123-453-00000. */
export const DEMO = {
  name: "Juana Santos",
  /** The TIN root, with dashes as the screens type it. */
  tinRoot: "000-123-453",
  /** The head office TIN as the host functions take it (screen_form_open, queue_form_draft, ...). */
  tin: "00012345300000",
  /** The head office TIN as shown. */
  tinDisplay: "000-123-453-00000",
} as const;

/**
 * Names the database file this engine thread opens, before anything else calls the door. Test environments
 * only (`EBIR_TEST_ENV=1`, which every recipe sets). Without it a test environment opens a fresh scratch
 * database in the temp folder, so most tests need nothing. A path inside the owner's data is refused.
 */
export function useDatabase(path: string): string {
  return Door.useDatabase(path);
}

/**
 * Boots the door and builds the form schemas (the host builds all 43 on its first form call: one process-wide once, 1 to 2 s in a
 * debug build and several on a slow runner). A headless run cuts every JS call off at 2 s, a bundle's load has 10 s: call this at the
 * top level of a test file or a fixture (never inside a test or a component), so the first test does not start cold.
 */
export function warmForms(): void {
  list_taxpayers();
  screen_forms_index();
}

/** Creates the demo taxpayer (Juana Santos, an individual with a head office in Quezon City) unless it exists. */
export function createDemoTaxpayer(): Taxpayer {
  const existing = list_taxpayers().find((taxpayer) => taxpayer.tin_root === DEMO.tinRoot.replaceAll("-", ""));
  if (existing) return existing;
  return create_taxpayer({
    tin_root: DEMO.tinRoot,
    name: DEMO.name,
    taxpayer_type: "Individual",
    head_office: {
      rdo_code: "039",
      registered_address: "Quezon City",
      zip_code: "1100",
      phone: "09170000000",
      email: "juana@example.com",
    },
  });
}
