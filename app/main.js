// buwiz BIR eFiling — gpui-shell application entry (shell redesign).
//
// Layout and behaviour follow DESIGN_SYSTEM.md in hexuria/buwiz-ui-redesign:
// a 40 px title bar with the context pill, a 248 px sidebar, the main
// surface inset 8 px with radius 12, and a 320 px right panel that is either
// the tax profile or notifications. Screens live in shell/*.js and build from
// the wrappers in shell/ui.js; reads go through the cached Store.
//
// Migration plan + tracking: docs/plans/gpui-shell-migration.md, issue #135.

import { View, div } from "gpui-kit";
import process from "process";
import { InputState, OtpState } from "gpui-base";
import { Store, errorText } from "./shell/store.js";
import {
  applyThemePreference,
  nextThemePreference,
  parseThemePreference,
  storedThemePreference,
  THEME_PREF_KEY,
} from "./shell/theme.js";
import { tokens, SIZE, BREAK, RADIUS, Toast, DigitEntry } from "./shell/ui.js";
import {
  renderTitleBar,
  renderSidebar,
  renderRightPanel,
  renderPalette,
  renderNewReturn,
} from "./shell/chrome.js";
import { renderHome } from "./shell/home.js";
import { recentEntry } from "./shell/recent.js";
import { renderLibrary } from "./shell/library.js";
import { renderCalendar } from "./shell/calendar_page.js";
import { renderFilings } from "./shell/filings.js";
import { renderSettings } from "./shell/settings.js";
import { renderTaxpayerPage, renderNewTaxpayer, TaxpayerActions } from "./shell/taxpayer.js";
import { renderFormPage, renderFormSplit, renderExistingOffer, renderRemovalConfirm, renderSamplePeriod, FormPageActions } from "./shell/form_page.js";
import { renderGallery } from "./shell/gallery.js";
import { renderSignIn, renderSetup } from "./shell/auth.js";
import { SecurityActions } from "./shell/security.js";
import { SetupChange } from "./shell/setup_change.js";
import { renderCronTasksPage } from "./shell/cron_tasks.js";

/** Views that need a taxpayer; with none they fall back to Home. */
const TAXPAYER_VIEWS = new Set(["profile", "forms", "templates", "form"]);
const RECENT_KEY = "redesign.recent";
const SELECTION_KEY = "redesign.selection";
const READ_KEY = "redesign.read_notifications";
const PREFS_KEY = "redesign.prefs";

// The retained app handle lets a form row request changes to shared chrome
// without treating the row's own event context as the application's context.
export default class BirRoot extends View {
  init(props, cx) {
    this.app = cx.new(BirApp, { ...props, invalidate: () => this.app.set_props({}) });
  }
  render() { return this.app; }
}

class BirApp extends View {
  init(_props, cx) {
    this.invalidate = _props.invalidate;
    this.store = new Store();
    this.info = this.store.systemInfo();
    this.isMac = this.info.os === "macos";

    this.themePref = parseThemePreference(this.store.setting(THEME_PREF_KEY));
    applyThemePreference(cx, this.themePref);

    // Navigation: one view, a back stack and a forward stack.
    this.view = "home";
    this.history = [];
    this.forward = [];
    this.sidebarOpen = true;
    /** null | "profile" | "notifs" */
    this.panel = null;
    /** The panel the title-bar panel button reopens. */
    this.lastPanel = "profile";
    /** null | "palette" | "new" | "existing" (a stored return of a sample's period) */
    this.overlay = null;
    this.existingOffer = null;
    /**
     * "Don't ask again for drafts" (the stored-return offer's fast lane): a
     * choice for this run only. It is lost on quit, and Lock app and Log out
     * reset it (endSession), so the next person to sign in is asked again.
     */
    this.fastLane = { drafts: false };
    this.accountMenu = false;
    this.toastState = null;

    const stored = this.readLocal(SELECTION_KEY, {});
    this.selRoot = stored.root || null;
    this.selCode = stored.code || null;
    this.selYear = null;
    // An entry always has a text id (one written with a number by an earlier
    // build is rebuilt).
    this.recent = this.readLocal(RECENT_KEY, []).map((r) => (typeof r.key === "string" ? r : recentEntry(r)));
    // First run against this database: what already happened is not news.
    this.readIds = new Set();
    const storedRead = this.readLocal(READ_KEY, null);
    this.readIds = new Set(storedRead || this.notifications().map((n) => n.id));
    if (!storedRead) this.writeLocal(READ_KEY, [...this.readIds]);
    this.knownEvents = new Set(this.notifications().map((n) => n.id));
    this.prefs = { showRecent: true, compactFormNav: false, ...this.readLocal(PREFS_KEY, {}) };

    // Screen state.
    this.palette = { input: InputState.new({ placeholder: "Search taxpayers, forms and pages" }), index: 0 };
    this.palette.input.on("change", (_e, cx) => {
      this.palette.index = 0;
      cx.notify();
    });
    this.palette.input.on("submit", (_e, cx) => this.runPaletteItem(this.palette.index, cx));
    this.palette.input.on("focus", () => (this.palette.focused = true));
    this.palette.input.on("blur", () => (this.palette.focused = false));
    this.newReturn = { code: null, period: null, periodOpen: false };
    this.library = { query: InputState.new({ placeholder: "Find a form, like 1601C or VAT" }), freq: "All" };
    this.library.query.on("change", (_e, cx) => cx.notify());
    this.taxpayerUi = new TaxpayerActions(this);
    this.formUi = new FormPageActions(this);

    this.initAuth();
    this.focusRoot = cx.focus_handle();
    // Development aid: BIR_SHELL_START="tp=123456788;theme=dark;view=library"
    // (also "palette=<query>", "edit=<profile section>", and on a form
    // "choose=<key>~<value>|...", "select=<key>", "more=<part>:<item>",
    // "save", "items=<a>-<b>", "solo=<part>", and the sign-in steps listed
    // at runAccessStep; none bypasses a PIN)
    // or "...;form=2551Q/2026/3" opens a screen at launch, so a screen can be
    // checked headlessly (one launch, one screenshot). Unset in normal use.
    const start = process.env.BIR_SHELL_START;
    if (start) cx.timer.after(400, (cx) => this.runStartRoute(start, cx));

    // In-app notifications work while the app runs: the submission worker
    // (painted app or bir-headless) shares this database, so new
    // submissions and BIR confirmations are picked up by polling and shown
    // as a toast plus an unread dot on the bell.
    cx.timer.every(30000, (cx) => this.pollEvents(cx));

    // Key bindings travel the focus path; give the root the keyboard at
    // launch so shortcuts work before anything else is clicked.
    cx.timer.after(50, () => this.focusRoot.focus());

    // gpui-shell re-runs `render` only when script state changes, and gives
    // scripts no resize event, so layout widths computed from the window size
    // (pixel widths keep text layout cheap) would go stale after a resize.
    // Poll the viewport and re-render when it changes.
    this.lastViewport = "";
    this.wasNarrow = null;
    cx.timer.every(200, (cx) => {
      if (this.view === "form") this.formUi.refreshLayout();
      const vp = window.viewport_size();
      const key = `${Math.round(vp.width)}x${Math.round(vp.height)}`;
      if (key !== this.lastViewport) {
        // Crossing below 860 px turns the sidebar and panel into overlays;
        // they start closed there rather than covering the page.
        const narrow = vp.width < BREAK.overlays;
        if (narrow && this.wasNarrow === false) {
          this.sidebarOpen = false;
          this.panel = null;
        }
        if (!narrow && this.wasNarrow === true) this.sidebarOpen = true;
        this.wasNarrow = narrow;
        this.lastViewport = key;
        cx.notify();
      }
    });

    // Shortcuts use Cmd on macOS and Ctrl elsewhere; both chords are bound
    // so the keymap is the same on every platform.
    const chords = [
      ["b", "toggle-sidebar"],
      ["k", "open-palette"],
      ["n", "new-return"],
      ["shift-t", "toggle-theme"],
      ["alt-b", "toggle-right-panel"],
      [",", "open-settings"],
      ["l", "lock-app"],
      ["[", "go-back"],
      ["]", "go-forward"],
    ];
    cx.bind_keys([
      ...chords.flatMap(([key, action]) => [
        { keystroke: `ctrl-${key}`, action },
        { keystroke: `cmd-${key}`, action },
      ]),
      // Undo of the last app action: Cmd-Z on macOS, Ctrl-Z elsewhere, and
      // never while a text box has the keyboard (its own undo wins there).
      { keystroke: this.isMac ? "cmd-z" : "ctrl-z", action: "undo-last", context: "!Input" },
      { keystroke: "escape", action: "dismiss" },
    ]);
  }

