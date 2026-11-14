// The app state (old app/main.js BirApp): one module, private signals, one owner per piece of state. Screens read the
// accessors and call the actions; nothing outside this file sets a signal.
//
//   navigation   route (view + params), history and forward stacks, go() back() forward() openReturn() openProfile()
//   layers       overlay (palette | new), panel (profile | notifs), sidebar, the palette's query and row
//   selection    the taxpayer, branch and tax year the frame is "on" (persisted)
//   session      open | admin | taxpayer; this wave always open: the seam for wave 4's locks
//   persistence  redesign.recent, redesign.selection, redesign.read_notifications (each suffixed @<database path>) and
//                redesign.prefs, in NSH Storage; the theme through bir get_setting / set_setting
//   polls        events 30 s, device filings 5 s, update status 1 s: startPolling(), owned by the root
import type { Accessor, Signal } from "nsh";
import { batch, computed, setColorScheme, signal, Storage, windowSize } from "nsh";
import { bir, doorStatus, type Branch, type ReturnRow, type Taxpayer, type UpdateStatus } from "../bir";
import { shellChromeVisible } from "../lib/chrome_gate";
import { errorText } from "../lib/errors";
import { POLL, startPolls } from "../lib/poll";
import { addRecent, bumpRecent, loadRecent, recentEntry, recentOpenOptions, type RecentEntry, type RecentInput } from "../lib/recent";
import type { FormTarget } from "../screens/form/FormPage";
import { BREAK } from "../theme/tokens";
import { ICONS, toast as kitToast, type IconName } from "../ui";
import { eventToast, takeFresh } from "./events";
import { isViewName, ROUTES, type Route, type ViewName, type ViewParams } from "./routes";
import { parseStart, parseStartForm } from "./start";
import { store } from "./store";
import { nextThemePreference, parseThemePreference, storedThemePreference, THEME_PREF_KEY, themePreferenceFromStart, type ThemePreference } from "./theme";

// ---------------------------------------------------------------- types

/** Who is at the keyboard. "open": nobody signed in (the admin has no code, so the owner acts as the admin). */
export type Session = { kind: "open" } | { kind: "admin" } | { kind: "taxpayer"; root: string };
/** The right panel's two contents. */
export type PanelName = "profile" | "notifs";
/** The dialogs the frame owns. */
export type OverlayName = "palette" | "new";
/** What is kept in redesign.prefs. */
export interface Prefs {
  /** The sidebar lists the last returns opened. */
  showRecent: boolean;
  /** The form page's part navigator is compact. */
  compactFormNav: boolean;
}
/** One entry of the notifications panel: a submission or a BIR confirmation, newest first. */
export interface Notification {
  id: string;
  title: string;
  sub: string;
  icon: IconName;
  tone: "success" | "neutral";
  /** A SQLite timestamp (UTC). */
  when: string;
  unread: boolean;
}
/** What openReturn takes: the return's identity, and how the form page should open it. */
export interface OpenReturnOptions {
  /** The branch TIN as host functions take it (14 digits, no dashes). */
  tin: string;
  code: string;
  year: number;
  /** Month, quarter, or 0 for an annual return. */
  period: number;
  /** The stored return's exact key (a transaction's MMDD, 2553's quarter); text from an older Recent entry is read as a number. */
  returnKey?: number | string | null;
  /** Default "return"; "sample" for a Form library sample (never a recent return). */
  mode?: FormTarget["mode"];
  snapshotId?: number;
  newKey?: number | null;
  newKeyFrom?: string | null;
}

const KEYS = {
  recent: "redesign.recent",
  selection: "redesign.selection",
  read: "redesign.read_notifications",
  prefs: "redesign.prefs",
} as const;

/** Why nothing can be opened yet. */
export const FRESH_HINT = "Add a taxpayer first, then open a form for them.";

/**
 * Wave 4 (first-run setup, D2) sets this to true: with no access mode chosen the chrome stays hidden. Until then a new
 * database opens straight into the app, as the dev builds do.
 */
