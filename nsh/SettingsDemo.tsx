// A test fixture, not a screen: the frame over the demo taxpayer (Juana Santos), for screens/settings/*.test.tsx and
// shots/settings.test.tsx. What a test asks for goes in `open(entry, { host: { params } })`:
//   demo      default true: create the demo taxpayer; false: an empty database
//   years     default true: this year with 1601C and 2551Q; false: no tax year yet
//   solo      true: choose "Just me" for the demo taxpayer (setup_solo), so the admin PIN, recovery key and setup change work
//   helpdesk  "2468": choose a help desk with this admin PIN instead (the demo taxpayer is then one of its taxpayers)
//   second    true: add a second taxpayer (generated: Sample Trading Corp, TIN 000-111-225) with the PIN 5678, so it can become the
//             owner; "nopin": the same without a PIN (it cannot be chosen yet)
//   totpSecret an authenticator key (base32) to give the admin before the app starts; the test makes the code with screens/auth/testTotp
//   pin       the admin PIN to set before the app starts ("1234"); needs solo. With a PIN the app starts at the sign-in screen,
//             so a test that wants Settings passes signin too
//   signin    true: the admin is signed in (session admin) once the frame is up; "taxpayer": the demo taxpayer is (session taxpayer)
//   jobs      true: a fake job table (a queued filing, a scheduled job, a failed one, an archived one, a completed one): delete,
//             archive, purge and cancel_queued change it, and every call is listed in the readout
//   log       "mixed" (every level), "long" (300 lines, to see the 200 cap), "empty" (no file yet), "error" (the read fails); else the real file
//   cloud     "out" (signed out; Sign in shows the gate, whose email sign-in is the fake of screens/auth/testEmail.ts), "in" (signed
//             in as juana@example.com; sign-out works), "practice" (signed in as the app-store reviewers' practice account)
//   import    "database" | "profile" | "invalid" | "cancelled" | "fail": what import_archive answers (the dialog is the host's)
//   ebir      F18, the eBIRForms import (the folder dialog is the host's): "found" (a folder with 1 taxpayer, 3 filed returns, 1 draft,
//             1 file not read; Import creates the committed fixture's taxpayer for real, so the frame's lists re-read), "standard" (the
//             same, plus a standard folder C:\eBIRForms with Scan), "empty", "wrong" (not an eBIRForms folder), "cancelled", "fail"
//             (the scan throws), "importfail" (the import throws), "slow" (the scan answers after 1.5 s), "foundonce" (the first folder dialog finds the folder,
//             every later one is dismissed). The taxpayer count shows as
//             demo-taxpayers.
//   google    true: a fake Gmail inbox for the demo taxpayer (connect, disconnect and tracking change it; the browser sign-in is not opened);
//             "unavailable": the same inbox in a build with no Google client id
//   hotkey    "refuse": set_global_hotkey is refused, as the host does for a chord it cannot register; "note": it is listed in the calls
//             readout (noise) and then done by the real host, so the box shows what was stored
//   status    "refused" | "invalid": where the desktop host says the global hotkey stands (there is no desktop host in this fixture)
//   update    true: update_status answers a real-looking status (auto download on)
//   noise     true: the fixture's fake host calls are listed (testID demo-calls) so a test sees what the page asked
//   readout   true: the app state in one line (testID demo-state): theme, recent, compact, view, params, session, cloud, admin owner, mode
//   start     the dev route ("view=settings;section=jobs")      theme  the stored theme setting ("Dark")
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { effect, host, mount, signal, Text, untracked, View } from "nsh";
import "./theme/fonts";
import { bir, type BackgroundJob, type CloudAccountStatus, type InboxState, type UpdateStatus } from "./bir";
import type { LegacyScanAnswer, LegacyScanSummary } from "./bir";
import { store } from "./shell/store";
import { fakeEmail } from "./screens/auth/testEmail";
import { createDemoTaxpayer } from "./bir/testing";
import { totpCode } from "./screens/auth/testTotp";
import { app } from "./shell/app";
import { setHotkeyStatus } from "./shell/desktop/hotkey_state";
import { Shell } from "./shell/Shell";

interface Params {
  demo?: boolean;
  years?: boolean;
  solo?: boolean;
  helpdesk?: string;
  second?: boolean | "nopin";
  pin?: string;
  totpSecret?: string;
  signin?: boolean | "taxpayer";
  google?: boolean | "unavailable";
  hotkey?: "refuse" | "note";
  readout?: boolean;
  jobs?: boolean;
  log?: "mixed" | "long" | "empty" | "error";
  cloud?: "out" | "in" | "practice";
  import?: "database" | "profile" | "invalid" | "cancelled" | "fail";
  ebir?: "found" | "standard" | "empty" | "wrong" | "cancelled" | "fail" | "importfail" | "slow" | "foundonce";
  status?: "refused" | "invalid";
  update?: boolean;
  noise?: boolean;
  waiting?: number;
  start?: string;
  theme?: string;
}