  runStartRoute(route, cx) {
    for (const step of route.split(";")) {
      const [key, value = ""] = step.split("=");
      if (key === "tp") this.selectTaxpayer(value, cx, { quiet: true });
      else if (key === "branch") this.selectBranch(value, cx);
      else if (key === "theme") this.setThemePreference(value, cx);
      else if (key === "view") this.go(value, cx);
      else if (key === "panel") this.togglePanel(value, cx);
      else if (key === "scope") this.library.scope = value;
      // Compact form sections for this run only; Settings is what saves it.
      else if (key === "compact") this.prefs = { ...this.prefs, compactFormNav: value !== "0" };
      else if (key === "form") {
        // form=CODE/YEAR/PERIOD[/KEY]: KEY opens exactly that stored return.
        const [code, year, period, key] = value.split("/");
        const b = this.branch();
        const exact = key !== undefined && key !== "" ? { returnKey: Number(key) } : {};
        if (b) this.openForm({ tin: b.tin, code, year: Number(year), period: Number(period), ...exact }, cx);
      } else if (key === "part") this.formUi.jumpTo(Number(value), cx);
      else if (key === "open") this.taxpayerUi.open[value] = true;
      else if (key === "edit") this.taxpayerUi.startEdit(value, cx);
      else if (key === "palette") {
        this.openPalette(cx);
        this.palette.input.set_value(value);
      }
      // Form proofs (dev only): once the open return's draft has loaded, set
      // boxes through the page's own choose (what picking an option does),
      // show one dropdown open, or save.
      else if (key === "choose") {
        const edits = {};
        for (const pair of value.split("|")) {
          const at = pair.indexOf("~");
          if (at > 0) edits[pair.slice(0, at)] = pair.slice(at + 1);
        }
        this.afterFormLoads(cx, (cx) => this.formUi.choose(edits, cx));
      } else if (key === "select") {
        this.afterFormLoads(cx, (cx) => {
          this.formUi.state.openSelect = value;
          this.formUi.refreshRows();
          cx.notify();
        });
      } else if (key === "more") {
        this.afterFormLoads(cx, (cx) => {
          this.formUi.state.moreOpen.add(value);
          this.formUi.refreshRows();
          cx.notify();
        });
      } else if (key === "save") this.afterFormLoads(cx, (cx) => this.formUi.save(cx));
      else if (key === "items") {
        const [from, to] = value.split("-").map(Number);
        this.formUi.devItems = { from, to: to ?? from };
      } else if (key === "solo") {
        this.formUi.devSolo = Number(value);
        this.formUi.jumpTo(Number(value), cx);
      } else this.runAccessStep(key, value, cx);
    }
    cx.notify();
  }

  /**
   * First launch, sign-in and Settings › Security steps for dev start routes.
   * Each types into the same cells a person does and presses the same
   * button, so every PIN is still checked by the host:
   * "setup=solo|helpdesk", "setuppin=<pin>" (logs the recovery key once),
   * "keydone", "newtp=<root>:<name>", "lock", "tin=<root>", "forgot",
   * "signin=<root>:<pin>", "override=<admin pin>", "recover=<key>",
   * "mypin=<pin>", "arm" (arms Delete on the Tax profile, never deletes),
   * "pick" (Enter in search). Settings › Computer setup: "chg" (Change),
   * "chg=owner|helpdesk", "chgpick=<root>", "chgnewpin=<pin>",
   * "chgconfirm=<admin pin>" (logs the new recovery key), "chgdone".
   */
  runAccessStep(key, value, cx) {
    const [a, b = ""] = value.split(":");
    if (key === "setup") return a === "solo" ? this.chooseSolo(cx) : this.chooseHelpdesk(cx);
    if (key === "setuppin") {
      this.setup.pinOtp.set_value(a);
      this.submitHelpdeskPin(cx);
      if (this.setup.key) console.log(`dev route: recovery key ${this.setup.key}`);
      return;
    }
    if (key === "keydone") return this.finishHelpdeskSetup(cx);
    if (key === "newtp") {
      if (!this.taxpayerUi.newTp) this.taxpayerUi.openNewTaxpayer(cx);
      this.taxpayerUi.newTp.tin.set(a);
      this.taxpayerUi.newTp.name.set_value(b);
      return this.taxpayerUi.createTaxpayer(cx);
    }
    if (key === "lock") return this.lockApp(cx);
    if (key === "tin") return this.signIn.tin.set(a);
    if (key === "forgot") return this.toggleForgot(cx);
    if (key === "signin") {
      this.signIn.tin.set(a);
      this.tpPinOtp.set_value(b);
      return this.submitSignIn(cx);
    }
    if (key === "override") {
      this.adminPinOtp.set_value(a);
      return this.submitOverride(cx);
    }
    if (key === "recover") {
      this.recoveryInput.set_value(value);
      return this.submitRecoveryKey(cx);
    }
    if (key === "mypin") {
      this.security.pinOtp.set_value(a);
      return this.security.setPin(cx);
    }
    if (key === "arm") return this.taxpayerUi.deleteTaxpayer(cx);
    if (key === "pick") return this.runPaletteItem(this.palette.index, cx);
    const sc = this.setupChange;
    if (key === "chg") return sc.go(a === "owner" ? "owner" : a === "helpdesk" ? "helpdesk" : "choose", cx);
    if (key === "chgpick") {
      const t = sc.candidates().find((c) => c.tin_root === a);
      if (t) sc.pickOwner(t, cx);
      return;
    }
    if (key === "chgnewpin") {
      sc.newPinOtp.set_value(a);
      return sc.pickHelpdeskPin(cx);
    }
    if (key === "chgconfirm") {
      sc.confirmPinOtp.set_value(a);
      sc.confirm(cx);
      if (sc.recoveryKey) console.log(`dev route: new recovery key ${sc.recoveryKey}`);
      return;
    }
    if (key === "chgdone") return sc.done(cx);
  }

