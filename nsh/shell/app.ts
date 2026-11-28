// The app state (old app/main.js BirApp): one module, private signals, one owner per piece of state. Screens read the
// accessors and call the actions; nothing outside this file sets a signal.
//
//   navigation   route (view + params), history and forward stacks (each stop remembers the taxpayer it was left on), go() back()
//                forward() openReturn() openProfile(); every move first waits for the screens' beforeLeave hooks (3 s at most)
//   layers       overlay (palette | new), panel (profile | notifs), sidebar, the palette's query and row
//   selection    the taxpayer, branch and tax year the frame is "on" (persisted)
//   session      open | admin | taxpayer; the Buwiz account gate (cloudLocked), first-run setup and the sign-in lock
//                (appLocked) in front of the frame: screens/auth draws them, this file keeps the state
//   persistence  redesign.recent, redesign.selection, redesign.read_notifications (each suffixed @<database path>; with the path
//                unknown they are not kept at all) and redesign.prefs, in NSH Storage; the theme through bir get_setting / set_setting
//   polls        events 30 s, device filings 5 s, update status 1 s: startPolling(), owned by the root; none shows a toast while a lock is up;
//                while a return is Queued the lists are read every 3 s for about 30 s after the Queued set last changed (startQueuedWatch,
//                also in the dev environment; it and the library warm-up wait for the frame to be shown: nothing is read behind a lock)
import type { Accessor, Signal } from "nsh";
import { batch, computed, effect, host, platform, setColorScheme, signal, Storage, untracked, windowSize } from "nsh";
import { Application, isAvailable } from "nsh/desktop";
import { bir, dataChangeToken, doorStatus, setAgentAccess, shutdown, type Branch, type ReturnRow, type Taxpayer, type UpdateStatus } from "../bir";
import { shellChromeVisible } from "../lib/chrome_gate";
import { errorText } from "../lib/errors";
import { DRY_RUN_WORD, isDryRun } from "../lib/statusWord";
import { POLL, startPolls } from "../lib/poll";
import { addRecent, bumpRecent, loadRecent, recentEntry, recentOpenOptions, type RecentEntry, type RecentInput } from "../lib/recent";
import type { FormTarget } from "../screens/form/FormPage";
import { BREAK } from "../theme/tokens";
import { deviceCopy, dismissToast, ICONS, toast as kitToast, type IconName, phoneLayout } from "../ui";
import { eventToast, takeFresh } from "./events";
import { lockPagesDebugCompiled } from "./lockDebug";
import { isViewName, ROUTES, type Route, type ViewName, type ViewParams } from "./routes";
import { allowedStart, parseStart, parseStartForm } from "./start";
import { POLL_KEYS, store, WORKER_KEYS } from "./store";
import { nextThemePreference, parseThemePreference, storedThemePreference, THEME_PREF_KEY, themePreferenceFromStart, type ThemePreference } from "./theme";