const FIRST_RUN_GATE = false;

// ---------------------------------------------------------------- private state

const booted = signal(false);
const route = signal<Route>({ view: "home", params: {}, seq: 0 });
const history = signal<Route[]>([]);
const forward = signal<Route[]>([]);
let seq = 0;

const sidebarOpen = signal(true);
const panel = signal<PanelName | null>(null);
const lastPanel = signal<PanelName>("profile");
const overlay = signal<OverlayName | null>(null);
const newReturn = signal<{ code: string | null; period: number | null }>({ code: null, period: null });
const paletteQuery = signal("");
const paletteIndex = signal(0);

const selRoot = signal<string | null>(null);
const selCode = signal<string | null>(null);
const selYear = signal<number | null>(null);
const recent = signal<RecentEntry[]>([]);
const readIds = signal<ReadonlySet<string>>(new Set());
let knownEvents = new Set<string>();
const prefs = signal<Prefs>({ showRecent: true, compactFormNav: false });
const themePref = signal<ThemePreference>("system");

const session = signal<Session>({ kind: "open" });
const appLocked = signal(false);
const cloudLocked = signal(false);
const setup = signal<{ pending: string | null; step: string }>({ pending: null, step: "choose" });
const updateStatus = signal<UpdateStatus | null>(null);
let lockedFrom: Session | null = null;
let databasePath = "";
let shellChromeSent: boolean | null = null;
let bootPromise: Promise<void> | null = null;

/** A read-only view of a signal: the accessor without its setters. */
function readOnly<T>(s: Signal<T>): Accessor<T> {
  return () => s();
}

// ---------------------------------------------------------------- persistence (NSH Storage)

/** Selection, Recent and read notifications name taxpayers, so they are kept per database: a scratch database must not inherit another's. */
function localKey(key: string): string {
  return key === KEYS.prefs ? key : `${key}@${databasePath}`;
}

