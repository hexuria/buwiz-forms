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
import { Store } from "./shell/store.js";
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
import { renderLibrary } from "./shell/library.js";
import { renderCalendar } from "./shell/calendar_page.js";
import { renderFilings } from "./shell/filings.js";
import { renderSettings } from "./shell/settings.js";
import { renderTaxpayerPage, renderNewTaxpayer, TaxpayerActions } from "./shell/taxpayer.js";
import { renderFormPage, renderFormSplit, FormPageActions } from "./shell/form_page.js";
import { renderGallery } from "./shell/gallery.js";
import { renderSignIn, renderTaxpayerLock } from "./shell/auth.js";
import { renderProfileManager, blankProfile, parseTin } from "./shell/profile_manager.js";
import { renderCronTasksPage } from "./shell/cron_tasks.js";

/** Views that need a taxpayer; with none they fall back to Home. */
const TAXPAYER_VIEWS = new Set(["profile", "forms", "templates", "form"]);
const RECENT_KEY = "redesign.recent";
const SELECTION_KEY = "redesign.selection";
const READ_KEY = "redesign.read_notifications";
const PREFS_KEY = "redesign.prefs";
/** `selRoot` after Back on a taxpayer's lock with nothing open before it. */
const NO_TAXPAYER = "";

export default class BirApp extends View {
  init(_props, cx) {
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
    /** null | "palette" | "new" */
    this.overlay = null;
    this.accountMenu = false;
    this.toastState = null;

    const stored = this.readLocal(SELECTION_KEY, {});
    this.selRoot = stored.root || null;
    this.selCode = stored.code || null;
    this.selYear = null;
    this.recent = this.readLocal(RECENT_KEY, []);
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
    // "save"; none bypasses a PIN)
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
        const [code, year, period] = value.split("/");
        const b = this.branch();
        if (b) this.openForm({ tin: b.tin, code, year: Number(year), period: Number(period) }, cx);
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
          cx.notify();
        });
      } else if (key === "more") {
        this.afterFormLoads(cx, (cx) => {
          this.formUi.state.moreOpen.add(value);
          cx.notify();
        });
      } else if (key === "save") this.afterFormLoads(cx, (cx) => this.formUi.save(cx));
      else if (key === "items") {
        const [from, to] = value.split("-").map(Number);
        this.formUi.devItems = { from, to: to ?? from };
      }
    }
    cx.notify();
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

  /**
   * The selected taxpayer, or the first open one. A selected taxpayer that is
   * locked is still returned: `render` then shows only its lock.
   */
  taxpayer() {
    if (this.selRoot === NO_TAXPAYER) return null;
    const list = this.taxpayers();
    return list.find((t) => t.tin_root === this.selRoot) || list.find((t) => !this.isRootLocked(t.tin_root)) || null;
  }

  branch() {
    const t = this.taxpayer();
    if (!t) return null;
    return t.branches.find((b) => b.code === this.selCode) || t.branches[0] || null;
  }

  /** No taxpayer is open: none exist yet, or every one is locked. */
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
    // Back on a taxpayer's lock returns here, never to the locked one.
    const current = this.taxpayer();
    if (current && current.tin_root !== root && !this.isRootLocked(current.tin_root)) {
      this.prevSel = { root: current.tin_root, code: this.selCode };
    }
    this.selRoot = root;
    this.selCode = opts.code && t.branches.some((b) => b.code === opts.code) ? opts.code : t.branches[0] ? t.branches[0].code : null;
    this.selYear = null;
    this.taxpayerUi.reset();
    this.writeLocal(SELECTION_KEY, { root: this.selRoot, code: this.selCode });
    if (this.isRootLocked(root)) {
      this.resetGate();
      this.panel = null;
      this.overlay = null;
      return cx.notify();
    }
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
    if (view !== this.view) {
      this.history.push(this.view);
      this.forward = [];
      this.view = view;
      this.taxpayerUi.leaveView();
    }
    this.accountMenu = false;
    if (this.narrow()) {
      this.sidebarOpen = false;
      this.panel = null;
    }
    cx.notify();
  }

  goBack(cx) {
    if (!this.history.length) return;
    this.forward.push(this.view);
    this.view = this.history.pop();
    cx.notify();
  }

  goForward(cx) {
    if (!this.forward.length) return;
    this.history.push(this.view);
    this.view = this.forward.pop();
    cx.notify();
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
    this.overlay = "palette";
    this.palette.index = 0;
    this.palette.input.set_value("");
    cx.notify();
  }

  closeOverlay(cx) {
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
    return this.taxpayers().length ? "Open a taxpayer first, then open a form for them." : "Add a taxpayer first, then open a form for them.";
  }

  openForm(opts, cx) {
    if (this.isFresh()) return this.toast(this.freshHint(), cx);
    if (this.isRootLocked(String(opts.tin).slice(0, 9))) {
      return this.selectTaxpayer(String(opts.tin).slice(0, 9), cx, { code: String(opts.tin).slice(9) });
    }
    if (!this.branchFor(opts.tin)) {
      return this.toast("That return belongs to a taxpayer that is not in this database.", cx, "triangle-alert");
    }
    this.overlay = null;
    this.formUi.open(opts, cx);
    this.pushRecent(opts);
    this.go("form", cx);
  }

  /** Rewrite Recent after a TIN or branch-code move, so the entries still open. */
  remapRecent(map) {
    this.recent = this.recent.map((r) => {
      const tin = map(String(r.tin));
      return { ...r, tin, key: `${tin}|${r.code}|${r.year}|${r.period}` };
    });
    this.writeLocal(RECENT_KEY, this.recent);
  }

  pushRecent(opts) {
    const key = `${opts.tin}|${opts.code}|${opts.year}|${opts.period}`;
    this.recent = [{ key, ...opts }, ...this.recent.filter((r) => r.key !== key)].slice(0, 4);
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

  /** Re-read filed returns and announce events that appeared since the last look. */
  pollEvents(cx) {
    this.store.forget("returns:filed");
    const fresh = this.notifications().filter((n) => !this.knownEvents.has(n.id));
    if (!fresh.length) return;
    for (const n of fresh) this.knownEvents.add(n.id);
    this.toast(fresh.length === 1 ? fresh[0].title : `${fresh.length} filing updates. Open notifications to see them.`, cx, fresh[0].icon);
  }

  markAllRead(cx) {
    for (const n of this.notifications()) this.readIds.add(n.id);
    this.writeLocal(READ_KEY, [...this.readIds]);
    cx.notify();
  }

  // ---------------------------------------------------------------- toast

  toast(message, cx, icon) {
    const id = (this.toastState ? this.toastState.id : 0) + 1;
    this.toastState = { id, message, icon };
    cx.timer.after(2600, (cx) => {
      if (this.toastState && this.toastState.id === id) {
        this.toastState = null;
        cx.notify();
      }
    });
    cx.notify();
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
    const hidden = this.store.authState().hide_tax_profiles && this.session.kind !== "taxpayer";
    const current = this.taxpayer();
    for (const t of hidden ? (current ? [current] : []) : this.taxpayers()) {
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
        run: (cx) => this.selectTaxpayer(t.tin_root, cx),
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

  /** Select a taxpayer's branch; a locked taxpayer shows its lock first. */
  openBranch(root, code, cx) {
    if (this.taxpayer() && this.taxpayer().tin_root === root && !this.isRootLocked(root)) return this.selectBranch(code, cx);
    this.selectTaxpayer(root, cx, { code });
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
  // Sessions: "open" (no one signed in; taxpayer PINs apply), "admin"
  // (signed in with the admin PIN or authenticator; every taxpayer opens,
  // since the admin PIN unlocks any of them anyway) and "taxpayer" (signed in
  // with a TIN and its PIN; only that taxpayer and its branches exist).
  // A PIN belongs to the taxpayer (TIN root): one unlock opens every branch.

  initAuth() {
    // One PIN per taxpayer: give every branch its head office's PIN.
    try {
      this.store.write("unify_taxpayer_credentials");
    } catch (_e) {}
    this.auth = this.store.authState();
    this.appLocked = (this.auth.enable_profile_pins && this.auth.has_admin_pin) || this.auth.has_admin_totp;
    this.session = { kind: "open" };
    this.authError = null;
    // Admin codes (sign-in and "Forgot PIN?") and the taxpayer's own codes
    // (sign-in and the lock). The last digit submits.
    this.adminPinOtp = OtpState.new(4, { masked: true });
    this.adminTotpOtp = OtpState.new(6);
    this.tpPinOtp = OtpState.new(4, { masked: true });
    this.tpTotpOtp = OtpState.new(6);
    this.adminPinOtp.on("complete", (_e, cx) => this.submitAdminCode(cx));
    this.adminTotpOtp.on("complete", (_e, cx) => this.submitAdminCode(cx));
    this.tpPinOtp.on("complete", (_e, cx) => this.submitTaxpayerCode(cx));
    this.tpTotpOtp.on("complete", (_e, cx) => this.submitTaxpayerCode(cx));
    // Settings' new admin PIN: never wired to an unlock.
    this.newAdminPinOtp = OtpState.new(4, { masked: true });
    // The legacy profile editor's new PIN, likewise.
    this.profilePinOtp = OtpState.new(4, { masked: true });
    this.signIn = { mode: "taxpayer", method: "pin", forgot: false, tin: new DigitEntry([3, 3, 3]) };
    this.signIn.tin.groups[2].on("complete", () => this.tpPinOtp.focus());
    this.gate = { forgot: false };
    this.unlockedRoots = new Set();
    this.prevSel = null;
    this.applyScope();
    this.settingsNotice = null;
  }

  /** Point the store's visibility at this session. */
  applyScope() {
    const scope = this.session.kind === "taxpayer" ? this.session.root : null;
    this.store.scopeRoot = scope;
    this.store.canSee = (root) => (!scope || root === scope) && !this.isRootLocked(root);
  }

  /** PIN and authenticator of a taxpayer (shared by all its branches). */
  taxpayerCreds(root) {
    return this.store.read(`tpauth:${root}`, "taxpayer_auth", [root], { has_pin: false, has_totp: false });
  }

  /** Whether a taxpayer still needs its PIN in this session. */
  isRootLocked(root) {
    if (!root || this.session.kind === "admin") return false;
    if (this.session.kind === "taxpayer") return false; // only its own taxpayer is visible
    if (!this.store.authState().enable_profile_pins || this.unlockedRoots.has(root)) return false;
    const c = this.taxpayerCreds(root);
    return Boolean(c.has_pin || c.has_totp);
  }

  /** Which sign-in doors exist. */
  signInDoors() {
    const a = this.store.authState();
    return { taxpayer: Boolean(a.enable_profile_pins), admin: Boolean(a.has_admin_pin || a.has_admin_totp) };
  }

  clearCodes() {
    for (const otp of [this.adminPinOtp, this.adminTotpOtp, this.tpPinOtp, this.tpTotpOtp]) otp.set_value("");
  }

  resetGate() {
    this.gate = { forgot: false };
    this.authError = null;
    this.clearCodes();
  }

  setSignInMode(mode, cx) {
    this.signIn.mode = mode;
    this.signIn.forgot = false;
    this.authError = null;
    this.clearCodes();
    cx.notify();
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

  toggleGateForgot(cx) {
    this.gate.forgot = !this.gate.forgot;
    this.authError = null;
    this.clearCodes();
    cx.notify();
  }

  /** The admin's PIN or authenticator code as typed; false when neither matches. */
  checkAdminCode() {
    const a = this.store.authState();
    try {
      if (a.has_admin_pin && this.adminPinOtp.value().length === 4 && this.store.call("verify_admin_pin", this.adminPinOtp.value())) return true;
      if (a.has_admin_totp && this.adminTotpOtp.value().length === 6 && this.store.call("verify_admin_totp", this.adminTotpOtp.value())) return true;
    } catch (_e) {}
    return false;
  }

  /** Last digit of an admin code: sign in as admin, or let a taxpayer in. */
  submitAdminCode(cx) {
    if (this.appLocked && !(this.signIn.mode === "taxpayer" && this.signIn.forgot && this.signInDoors().taxpayer)) {
      return this.submitAdminSignIn(cx);
    }
    return this.submitOverride(cx);
  }

  /** Last digit of a taxpayer code: sign in, or open the locked taxpayer. */
  submitTaxpayerCode(cx) {
    return this.appLocked ? this.submitTaxpayerSignIn(cx) : this.tryUnlockProfile(cx);
  }

  /** The TIN typed on the sign-in screen, if it names a taxpayer. */
  signInRoot() {
    const root = this.signIn.tin.value();
    if (root.length !== 9) return { error: "Type all 9 digits of the TIN." };
    if (!this.store.read("taxpayers", "list_taxpayers", [], []).some((t) => t.tin_root === root)) {
      return { error: "No taxpayer has that TIN on this computer." };
    }
    return { root };
  }

  fail(message, cx) {
    this.authError = message;
    this.clearCodes();
    cx.notify();
  }

  submitAdminSignIn(cx) {
    if (!this.checkAdminCode()) return this.fail("Wrong admin PIN or code. Try again.", cx);
    this.startSession({ kind: "admin" }, cx);
  }

  submitTaxpayerSignIn(cx) {
    const { root, error } = this.signInRoot();
    if (error) return this.fail(error, cx);
    const c = this.taxpayerCreds(root);
    let ok = false;
    try {
      if (this.signIn.method === "totp") {
        ok = c.has_totp && this.tpTotpOtp.value().length === 6 && this.store.call("verify_taxpayer_totp", root, this.tpTotpOtp.value());
      } else {
        ok = c.has_pin && this.tpPinOtp.value().length === 4 && this.store.call("verify_taxpayer_pin", root, this.tpPinOtp.value());
      }
    } catch (_e) {}
    if (!ok) {
      return this.fail(c.has_pin || c.has_totp ? "Wrong TIN or PIN. Try again." : "That taxpayer has no PIN yet. Ask the admin to open it.", cx);
    }
    this.startSession({ kind: "taxpayer", root }, cx);
  }

  /**
   * "Forgot PIN?": the admin's code opens a taxpayer. On the sign-in screen
   * that starts a session for the TIN typed there; on a taxpayer's lock it
   * opens that taxpayer. Either way a new PIN is set under Security.
   */
  submitOverride(cx) {
    if (this.appLocked) {
      const { root, error } = this.signInRoot();
      if (error) return this.fail(error, cx);
      if (!this.checkAdminCode()) return this.fail("Wrong admin PIN or code. Try again.", cx);
      this.startSession({ kind: "taxpayer", root }, cx);
    } else {
      const t = this.taxpayer();
      if (!t || !this.checkAdminCode()) return this.fail("Wrong admin PIN or code. Try again.", cx);
      this.unlockedRoots.add(t.tin_root);
      this.store.bump();
      this.resetGate();
      cx.notify();
    }
    this.toast("Opened by the admin. Set a new PIN under Tax profile › Security.", cx, "lock");
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
    }
    cx.notify();
  }

  /** No admin credential exists, so there is no admin view to protect. */
  openWithoutSignIn(cx) {
    if (this.signInDoors().admin) return;
    this.startSession({ kind: "open" }, cx);
  }

  /** Lock app and Log out both end the session and show sign-in. */
  endSession(cx, message) {
    const doors = this.signInDoors();
    this.accountMenu = false;
    if (!doors.admin && !doors.taxpayer) {
      return this.toast("Set an admin PIN, or turn on Taxpayer PINs, in Settings to lock the app.", cx, "lock");
    }
    this.lockedFrom = { kind: this.session.kind, root: this.session.root };
    this.session = { kind: "open" };
    this.unlockedRoots.clear();
    this.panel = null;
    this.overlay = null;
    this.signIn.mode = this.lockedFrom.kind === "admin" || !doors.taxpayer ? "admin" : "taxpayer";
    this.signIn.method = "pin";
    this.signIn.forgot = false;
    this.signIn.tin.set("");
    this.authError = null;
    this.clearCodes();
    this.store.bump();
    this.applyScope();
    this.appLocked = true;
    if (message) this.toast(message, cx, "log-out");
    cx.notify();
  }

  lockApp(cx) {
    this.endSession(cx);
  }

  logOut(cx) {
    const doors = this.signInDoors();
    if (!doors.admin && !doors.taxpayer) {
      // Nothing to sign back in with: forget opened taxpayers, start over.
      this.unlockedRoots.clear();
      this.accountMenu = false;
      this.view = "home";
      this.history = [];
      this.forward = [];
      return this.toast("Signed out.", cx, "log-out");
    }
    this.endSession(cx, "Signed out.");
    // A sign-in after Log out starts from Home, whoever signs in.
    this.lockedFrom = null;
  }

  /** The lock on the selected taxpayer: its own PIN or authenticator code. */
  tryUnlockProfile(cx) {
    const t = this.taxpayer();
    if (!t) return;
    const c = this.taxpayerCreds(t.tin_root);
    let ok = false;
    try {
      if (c.has_pin && this.tpPinOtp.value().length === 4) ok = this.store.call("verify_taxpayer_pin", t.tin_root, this.tpPinOtp.value());
      if (!ok && c.has_totp && this.tpTotpOtp.value().length === 6) ok = this.store.call("verify_taxpayer_totp", t.tin_root, this.tpTotpOtp.value());
    } catch (_e) {}
    if (!ok) return this.fail("Wrong PIN or code. Try again.", cx);
    this.unlockedRoots.add(t.tin_root);
    this.store.bump();
    this.resetGate();
    cx.notify();
  }

  /** Back on a taxpayer's lock: the taxpayer open before it, or none. */
  cancelUnlock(cx) {
    const prev = this.prevSel;
    this.prevSel = null;
    this.resetGate();
    if (prev && this.taxpayers().some((t) => t.tin_root === prev.root) && !this.isRootLocked(prev.root)) {
      this.selRoot = prev.root;
      this.selCode = prev.code;
    } else {
      this.selRoot = NO_TAXPAYER;
      this.selCode = null;
      this.view = "home";
    }
    this.writeLocal(SELECTION_KEY, { root: this.selRoot, code: this.selCode });
    cx.notify();
  }

  // ---------------------------------------------------------------- settings actions (retained)

  setAuthFlag(key, checked, cx) {
    try {
      this.store.write("set_setting", key, String(checked));
      this.auth = this.store.authState();
    } catch (e) {
      this.settingsNotice = String(e);
    }
    cx.notify();
  }

  setAdminPin(cx) {
    const pin = this.newAdminPinOtp.value();
    if (pin.length !== 4) return this.toast("Enter the new 4-digit PIN first.", cx, "lock");
    try {
      this.store.write("set_admin_pin", pin);
      this.newAdminPinOtp.set_value("");
      this.auth = this.store.authState();
      this.toast("Admin PIN saved.", cx, "lock");
    } catch (e) {
      this.toast(String(e), cx, "triangle-alert");
    }
  }

  clearAdminPin(cx) {
    try {
      this.store.write("set_admin_pin", null);
      this.auth = this.store.authState();
      this.toast("Admin PIN removed.", cx, "lock");
    } catch (e) {
      this.toast(String(e), cx, "triangle-alert");
    }
  }

  toggleAdminTotp(cx) {
    try {
      if (this.store.authState().has_admin_totp) {
        this.store.write("delete_setting", "app_totp_secret");
        this.newTotpSecret = null;
        this.toast("Authenticator removed.", cx, "lock");
      } else {
        const { secret } = this.store.call("generate_totp_secret", "e-BIRForms admin");
        this.store.write("set_setting", "app_totp_secret", secret);
        this.newTotpSecret = secret;
        this.toast("Authenticator added. Copy the secret into your app now.", cx, "lock");
      }
      this.auth = this.store.authState();
    } catch (e) {
      this.toast(String(e), cx, "triangle-alert");
    }
  }

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
        this.jobLogCache = { path: "", lines: [String(e)] };
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

  // ---------------------------------------------------------------- legacy profile editor (retained)
  // Archive, delete and per-profile PIN live on the pre-redesign editor until
  // the tax-profile screens absorb them; Profile › "Email, security and
  // export" links here so none of them becomes unreachable.

  openLegacyEditor(cx) {
    const b = this.branch();
    if (!b) return;
    let record = null;
    try {
      record = this.store.call("profile_detail", b.tin);
    } catch (e) {
      this.editorError = String(e);
    }
    this.editing = record || blankProfile();
    this.editingTinDisplay = b.tin;
    this.editingAuth = { has_pin: false, has_totp: false };
    try {
      this.editingAuth = this.store.call("taxpayer_auth", b.tin_root);
    } catch (_e) {}
    this.deleteArmed = false;
    this.editorError = null;
    this.editorNotice = null;
    const tin = this.editing.tin;
    const values = {
      full_name: this.editing.full_name || "",
      tin: tin && tin.segment1 ? `${tin.segment1}${tin.segment2}${tin.segment3}${tin.branch}` : "",
      rdo_code: this.editing.rdo_code || "",
      email: this.editing.email || "",
      phone: this.editing.phone || "",
      line_of_business: this.editing.line_of_business || "",
      registered_address: this.editing.registered_address || "",
      zip_code: this.editing.zip_code || "",
    };
    this.editorInputs = {};
    for (const [key, value] of Object.entries(values)) this.editorInputs[key] = InputState.new({ value });
    this.go("profile_manager", cx);
  }

  navigate(view, cx) {
    this.go(view === "global_dashboard" ? "home" : view === "dashboard" ? "profile" : view, cx);
  }

  editingTin() {
    const t = this.editing.tin;
    return `${t.segment1}${t.segment2}${t.segment3}${t.branch}`;
  }

  saveEditingProfile(cx) {
    const tin = parseTin(this.editorInputs.tin.value());
    if (!tin) {
      this.editorError = "TIN must be 12–14 digits (dashes optional).";
      return cx.notify();
    }
    const payload = { ...this.editing, tin };
    for (const key of ["full_name", "rdo_code", "email", "phone", "line_of_business", "registered_address", "zip_code"]) {
      payload[key] = this.editorInputs[key].value().trim();
    }
    if (!payload.full_name) {
      this.editorError = "Full name is required.";
      return cx.notify();
    }
    try {
      this.store.write("save_profile", payload);
      this.editing = null;
      this.toast("Changes saved", cx);
      this.go("profile", cx);
    } catch (e) {
      this.editorError = String(e);
      cx.notify();
    }
  }

  toggleArchiveProfile(cx) {
    try {
      this.store.write("set_profile_archived", this.editingTin(), !this.editing.is_archived);
      this.editing.is_archived = !this.editing.is_archived;
      this.editorNotice = this.editing.is_archived ? "Profile archived." : "Profile unarchived.";
    } catch (e) {
      this.editorError = String(e);
    }
    cx.notify();
  }

  deleteProfile(cx) {
    if (!this.deleteArmed) {
      this.deleteArmed = true;
      return cx.notify();
    }
    try {
      this.store.write("delete_profile", this.editingTin());
      this.deleteArmed = false;
      this.editing = null;
      this.selRoot = null;
      this.selCode = null;
      this.go("home", cx);
    } catch (e) {
      this.editorError = String(e);
      cx.notify();
    }
  }

  setProfilePin(cx) {
    const pin = this.profilePinOtp.value();
    if (pin.length !== 4) {
      this.editorError = "Enter the new 4-digit PIN first.";
      return cx.notify();
    }
    try {
      const root = this.editingTin().slice(0, 9);
      this.store.write("set_taxpayer_pin", root, pin);
      this.unlockedRoots.add(root);
      this.profilePinOtp.set_value("");
      this.editingAuth = this.store.call("taxpayer_auth", root);
      this.editorError = null;
      this.editorNotice = "PIN saved.";
    } catch (e) {
      this.editorError = String(e);
    }
    cx.notify();
  }

  clearProfilePin(cx) {
    try {
      const root = this.editingTin().slice(0, 9);
      this.store.write("set_taxpayer_pin", root, null);
      this.editingAuth = this.store.call("taxpayer_auth", root);
      this.editorNotice = "PIN cleared.";
    } catch (e) {
      this.editorError = String(e);
    }
    cx.notify();
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
      case "profile_manager":
        return this.editing ? renderProfileManager(cx, this) : renderHome(cx, this, width);
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
    // Sign-in, then a locked taxpayer, take the whole window: nothing that
    // names a taxpayer is drawn behind them.
    if (this.appLocked) return renderSignIn(cx, this);
    const current = this.taxpayer();
    if (current && this.isRootLocked(current.tin_root)) return renderTaxpayerLock(cx, this, current.tin_root);
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
    if (this.toastState) root.child(Toast(cx, this.toastState.message, this.toastState.icon));
    return root;
  }
}