  /**
   * Run `fn` once the form page's draft has loaded (dev start routes). Steps
   * run one at a time in the order the route names them, so `choose` always
   * lands before `save`.
   */
  afterFormLoads(cx, fn) {
    const queue = (this.formSteps = this.formSteps || []);
    queue.push(fn);
    if (queue.length > 1) return;
    const tick = (cx) => {
      const s = this.formUi.state;
      if (!(s && s.draft && !s.loading)) return cx.timer.after(150, tick);
      const step = queue.shift();
      step(cx);
      if (queue.length) cx.timer.after(150, tick);
    };
    cx.timer.after(150, tick);
  }

  // ---------------------------------------------------------------- local storage

  /**
   * Local state that names taxpayers (selection, Recent, read notifications)
   * is kept per database: localStorage belongs to the app, and a scratch
   * database must not inherit another database's taxpayers.
   */
  localKey(key) {
    return key === PREFS_KEY ? key : `${key}@${this.info.database_path || ""}`;
  }

  readLocal(key, fallback) {
    try {
      const raw = window.localStorage.getItem(this.localKey(key));
      return raw ? JSON.parse(raw) : fallback;
    } catch (_e) {
      return fallback;
    }
  }

  writeLocal(key, value) {
    try {
      window.localStorage.setItem(this.localKey(key), JSON.stringify(value));
    } catch (_e) {}
  }

  // ---------------------------------------------------------------- selection

  /** Taxpayers this session may see: one after a taxpayer sign-in, else all. */
  taxpayers() {
    return this.store.taxpayers();
  }

  /** The selected taxpayer, or the first one. */
  taxpayer() {
    const list = this.taxpayers();
    return list.find((t) => t.tin_root === this.selRoot) || list[0] || null;
  }

  branch() {
    const t = this.taxpayer();
    if (!t) return null;
    return t.branches.find((b) => b.code === this.selCode) || t.branches[0] || null;
  }

  /** No taxpayer is open: none exist yet. */
  isFresh() {
    return !this.taxpayer();
  }

  /** The branch's tax years, oldest first. */
  years() {
    const b = this.branch();
    return b ? this.store.taxYears(b.tin) : [];
  }

  /** The selected year, kept valid: this year if the branch has it, else its latest. */
  year() {
    const ys = this.years();
    if (!ys.includes(this.selYear)) {
      const now = new Date().getFullYear();
      this.selYear = ys.includes(now) ? now : ys.length ? ys[ys.length - 1] : null;
    }
    return this.selYear;
  }

  yearForms() {
    const b = this.branch();
    const y = this.year();
    return b && y ? this.store.yearlyForms(b.tin, y) : [];
  }

  templates() {
    const b = this.branch();
    return b ? this.store.templates(b.tin) : [];
  }

  selectTaxpayer(root, cx, opts = {}) {
    const t = this.taxpayers().find((x) => x.tin_root === root);
    if (!t) return;
    this.selRoot = root;
    this.selCode = opts.code && t.branches.some((b) => b.code === opts.code) ? opts.code : t.branches[0] ? t.branches[0].code : null;
    this.selYear = null;
    this.taxpayerUi.reset();
    this.writeLocal(SELECTION_KEY, { root: this.selRoot, code: this.selCode });
    if (!opts.quiet) this.toast(`Switched to ${t.name}`, cx);
    cx.notify();
  }

  selectBranch(code, cx) {
    this.selCode = code;
    this.selYear = null;
    this.taxpayerUi.reset();
    this.writeLocal(SELECTION_KEY, { root: this.selRoot, code: this.selCode });
    cx.notify();
  }

  selectYear(year, cx) {
    this.selYear = year;
    this.taxpayerUi.leaveYear();
    cx.notify();
  }

  // ---------------------------------------------------------------- navigation

  narrow() {
    return window.viewport_size().width < BREAK.overlays;
  }

  go(view, cx) {
    // A removal's Undo is for the page it was offered on.
    this.formUi?.leavingPage?.();
    if (view !== this.view) {
      this.history.push(this.view);
      this.forward = [];
      this.enterView(view, cx);
    }
    this.closeTransient();
    cx.notify();
  }

  goBack(cx) {
    if (!this.history.length) return;
    this.forward.push(this.view);
    this.enterView(this.history.pop(), cx);
    this.closeTransient();
    cx.notify();
  }

  goForward(cx) {
    if (!this.forward.length) return;
    this.history.push(this.view);
    this.enterView(this.forward.pop(), cx);
    this.closeTransient();
    cx.notify();
  }

  /** Switch views with the cleanup every way of leaving one needs. */
  enterView(view, cx) {
    // A recovery key on screen is shown once: leaving Settings hides it for
    // good. A finished Computer setup change also signs the old admin out.
    if (this.view === "settings") {
      this.security.leave();
      this.setupChange.leave(cx);
    }
    this.view = view;
    this.taxpayerUi.leaveView();
  }

  /** Moving anywhere closes the account menu, and the overlays on a narrow window. */
  closeTransient() {
    this.accountMenu = false;
    if (this.narrow()) {
      this.sidebarOpen = false;
      this.panel = null;
    }
  }

  toggleSidebar(cx) {
    this.sidebarOpen = !this.sidebarOpen;
    if (this.sidebarOpen && this.narrow()) this.panel = null;
    this.accountMenu = false;
    cx.notify();
  }

  /** A panel closes from its own trigger (the pill or the bell) and Esc. */
  togglePanel(which, cx) {
    this.panel = this.panel === which ? null : which;
    if (this.panel) this.lastPanel = this.panel;
    if (this.panel && this.narrow()) this.sidebarOpen = false;
    this.taxpayerUi.panelEdit = null;
    cx.notify();
  }

  /** The title-bar panel button (⌘⌥B): hide the panel, or reopen the last one. */
  toggleRightPanel(cx) {
    this.togglePanel(this.panel || this.lastPanel, cx);
  }

  openPalette(cx) {
    // An open removal question is answered Cancel first, as Esc does.
    if (this.formUi?.askingRemoval?.()) this.formUi.cancelRemoval(cx);
    // The palette replaces any stored-return offer on screen.
    this.existingOffer = null;
    this.overlay = "palette";
    this.palette.index = 0;
    this.palette.input.set_value("");
    cx.notify();
  }

  closeOverlay(cx) {
    // Esc or a click outside the stored-return offer is "Keep this form".
    if (this.overlay === "existing" && this.existingOffer) return this.formUi.takeExisting("keep", cx);
    // Esc on "this removes the rows you typed" is Cancel.
    if (this.overlay === "removal") return this.formUi.cancelRemoval(cx);
    this.existingOffer = null;
    this.samplePeriod = null;
    this.overlay = null;
    this.newReturn.periodOpen = false;
    this.focusRoot.focus();
    cx.notify();
  }

  /** Esc: the topmost thing closes — an overlay, then a menu, then a panel. */
  dismiss(cx) {
    if (this.overlay) return this.closeOverlay(cx);
    if (this.accountMenu) {
      this.accountMenu = false;
      return cx.notify();
    }
    if (this.panel) {
      this.panel = null;
      return cx.notify();
    }
    if (this.narrow() && this.sidebarOpen) {
      this.sidebarOpen = false;
      cx.notify();
    }
  }