const calls = signal<string[]>([]);
function note(text: string): void {
  calls.set([...calls(), text]);
}

/** The functions the fixture replaces, as the store and the screens reach them (bir[name] at each call). */
type Swappable = Record<string, (...args: never[]) => unknown>;
function swap(name: string, make: (real: (...args: unknown[]) => unknown) => (...args: unknown[]) => unknown): void {
  const table = bir as unknown as Swappable;
  const real = table[name] as unknown as (...args: unknown[]) => unknown;
  table[name] = make(real) as never;
}

function fakeJobs(): void {
  let rows: BackgroundJob[] = [
    {
      kind: "submission", id: 41, tin: "00012345300000", form_code: "2551Q", taxable_year: 2026, quarter: 1, month: null, name: "Submit 2551Q Q1 2026 for Juana Santos",
      job_type: "System", cron_expr: null, status: "Queued", retries: 0, last_run_at: null, next_run_at: null, created_at: "2026-11-14 09:00:00", output_log: null,
    },
    {
      kind: "job", id: 1, name: "Check the inbox for BIR confirmations", job_type: "Cron", cron_expr: "*/15 * * * *", status: "Completed", retries: 0,
      last_run_at: "2026-11-15 08:45:00", next_run_at: "2026-11-15 09:00:00", created_at: "2026-11-01 08:00:00", output_log: "ok",
    },
    {
      kind: "job", id: 2, name: "Send reminder digest", job_type: "Cron", cron_expr: "0 8 * * 1", status: "Failed", retries: 3,
      last_run_at: "2026-11-10 08:00:00", next_run_at: null, created_at: "2026-11-01 08:00:00", output_log: "smtp refused",
    },
    {
      kind: "job", id: 3, name: "Old export", job_type: "Manual", cron_expr: null, status: "Archived", retries: 0,
      last_run_at: null, next_run_at: null, created_at: "2026-10-01 08:00:00", output_log: null,
    },
    {
      kind: "job", id: 4, name: "Rebuild the form index", job_type: "System", cron_expr: null, status: "Completed", retries: 0,
      last_run_at: "2026-11-12 10:00:00", next_run_at: null, created_at: "2026-11-12 10:00:00", output_log: null,
    },
  ];
  swap("list_background_jobs", () => () => rows);
  swap("delete_background_job", () => (id) => {
    note(`delete_background_job ${id}`);
    rows = rows.filter((r) => !(r.kind === "job" && r.id === id));
    return null;
  });
  swap("archive_background_job", () => (id) => {
    note(`archive_background_job ${id}`);
    rows = rows.map((r) => (r.kind === "job" && r.id === id ? { ...r, status: "Archived" } : r));
    return null;
  });
  swap("purge_archived_jobs", () => () => {
    note("purge_archived_jobs");
    rows = rows.filter((r) => r.status !== "Archived");
    return null;
  });
  swap("cancel_queued", () => (...args) => {
    note(`cancel_queued ${args.join(",")}`);
    rows = rows.filter((r) => r.kind !== "submission");
    return null;
  });
}

function logLines(kind: "mixed" | "long"): string {
  const stamp = "2026-11-15T08:00:00Z";
  if (kind === "long") return Array.from({ length: 300 }, (_, i) => `${stamp}  INFO bir::app line ${i + 1}`).join("\n") + "\n";
  return [
    `${stamp}  INFO bir::app started eBIRForms 0.2.0`,
    `${stamp}  INFO bir::db opened the database`,
    `${stamp}  WARN bir::inbox the mailbox did not answer`,
    `${stamp} ERROR bir::submit could not reach the BIR server`,
    `${stamp}  INFO bir::submit will try again in 5 minutes`,
    "a continuation line with no level",
    `${stamp} FATAL bir::db the disk is full`,
  ].join("\n") + "\n";
}

function fakeLog(kind: "mixed" | "long" | "empty" | "error"): void {
  // Each read adds a line saying which read it is, so Refresh and opening the tab are visible.
  let reads = 0;
  swap("job_log_tail", () => () => {
    reads += 1;
    if (kind === "error") throw new Error("The log file could not be read.");
    if (kind === "empty") return { file_name: "", lines: 0, content: "" };
    const content = logLines(kind) + `2026-11-15T08:00:${String(reads).padStart(2, "0")}Z  INFO bir::fixture log read ${reads}\n`;
    return { file_name: "ebirforms.2026-11-15.log", lines: content.split("\n").length - 1, content };
  });
}