/** A dev route needs the test environment; the lock debug route additionally needs its debug-only Rust entry point. */
function devRouteAllowed(view: ViewName): boolean {
  const testEnvironment = (host.params as { testEnvironment?: boolean } | undefined)?.testEnvironment === false ? false : doorStatus().testEnvironment;
  return testEnvironment && (view !== "lockdebug" || lockPagesDebugCompiled());
}

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
 * First-run setup ("Who will use eBIRForms on this device?") stands in front of the frame while the database has no access
 * mode. The shipped app turns it on (App.tsx: Shell's `firstRun`); the test fixtures leave it off, so their fresh databases
 * open straight into the frame as before (screens/auth's fixture turns it on).
 */
const FIRST_RUN_GATE = signal(false);

// ---------------------------------------------------------------- private state

/** The taxpayer, branch and year the frame is on. */
interface Selection {
  root: string | null;
  code: string | null;
  year: number | null;
}
/** A place in the history: the route, and the selection the frame had when the person left it. */
interface Stop {
  route: Route;
  sel: Selection;
}

const booted = signal(false);
const route = signal<Route>({ view: "home", params: {}, seq: 0 });
const history = signal<Stop[]>([]);
const forward = signal<Stop[]>([]);
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
/** Why the access state (does the admin have a PIN or an authenticator?) could not be read; set, the app stays locked and the lock screen shows it. */
const accessError = signal<string | null>(null);
let lockedFrom: Session | null = null;
/** Why the account gate cannot open (its read, or the session opening, failed); shown on the gate card. */
const gateError = signal<string | null>(null);
/** The kept values (theme, selection, Recent, ...) are loaded: once, when the account gate first opens. */
let keptLoaded = false;
/** The dev start route's steps for the sign-in and setup screens (screens/auth registers them). */
let accessSteps: ((key: string, value: string) => void) | null = null;
let databasePath = "";
let shellChromeSent: boolean | null = null;
let bootPromise: Promise<void> | null = null;

/** A read-only view of a signal: the accessor without its setters. */
function readOnly<T>(s: Signal<T>): Accessor<T> {
  return () => s();
}

// ---------------------------------------------------------------- persistence (NSH Storage)

/**
 * The Storage key of one of the app's kept values. Selection, Recent and read notifications name taxpayers, so they are
 * kept per database (the key ends in "@<database path>", as the old app's did): a scratch database must not inherit
 * another's. With the path unknown they have no key, and are not kept at all rather than shared between databases.
 */
export function storageKey(key: string, database: string): string | null {
  if (key === KEYS.prefs) return key;
  return database ? `${key}@${database}` : null;
}

async function readLocal<T>(key: string, fallback: T): Promise<T> {
  const stored = storageKey(key, databasePath);
  if (stored === null) return fallback;
  try {
    const raw = await Storage.getItem(stored);
    return raw ? (JSON.parse(raw) as T) : fallback;
  } catch {
    return fallback;
  }
}

function writeLocal(key: string, value: unknown): void {
  const stored = storageKey(key, databasePath);
  if (stored === null) return;
  Storage.setItem(stored, JSON.stringify(value)).catch(() => {});
}

// ---------------------------------------------------------------- layout (from the window's width)

/** The narrow (phone) layout: a header, a bottom tab bar, sheets. Every phone and tablet at any width; a desktop window under 600. */
const narrow = computed(() => phoneLayout());
/** The sidebar and the panel float over the page (under 860), and never dock in the narrow layout, which draws neither. */
const overlayMode = computed(() => narrow() || windowSize().width < BREAK.overlays);
const sidebarDocked = computed(() => sidebarOpen() && !overlayMode());
const panelDocked = computed(() => panel() !== null && !overlayMode());
/** The main surface's width: the window less what is docked beside it and the inset. */
const mainWidth = computed(() => {
  // Narrow: the surface is the window's width (no sidebar, no panel, no inset).
  if (narrow()) return Math.max(240, windowSize().width);
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
// The owner ("Just me") and the help desk admin always start a new tax profile; a help desk
// client (a taxpayer signed in with their own TIN and PIN) may only ever add the first one.
// Never while the app is locked, so no surface can leak it there.
const canAddTaxpayer = () => !appLocked() && (session().kind !== "taxpayer" || !store.taxpayers().length);

/** Toolbar, sidebar, taxpayer picker and filing menus: false on the account gate, first launch and the sign-in screen. */
const chromeVisible = computed(
  () =>
    booted() &&
    shellChromeVisible({
      booted: true,
      cloudLocked: cloudLocked(),
      appLocked: appLocked(),
      mode: authState().mode ?? (FIRST_RUN_GATE() ? null : "solo"),
      setupPending: setup().pending,
      setupStep: setup().step,
    }),
);

/** First launch has not picked "Just me" or "I help other taxpayers" yet (only where first-run setup is on). */
const needsSetup = () => FIRST_RUN_GATE() && !authState().mode;

function applyScope(): void {
  const s = session();
  store.setScope(s.kind === "taxpayer" ? s.root : null);
}

/**
 * Whether the admin has a PIN or an authenticator, asked of the host now. Never the cached read (store.authState): its fallback while
 * the host cannot answer is "no code", and a lock must not open on a fallback. `true`/`false` is the host's own answer; `null` means
 * the host could not say (its sentence is in accessError, and the app is to stay locked).
 */
function readAccess(): boolean | null {
  try {
    const state = bir.auth_state();
    store.forgetWhere(/^auth_state$/);
    accessError.set(null);
    return Boolean(state.has_admin_pin || state.has_admin_totp);
  } catch (error) {
    accessError.set(errorText(error));
    return null;
  }
}

/** The lock screen's Retry after the access state could not be read: asks again; the sign-in (or the frame) comes only on an answer. */
function retryAccess(): void {
  if (accessError() === null) return;
  const code = readAccess();
  // One line per press, on the console (the daily log needs the database this card may be locked out of).
  console.warn(`Try again on the locked card: ${code === null ? `still locked: ${accessError()}` : "opened"}`);
  if (code !== null) appLocked.set(code);
}

/** The agent host must refuse while the app is locked: the Buwiz gate, the sign-in lock, or not booted yet (first-run setup is not a lock). */
const agentLocked = () => !booted() || cloudLocked() || appLocked();

let agentAccessSent: string | null = null;

/** Tells the agent host who may use the app (src/agent.rs). Call it from an effect: it reads the signals it depends on. No-op when unchanged. */
function syncAgentAccess(): void {
  const locked = agentLocked();
  const taxpayerSession = session().kind === "taxpayer";
  const sent = `${locked}|${taxpayerSession}`;
  if (agentAccessSent === sent) return;
  agentAccessSent = sent;
  try {
    setAgentAccess(locked, taxpayerSession);
  } catch {
    agentAccessSent = null;
  }
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
  if (!b) return [];
  const listed = store.taxYears(b.tin);
  const r = route();
  const requested = r.view === "profile" && r.params.fromForm === true ? r.params.year : undefined;
  if (typeof requested !== "number" || !Number.isInteger(requested) || listed.includes(requested)) return listed;
  const bounds = store.yearBounds();
  if (requested < bounds.min || requested > bounds.max) return listed;
  // A return can exist before its year is added to the profile's forms list.
  return [...listed, requested].sort((a, b) => a - b);
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

/** What the frame is on now (the effective taxpayer and branch, as saveSelection keeps them). */
function currentSelection(): Selection {
  return { root: taxpayer()?.tin_root ?? null, code: branch()?.code ?? null, year: selYear() };
}

/**
 * Puts the frame back on the taxpayer and branch a stop was left on (Back and Forward), and the year with them. Nothing changes
 * when the frame is there already, or when that taxpayer is gone from the database.
 */
function restoreSelection(sel: Selection): void {
  if (!sel.root) return;
  if (taxpayer()?.tin_root === sel.root && branch()?.code === sel.code) return;
  if (!store.taxpayers().some((t) => t.tin_root === sel.root)) return;
  selectTaxpayer(sel.root, { quiet: true, code: sel.code ?? undefined });
  selYear.set(sel.year);
}

// ---------------------------------------------------------------- toasts

function iconOf(name: string | undefined): IconName | undefined {
  return name && name in ICONS ? (name as IconName) : undefined;
}

/**
 * A toast at the bottom (the Toaster mounted by the frame); answers its id (dismissToast closes it). `icon`: an icon name of
 * ui/icons; others show the default. `action`: a button on the toast (the form page's Undo), shown with the countdown bar.
 */
export function toast(message: string, icon?: string, action?: { label: string; onPress: () => void }): string {
  return kitToast(message, { icon: iconOf(icon), action });
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

// ---- leaving a screen

/** What a screen registers with beforeLeave: it runs before the app leaves the screen, and may take a while. */
export type LeaveHook = () => void | Promise<void>;
/** The longest the app waits for the beforeLeave hooks (all of them together); then it leaves anyway. */
export const LEAVE_HOOKS_CAP_MS = 3000;

const leaveHooks = new Set<LeaveHook>();
/** The moves that wait for the hooks, in order; null when none does. */
let leaving: Promise<void> | null = null;

/**
 * A screen with work to finish before the app leaves it (the form page's pending edits) registers it here, and unregisters
 * with the function this returns when it goes. go(), replace(), back(), forward() and quit() wait for every registered
 * hook, 3 seconds at most, before they move; a hook that throws is logged and does not stop the move. With no hook
 * registered a move happens at once, as it always did.
 */
function beforeLeave(hook: LeaveHook): () => void {
  const entry: LeaveHook = () => hook(); // two registrations of one function stay two
  leaveHooks.add(entry);
  return () => {
    leaveHooks.delete(entry);
  };
}

// ---- the session changes

/** What a screen registers with onSessionChange: it puts the screen's own state back to how a first visit finds it. */
export type SessionChangeHook = () => void;

const sessionHooks = new Set<SessionChangeHook>();

/**
 * A screen that keeps state in module-level signals (its filters, a pick, a search, an open edit with typed values) registers
 * the reset of that state here, so it does not outlive the session it belongs to: the next person in help-desk mode starts
 * from the defaults and sees nothing of the last one's selection. The hooks run, synchronously and in registration order,
 * when the app locks, signs out, unlocks (a sign-in, the account gate opening again) and when another taxpayer signs in; a
 * hook that throws is logged and does not stop the others. Returns the function that unregisters it.
 */
function onSessionChange(hook: SessionChangeHook): () => void {
  const entry: SessionChangeHook = () => hook(); // two registrations of one function stay two
  sessionHooks.add(entry);
  return () => {
    sessionHooks.delete(entry);
  };
}

function sessionChanged(): void {
  for (const hook of [...sessionHooks]) {
    try {
      hook();
    } catch (error) {
      store.logScreen("warn", `resetting a screen for the next session: ${errorText(error)}`);
    }
  }
}

// The frame's own state of the same kind: the palette's search and the New return dialog's picks.
onSessionChange(() => {
  batch(() => {
    paletteQuery.set("");
    paletteIndex.set(0);
    newReturn.set({ code: null, period: null });
  });
});

/** Runs every registered hook and waits for them all, but not longer than LEAVE_HOOKS_CAP_MS. */
async function runLeaveHooks(): Promise<void> {
  if (!leaveHooks.size) return;
  let timer: ReturnType<typeof setTimeout> | undefined;
  let capped = false;
  const cap = new Promise<void>((done) => {
    timer = setTimeout(() => {
      capped = true;
      done();
    }, LEAVE_HOOKS_CAP_MS);
  });
  const finished = Promise.all(
    [...leaveHooks].map(async (hook) => {
      try {
        await hook();
      } catch (error) {
        store.logScreen("warn", `before leaving a screen: ${errorText(error)}`);
      }
    }),
  ).then(() => undefined);
  await Promise.race([finished, cap]);
  clearTimeout(timer);
  if (capped) store.logScreen("warn", `A screen did not finish its work in ${LEAVE_HOOKS_CAP_MS / 1000} seconds; the app moved on.`);
}

/** Does `move` now when no screen has a hook to wait for, else once the hooks are done (after any move that waits already). */
function leave(move: () => void): void {
  if (!leaveHooks.size && leaving === null) return move();
  const step: Promise<void> = (leaving ?? Promise.resolve())
    .then(runLeaveHooks)
    .then(move)
    .catch((error) => store.logScreen("error", `moving: ${errorText(error)}`))
    .then(() => {
      if (leaving === step) leaving = null;
    });
  leaving = step;
}

/** Resolves when every move that waited for the hooks has happened (at once when none waits). */
function settled(): Promise<void> {
  return leaving ?? Promise.resolve();
}

/** The steps that end the run, as one object so a test can stand in for them; the app never changes it. */
export const quitSteps = {
  /**
   * Whether this platform ends the run when asked: the desktops. Elsewhere NSH's `Application.quit()` and `restart()` do nothing
   * (a phone, where only the agent's quit gets here), and an app whose door was shut would go on running with every call refused.
   */
  ends: (): boolean => isAvailable && platform.desktop,
  /** The door's quit path: the submission worker stops and the database is checkpointed (safe to call twice). */
  shutdown: (): void => shutdown(),
  /** The application ends. */
  end: (): void => Application.quit(),
};

/**
 * What ends the run once every screen has finished its work: the door's quit path first, then `then` (the app ends). A caller that
 * ends the run its own way (the updater's install and restart) calls this inside its exit, so no way out skips the door's shutdown.
 * Where the platform does not end the run (`quitSteps.ends`), neither happens: the app stays open and its door keeps answering.
 */
function endRun(then: () => void = () => quitSteps.end()): void {
  if (!quitSteps.ends()) {
    store.logScreen("warn", "quit: this platform does not end the run; the app stays open");
    return;
  }
  try {
    quitSteps.shutdown();
  } catch (error) {
    store.logScreen("warn", `quit: ${errorText(error)}`);
  }
  then();
}

/**
 * Quits the app (desktops) once the screens' beforeLeave hooks are done, 3 seconds at most, then the door shuts down and the app ends:
 * every caller gets all three. `exit` replaces what happens after the hooks (a test's stand-in; the updater, which calls endRun inside it).
 */
async function quit(exit: () => void = endRun): Promise<void> {
  await runLeaveHooks();
  exit();
}

// ---- moves

/**
 * Opens a view. Views that need a taxpayer fall back to Home when none exists; dev views only open in the dev
 * environment. Moving to another view (or to the same view with other params) pushes the history and clears Forward.
 * The history keeps the taxpayer the frame was on, so Back and Forward put it back.
 */
export function go(name: ViewName | "cron_tasks", params: ViewParams = {}): void {
  navigate(name, params, currentSelection());
}

/** go() for a caller that changed the selection as part of the move: `from` is what the screen being left was on. */
function navigate(name: ViewName | "cron_tasks", params: ViewParams, from: Selection): void {
  leave(() => {
    let target: ViewName;
    if (name === "cron_tasks") {
      // Background tasks live in Settings > Jobs.
      target = "settings";
      params = { ...params, section: "jobs" };
    } else {
      target = name;
    }
    const meta = ROUTES[target];
    if ((meta.needsTaxpayer && isFresh()) || (meta.dev && !devRouteAllowed(target))) {
      target = "home";
      params = {};
    }
    const here = route();
    if (target !== here.view || !sameParams(here.params, params)) {
      batch(() => {
        history.set([...history().slice(-49), { route: here, sel: from }]);
        forward.set([]);
        route.set({ view: target, params, seq: ++seq });
      });
    }
    closeTransient();
  });
}

/** Shows another view in place of the current one: no history entry (a switch between two views of one page, a redirect). */
export function replace(name: ViewName, params: ViewParams = {}): void {
  leave(() => {
    const meta = ROUTES[name];
    if ((meta.needsTaxpayer && isFresh()) || (meta.dev && !devRouteAllowed(name))) return;
    route.set({ view: name, params, seq: ++seq });
    closeTransient();
  });
}

/** Back one step, onto the taxpayer that screen was left on. False when there is none. */
export function back(): boolean {
  if (lastAlive(history()) < 0) return false;
  leave(() => {
    const h = history();
    // Stops whose taxpayer is gone are dropped on the way.
    const at = lastAlive(h);
    if (at < 0) return;
    const stop = h[at];
    const here: Stop = { route: route(), sel: currentSelection() };
    restoreSelection(stop.sel);
    batch(() => {
      forward.set([...forward(), here]);
      history.set(h.slice(0, at));
      route.set({ ...stop.route, seq: ++seq });
    });
    closeTransient();
  });
  return true;
}

/** Forward one step, onto the taxpayer that screen was left on. False when there is none. */
export function forwardOne(): boolean {
  if (lastAlive(forward()) < 0) return false;
  leave(() => {
    const f = forward();
    const at = lastAlive(f);
    if (at < 0) return;
    const stop = f[at];
    const here: Stop = { route: route(), sel: currentSelection() };
    restoreSelection(stop.sel);
    batch(() => {
      history.set([...history(), here]);
      forward.set(f.slice(0, at));
      route.set({ ...stop.route, seq: ++seq });
    });
    closeTransient();
  });
  return true;
}

/** The key a FormTarget holds for a stored return's key: a number, or null when there is none (text that is no number is none). */
function targetKey(key: number | string | null | undefined): number | null {
  return key == null || Number.isNaN(Number(key)) ? null : Number(key);
}

/** A stored return, by what names it. */
interface SavedReturn {
  tin: string;
  formCode: string;
  year: number;
  period: number;
  key: number | null;
}

/** An amendment that is stored now is a return: every place that would still start it again (the page on screen, Back, Forward) opens the saved return. */
function settleAmendment(saved: SavedReturn): void {
  const settle = (r: Route): Route => {
    const t = r.view === "form" ? (r.params.target as FormTarget | undefined) : undefined;
    if (!t || t.mode !== "amend") return r;
    if (t.tin !== saved.tin || t.formCode !== saved.formCode || t.year !== saved.year || t.period !== saved.period || targetKey(t.key) !== saved.key) return r;
    return { ...r, params: { ...r.params, target: { ...t, mode: "return", key: saved.key } } };
  };
  const settleStops = (stops: Stop[]): Stop[] => {
    const next = stops.map((s) => {
      const r = settle(s.route);
      return r === s.route ? s : { ...s, route: r };
    });
    return next.some((s, i) => s !== stops[i]) ? next : stops;
  };
  batch(() => {
    const here = settle(route());
    if (here !== route()) route.set(here);
    const h = settleStops(history());
    if (h !== history()) history.set(h);
    const f = settleStops(forward());
    if (f !== forward()) forward.set(f);
  });
}

/**
 * The form page shows another return than the one it opened (a sample became its return, an offer loaded one): the route
 * follows, the page stays. A return that is stored now is no new amendment any more, wherever the history names it.
 */
function retarget(target: FormTarget): void {
  const cur = route();
  if (cur.view === "form") route.set({ ...cur, params: { target } });
  if (target.mode === "return") {
    settleAmendment({ tin: target.tin, formCode: target.formCode, year: target.year, period: target.period, key: targetKey(target.key) });
  }
}

export function openProfile(editSection?: string): void {
  go("profile", editSection ? { edit: editSection } : {});
}

/** Search picked a taxpayer: it opens on its Tax profile, wherever search was opened from. */
export function openTaxpayer(root: string): void {
  const from = currentSelection();
  if (taxpayer()?.tin_root !== root) selectTaxpayer(root);
  navigate("profile", {}, from);
}

/** Search picked a branch: select it and open the Tax profile, as for a taxpayer. */
export function openBranch(root: string, code: string): void {
  const from = currentSelection();
  if (taxpayer()?.tin_root === root) selectBranch(code);
  else selectTaxpayer(root, { code });
  navigate("profile", {}, from);
}

// ---------------------------------------------------------------- recent returns

function pushRecent(opts: RecentInput): void {
  const next = addRecent(recent(), opts);
  if (next === recent()) return;
  recent.set(next);
  writeLocal(KEYS.recent, next);
}

/** A return was changed and saved: it moves to the top of Recent, and an amendment's first save makes it a stored return in the history. */
function bumpRecentReturn(opts: RecentInput): void {
  settleAmendment({ tin: String(opts.tin), formCode: opts.code, year: opts.year, period: opts.period, key: targetKey(opts.returnKey) });
  const next = bumpRecent(recent(), opts);
  recent.set(next);
  writeLocal(KEYS.recent, next);
}

/** A route that names a return (the form page) with its TIN moved by `map`; the same route when it names none or the TIN stays. */
function remapRoute(r: Route, map: (tin: string) => string): Route {
  const t = r.view === "form" ? (r.params.target as FormTarget | undefined) : undefined;
  if (!t) return r;
  const tin = map(String(t.tin));
  return tin === String(t.tin) ? r : { ...r, params: { ...r.params, target: { ...t, tin } } };
}

/** A selection (taxpayer root and branch code) moved by `map`, which takes and gives the 14-digit TIN. */
function remapSelection(sel: Selection, map: (tin: string) => string): Selection {
  if (!sel.root) return sel;
  const was = sel.root + (sel.code ?? "00000");
  const now = map(was);
  return now === was ? sel : { ...sel, root: now.slice(0, 9), code: sel.code === null ? null : now.slice(9) };
}

function remapStops(stops: Stop[], map: (tin: string) => string): Stop[] {
  const next = stops.map((s) => {
    const r = remapRoute(s.route, map);
    const sel = remapSelection(s.sel, map);
    return r === s.route && sel === s.sel ? s : { route: r, sel };
  });
  return next.some((s, i) => s !== stops[i]) ? next : stops;
}

/**
 * Rewrite what names a TIN after a TIN or branch-code move, so it still opens: Recent, every stop of the history and of Forward (the
 * route's return and the selection it was left on), and the page on screen. `map` takes and gives the 14-digit TIN.
 */
function remapRecent(map: (tin: string) => string): void {
  const next = recent().map((r) => recentEntry({ ...r, tin: map(String(r.tin)) }));
  recent.set(next);
  writeLocal(KEYS.recent, next);
  batch(() => {
    const h = remapStops(history(), map);
    if (h !== history()) history.set(h);
    const f = remapStops(forward(), map);
    if (f !== forward()) forward.set(f);
    const here = remapRoute(route(), map);
    if (here !== route()) route.set(here);
  });
}

/**
 * Whether a stop can still be shown: its taxpayer (the selection it was left on, the return its page names) is in the database.
 * A taxpayer that was deleted leaves stops that would show the profile, or a form, of whoever is first now. With no taxpayer at all
 * (or none the host would list) nothing can be told: the stop stays, and the view falls to Home by itself.
 */
function stopAlive(stop: Stop): boolean {
  const roots = new Set(store.taxpayers().map((t) => t.tin_root));
  if (!roots.size) return true;
  const target = stop.route.view === "form" ? (stop.route.params.target as FormTarget | undefined) : undefined;
  if (stop.sel.root && !roots.has(stop.sel.root)) return false;
  if (target && !roots.has(String(target.tin).slice(0, 9))) return false;
  return true;
}

/** The index of the stop nearest the end that can still be shown, or -1. */
function lastAlive(stops: Stop[]): number {
  for (let i = stops.length - 1; i >= 0; i--) if (stopAlive(stops[i])) return i;
  return -1;
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
  // Back goes to the taxpayer the screen being left was on, not the one this return belongs to.
  const from = currentSelection();
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
    key: targetKey(opts.returnKey),
    mode,
    snapshotId: opts.snapshotId,
    newKey: opts.newKey,
    newKeyFrom: opts.newKeyFrom,
  };
  navigate("form", { target }, from);
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

/**
 * Submissions sent and confirmation emails received, newest first: the return's own status is the record. A send the worker only
 * simulated (`isDryRun`: a dry run, a practice account, a Buwiz test TIN) is a test send, not a submission: it says the host's one
 * sentence for it (`send.line`, send_state.rs `TEST_SENT_LINE`: "Test send. Nothing reached the BIR.").
 */
const notifications = computed<Notification[]>(() => {
  const seen = readIds();
  const items: Notification[] = [];
  for (const r of store.returns("filed") as ReturnRow[]) {
    const confirmed = r.status === "Confirmed" || r.status === "Paid";
    if (!confirmed && r.status !== "Submitted") continue;
    const id = `return:${r.id}:${confirmed ? "confirmed" : "submitted"}`;
    const who = r.branch_count > 1 ? `${r.taxpayer_name} · ${r.branch_name}` : r.taxpayer_name;
    const dry = !confirmed && isDryRun(r);
    const testLine = r.send?.stage === "test_sent" ? r.send.line : DRY_RUN_WORD;
    items.push({
      id,
      title: confirmed
        ? `BIR confirmed ${r.form_code} for ${r.period_label}`
        : dry
          ? `${r.form_code} for ${r.period_label}: ${testLine}`
          : `${r.form_code} for ${r.period_label} was submitted`,
      sub: confirmed
        ? `Confirmation email received · ${who}`
        : dry
          ? who
          : r.submitted_via_relay
            ? `Sent to the mock relay, not to BIR: no BIR email will come · ${who}`
            : `Waiting for BIR's confirmation email · ${who}`,
      icon: confirmed ? "circle-check" : dry ? "info" : "send",
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

let eventChangeToken: string | null = null;

/**
 * Re-reads what the submission worker changes behind this app's back (by default POLL_KEYS: returns, their deadlines and yearly form
 * rows, background jobs; the Queued watch passes the wider WORKER_KEYS) and announces events that appeared since the last look.
 */
function pollEvents(keys: RegExp = POLL_KEYS): void {
  // While a lock is up nobody has signed in: another taxpayer's filing events must not toast onto the lock screen (the next person
  // to sign in hears of what is theirs on the next poll: knownEvents is only updated here).
  if (cloudLocked() || appLocked()) return;
  let refresh = true;
  if (keys === POLL_KEYS) {
    let token: string | null = null;
    try {
      token = dataChangeToken();
    } catch {}
    refresh = token === null || token !== eventChangeToken;
    eventChangeToken = token;
  }
  if (refresh) store.forgetWhere(keys);
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
  // The upload went on whoever is at the keyboard; what it says is for the person who signs in, not for the lock screen.
  if (note?.message && !appLocked()) toast(note.message, note.icon || "send");
}

/** The host checks for a release on its own thread; polling is how the prompt appears. */
function watchUpdate(): void {
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

/** How often the lists are read again while a return is Queued (instead of the events poll's 30 s). */
export const QUEUED_POLL_MS = 3000;
/** How long after the Queued set last changed (or a queue action) the watch keeps its fast pace. */
export const QUEUED_FAST_MS = 30_000;

/** The watch's pace: first look, tick, fast phase duration and slow re-read interval. A test shortens them. */
export interface QueuedTiming {
  firstMs: number;
  tickMs: number;
  fastMs: number;
  slowMs: number;
}

/** The ids of the Queued returns in the cached filed list (read now when it was dropped), as one sorted text; "" for none. Always "" behind a lock: nothing is read for nobody. */
function queuedKey(): string {
  if (cloudLocked() || appLocked()) return "";
  return untracked(() => store.returns("filed"))
    .filter((r) => r.status === "Queued")
    .map((r) => r.id)
    .sort((a, b) => a - b)
    .join(",");
}

/**
 * One look of the Queued watch. It does nothing until the frame is shown (booted, the account gate open, first-run setup done, no
 * sign-in in front). With a return Queued it reads the worker's lists (WORKER_KEYS) on each look for fastMs after the Queued set last
 * changed or a queue action was made; a return that stays Queued (the worker is off, SFTP is down, the computer is offline) is then
 * read every slowMs even when the normal events poll is disabled. A change in the set, a new queue
 * action, or the frame coming back from a lock starts the fast phase again.
 */
function queuedLook(fastMs: number, slowMs: number): () => void {
  let seen = "";
  let changedAt = 0;
  let lastReadAt = 0;
  return () => {
    if (!chromeVisible()) {
      seen = "";
      return;
    }
    const queued = queuedKey();
    const now = Date.now();
    if (queued !== seen) {
      seen = queued;
      changedAt = now;
    }
    changedAt = Math.max(changedAt, store.lastQueueAction());
    if (queued !== "" && (now - changedAt < fastMs || now - lastReadAt >= slowMs)) {
      lastReadAt = now;
      pollEvents(WORKER_KEYS);
    }
  };
}

/**
 * The submission worker sends a Queued return within a second or two and tells nobody, so Filings and Home would say Queued until
 * something else moved the cache (the old app relied on its 30 s events poll, and a refused Cancel queue). While any return is Queued
 * the worker's lists are read every 3 s (for about 30 s after the Queued set last changed, then every 30 s), in the dev
 * environment too (where the other polls wait for BIR_SHELL_POLL=1). The root starts it and calls the returned function on unmount.
 */
export function startQueuedWatch(timing: Partial<QueuedTiming> = {}): () => void {
  const t: QueuedTiming = { firstMs: 1000, tickMs: QUEUED_POLL_MS, fastMs: QUEUED_FAST_MS, slowMs: POLL.events, ...timing };
  return startPolls(
    [{ name: "queued", every: t.tickMs, first: t.firstMs, run: queuedLook(t.fastMs, t.slowMs) }],
    (name, error) => store.logScreen("warn", `poll ${name}: ${errorText(error)}`),
  );
}

/** How long after the frame is first shown the form library is built (the first screen has been drawn by then). */
export const WARM_DELAY_MS = 600;

/**
 * Builds the form library's index once (a sync host call: about a second in a debug build), WARM_DELAY_MS after the frame is first
 * shown, so the first visit to the Form library, the palette's forms and New return do not pay for it. It waits for the frame: not
 * before boot, and not behind the account gate, first-run setup or the sign-in screen, where the build would freeze the window while
 * someone types. Call it where an effect can live (the root). Returns the function that cancels it.
 */
export function warmLibrary(): () => void {
  let timer: ReturnType<typeof setTimeout> | undefined;
  let started = false;
  effect(() => {
    // Always read it, so the effect hears of every lock and unlock (a read skipped by an early return would end the subscription).
    const shown = chromeVisible();
    if (started || !shown) return;
    started = true;
    timer = setTimeout(() => {
      timer = undefined;
      // Locked again within the delay: wait for the next time the frame shows.
      if (!chromeVisible()) {
        started = false;
        return;
      }
      try {
        store.screenIndex();
      } catch {
        // The index is asked for again where it is used; the store logs a failure.
      }
    }, WARM_DELAY_MS);
  });
  return () => {
    started = true;
    if (timer !== undefined) clearTimeout(timer);
  };
}

/** One run of each poll that can toast, for a fixture that plays a poll at a chosen moment (the app's own runs are startPolling's). */
export const pollSteps = { events: (): void => pollEvents(), deviceFilings: (): void => pullDeviceFilings(), queued: (): void => (queuedKey() !== "" ? pollEvents(WORKER_KEYS) : undefined) };

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
  // Asked of the host now: a read that fails locks anyway (the lock screen says why and offers Retry), never opens.
  if (s.kind !== "taxpayer" && readAccess() === false) {
    toast("Set your PIN in Settings first. With no PIN there is nothing to sign back in with.", "lock");
    return;
  }
  lockedFrom = s;
  // What the last person queued is read no more: the re-reads of a queue action end with the session they were for.
  store.cancelRereads();
  batch(() => {
    session.set({ kind: "open" });
    panel.set(null);
    overlay.set(null);
    appLocked.set(true);
  });
  // What the last person was told stays with them: no toast outlives the session onto the lock screen.
  dismissToast();
  sessionChanged();
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
  sessionChanged();
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

/** Computer setup changed (Settings): the admin moved, so the old session ends and nobody keeps their place. */
function signOutAfterSetupChange(): void {
  endSession(deviceCopy("Computer setup changed. Sign in as the admin with the new owner's PIN or the help desk PIN."));
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
  const steps = parseStart(raw);
  const routeParams = Object.fromEntries(steps.filter(({ key }) => key !== "view").map(({ key, value }) => [key, value]));
  for (const { key, value } of steps) {
    if (key === "tp") selectTaxpayer(value, { quiet: true });
    else if (key === "branch") selectBranch(value);
    else if (key === "theme") setThemePreference(parseThemePreference(value));
    else if (key === "view" && value === "lockdebug") go("lockdebug", routeParams);
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
    } else accessSteps?.(key, value);
  }
}

/** What the app keeps (theme, selection, Recent, read notifications, prefs), and whether sign-in comes first: read once the account gate is open. */
async function loadKept(start?: string): Promise<void> {
  keptLoaded = true;
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
  // The admin has a PIN or an authenticator: the sign-in screen comes first (old initAuth). With none, nobody can lock anyone out.
  // Asked of the host directly: the app opens only on its clear "no code"; a read that fails keeps it locked (accessError).
  appLocked.set(readAccess() !== false);
}

/** The dev route boot() was given, run once the account gate is open. */
let bootStart: string | undefined;
let startParsed: { raw: string | undefined; params: Readonly<Record<string, string>> } | null = null;

/**
 * The dev start route's steps as key -> value (the last of a repeated key wins), for the screens that take a start key of their
 * own (Library reads `scope`). Read-only and fixed once boot() has the route; empty with none. The frame acts on the keys it
 * knows; every other key is only listed here, and a screen that does not know a key ignores it.
 */
function startParams(): Readonly<Record<string, string>> {
  if (startParsed === null || startParsed.raw !== bootStart) {
    startParsed = { raw: bootStart, params: Object.freeze(Object.fromEntries(parseStart(bootStart).map((step) => [step.key, step.value]))) };
  }
  return startParsed.params;
}

/**
 * Asks the host whether the Buwiz account gate is open; if so loads what the app keeps (loadKept), then shows the frame (or
 * what stands in front of it: the gate, first-run setup, sign-in). Idempotent. `start` is the dev route (App.tsx passes
 * BIR_SHELL_START): "tp=<root>;branch=<code>;theme=dark;view=library;panel=profile;palette=<text>;form=2551Q/2026/3".
 * `firstRun`: first-run setup stands in front of the frame while the database has no access mode (the shipped app).
 * The route is honoured only where the door reports a test environment (its access steps take PINs and keys); `testEnvironment`
 * is for a test of that rule.
 */
export function boot(requested?: string, opts: { firstRun?: boolean; testEnvironment?: boolean } = {}): Promise<void> {
  bootPromise ??= (async () => {
    const start = allowedStart(requested, opts.testEnvironment ?? doorStatus().testEnvironment);
    FIRST_RUN_GATE.set(opts.firstRun === true);
    bootStart = start;
    // The dev environment's host bypass (EBIR_TEST_ENV=1 with BIR_AUTH_BYPASS=1) answers unlocked, as in the old app.
    let unlocked = false;
    try {
      unlocked = bir.cloud_account_gate().unlocked;
    } catch (error) {
      gateError.set(errorText(error));
    }
    if (unlocked) await loadKept(start);
    else {
      // Nothing that names a taxpayer is read until the device flow is approved; the theme follows the route or the system.
      themePref.set(themePreferenceFromStart(start) ?? "system");
      setColorScheme(themePref());
      cloudLocked.set(true);
    }
    booted.set(true);
    if (unlocked && start) runStartRoute(start);
  })();
  return bootPromise;
}

/**
 * The Buwiz account approved this computer (the gate's device flow, or Settings › Account): the host opens the session and
 * the filing UI shows, loading what the app keeps the first time. False, with the host's reason on the gate, when it refused.
 */
async function finishUnlock(): Promise<boolean> {
  try {
    bir.cloud_account_session_opened();
  } catch (error) {
    batch(() => {
      gateError.set(errorText(error));
      cloudLocked.set(true);
    });
    return false;
  }
  gateError.set(null);
  if (!keptLoaded) {
    await loadKept(bootStart);
    cloudLocked.set(false);
    if (bootStart) runStartRoute(bootStart);
  } else {
    // The gate opening again is a new session; a sign-in from Settings while the app is already open is not.
    if (cloudLocked()) sessionChanged();
    store.bump();
    cloudLocked.set(false);
  }
  return true;
}

/** What runs each time the account gate comes back (lockCloud): the sign-in flow starts over on its first page. */
const cloudLockHooks: (() => void)[] = [];

/** Hears every lockCloud, before the gate shows (screens/auth/signInFlow.ts puts the sign-in back on its first page). */
function onCloudLock(hook: () => void): void {
  cloudLockHooks.push(hook);
}

/**
 * Signed out of the Buwiz account (the host already dropped the refresh token), or Settings' Sign in: the account gate comes
 * back, on its first page, whichever way it was asked for (the menu's Sign Out, Settings).
 */
function lockCloud(): void {
  try {
    bir.cloud_account_session_closed();
  } catch {
    // The host already locked the process.
  }
  batch(() => {
    for (const hook of cloudLockHooks) hook();
    gateError.set(null);
    cloudLocked.set(true);
  });
  store.cancelRereads();
  dismissToast();
  sessionChanged();
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
  canBack: () => lastAlive(history()) >= 0,
  /** The page Back would show (a header names it, a sub-page asks whether its parent is there); null when there is none. */
  previous: (): Route | null => history()[lastAlive(history())]?.route ?? null,
  canForward: () => lastAlive(forward()) >= 0,
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
  /** Why the access state could not be read (the lock screen shows it with Retry), or null. */
  accessError: readOnly(accessError),
  retryAccess,
  /** Whether the admin has a PIN or an authenticator, asked of the host now: null when it could not say. Never the cached read. */
  adminCodeNow: readAccess,
  /** The agent host refuses while true (the Buwiz gate, the sign-in lock, not booted); read by the root and the agent bridge. */
  agentLocked,
  syncAgentAccess,
  /** Why the account gate cannot open, or null. */
  gateError: readOnly(gateError),
  /** First-run setup's step ("choose" | "helpdesk-pin" | "helpdesk-key") and "solo" while Just me types its first taxpayer. */
  setup: readOnly(setup),
  /** First-run setup is on for this run (the shipped app). */
  firstRun: readOnly(FIRST_RUN_GATE),
  needsSetup,
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
  settled,
  beforeLeave,
  onSessionChange,
  quit,
  endRun,
  retarget,
  startParams,
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
  setGateError: (message: string | null) => gateError.set(message),
  finishUnlock,
  lockCloud,
  onCloudLock,
  signOutAfterSetupChange,
  /** The sign-in and setup screens' dev start-route steps (keys the frame does not know). */
  setAccessSteps: (run: ((key: string, value: string) => void) | null) => {
    accessSteps = run;
  },
  syncShellChrome,
  toast,
};