  // ---------------------------------------------------------------- new return

  /** The title-bar / sidebar "+": the start-a-return dialog for the branch's year. */
  startNewReturn(cx, code) {
    // An open removal question is answered Cancel first, as Esc does.
    if (this.formUi?.askingRemoval?.()) this.formUi.cancelRemoval(cx);
    if (this.isFresh()) return this.toast(this.freshHint(), cx);
    const b = this.branch();
    if (!this.years().length) {
      return this.toast(`${b.name} has no tax year yet. Add one to choose the forms it files.`, cx);
    }
    const forms = this.yearForms();
    if (!forms.length) return this.toast(`No forms for ${this.year()} yet. Choose them under Forms first.`, cx);
    if (code && !forms.includes(code)) {
      return this.toast(`${this.taxpayer().name} does not file ${code} in ${this.year()}. Add it under Forms first.`, cx);
    }
    this.newReturn = { code: code || forms[0], period: null, periodOpen: false };
    // New return replaces any stored-return offer on screen.
    this.existingOffer = null;
    this.overlay = "new";
    cx.notify();
  }

  // ---------------------------------------------------------------- forms

  /** Open a return on the form page. `period`: month, quarter or 0 for annual. */
  /** The branch a full TIN names in this database, if any. */
  branchFor(tin) {
    const t = this.taxpayers().find((x) => x.tin_root === String(tin).slice(0, 9));
    return t ? t.branches.find((b) => b.tin === String(tin)) || null : null;
  }

  /** Why nothing can be opened yet. */
  freshHint() {
    return "Add a taxpayer first, then open a form for them.";
  }

  /**
   * `keep`: the form on screen stays alive for an Undo; `then(error, cx)`
   * runs once the host answered (see FormPageActions.open).
   */
  openForm(opts, cx, keep = false, then = null) {
    if (this.isFresh()) return this.toast(this.freshHint(), cx);
    if (!this.branchFor(opts.tin)) {
      return this.toast("That return belongs to a taxpayer that is not in this database.", cx, "triangle-alert");
    }
    this.overlay = null;
    this.formUi.open(opts, cx, keep, then);
    // A library sample is no return, so it is not a recent one.
    if (!opts.sample) this.pushRecent(opts);
    this.go("form", cx);
  }

  /** Rewrite Recent after a TIN or branch-code move, so the entries still open. */
  remapRecent(map) {
    this.recent = this.recent.map((r) => recentEntry({ ...r, tin: map(String(r.tin)) }));
    this.writeLocal(RECENT_KEY, this.recent);
  }

  pushRecent(opts) {
    const entry = recentEntry(opts);
    this.recent = [entry, ...this.recent.filter((r) => r.key !== entry.key)].slice(0, 4);
    this.writeLocal(RECENT_KEY, this.recent);
  }

  // ---------------------------------------------------------------- theme

  setThemePreference(pref, cx) {
    this.themePref = pref;
    try {
      this.store.write("set_setting", THEME_PREF_KEY, storedThemePreference(pref));
    } catch (_e) {}
    applyThemePreference(cx, pref);
    cx.notify();
  }

  cycleTheme(cx) {
    this.setThemePreference(nextThemePreference(this.themePref), cx);
  }

  setPref(key, value, cx) {
    this.prefs[key] = value;
    this.writeLocal(PREFS_KEY, this.prefs);
    cx.notify();
  }

  // ---------------------------------------------------------------- notifications

  /**
   * Submissions sent and confirmation emails received, newest first. The
   * return's own status is the record: Submitted when the worker sent it,
   * Confirmed (or Paid) once BIR's confirmation email was matched to it.
   */
  notifications() {
    const items = [];
    for (const r of this.store.returns("filed")) {
      const confirmed = r.status === "Confirmed" || r.status === "Paid";
      if (!confirmed && r.status !== "Submitted") continue;
      const id = `return:${r.id}:${confirmed ? "confirmed" : "submitted"}`;
      const who = r.branch_count > 1 ? `${r.taxpayer_name} · ${r.branch_name}` : r.taxpayer_name;
      items.push({
        id,
        title: confirmed
          ? `BIR confirmed ${r.form_code} for ${r.period_label}`
          : `${r.form_code} for ${r.period_label} was submitted`,
        sub: confirmed ? `Confirmation email received · ${who}` : `Waiting for BIR's confirmation email · ${who}`,
        icon: confirmed ? "circle-check" : "send",
        tone: confirmed ? "success" : "neutral",
        when: r.updated_at,
        unread: !this.readIds.has(id),
      });
    }
    return items;
  }

  /**
   * Re-read what the submission worker changes behind this app's back
   * (returns, their deadlines and yearly form rows, background jobs), redraw,
   * and announce events that appeared since the last look.
   */
  pollEvents(cx) {
    this.store.forgetWhere((key) => /^(returns:|deadlines|forms:|jobs$)/.test(key));
    const fresh = this.notifications().filter((n) => !this.knownEvents.has(n.id));
    for (const n of fresh) this.knownEvents.add(n.id);
    if (fresh.length) {
      this.toast(fresh.length === 1 ? fresh[0].title : `${fresh.length} filing updates. Open notifications to see them.`, cx, fresh[0].icon);
    }
    cx.notify();
  }

  markAllRead(cx) {
    for (const n of this.notifications()) this.readIds.add(n.id);
    this.writeLocal(READ_KEY, [...this.readIds]);
    cx.notify();
  }

  // ---------------------------------------------------------------- toast

  /**
   * "<what happened>. Undo" for six seconds with a countdown bar; Undo (or
   * Cmd/Ctrl+Z while it shows) runs `undo` once. One level: a newer toast
   * replaces it.
   */
  toastUndo(message, undo, cx) {
    const id = (this.toastState ? this.toastState.id : 0) + 1;
    const total = 6000;
    const ends = Date.now() + total;
    this.toastState = { id, message, icon: "undo-2", action: { label: "Undo", run: undo }, ends, total, progress: 1 };
    const tick = (cx) => {
      const st = this.toastState;
      if (!st || st.id !== id) return;
      const left = st.ends - Date.now();
      if (left <= 0) {
        this.toastState = null;
        this.formUi.dropKept();
      } else {
        st.progress = left / st.total;
        cx.timer.after(200, tick);
      }
      this.invalidate();
    };
    cx.timer.after(200, tick);
    this.invalidate();
  }

  /**
   * Undo the last action while its toast shows. Never while a text box has
   * the keyboard: Cmd/Ctrl-Z there undoes typing, not a modal's choice.
   */
  undoLast(cx) {
    const st = this.toastState;
    if (!st || !st.action) return;
    // Nothing is undone under the "this removes the rows you typed" question.
    if (this.formUi?.askingRemoval?.()) return;
    if (this.textHasFocus()) return;
    this.toastState = null;
    st.action.run(cx);
    this.invalidate();
  }

  /** A text box has the keyboard (a form box, or the palette's search). */
  textHasFocus() {
    return Boolean(this.formUi.state?.focusKey) || Boolean(this.overlay === "palette" && this.palette.focused);
  }