const SIGNED_OUT: CloudAccountStatus = {
  server_url: "https://buwiz.example.test", server_url_source: "build", account_url: "", signed_in: false, email: null, user_id: null, offline: false, practice: false,
  detail: "Not signed in.",
};
const SIGNED_IN: CloudAccountStatus = { ...SIGNED_OUT, signed_in: true, email: "juana@example.com", user_id: "u-1", detail: "Signed in. Your account is checked now and then." };

function fakeCloud(kind: "out" | "in" | "practice"): void {
  let status = kind === "practice" ? { ...SIGNED_IN, practice: true } : kind === "in" ? SIGNED_IN : SIGNED_OUT;
  swap("cloud_account_status", () => () => status);
  swap("cloud_account_refresh", () => async () => {
    note("cloud_account_refresh");
    return status;
  });
  // The gate that Sign in shows never reaches the host's email sign-in.
  fakeEmail(bir as unknown as Record<string, unknown>, {}, note, SIGNED_IN);
  swap("cloud_account_sign_out", () => async () => {
    note("cloud_account_sign_out");
    status = SIGNED_OUT;
    return status;
  });
  swap("cloud_account_session_opened", () => () => {
    note("cloud_account_session_opened");
    return null;
  });
  swap("cloud_account_session_closed", () => () => {
    note("cloud_account_session_closed");
    return null;
  });
}

function fakeImport(kind: NonNullable<Params["import"]>): void {
  swap("import_archive", () => async () => {
    note("import_archive");
    if (kind === "fail") throw new Error("The backup's database could not be restored. The current database was left as it is.");
    return { kind };
  });
}

function fakeEbir(kind: NonNullable<Params["ebir"]>): void {
  const folder = kind === "standard" ? "C:\\eBIRForms" : "/Volumes/TRANSFER/eBIRForms";
  const found: LegacyScanSummary = {
    kind: "scanned", scan: 7, folder, taxpayers: 1, filed_returns: 3, drafts: 1,
    skipped: [{ file: "garbage.xml", reason: "invalid return filename" }],
  };
  let asked = 0;
  const answer = (): LegacyScanAnswer => {
    asked += 1;
    if (kind === "cancelled" || (kind === "foundonce" && asked > 1)) return { kind: "cancelled" };
    if (kind === "fail") throw new Error("The file dialog could not open: no window");
    if (kind === "empty") return { ...found, kind: "empty", scan: 0, taxpayers: 0, filed_returns: 0, drafts: 2, skipped: [] };
    if (kind === "wrong") return { ...found, kind: "not_ebirforms", scan: 0, taxpayers: 0, filed_returns: 0, drafts: 0, skipped: [] };
    return found;
  };
  swap("legacy_standard_folders", () => () => (kind === "standard" ? [{ index: 0, path: "C:\\eBIRForms" }] : []));
  swap("legacy_scan_folder", () => async () => {
    note("legacy_scan_folder");
    if (kind === "slow") await new Promise<void>((done) => setTimeout(done, 1500));
    return answer();
  });
  swap("legacy_scan_standard", () => (index) => {
    note(`legacy_scan_standard ${index}`);
    return answer() as LegacyScanSummary;
  });
  swap("legacy_scan_cancel", () => () => {
    note("legacy_scan_cancel");
    return null;
  });
  swap("legacy_import", () => (scan) => {
    note(`legacy_import ${scan}`);
    if (kind === "importfail") throw new Error("The folder changed since it was read. Scan it again before importing.");
    // What the committed fixture folder brings in (crates/bir-core/tests/legacy-ebirforms), made for real so the lists re-read.
    if (!bir.list_taxpayers().some((t) => t.tin_root === "123456788")) {
      bir.create_taxpayer({ tin_root: "123-456-788", name: "Juan Dela Cruz Test", taxpayer_type: "Individual", head_office: { rdo_code: "039", registered_address: "123 Test Street", zip_code: "1100", email: "legacy@example.test" } });
    }
    return { taxpayers_created: 1, taxpayers_updated: 0, taxpayers_unchanged: 0, fields_filled: 8, returns_imported: 3, returns_already_present: 0, skipped: 1 };
  });
}

function fakeUpdate(): void {
  let auto = true;
  swap("update_status", () => (): UpdateStatus => ({ phase: "idle", version: null, message: null, auto_download: auto, current_version: "0.2.0" }));
  swap("set_auto_update_download", () => (on) => {
    note(`set_auto_update_download ${on}`);
    auto = Boolean(on);
    return null;
  });
}

