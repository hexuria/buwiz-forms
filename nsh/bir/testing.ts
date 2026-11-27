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

/** The canned TIN-list variants of the tins fixtures (D2 tests). */
export type FakeTinsKind = "activated" | "pending" | "fix" | "not_activated" | "test" | "stale" | "multi" | "incomplete" | "error";

/** The refusal fake activate_tin throws (the host's own sentences), or "ok" for the added row. */
export type FakeActivateKind = "ok" | "bad_tin" | "name_missing" | "name_too_long" | "bad_request" | "forbidden" | "test_tin" | "tin_already_active" | "tin_already_added" | "claim_already_pending" | "too_many_unverified" | "rate_limited" | "other";

/** Every refusal sentence the host words, keyed by the fixture's name for it. */
export const FAKE_REFUSALS: Record<Exclude<FakeActivateKind, "ok">, string> = {
  bad_tin: "This TIN is not valid.",
  name_missing: "Add the registered name first.",
  name_too_long: "This name is too long. Shorten it and try again.",
  bad_request: "Buwiz could not read this request. Update eBIRForms and try again.",
  forbidden: "This sign-in cannot add TINs. Sign out of Buwiz and sign in again.",
  test_tin: "Test TINs are added by Buwiz.",
  tin_already_active: "This TIN is already activated on your account.",
  tin_already_added: "This TIN is already on your account, waiting to be verified.",
  claim_already_pending: "Buwiz is checking this TIN.",
  too_many_unverified: "Your account has 20 TINs waiting to be verified. Verify or remove one on your account first.",
  rate_limited: "Too many tries. Try again in 12 minutes.",
  other: "The Buwiz server could not add this TIN (fizzy).",
};

/**
 * Fakes the account's TIN list and the Activate and queue verbs on the bir module object of a fixture (D2). The demo
 * TIN's head office is the row; "multi" adds the peer's. Every call is read out (`account_tins(0)`, `activate_tin(...)`,
 * `queue_form_draft(...)`, `screen_form_queue(...)`) so a test proves what the host was asked, and with which max_age.
 */
export function installFakeTins(
  bir: Record<string, unknown>,
  kind: FakeTinsKind,
  activateKind: FakeActivateKind | undefined,
  readout: (line: string) => void,
  opts: { errorAfter?: number; ownerFlipAfter?: number; refusalText?: string } = {},
): void {
  const now = Math.floor(Date.now() / 1000);
  const root = DEMO.tinRoot.replaceAll("-", "");
  const base = { tin_root: root, branch: "00000", action_url: null as string | null, test_only: false, stale: false, fetched_at: now };
  const rows = () => {
    if (kind === "activated") return [{ ...base, state: "activated", message: null }];
    if (kind === "pending") return [{ ...base, state: "pending", message: "Buwiz is verifying this TIN. It usually takes a day." }];
    if (kind === "fix") return [{ ...base, state: "fix", message: "The registered name does not match. Check it on your account.", action_url: "https://account.buwiz.example.test/tins/fix/1" }];
    if (kind === "not_activated") return [{ ...base, state: "not_activated", message: null }];
    if (kind === "test") return [{ ...base, state: "activated", message: null, test_only: true }];
    if (kind === "multi") return [
      { ...base, state: "activated", message: null },
      { ...base, tin_root: "000111233", branch: "00000", state: "pending", message: "Buwiz is verifying this TIN." },
    ];
    if (kind === "stale") return [{ ...base, state: "activated", message: null, stale: true, fetched_at: now - 86400 * 3 }];
    return [];
  };
  // The sign-in the list belongs to: owner A, or owner B once ownerFlipAfter reads are behind (the account-switch test).
  // Both are signed in: the chips only make sense signed in, and the switch is what must clear the kept list.
  let reads = 0;
  const ownerId = () => (opts.ownerFlipAfter !== undefined && reads > opts.ownerFlipAfter ? "owner-b" : "owner-a");
  bir.cloud_account_status = () => ({
    server_url: "http://localhost:3008",
    server_url_source: "build",
    account_url: "",
    signed_in: true,
    email: `${ownerId()}@example.com`,
    user_id: ownerId(),
    offline: false,
    practice: false,
    detail: "",
  });
  bir.account_tins = async (maxAge: number) => {
    readout(`account_tins(${maxAge})`);
    reads += 1;
    if (kind === "error" || (opts.errorAfter !== undefined && reads > opts.errorAfter)) throw new Error("Sign in to Buwiz to see your TINs.");
    // An incomplete list still carries the rows it has: completeness is its own fact, and the gate must read it.
    if (kind === "incomplete") return { rows: [{ ...base, state: "not_activated", message: null }], complete: false, stale: false, fetched_at: now };
    return { rows: rows(), complete: true, stale: kind === "stale", fetched_at: now };
  };
  bir.activate_tin = async (tinFull: string) => {
    readout(`activate_tin(${tinFull})`);
    if (opts.refusalText) throw new Error(opts.refusalText);
    if (activateKind && activateKind !== "ok") throw new Error(FAKE_REFUSALS[activateKind]);
    return { ...base, tin_root: tinFull.slice(0, 9), branch: tinFull.slice(9), state: "pending", message: "Buwiz is verifying this TIN." };
  };
  bir.queue_form_draft = (...args: unknown[]) => {
    readout(`queue_form_draft(${String(args[0])})`);
    return null;
  };
  bir.screen_form_queue = (...args: unknown[]) => {
    readout(`screen_form_queue(${String(args[0])})`);
    return null;
  };
}