  toast(message, cx, icon) {
    const id = (this.toastState ? this.toastState.id : 0) + 1;
    this.toastState = { id, message, icon };
    cx.timer.after(2600, (cx) => {
      if (this.toastState && this.toastState.id === id) {
        this.toastState = null;
        this.invalidate();
      }
    });
    this.invalidate();
  }

  // ---------------------------------------------------------------- palette

  paletteItems() {
    const q = this.palette.input.value().trim().toLowerCase();
    const fresh = this.isFresh();
    const items = [];
    // A TIN typed as digits, with or without dashes: 10 or more digits name
    // a branch (root + branch code), so the matching branches are listed.
    const digits = q.replace(/[\s-]/g, "");
    const byTin = /^\d+$/.test(digits);
    for (const t of this.taxpayers()) {
      if (byTin && digits.length > 9) {
        for (const b of t.branches) {
          if (!b.tin.startsWith(digits)) continue;
          items.push({
            group: "Taxpayers",
            label: t.branches.length > 1 ? `${t.name} · ${b.name}` : t.name,
            hint: b.tin_display,
            avatar: t,
            match: true,
            run: (cx) => this.openBranch(t.tin_root, b.code, cx),
          });
        }
        continue;
      }
      items.push({
        group: "Taxpayers",
        label: t.name,
        hint: t.tin_display,
        avatar: t,
        match: byTin && t.tin_root.startsWith(digits),
        run: (cx) => this.openTaxpayer(t.tin_root, cx),
      });
      // Signed in as one taxpayer, its branches are one step away.
      if (this.session.kind === "taxpayer" && t.branches.length > 1 && !byTin) {
        for (const b of t.branches) {
          items.push({ group: "Branches", label: b.name, hint: b.tin_display, icon: "git-branch", run: (cx) => this.openBranch(t.tin_root, b.code, cx) });
        }
      }
    }
    const pages = [
      ["home", "house", "Home"],
      ["library", "file-text", "Form library"],
      ["calendar", "calendar", "Calendar"],
      ["filings", "history", "Filings"],
      ["profile", "user", "Tax profile"],
      ["forms", "file-text", "Forms"],
      ["templates", "layout-template", "Templates"],
      ["settings", "settings", "Settings"],
    ];
    for (const [view, icon, label] of pages) {
      if (fresh && TAXPAYER_VIEWS.has(view)) continue;
      items.push({ group: "Go to", label, icon, run: (cx) => this.go(view, cx) });
    }
    if (q) {
      for (const f of this.store.screenIndex()) {
        items.push({ group: "Forms", label: `${f.c} · ${f.t}`, icon: "file-text", run: (cx) => this.startNewReturn(cx, f.c) });
      }
    }
    items.push({
      group: "Actions",
      label: "Toggle sidebar",
      icon: "panel-left",
      hint: this.isMac ? "⌘B" : "Ctrl B",
      run: (cx) => this.toggleSidebar(cx),
    });
    items.push({
      group: "Actions",
      label: this.panel ? "Hide right panel" : "Show right panel",
      icon: "panel-right",
      hint: this.isMac ? "⌘⌥B" : "Ctrl Alt B",
      run: (cx) => this.toggleRightPanel(cx),
    });
    items.push({ group: "Actions", label: "Change theme", icon: "moon", run: (cx) => this.cycleTheme(cx) });
    items.push({ group: "Actions", label: "Components", icon: "layout-template", run: (cx) => this.go("gallery", cx) });
    if (!q) return items;
    return items.filter((x) => x.match || `${x.label} ${x.hint || ""}`.toLowerCase().includes(q));
  }

  /** Search picked a taxpayer: it opens on its Tax profile, wherever search was opened from. */
  openTaxpayer(root, cx) {
    if (!this.taxpayer() || this.taxpayer().tin_root !== root) this.selectTaxpayer(root, cx);
    this.go("profile", cx);
  }

  /** Search picked a branch: select it and open the Tax profile, as for a taxpayer. */
  openBranch(root, code, cx) {
    if (this.taxpayer() && this.taxpayer().tin_root === root) this.selectBranch(code, cx);
    else this.selectTaxpayer(root, cx, { code });
    this.go("profile", cx);
  }

  runPaletteItem(index, cx) {
    const items = this.paletteItems();
    const item = items[index];
    this.overlay = null;
    this.focusRoot.focus();
    if (item) item.run(cx);
    else cx.notify();
  }

  /**
   * Scripts cannot move the keyboard into an Input, so the palette opens with
   * the root focused; until the field is clicked, the root feeds it typed
   * characters and handles the arrows and Enter.
   */
  paletteKey(e, cx) {
    if (this.overlay !== "palette" || this.palette.focused) return;
    const input = this.palette.input;
    if (e.key === "down" || e.key === "up") return this.movePalette(e.key === "down" ? 1 : -1, cx);
    if (e.key === "enter") return this.runPaletteItem(this.palette.index, cx);
    if (e.key === "backspace") {
      input.set_value(input.value().slice(0, -1));
    } else if (e.key_char && !e.modifiers.platform && !e.modifiers.control) {
      input.set_value(input.value() + e.key_char);
    } else return;
    this.palette.index = 0;
    cx.notify();
  }

  movePalette(delta, cx) {
    const n = this.paletteItems().length;
    if (!n) return;
    this.palette.index = (this.palette.index + delta + n) % n;
    cx.notify();
  }

  // ---------------------------------------------------------------- auth
  //
  // The admin is whoever set this computer up. First launch picks the access
  // mode: "solo" (just me or my business: the owner's own taxpayer is the
  // admin account, and its PIN is the admin PIN) or "helpdesk" (an RDO desk,
  // an accountant: the admin account is the reserved TIN 000-000-000, and
  // every other taxpayer signs in as itself).
  //
  // Sessions: "admin" (every taxpayer, the admin settings), "taxpayer" (only
  // that taxpayer and its branches) and "open" (nobody signed in). "open"
  // exists only while the admin has no PIN or authenticator, so the person at
  // the keyboard is the owner and acts as the admin. A PIN guards one
  // taxpayer's sign-in, never the whole app: nobody can lock everyone out.

  initAuth() {
    this.session = { kind: "open" };
    this.appLocked = this.adminHasCode();
    this.authError = null;
    // Sign-in codes (the TIN's own PIN or authenticator), the admin's codes
    // ("Forgot PIN? Ask the admin") and the admin's recovery key. The last
    // digit of a code submits.
    this.tpPinOtp = OtpState.new(4, { masked: true });
    this.tpTotpOtp = OtpState.new(6);
    this.adminPinOtp = OtpState.new(4, { masked: true });
    this.adminTotpOtp = OtpState.new(6);
    this.tpPinOtp.on("complete", (_e, cx) => this.submitSignIn(cx));
    this.tpTotpOtp.on("complete", (_e, cx) => this.submitSignIn(cx));
    this.adminPinOtp.on("complete", (_e, cx) => this.submitOverride(cx));
    this.adminTotpOtp.on("complete", (_e, cx) => this.submitOverride(cx));
    this.recoveryInput = InputState.new({ placeholder: "XXXX-XXXX-XXXX-XXXX" });
    this.recoveryInput.on("submit", (_e, cx) => this.submitRecoveryKey(cx));
    this.signIn = { method: "pin", forgot: false, busy: false, tin: new DigitEntry([3, 3, 3]) };
    this.signIn.tin.groups[2].on("complete", () => this.tpPinOtp.focus());
    // The cells below the TIN depend on whose TIN it is (the admin's recovery
    // or a taxpayer's), so typing it redraws the card.
    this.signIn.tin.onChange = (cx) => {
      this.authError = null;
      cx.notify();
    };
    // First launch: the chosen mode's next step, and "solo" while its first
    // taxpayer is being typed on the New taxpayer page.
    this.setup = { step: "choose", pinOtp: OtpState.new(4, { masked: true }), key: null };
    this.setupPending = null;
    this.security = new SecurityActions(this);
    this.setupChange = new SetupChange(this);
    this.applyScope();
  }