function fakeGoogle(available: boolean): void {
  let state: InboxState = {
    tin: "00012345300000", inbox_email: "juana@example.com", auth_method: "AppPassword", tracking_enabled: false, imap_login: "", imap_host: "imap.gmail.com",
    has_app_password: false, oauth_connected: false, google_available: available, oauth_inbox_email: null, oauth_updated_at: null, still_pending: false, matching_tins: ["00012345300000"],
    last_check_at: null, last_check_status: null, last_check_log: null,
  };
  swap("inbox_state", () => () => state);
  swap("connect_inbox_oauth", () => async () => {
    note("connect_inbox_oauth");
    state = { ...state, auth_method: "GoogleOAuth", oauth_connected: true, oauth_inbox_email: "juana.inbox@example.com", tracking_enabled: true };
    return state;
  });
  swap("disconnect_inbox_oauth", () => () => {
    note("disconnect_inbox_oauth");
    state = { ...state, oauth_connected: false, oauth_inbox_email: null, tracking_enabled: false };
    return state;
  });
  swap("set_inbox_tracking", () => (_tin, on) => {
    note(`set_inbox_tracking ${on}`);
    state = { ...state, tracking_enabled: Boolean(on) };
    return state;
  });
}

function fakeHotkeyRefusal(): void {
  swap("set_global_hotkey", () => (combo) => {
    note(`set_global_hotkey ${combo}`);
    throw new Error("That shortcut is not one this computer can register. Use a letter, digit, or F-key with modifiers, like super+shift+E.");
  });
}

function noteHotkey(): void {
  swap("set_global_hotkey", (real) => (combo) => {
    note(`set_global_hotkey ${combo}`);
    return real(combo);
  });
}

function seed(p: Params): void {
  const taxpayer = createDemoTaxpayer();
  const head = taxpayer.branches[0];
  const year = new Date().getFullYear();
  if (p.years !== false && bir.list_tax_years(head.tin).length === 0) {
    bir.add_tax_year(head.tin, year, false);
    bir.set_yearly_forms(head.tin, year, ["1601C", "2551Q"]);
  }
  if (p.second && !bir.list_taxpayers().some((t) => t.tin_root === "000111225")) {
    bir.create_taxpayer({ tin_root: "000-111-225", name: "Sample Trading Corp", taxpayer_type: "Corporation", head_office: { rdo_code: "039", registered_address: "Quezon City", zip_code: "1100", email: "sample@example.com" } });
    if (p.second !== "nopin") bir.set_taxpayer_pin("000111225", "5678");
  }
  if (bir.auth_state().mode === null) {
    if (p.helpdesk) bir.setup_helpdesk(p.helpdesk);
    else if (p.solo) bir.setup_solo(taxpayer.tin_root);
  }
  if (p.pin) bir.set_admin_pin(p.pin);
  if (p.totpSecret) bir.set_admin_totp(p.totpSecret, totpCode(p.totpSecret));
  if (p.theme) bir.set_setting("theme_preference", p.theme);
}

function Demo() {
  const p = host.params as Params;
  if (p.demo !== false) seed(p);
  if (p.jobs) fakeJobs();
  if (p.log) fakeLog(p.log);
  if (p.cloud) fakeCloud(p.cloud);
  if (typeof p.waiting === "number") {
    const count = p.waiting;
    swap("returns_waiting_count", () => () => count);
  }
  if (p.import) fakeImport(p.import);
  if (p.ebir) fakeEbir(p.ebir);
  if (p.update) fakeUpdate();
  if (p.google) fakeGoogle(p.google === true);
  if (p.hotkey === "refuse") fakeHotkeyRefusal();
  if (p.hotkey === "note") noteHotkey();
  if (p.status) setHotkeyStatus(p.status);
  if (p.signin) {
    effect(() => {
      if (!app.booted()) return;
      untracked(() => app.startSession(p.signin === "taxpayer" ? { kind: "taxpayer", root: "000123453" } : { kind: "admin" }));
    });
  }
  return (
    <View class="flex-1">
      <Shell start={p.start} polling={false} />
      <If when={p.readout}>
        <View class="absolute top-0 left-0" style={{ pointerEvents: "none" }}>
          <Text testID="demo-state">
            {[
              `theme=${app.themePref()}`,
              `recent=${app.prefs().showRecent}`,
              `compact=${app.prefs().compactFormNav}`,
              `view=${app.view()} ${JSON.stringify(app.params())}`,
              `session=${app.session().kind}`,
              `locked=${app.appLocked()}`,
              `cloudLocked=${app.cloudLocked()}`,
              `admin=${app.authState().admin_root}`,
              `mode=${app.authState().mode}`,
            ].join(" ")}
          </Text>
        </View>
      </If>
      <If when={p.ebir && p.noise}>
        <View class="absolute top-0 right-0" style={{ pointerEvents: "none" }}>
          <Text testID="demo-taxpayers">{`taxpayers=${store.taxpayers().length}`}</Text>
        </View>
      </If>
      <If when={p.noise}>
        <View class="absolute bottom-0 left-0" style={{ pointerEvents: "none" }}>
          <Text testID="demo-calls">{`calls: ${calls().join(" | ")}`}</Text>
        </View>
      </If>
    </View>
  );
}

mount(Demo);
