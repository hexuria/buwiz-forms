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
import { InputState, OtpState } from "gpui-base";
import { Store, isoDate, shortDate } from "./shell/store.js";
import {
  applyThemePreference,
  nextThemePreference,
  parseThemePreference,
  storedThemePreference,
  THEME_PREF_KEY,
} from "./shell/theme.js";
import { tokens, SIZE, BREAK, RADIUS, Toast } from "./shell/ui.js";
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
import { renderFormPage, FormPageActions } from "./shell/form_page.js";
import { renderGallery } from "./shell/gallery.js";
import { renderLockScreen, renderUnlockGate } from "./shell/auth.js";
import { renderProfileManager, blankProfile, parseTin } from "./shell/profile_manager.js";
import { renderCronTasksPage } from "./shell/cron_tasks.js";

/** Views that need a taxpayer; with none they fall back to Home. */
const TAXPAYER_VIEWS = new Set(["profile", "forms", "templates", "form"]);
const RECENT_KEY = "redesign.recent";
const SELECTION_KEY = "redesign.selection";
const READ_KEY = "redesign.read_notifications";
const PREFS_KEY = "redesign.prefs";

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
    /** null | "palette" | "new" */
    this.overlay = null;
    this.accountMenu = false;
    this.toastState = null;

    const stored = this.readLocal(SELECTION_KEY, {});
    this.selRoot = stored.root || null;
    this.selCode = stored.code || null;
    this.selYear = null;
    this.recent = this.readLocal(RECENT_KEY, []);
    this.readIds = new Set(this.readLocal(READ_KEY, []));
    this.prefs = { showRecent: true, ...this.readLocal(PREFS_KEY, {}) };

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

  // ---------------------------------------------------------------- local storage

  readLocal(key, fallback) {
    try {
      const raw = window.localStorage.getItem(key);
      return raw ? JSON.parse(raw) : fallback;
    } catch (_e) {
      return fallback;
    }
  }

  writeLocal(key, value) {
    try {
      window.localStorage.setItem(key, JSON.stringify(value));
    } catch (_e) {}
  }

  // ---------------------------------------------------------------- selection

  taxpayers() {
    return this.store.taxpayers();
  }

  taxpayer() {
    const list = this.taxpayers();
    return list.find((t) => t.tin_root === this.selRoot) || list[0] || null;
  }

  branch() {
    const t = this.taxpayer();
    if (!t) return null;
    return t.branches.find((b) => b.code === this.selCode) || t.branches[0] || null;
  }

  isFresh() {
    return this.taxpayers().length === 0;
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
    this.selCode = t.branches[0] ? t.branches[0].code : null;
    this.selYear = null;
    this.taxpayerUi.reset();
    this.writeLocal(SELECTION_KEY, { root: this.selRoot, code: this.selCode });
    if (!this.gateProfile(cx)) return;
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
    if (this.panel && this.narrow()) this.sidebarOpen = false;
    this.taxpayerUi.panelEdit = null;
    cx.notify();
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
    if (this.isFresh()) return this.toast("Add a taxpayer first, then open a form for them.", cx);
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
  openForm(opts, cx) {
    if (this.isFresh()) return this.toast("Add a taxpayer first, then open a form for them.", cx);
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

  notifications() {
    const items = [];
    const today = new Date();
    const soon = this.store.deadlines(isoDate(today), isoDate(new Date(today.getTime() + 14 * 86400000)));
    for (const d of soon) {
      if (d.status !== "Not started" && d.status !== "Draft") continue;
      const id = `due:${d.tin}:${d.form_code}:${d.taxable_year}:${d.period}`;
      items.push({
        id,
        title: `${d.form_code} for ${d.period_label} is due ${shortDate(d.due_date)}`,
        sub: d.branch_count > 1 ? `${d.taxpayer_name} · ${d.branch_name}` : d.taxpayer_name,
        icon: "triangle-alert",
        tone: "warning",
        when: d.due_date,
        unread: !this.readIds.has(id),
      });
    }
    for (const a of this.store.announcements()) {
      const id = `ann:${a.id}`;
      items.push({
        id,
        title: a.title,
        sub: a.source,
        icon: "info",
        tone: "neutral",
        when: a.published_at,
        unread: !a.read_status && !this.readIds.has(id),
      });
    }
    return items;
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
    const hidden = this.store.authState().hide_tax_profiles;
    for (const t of hidden ? [] : this.taxpayers()) {
      items.push({ group: "Taxpayers", label: t.name, hint: t.tin_display, avatar: t, run: (cx) => this.selectTaxpayer(t.tin_root, cx) });
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
    items.push({ group: "Actions", label: "Change theme", icon: "moon", run: (cx) => this.cycleTheme(cx) });
    items.push({ group: "Actions", label: "Components", icon: "layout-template", run: (cx) => this.go("gallery", cx) });
    if (!q) return items;
    return items.filter((x) => `${x.label} ${x.hint || ""}`.toLowerCase().includes(q));
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

  // ---------------------------------------------------------------- auth (retained)

  initAuth() {
    this.auth = this.store.authState();
    this.appLocked =
      (this.auth.enable_profile_pins && this.auth.has_admin_pin) || this.auth.has_admin_totp;
    this.authError = null;
    this.adminPinOtp = OtpState.new(4, { masked: true });
    this.adminTotpOtp = OtpState.new(6);
    this.profilePinOtp = OtpState.new(4, { masked: true });
    this.profileTotpOtp = OtpState.new(6);
    for (const [otp, handler] of [
      [this.adminPinOtp, (cx) => this.tryUnlockApp(cx)],
      [this.adminTotpOtp, (cx) => this.tryUnlockApp(cx)],
      [this.profilePinOtp, (cx) => this.tryUnlockProfile(cx)],
      [this.profileTotpOtp, (cx) => this.tryUnlockProfile(cx)],
    ]) {
      otp.on("complete", (_event, cx) => handler(cx));
    }
    this.pendingUnlockTin = null;
    this.unlockedTins = new Set();
    this.profileAuthCache = null;
    this.settingsNotice = null;
    this.newTotpSecret = null;
    // Legacy profile editor state (shell/profile_manager.js).
    this.editing = null;
    this.editingTinDisplay = "";
    this.editorInputs = {};
    this.editingAuth = null;
    this.editorError = null;
    this.editorNotice = null;
    this.deleteArmed = false;
  }

  /** Per-profile PIN/TOTP gate, as the painted app's needs_auth. Returns true when open. */
  gateProfile(cx) {
    const b = this.branch();
    if (!b || !this.auth.enable_profile_pins || this.unlockedTins.has(b.tin)) return true;
    let auth = { has_pin: false, has_totp: false };
    try {
      auth = this.store.call("profile_auth", b.tin);
    } catch (_e) {}
    if (!auth.has_pin && !auth.has_totp) return true;
    this.pendingUnlockTin = b.tin;
    this.profileAuthCache = auth;
    this.profilePinOtp.set_value("");
    this.profileTotpOtp.set_value("");
    this.authError = null;
    this.go("unlock_profile", cx);
    return false;
  }

  lockApp(cx) {
    const auth = this.store.authState();
    if (!auth.has_admin_pin && !auth.has_admin_totp) {
      this.accountMenu = false;
      return this.toast("Set an admin PIN in Settings to lock the app.", cx, "lock");
    }
    this.auth = auth;
    this.accountMenu = false;
    this.appLocked = true;
    cx.notify();
  }

  logOut(cx) {
    this.unlockedTins.clear();
    this.accountMenu = false;
    const auth = this.store.authState();
    if (auth.has_admin_pin || auth.has_admin_totp) {
      this.auth = auth;
      this.appLocked = true;
    }
    this.view = "home";
    this.history = [];
    this.forward = [];
    this.toast("Signed out.", cx, "log-out");
  }

  tryUnlockApp(cx) {
    let ok = false;
    try {
      if (this.auth.has_admin_pin && this.adminPinOtp.value().length === 4) {
        ok = this.store.call("verify_admin_pin", this.adminPinOtp.value());
      }
      if (!ok && this.auth.has_admin_totp && this.adminTotpOtp.value().length === 6) {
        ok = this.store.call("verify_admin_totp", this.adminTotpOtp.value());
      }
    } catch (e) {
      this.authError = String(e);
    }
    this.adminPinOtp.set_value("");
    this.adminTotpOtp.set_value("");
    if (ok) {
      this.appLocked = false;
      this.authError = null;
    } else {
      this.authError = "Wrong PIN or code. Try again.";
    }
    cx.notify();
  }

  tryUnlockProfile(cx) {
    const tin = this.pendingUnlockTin;
    if (!tin) return;
    const auth = this.profileAuthCache || {};
    let ok = false;
    try {
      if (auth.has_pin && this.profilePinOtp.value().length === 4) {
        ok = this.store.call("verify_profile_pin", tin, this.profilePinOtp.value());
      }
      if (!ok && auth.has_totp && this.profileTotpOtp.value().length === 6) {
        ok = this.store.call("verify_profile_totp", tin, this.profileTotpOtp.value());
      }
    } catch (e) {
      this.authError = String(e);
    }
    if (ok) {
      this.unlockedTins.add(tin);
      this.pendingUnlockTin = null;
      this.profileAuthCache = null;
      this.authError = null;
      this.go("profile", cx);
    } else {
      this.authError = "Wrong PIN or code. Try again.";
      this.profilePinOtp.set_value("");
      this.profileTotpOtp.set_value("");
      cx.notify();
    }
  }

  cancelUnlock(cx) {
    this.pendingUnlockTin = null;
    this.profileAuthCache = null;
    this.authError = null;
    this.go("home", cx);
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
    const pin = this.adminPinOtp.value();
    if (pin.length !== 4) return this.toast("Enter the new 4-digit PIN first.", cx, "lock");
    try {
      this.store.write("set_admin_pin", pin);
      this.adminPinOtp.set_value("");
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
      this.editingAuth = this.store.call("profile_auth", b.tin);
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
      this.store.write("set_profile_pin", this.editingTin(), pin);
      this.profilePinOtp.set_value("");
      this.editingAuth = this.store.call("profile_auth", this.editingTin());
      this.editorError = null;
      this.editorNotice = "PIN saved.";
    } catch (e) {
      this.editorError = String(e);
    }
    cx.notify();
  }

  clearProfilePin(cx) {
    try {
      this.store.write("set_profile_pin", this.editingTin(), null);
      this.editingAuth = this.store.call("profile_auth", this.editingTin());
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
      case "unlock_profile": {
        const b = this.branch();
        return renderUnlockGate(cx, this, { full_name: this.taxpayer()?.name || "", tin: b ? b.tin : "" });
      }
      case "profile_manager":
        return this.editing ? renderProfileManager(cx, this) : renderHome(cx, this, width);
      case "cron_tasks":
        return renderCronTasksPage(cx, this);
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
    if (this.appLocked) return renderLockScreen(cx, this);
    const L = this.layout();

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
      .child(
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
      );

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