  authState() {
    return this.store.authState();
  }

  /** First launch has not picked "Just me" or "I help other taxpayers" yet. */
  needsSetup() {
    return !this.authState().mode;
  }

  isHelpdesk() {
    return this.authState().mode === "helpdesk";
  }

  /** The admin can sign in with a PIN or an authenticator code. */
  adminHasCode() {
    const a = this.authState();
    return Boolean(a.has_admin_pin || a.has_admin_totp);
  }

  /** The admin account's TIN root: the owner's taxpayer, or 000000000 for a help desk. */
  adminRoot() {
    return this.authState().admin_root || null;
  }

  /** Admin rights: signed in as the admin, or nobody signed in while the admin has no code. */
  isAdmin() {
    return this.session.kind === "admin" || (this.session.kind === "open" && !this.adminHasCode());
  }

  /**
   * New taxpayers: the first one on an empty computer, and afterwards only a
   * help desk's admin. "Just me" stays one taxpayer with its branches.
   */
  canAddTaxpayer() {
    if (!this.taxpayers().length) return true;
    return this.isAdmin() && this.isHelpdesk();
  }

  /** Point the store's visibility at this session. */
  applyScope() {
    const scope = this.session.kind === "taxpayer" ? this.session.root : null;
    this.store.scopeRoot = scope;
    this.store.canSee = (root) => !scope || root === scope;
  }

  /** PIN and authenticator of a taxpayer (shared by all its branches). */
  taxpayerCreds(root) {
    return this.store.read(`tpauth:${root}`, "taxpayer_auth", [root], { has_pin: false, has_totp: false });
  }

  clearCodes() {
    for (const otp of [this.adminPinOtp, this.adminTotpOtp, this.tpPinOtp, this.tpTotpOtp]) otp.set_value("");
    this.recoveryInput.set_value("");
  }

  toggleSignInMethod(cx) {
    this.signIn.method = this.signIn.method === "totp" ? "pin" : "totp";
    this.authError = null;
    this.clearCodes();
    cx.notify();
  }

  toggleForgot(cx) {
    this.signIn.forgot = !this.signIn.forgot;
    this.authError = null;
    this.clearCodes();
    cx.notify();
  }

  /** The TIN typed on the sign-in screen: the admin account, a taxpayer, or why neither. */
  signInTarget() {
    const root = this.signIn.tin.value();
    if (root.length !== 9) return { error: "Type all 9 digits of the TIN." };
    if (root === this.adminRoot()) return { root, admin: true };
    if (!this.store.read("taxpayers", "list_taxpayers", [], []).some((t) => t.tin_root === root)) {
      return { error: "No taxpayer has that TIN on this computer." };
    }
    return { root, admin: false };
  }

  fail(message, cx) {
    this.authError = message;
    this.clearCodes();
    cx.notify();
  }

  /** TIN plus that account's own PIN or authenticator code. */
  submitSignIn(cx) {
    const target = this.signInTarget();
    if (target.error) return this.fail(target.error, cx);
    const totp = this.signIn.method === "totp";
    const code = totp ? this.tpTotpOtp.value() : this.tpPinOtp.value();
    if (code.length !== (totp ? 6 : 4)) return this.fail(totp ? "Type all 6 digits of the code." : "Type all 4 digits of the PIN.", cx);
    let ok = false;
    try {
      if (target.admin) ok = this.store.call(totp ? "verify_admin_totp" : "verify_admin_pin", code);
      else ok = this.store.call(totp ? "verify_taxpayer_totp" : "verify_taxpayer_pin", target.root, code);
    } catch (e) {
      // Too many wrong tries, or the check itself failed: say which.
      return this.fail(errorText(e), cx);
    }
    if (!ok) {
      if (!target.admin) {
        const c = this.taxpayerCreds(target.root);
        if (!c.has_pin && !c.has_totp) return this.fail("That taxpayer has no PIN yet. Ask the admin to open it.", cx);
      }
      return this.fail(totp ? "Wrong TIN or code. Try again." : "Wrong TIN or PIN. Try again.", cx);
    }
    this.startSession(target.admin ? { kind: "admin" } : { kind: "taxpayer", root: target.root }, cx);
  }

  /** Why the admin's PIN or authenticator code as typed does not open; null when one matches. */
  adminCodeError() {
    const a = this.authState();
    try {
      if (a.has_admin_pin && this.adminPinOtp.value().length === 4 && this.store.call("verify_admin_pin", this.adminPinOtp.value())) return null;
      if (a.has_admin_totp && this.adminTotpOtp.value().length === 6 && this.store.call("verify_admin_totp", this.adminTotpOtp.value())) return null;
    } catch (e) {
      return errorText(e);
    }
    return "Wrong admin PIN or code. Try again.";
  }

  /** "Forgot PIN? Ask the admin": the admin's code opens the taxpayer typed above. */
  submitOverride(cx) {
    const target = this.signInTarget();
    if (target.error) return this.fail(target.error, cx);
    if (target.admin) return this.fail("That is the admin's TIN. Use the recovery key or your Mac login below.", cx);
    const error = this.adminCodeError();
    if (error) return this.fail(error, cx);
    this.startSession({ kind: "taxpayer", root: target.root }, cx);
    this.toast("Opened by the admin. Set a new PIN in Settings.", cx, "lock");
  }

  /** The admin's one-time recovery key: signs the admin in, then a new PIN is set in Settings. */
  submitRecoveryKey(cx) {
    const key = this.recoveryInput.value().trim();
    if (!key) return this.fail("Type the recovery key you wrote down when the admin PIN was set.", cx);
    let ok = false;
    try {
      ok = this.store.write("use_recovery_key", key);
    } catch (e) {
      return this.fail(errorText(e), cx);
    }
    if (!ok) return this.fail("That recovery key does not match. Check it and try again.", cx);
    this.recoveredAdmin(cx, "Recovery key used. It works only once: set a new admin PIN and make a new key.");
  }

  /**
   * Last resort: whoever can log in to this Mac can reset the admin PIN. The
   * click's `cx` ends with the click, so the wait runs in a task whose
   * context outlives the prompt.
   */
  submitMacLogin(cx) {
    if (this.signIn.busy) return;
    this.signIn.busy = true;
    this.authError = null;
    cx.notify();
    cx.spawn(async (acx) => {
      let ok = false;
      let error = null;
      try {
        ok = await this.store.call("mac_login", "reset the eBIRForms admin PIN");
      } catch (e) {
        error = errorText(e);
      }
      this.signIn.busy = false;
      if (!ok) return this.fail(error || "Mac login was cancelled or did not match.", acx);
      this.recoveredAdmin(acx, "Signed in with your Mac login. Set a new admin PIN now.");
    });
  }