async function readLocal<T>(key: string, fallback: T): Promise<T> {
  try {
    const raw = await Storage.getItem(localKey(key));
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function writeLocal(key: string, value: unknown): void {
  Storage.setItem(localKey(key), JSON.stringify(value)).catch(() => {});
}

// ---------------------------------------------------------------- layout (from the window's width)

/** Under 600: the narrow layout (no sidebar, a bottom tab bar). */
const narrow = computed(() => windowSize().width < BREAK.phone);
/** Under 860: the sidebar and the panel float over the page. */
const overlayMode = computed(() => windowSize().width < BREAK.overlays);
const sidebarDocked = computed(() => sidebarOpen() && !overlayMode());
const panelDocked = computed(() => panel() !== null && !overlayMode());
/** The main surface's width: the window less what is docked beside it and the inset. */
const mainWidth = computed(() => {
  const left = sidebarDocked() ? 248 : 8;
  const right = panelDocked() ? 320 : 8;
  return Math.max(240, windowSize().width - left - right - 2);
});

// ---------------------------------------------------------------- auth (the seam for wave 4)

const authState = () => store.authState();
const isHelpdesk = () => authState().mode === "helpdesk";
const adminHasCode = () => Boolean(authState().has_admin_pin || authState().has_admin_totp);
/** Admin rights: signed in as the admin, or nobody while the admin has no code (the owner is at the keyboard). */
const isAdmin = () => session().kind === "admin" || (session().kind === "open" && !adminHasCode());
const canAddTaxpayer = () => !store.taxpayers().length || (isAdmin() && isHelpdesk());

/** Toolbar, sidebar, taxpayer picker and filing menus: false on the account gate, first launch and the sign-in screen. */
const chromeVisible = computed(
  () =>
    booted() &&
    shellChromeVisible({
      booted: true,
      cloudLocked: cloudLocked(),
      appLocked: appLocked(),
      mode: authState().mode ?? (FIRST_RUN_GATE ? null : "solo"),
      setupPending: setup().pending,
      setupStep: setup().step,
    }),
);

function applyScope(): void {
  const s = session();
  store.setScope(s.kind === "taxpayer" ? s.root : null);
}

// ---------------------------------------------------------------- selection

const taxpayer = computed<Taxpayer | null>(() => {
  const list = store.taxpayers();
  return list.find((t) => t.tin_root === selRoot()) ?? list[0] ?? null;
});
const branch = computed<Branch | null>(() => {
  const t = taxpayer();
  if (!t) return null;
  return t.branches.find((b) => b.code === selCode()) ?? t.branches[0] ?? null;
});
const isFresh = () => taxpayer() === null;
const years = computed(() => {
  const b = branch();
  return b ? store.taxYears(b.tin) : [];
});
/** The selected year, kept valid: this year if the branch has it, else its latest. */
const year = computed<number | null>(() => {
  const ys = years();
  const picked = selYear();
  if (picked !== null && ys.includes(picked)) return picked;
  const now = new Date().getFullYear();
  return ys.includes(now) ? now : ys.length ? ys[ys.length - 1] : null;
});
const yearForms = computed(() => {
  const b = branch();
  const y = year();
  return b && y ? store.yearlyForms(b.tin, y) : [];
});
const templates = computed(() => {
  const b = branch();
  return b ? store.templates(b.tin) : [];
});

/** The branch a full TIN names in this database, if any. */
function branchFor(tin: string): Branch | null {
  const t = store.taxpayers().find((x) => x.tin_root === String(tin).slice(0, 9));
  return t ? (t.branches.find((b) => b.tin === String(tin)) ?? null) : null;
}

function selectTaxpayer(root: string, opts: { quiet?: boolean; code?: string } = {}): void {
  const t = store.taxpayers().find((x) => x.tin_root === root);
  if (!t) return;
  batch(() => {
    selRoot.set(root);
    selCode.set(opts.code && t.branches.some((b) => b.code === opts.code) ? opts.code : (t.branches[0]?.code ?? null));
    selYear.set(null);
  });
  saveSelection();
  if (!opts.quiet) toast(`Switched to ${t.name}`);
}

function selectBranch(code: string): void {
  batch(() => {
    selCode.set(code);
    selYear.set(null);
  });
  saveSelection();
}

/** Keeps the taxpayer and branch the frame is on (the effective ones: nothing chosen yet means the first taxpayer). */
function saveSelection(): void {
  writeLocal(KEYS.selection, { root: taxpayer()?.tin_root ?? null, code: branch()?.code ?? null });
}

function selectYear(y: number): void {
  selYear.set(y);
}

// ---------------------------------------------------------------- toasts

function iconOf(name: string | undefined): IconName | undefined {
  return name && name in ICONS ? (name as IconName) : undefined;
}

/** A toast at the bottom (the Toaster mounted by the frame). `icon`: an icon name of ui/icons; others show the default. */
export function toast(message: string, icon?: string): string {
  return kitToast(message, { icon: iconOf(icon) });
}

// ---------------------------------------------------------------- navigation

/** The view the frame shows: a view that needs a taxpayer is Home while none exists. */
const view = computed<ViewName>(() => {
  const v = route().view;
  return ROUTES[v].needsTaxpayer && isFresh() ? "home" : v;
});

function sameParams(a: ViewParams, b: ViewParams): boolean {
  return JSON.stringify(a) === JSON.stringify(b);
}

/** Moving anywhere closes the layers that float over the page (the sidebar and panel below 860 points). */
function closeTransient(): void {
  if (overlayMode()) {
    batch(() => {
      sidebarOpen.set(false);
      panel.set(null);
    });
  }
}

/**
 * Opens a view. Views that need a taxpayer fall back to Home when none exists; dev views only open in the dev
 * environment. Moving to another view (or to the same view with other params) pushes the history and clears Forward.
 */
export function go(name: ViewName | "cron_tasks", params: ViewParams = {}): void {
  let target: ViewName;
  if (name === "cron_tasks") {
    // Background tasks live in Settings > Jobs.
    target = "settings";
    params = { ...params, section: "jobs" };
  } else {
    target = name;
  }
  const meta = ROUTES[target];
  if ((meta.needsTaxpayer && isFresh()) || (meta.dev && !doorStatus().testEnvironment)) {
    target = "home";
    params = {};
  }
  const cur = route();
  if (target !== cur.view || !sameParams(cur.params, params)) {
    batch(() => {
      history.set([...history().slice(-49), cur]);
      forward.set([]);
      route.set({ view: target, params, seq: ++seq });
    });
  }
  closeTransient();
}

/** Shows another view in place of the current one: no history entry (a switch between two views of one page, a redirect). */
export function replace(name: ViewName, params: ViewParams = {}): void {
  const meta = ROUTES[name];
  if ((meta.needsTaxpayer && isFresh()) || (meta.dev && !doorStatus().testEnvironment)) return;
  route.set({ view: name, params, seq: ++seq });
  closeTransient();
}

/** Back one step. False when there is none. */
export function back(): boolean {
  const h = history();
  if (!h.length) return false;
  batch(() => {
    forward.set([...forward(), route()]);
    history.set(h.slice(0, -1));
    route.set({ ...h[h.length - 1], seq: ++seq });
  });
  closeTransient();
  return true;
}

/** Forward one step. False when there is none. */
export function forwardOne(): boolean {
  const f = forward();
  if (!f.length) return false;
  batch(() => {
    history.set([...history(), route()]);
    forward.set(f.slice(0, -1));
    route.set({ ...f[f.length - 1], seq: ++seq });
  });
  closeTransient();
  return true;
}

/** The form page shows another return than the one it opened (a sample became its return): the route follows, the page stays. */
function retarget(target: FormTarget): void {
  const cur = route();
  if (cur.view === "form") route.set({ ...cur, params: { target } });
}

export function openProfile(editSection?: string): void {
  go("profile", editSection ? { edit: editSection } : {});
}

/** Search picked a taxpayer: it opens on its Tax profile, wherever search was opened from. */
export function openTaxpayer(root: string): void {
  if (taxpayer()?.tin_root !== root) selectTaxpayer(root);
  go("profile");
}

/** Search picked a branch: select it and open the Tax profile, as for a taxpayer. */
export function openBranch(root: string, code: string): void {
  if (taxpayer()?.tin_root === root) selectBranch(code);
  else selectTaxpayer(root, { code });
  go("profile");
}

// ---------------------------------------------------------------- recent returns

function pushRecent(opts: RecentInput): void {
  const next = addRecent(recent(), opts);
  if (next === recent()) return;
  recent.set(next);
  writeLocal(KEYS.recent, next);
}

/** A return was changed and saved: it moves to the top of Recent. */
function bumpRecentReturn(opts: RecentInput): void {
  const next = bumpRecent(recent(), opts);
  recent.set(next);
  writeLocal(KEYS.recent, next);
}

/** Rewrite Recent after a TIN or branch-code move, so the entries still open. */
function remapRecent(map: (tin: string) => string): void {
  const next = recent().map((r) => recentEntry({ ...r, tin: map(String(r.tin)) }));
  recent.set(next);
  writeLocal(KEYS.recent, next);
}

/** Opens a return on the form page: selects its taxpayer and branch (the context pill names whose return it is). */
export function openReturn(opts: OpenReturnOptions): void {
  if (isFresh()) {
    toast(FRESH_HINT);
    return;
  }
  const owner = branchFor(opts.tin);
  if (!owner) {
    toast("That return belongs to a taxpayer that is not in this database.", "triangle-alert");
    return;
  }
  if (branch()?.tin !== owner.tin) selectTaxpayer(String(opts.tin).slice(0, 9), { quiet: true, code: owner.code });
  overlay.set(null);
  const mode = opts.mode ?? "return";
  // A library sample is no return, and a filed original is a snapshot: neither is a recent return.
  if (mode === "return" || mode === "amend") {
    pushRecent({ tin: opts.tin, code: opts.code, year: opts.year, period: opts.period, returnKey: opts.returnKey ?? null });
  }
  const target: FormTarget = {
    tin: opts.tin,
    formCode: opts.code,
    year: opts.year,
    period: opts.period,
    key: opts.returnKey == null || Number.isNaN(Number(opts.returnKey)) ? null : Number(opts.returnKey),
    mode,
    snapshotId: opts.snapshotId,
    newKey: opts.newKey,
    newKeyFrom: opts.newKeyFrom,
  };
  go("form", { target });
}

// ---------------------------------------------------------------- layers

function toggleSidebar(): void {
  if (!chromeVisible() || narrow()) return;
  batch(() => {
    const open = !sidebarOpen();
    sidebarOpen.set(open);
    if (open && overlayMode()) panel.set(null);
  });
}

/** A panel closes from its own trigger (the pill or the bell) and Esc. */
function togglePanel(which: PanelName): void {
  if (!chromeVisible()) return;
  batch(() => {
    const next = panel() === which ? null : which;
    panel.set(next);
    if (next) {
      lastPanel.set(next);
      if (overlayMode()) sidebarOpen.set(false);
    }
  });
}

/** The title-bar panel button: hide the panel, or reopen the last one. */
function toggleRightPanel(): void {
  togglePanel(panel() ?? lastPanel());
}

function closePanel(): void {
  panel.set(null);
}

/**
 * The window crossed 860 points (or just opened): below it the sidebar and the panel float over the page, so they start
 * closed there; back above it the sidebar docks open again. The root calls it from an effect on layout.overlayMode.
 */
let wasOverlay: boolean | null = null;
function followWidth(overlays: boolean): void {
  const before = wasOverlay;
  wasOverlay = overlays;
  if (overlays && before !== true) {
    batch(() => {
      sidebarOpen.set(false);
      if (before === false) panel.set(null);
    });
  } else if (!overlays && before === true) {
    sidebarOpen.set(true);
  }
}

/** The scrim's press: the sidebar and the panel that float over the page both close. */
function closeFloating(): void {
  batch(() => {
    sidebarOpen.set(false);
    panel.set(null);
  });
}

function openPalette(): void {
  if (!chromeVisible()) return;
  batch(() => {
    overlay.set("palette");
    paletteIndex.set(0);
    paletteQuery.set("");
  });
}

function closeOverlay(): void {
  overlay.set(null);
}

/** Esc: the topmost layer closes (a dialog, then the panel, then the floating sidebar). True when something closed. */
function dismiss(): boolean {
  if (overlay()) {
    closeOverlay();
    return true;
  }
  if (panel()) {
    panel.set(null);
    return true;
  }
  if (overlayMode() && sidebarOpen()) {
    sidebarOpen.set(false);
    return true;
  }
  return false;
}

/** The title-bar and sidebar "+": the start-a-return dialog for the branch's year, or why it cannot open. */
function startNewReturn(code?: string): void {
  if (!chromeVisible()) return;
  if (isFresh()) return void toast(FRESH_HINT);
  const b = branch();
  const t = taxpayer();
  if (!b || !t) return;
  if (!years().length) return void toast(`${b.name} has no tax year yet. Add one to choose the forms it files.`);
  const forms = yearForms();
  if (!forms.length) return void toast(`No forms for ${year()} yet. Choose them under Forms first.`);
  if (code && !forms.includes(code)) return void toast(`${t.name} does not file ${code} in ${year()}. Add it under Forms first.`);
  batch(() => {
    newReturn.set({ code: code || forms[0], period: null });
    overlay.set("new");
  });
}

function pickNewReturnForm(code: string): void {
  newReturn.set({ code, period: null });
}

function pickNewReturnPeriod(period: number): void {
  newReturn.update((n) => ({ ...n, period }));
}

function setPaletteQuery(text: string): void {
  batch(() => {
    paletteQuery.set(text);
    paletteIndex.set(0);
  });
}

function setPaletteIndex(i: number): void {
  paletteIndex.set(i);
}

// ---------------------------------------------------------------- theme and prefs

function setThemePreference(pref: ThemePreference): void {
  themePref.set(pref);
  try {
    store.writeFor(`setting:${THEME_PREF_KEY}`, "set_setting", THEME_PREF_KEY, storedThemePreference(pref));
  } catch {
    /* the preference still applies for this run */
  }
  setColorScheme(pref);
}

function cycleTheme(): void {
  setThemePreference(nextThemePreference(themePref()));
}

function setPref<K extends keyof Prefs>(key: K, value: Prefs[K]): void {
  const next = { ...prefs(), [key]: value };
  prefs.set(next);
  writeLocal(KEYS.prefs, next);
}

// ---------------------------------------------------------------- notifications

/** Submissions sent and confirmation emails received, newest first: the return's own status is the record. */
const notifications = computed<Notification[]>(() => {
  const seen = readIds();
  const items: Notification[] = [];
  for (const r of store.returns("filed") as ReturnRow[]) {
    const confirmed = r.status === "Confirmed" || r.status === "Paid";
    if (!confirmed && r.status !== "Submitted") continue;
    const id = `return:${r.id}:${confirmed ? "confirmed" : "submitted"}`;
    const who = r.branch_count > 1 ? `${r.taxpayer_name} · ${r.branch_name}` : r.taxpayer_name;
    items.push({
      id,
      title: confirmed ? `BIR confirmed ${r.form_code} for ${r.period_label}` : `${r.form_code} for ${r.period_label} was submitted`,
      sub: confirmed ? `Confirmation email received · ${who}` : `Waiting for BIR's confirmation email · ${who}`,
      icon: confirmed ? "circle-check" : "send",
      tone: confirmed ? "success" : "neutral",
      when: r.updated_at,
      unread: !seen.has(id),
    });
  }
  return items;
});
const unreadCount = computed(() => notifications().filter((n) => n.unread).length);

function markAllRead(): void {
  const next = new Set(readIds());
  for (const n of notifications()) next.add(n.id);
  readIds.set(next);
  writeLocal(KEYS.read, [...next]);
}

/** A notification is about a filed return: read it, and show Filings. */
function openNotification(id: string): void {
  const next = new Set(readIds());
  next.add(id);
  readIds.set(next);
  writeLocal(KEYS.read, [...next]);
  go("filings");
}

// ---------------------------------------------------------------- polls

/**
 * Re-reads what the submission worker changes behind this app's back (returns, their deadlines and yearly form rows,
 * background jobs) and announces events that appeared since the last look.
 */
function pollEvents(): void {
  if (cloudLocked()) return;
  store.forgetWhere(/^(returns:|deadlines|forms:|jobs$)/);
  const said = eventToast(takeFresh(knownEvents, notifications()));
  if (said) toast(said.message, said.icon);
}

/** An assistant queued a filing on the account server: this computer uploads it. A missing grant leaves the row waiting. */
function pullDeviceFilings(): void {
  if (cloudLocked()) return;
  let note: { message: string; icon: string } | null = null;
  try {
    note = bir.pull_device_filings();
  } catch {
    return;
  }
  if (note?.message) toast(note.message, note.icon || "send");
}

/** The host checks for a release on its own thread; polling is how the prompt appears. */
function watchUpdate(): void {
  if (cloudLocked()) return;
  let next: UpdateStatus;
  try {
    next = bir.update_status();
  } catch {
    return;
  }
  const prev = updateStatus();
  if (prev && prev.phase === next.phase && prev.version === next.version && prev.message === next.message && prev.auto_download === next.auto_download) return;
  updateStatus.set(next);
}

/** Starts the three polls; the root calls it on mount and the returned function on unmount. */
export function startPolling(): () => void {
  return startPolls(
    [
      { name: "events", every: POLL.events, run: pollEvents },
      { name: "device-filings", every: POLL.deviceFilings, run: pullDeviceFilings },
      { name: "update", every: POLL.update, first: 500, run: watchUpdate },
    ],
    (name, error) => store.logScreen("warn", `poll ${name}: ${errorText(error)}`),
  );
}

// ---------------------------------------------------------------- session (the seam for wave 4)

/** Nobody can sign back in without a code: with no admin code a taxpayer-less session cannot lock. */
function endSession(message?: string): void {
  const s = session();
  if (s.kind !== "taxpayer" && !adminHasCode()) {
    toast("Set your PIN in Settings first. With no PIN there is nothing to sign back in with.", "lock");
    return;
  }
  lockedFrom = s;
  batch(() => {
    session.set({ kind: "open" });
    panel.set(null);
    overlay.set(null);
    appLocked.set(true);
  });
  store.bump();
  applyScope();
  if (message) toast(message, "log-out");
}

/** Sign-in succeeded (wave 4's sign-in screens call it): a different person starts from Home, with nothing of the last session. */
function startSession(next: Session): void {
  const same = lockedFrom !== null && lockedFrom.kind === next.kind && (lockedFrom as { root?: string }).root === (next as { root?: string }).root;
  batch(() => {
    session.set(next);
    appLocked.set(false);
  });
  lockedFrom = null;
  store.bump();
  applyScope();
  if (next.kind === "taxpayer") {
    const t = store.taxpayers()[0];
    batch(() => {
      selRoot.set(next.root);
      if (!same || !t || !t.branches.some((b) => b.code === selCode())) selCode.set(t?.branches[0]?.code ?? null);
    });
    saveSelection();
  }
  if (!same) {
    batch(() => {
      history.set([]);
      forward.set([]);
      route.set({ view: "home", params: {}, seq: ++seq });
      panel.set(null);
      overlay.set(null);
    });
  }
}

function lockApp(): void {
  if (!chromeVisible()) return;
  endSession();
}

function logOut(): void {
  endSession("Signed out.");
  // A sign-in after Log out starts from Home, whoever signs in.
  lockedFrom = null;
}

/** Tells the host so the menu bar matches the window. No-op when unchanged; a host that cannot is asked again next time. */
function syncShellChrome(open: boolean): void {
  if (shellChromeSent === open) return;
  shellChromeSent = open;
  try {
    bir.shell_set_chrome(open);
  } catch {
    shellChromeSent = null;
  }
}

// ---------------------------------------------------------------- start (boot and the dev route)

function runStartRoute(raw: string): void {
  for (const { key, value } of parseStart(raw)) {
    if (key === "tp") selectTaxpayer(value, { quiet: true });
    else if (key === "branch") selectBranch(value);
    else if (key === "theme") setThemePreference(parseThemePreference(value));
    else if (key === "view" && (isViewName(value) || value === "cron_tasks")) go(value as ViewName);
    else if (key === "section") go("settings", { section: value });
    else if (key === "panel") togglePanel(value === "notifs" ? "notifs" : "profile");
    else if (key === "palette") {
      openPalette();
      setPaletteQuery(value);
    } else if (key === "compact") {
      // Compact form sections for this run only; Settings is what saves it.
      prefs.set({ ...prefs(), compactFormNav: value !== "0" });
    } else if (key === "form") {
      const f = parseStartForm(value);
      const b = branch();
      if (f && b) openReturn({ tin: b.tin, code: f.code, year: f.year, period: f.period, returnKey: f.key });
    }
  }
}

/**
 * Loads what the app keeps (theme, selection, Recent, read notifications, prefs), then shows the frame. Idempotent. `start` is
 * the dev route (App.tsx passes BIR_SHELL_START): "tp=<root>;branch=<code>;theme=dark;view=library;panel=profile;palette=<text>;form=2551Q/2026/3".
 */
export function boot(start?: string): Promise<void> {
  bootPromise ??= (async () => {
    // Anything read before the frame mounted (a computed made at import, a fixture's own writes) is read again.
    store.bump();
    databasePath = store.systemInfo().database_path ?? "";
    themePref.set(themePreferenceFromStart(start) ?? parseThemePreference(store.setting(THEME_PREF_KEY)));
    setColorScheme(themePref());
    const stored = await readLocal<{ root?: string | null; code?: string | null }>(KEYS.selection, {});
    batch(() => {
      selRoot.set(stored.root || null);
      selCode.set(stored.code || null);
    });
    // An entry always has a text id (one written with a number by an earlier build is rebuilt).
    recent.set(loadRecent(await readLocal<unknown>(KEYS.recent, [])));
    // First run against this database: what already happened is not news.
    const storedRead = await readLocal<string[] | null>(KEYS.read, null);
    const ids = storedRead ?? notifications().map((n) => n.id);
    readIds.set(new Set(ids));
    if (!storedRead) writeLocal(KEYS.read, ids);
    knownEvents = new Set(notifications().map((n) => n.id));
    prefs.set({ showRecent: true, compactFormNav: false, ...(await readLocal<Partial<Prefs>>(KEYS.prefs, {})) });
    applyScope();
    booted.set(true);
    if (start) runStartRoute(start);
  })();
  return bootPromise;
}

// ---------------------------------------------------------------- the public handle

/** The app state for the frame and for screens: read-only accessors, and the actions that change them. */
export const app = {
  // Where the app is.
  booted: readOnly(booted),
  /** The route as navigated (a taxpayer view stays named while no taxpayer exists; `view` is what shows). */
  route: readOnly(route),
  /** The view that shows. */
  view,
  params: (): ViewParams => route().params,
  canBack: () => history().length > 0,
  canForward: () => forward().length > 0,
  // Selection.
  taxpayers: () => store.taxpayers(),
  taxpayer,
  branch,
  isFresh,
  years,
  year,
  yearForms,
  templates,
  branchFor,
  // Layers.
  sidebarOpen: readOnly(sidebarOpen),
  panel: readOnly(panel),
  lastPanel: readOnly(lastPanel),
  overlay: readOnly(overlay),
  newReturn: readOnly(newReturn),
  paletteQuery: readOnly(paletteQuery),
  paletteIndex: readOnly(paletteIndex),
  // Layout (from the window's width).
  layout: { narrow, overlayMode, sidebarDocked, panelDocked, mainWidth },
  // Preferences, Recent, notifications, theme.
  recent: readOnly(recent),
  prefs: readOnly(prefs),
  themePref: readOnly(themePref),
  notifications,
  unreadCount,
  // Session and the gate.
  session: readOnly(session),
  appLocked: readOnly(appLocked),
  cloudLocked: readOnly(cloudLocked),
  chromeVisible,
  isAdmin,
  isHelpdesk,
  adminHasCode,
  canAddTaxpayer,
  authState,
  updateStatus: readOnly(updateStatus),
  // Actions.
  go,
  replace,
  back,
  forward: forwardOne,
  retarget,
  openReturn,
  openProfile,
  openTaxpayer,
  openBranch,
  openRecent: (r: RecentEntry) => openReturn(recentOpenOptions(r)),
  selectTaxpayer,
  selectBranch,
  selectYear,
  toggleSidebar,
  togglePanel,
  toggleRightPanel,
  closePanel,
  closeFloating,
  followWidth,
  openPalette,
  closeOverlay,
  dismiss,
  startNewReturn,
  pickNewReturnForm,
  pickNewReturnPeriod,
  setPaletteQuery,
  setPaletteIndex,
  setThemePreference,
  cycleTheme,
  setPref,
  pushRecent,
  bumpRecent: bumpRecentReturn,
  remapRecent,
  markAllRead,
  openNotification,
  lockApp,
  logOut,
  endSession,
  startSession,
  setAppLocked: (locked: boolean) => appLocked.set(locked),
  setCloudLocked: (locked: boolean) => cloudLocked.set(locked),
  setSetup: (pending: string | null, step: string) => setup.set({ pending, step }),
  syncShellChrome,
  toast,
};