  recoveredAdmin(cx, message) {
    this.startSession({ kind: "admin" }, cx);
    this.go("settings", cx);
    this.toast(message, cx, "lock");
  }

  startSession(session, cx) {
    const same = this.lockedFrom && this.lockedFrom.kind === session.kind && this.lockedFrom.root === session.root;
    this.session = session;
    this.appLocked = false;
    this.lockedFrom = null;
    this.authError = null;
    this.clearCodes();
    this.signIn.forgot = false;
    this.signIn.tin.set("");
    this.store.bump();
    this.applyScope();
    if (session.kind === "taxpayer") {
      this.selRoot = session.root;
      const t = this.taxpayers()[0];
      if (!same || !t || !t.branches.some((b) => b.code === this.selCode)) this.selCode = t && t.branches[0] ? t.branches[0].code : null;
      this.writeLocal(SELECTION_KEY, { root: this.selRoot, code: this.selCode });
    }
    if (!same) {
      // A different person: start from Home, with nothing of the last session.
      this.view = "home";
      this.history = [];
      this.forward = [];
      this.panel = null;
      this.overlay = null;
      this.taxpayerUi.reset();
      this.security.leave();
    }
    cx.notify();
  }

  /** The admin has no code, so there is nothing to sign in with: the owner is at the keyboard. */
  openWithoutSignIn(cx) {
    if (this.adminHasCode()) return;
    this.startSession({ kind: "open" }, cx);
  }

  /** Lock app and Log out both end the session and show sign-in. */
  endSession(cx, message) {
    this.accountMenu = false;
    if (this.session.kind !== "taxpayer" && !this.adminHasCode()) {
      return this.toast("Set your PIN in Settings first. With no PIN there is nothing to sign back in with.", cx, "lock");
    }
    this.lockedFrom = { kind: this.session.kind, root: this.session.root };
    this.session = { kind: "open" };
    this.fastLane = { drafts: false };
    this.panel = null;
    this.overlay = null;
    this.existingOffer = null;
    this.signIn.method = "pin";
    this.signIn.forgot = false;
    this.signIn.tin.set("");
    this.authError = null;
    this.clearCodes();
    this.security.leave();
    this.setupChange.reset();
    this.store.bump();
    this.applyScope();
    this.appLocked = true;
    if (message) this.toast(message, cx, "log-out");
    cx.notify();
  }

  lockApp(cx) {
    this.endSession(cx);
  }

  /** Computer setup changed: the admin moved, so the old session ends. */
  signOutAfterSetupChange(cx) {
    this.endSession(cx, "Computer setup changed. Sign in as the admin with the new owner's PIN or the help desk PIN.");
    this.lockedFrom = null;
  }

  logOut(cx) {
    this.endSession(cx, "Signed out.");
    // A sign-in after Log out starts from Home, whoever signs in.
    this.lockedFrom = null;
  }

  // ---------------------------------------------------------------- first launch

  /** "Just me or my business": the owner's taxpayer becomes the admin account. */
  chooseSolo(cx) {
    const first = this.store.read("taxpayers", "list_taxpayers", [], [])[0];
    if (first) {
      try {
        this.store.write("setup_solo", first.tin_root);
      } catch (e) {
        return this.toast(String(e), cx, "triangle-alert");
      }
      return cx.notify();
    }
    // No taxpayer yet: type it first; Create finishes the setup.
    this.setupPending = "solo";
    this.taxpayerUi.openNewTaxpayer(cx);
  }

  /** The New taxpayer page was left during first launch: back to the choice. */
  cancelSetup(cx) {
    this.setupPending = null;
    this.setup.step = "choose";
    cx.notify();
  }

  /** Called once the first taxpayer exists, when first launch chose "solo". */
  finishSoloSetup(root) {
    if (this.setupPending !== "solo") return;
    this.setupPending = null;
    this.store.write("setup_solo", root);
  }

  /** "I help other taxpayers": choose the admin PIN, then show the recovery key once. */
  chooseHelpdesk(cx) {
    this.setup.step = "helpdesk-pin";
    this.setup.pinOtp.set_value("");
    this.authError = null;
    cx.notify();
  }

  submitHelpdeskPin(cx) {
    const pin = this.setup.pinOtp.value();
    if (pin.length !== 4) return this.fail("Type all 4 digits of the admin PIN.", cx);
    try {
      this.setup.key = this.store.write("setup_helpdesk", pin).recovery_key;
      this.setup.step = "helpdesk-key";
      this.authError = null;
    } catch (e) {
      this.authError = String(e);
    }
    cx.notify();
  }

  /** The recovery key was written down: the admin starts on an empty Home. */
  finishHelpdeskSetup(cx) {
    // Only the key screen right after choosing the PIN signs the admin in.
    if (this.setup.step !== "helpdesk-key") return;
    this.setup = { step: "choose", pinOtp: this.setup.pinOtp, key: null };
    this.startSession({ kind: "admin" }, cx);
  }

  // ---------------------------------------------------------------- settings actions (retained)

  runImport(cx) {
    try {
      const result = this.store.write("import_archive");
      if (result.kind === "database") this.toast("Database imported.", cx, "upload");
      else if (result.kind === "profile") this.toast("Taxpayer data restored.", cx, "upload");
      else if (result.kind === "invalid") {
        this.toast("That zip has no taxpayer or database export in it.", cx, "triangle-alert");
      } else cx.notify();
    } catch (e) {
      this.toast(`Import failed: ${e}`, cx, "triangle-alert");
    }
  }

  // ---------------------------------------------------------------- background tasks (retained page)

  backgroundJobs() {
    return this.store.backgroundJobs();
  }

  setCronTab(tab, cx) {
    this.cronTab = tab;
    if (tab === "logs") this.jobLogCache = null;
    cx.notify();
  }

  setCronFilter(name, cx) {
    this.cronFilter = name;
    cx.notify();
  }

  deleteJob(id, cx) {
    try {
      this.store.write("delete_background_job", id);
    } catch (e) {
      this.cronError = String(e);
    }
    cx.notify();
  }

  archiveJob(id, cx) {
    try {
      this.store.write("archive_background_job", id);
    } catch (e) {
      this.cronError = String(e);
    }
    cx.notify();
  }

  purgeArchivedJobs(cx) {
    try {
      this.store.write("purge_archived_jobs");
      this.cronFilter = "All Jobs";
    } catch (e) {
      this.cronError = String(e);
    }
    cx.notify();
  }

  jobLogTail() {
    if (this.jobLogCache === null || this.jobLogCache === undefined) {
      try {
        this.jobLogCache = this.store.call("job_log_tail");
      } catch (e) {
        // The shape logsTab reads, with the reason it shows.
        this.jobLogCache = { file_name: "", lines: 0, content: "", error: errorText(e) };
      }
    }
    return this.jobLogCache;
  }

  refreshJobLog(cx) {
    this.jobLogCache = null;
    cx.notify();
  }

  cancelQueuedEntry(entry, cx) {
    try {
      this.store.write("cancel_queued", entry.tin, entry.form_code, entry.taxable_year, entry.quarter ?? entry.month ?? null);
      this.toast("Queue cancelled. The return is an editable draft again.", cx);
    } catch (e) {
      this.toast(String(e), cx, "triangle-alert");
    }
  }

  // ---------------------------------------------------------------- render

  renderView(cx, width) {
    let view = this.view;
    if (this.isFresh() && TAXPAYER_VIEWS.has(view)) view = "home";
    switch (view) {
      case "home":
        return renderHome(cx, this, width);
      case "library":
        return renderLibrary(cx, this, width);
      case "calendar":
        return renderCalendar(cx, this, width);
      case "filings":
        return renderFilings(cx, this, width);
      case "settings":
        return renderSettings(cx, this, width);
      case "profile":
      case "forms":
      case "templates":
        return renderTaxpayerPage(cx, this, view, width);
      case "new_taxpayer":
      case "new_branch":
        return renderNewTaxpayer(cx, this, width);
      case "form":
        return renderFormPage(cx, this, width);
      case "gallery":
        return renderGallery(cx, this, width);
      case "cron_tasks":
        return renderCronTasksPage(cx, this, width);
      default:
        return renderHome(cx, this, width);
    }
  }

  /** Width of the main surface's content column, from the window size. */
  layout() {
    const vp = window.viewport_size();
    const narrow = vp.width < BREAK.overlays;
    const sidebarDocked = this.sidebarOpen && !narrow;
    const panelDocked = Boolean(this.panel) && !narrow;
    const left = sidebarDocked ? SIZE.sidebar : SIZE.mainInset;
    const right = panelDocked ? SIZE.panel : SIZE.mainInset;
    const mainWidth = Math.max(240, vp.width - left - right - 2);
    const wide = this.view === "form" || this.view === "library";
    const padX = mainWidth < BREAK.narrowContent ? 16 : 32;
    const column = Math.min(wide ? 1120 : SIZE.contentMax, mainWidth) - padX * 2;
    return { vp, narrow, sidebarDocked, panelDocked, mainWidth, padX, column: Math.max(200, column) };
  }

  render(cx) {
    const t = tokens(cx);
    // First launch and sign-in take the whole window: nothing that names a
    // taxpayer is drawn behind them. First launch's "Just me" types its
    // taxpayer on the normal New taxpayer page; a help desk's recovery key
    // stays on screen after the mode is saved, until it was written down.
    if ((this.needsSetup() && this.setupPending !== "solo") || this.setup.step === "helpdesk-key") return renderSetup(cx, this);
    if (this.appLocked) return renderSignIn(cx, this);
    const L = this.layout();

    // A form on a wide window brings its own two scroll areas (the part
    // navigator pinned beside the form); everything else scrolls as one.
    const formSplit = this.view === "form" && !this.isFresh() ? renderFormSplit(cx, this, L) : null;
    const main = div()
      .id("main-surface")
      .flex()
      .flex_col()
      .flex_1()
      .min_w(0)
      .min_h(0)
      .mb(SIZE.mainInset)
      .when(!L.sidebarDocked, (el) => el.ml(SIZE.mainInset))
      .when(!L.panelDocked, (el) => el.mr(SIZE.mainInset))
      .rounded(RADIUS.card)
      .border(1)
      .border_color(t.border)
      .bg(t.surface)
      .overflow_hidden()
      .when(Boolean(formSplit), (el) => el.child(formSplit))
      .when(!formSplit, (el) => el.child(
        div()
          .id("main-scroll")
          .flex_1()
          .min_h(0)
          .overflow_y_scroll()
          .overflow_x_hidden()
          .child(
            div()
              .flex()
              .flex_col()
              .items_center()
              .w_full()
              .px(L.padX)
              .pt(L.mainWidth < BREAK.narrowContent ? 20 : 28)
              .pb(56)
              .child(div().flex().flex_col().w(L.column).child(this.renderView(cx, L.column))),
          ),
      ));

    const body = div().flex().flex_row().flex_1().min_h(0).w_full();
    if (L.sidebarDocked) body.child(renderSidebar(cx, this));
    body.child(main);
    if (L.panelDocked) body.child(renderRightPanel(cx, this));

    const root = div()
      .id("app-root")
      .relative()
      .flex()
      .flex_col()
      .size_full()
      .bg(t.bg)
      .text_color(t.fg)
      .track_focus(this.focusRoot)
      .on_action("toggle-sidebar", (_e, cx) => this.toggleSidebar(cx))
      .on_action("open-palette", (_e, cx) => this.openPalette(cx))
      .on_action("new-return", (_e, cx) => this.startNewReturn(cx))
      .on_action("toggle-theme", (_e, cx) => this.cycleTheme(cx))
      .on_action("toggle-right-panel", (_e, cx) => this.toggleRightPanel(cx))
      .on_action("open-settings", (_e, cx) => this.go("settings", cx))
      .on_action("lock-app", (_e, cx) => this.lockApp(cx))
      .on_action("go-back", (_e, cx) => this.goBack(cx))
      .on_action("go-forward", (_e, cx) => this.goForward(cx))
      .on_action("dismiss", (_e, cx) => this.dismiss(cx))
      .on_action("undo-last", (_e, cx) => this.undoLast(cx))
      .on_key_down((e, cx) => this.paletteKey(e, cx))
      .child(renderTitleBar(cx, this, L))
      .child(body);

    // Below 860 px the sidebar and the panel float over the main surface.
    if (L.narrow && (this.sidebarOpen || this.panel)) {
      root.child(
        div()
          .id("overlay-scrim")
          .absolute()
          .top(SIZE.titleBar)
          .left(0)
          .right(0)
          .bottom(0)
          .bg(t.scrim)
          .on_mouse_down("left", (_e, cx) => {
            this.sidebarOpen = false;
            this.panel = null;
            cx.notify();
          }),
      );
      if (this.sidebarOpen) {
        root.child(
          div()
            .absolute()
            .top(SIZE.titleBar)
            .left(0)
            .bottom(0)
            .bg(t.bg)
            .shadow_xl()
            .child(renderSidebar(cx, this)),
        );
      }
      if (this.panel) {
        root.child(
          div()
            .absolute()
            .top(SIZE.titleBar)
            .right(0)
            .bottom(0)
            .bg(t.bg)
            .shadow_xl()
            .child(renderRightPanel(cx, this)),
        );
      }
    }
    if (this.overlay === "palette") root.child(renderPalette(cx, this));
    if (this.overlay === "new") root.child(renderNewReturn(cx, this));
    if (this.overlay === "existing") root.child(renderExistingOffer(cx, this));
    if (this.overlay === "removal") root.child(renderRemovalConfirm(cx, this));
    if (this.overlay === "sample-period") root.child(renderSamplePeriod(cx, this));
    if (this.toastState) {
      const st = this.toastState;
      root.child(
        Toast(cx, st.message, st.icon, st.action ? { action: { label: st.action.label, onClick: (_e, cx) => this.undoLast(cx) }, progress: st.progress } : {}),
      );
    }
    return root;
  }
}
