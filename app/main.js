// buwiz BIR eFiling — gpui-shell application entry.
//
// The app shell ported from bir-desktop's AppState::render: a collapsible
// profile sidebar, a 36px top strip (bell + theme), a routed page area, the
// right-edge notifications drawer, and the seal footer. Page bodies live in
// shell/pages.js; chrome in shell/chrome.js; theme in shell/theme.js.
//
// Migration plan + tracking: docs/plans/gpui-shell-migration.md, issue #135.

import { View, div } from "gpui-kit";
import { InputState, OtpState, v_flex } from "gpui-base";
import {
  system_info,
  list_profiles,
  forms_catalog,
  get_setting,
  set_setting,
  delete_setting,
  profile_detail,
  save_profile,
  set_profile_archived,
  delete_profile,
  set_profile_pin,
  set_admin_pin,
  auth_state,
  profile_auth,
  verify_profile_pin,
  verify_profile_totp,
  verify_admin_pin,
  verify_admin_totp,
  generate_totp_secret,
  list_drafts,
  get_form_draft,
  save_form_draft,
} from "bir";
import {
  applyThemePreference,
  nextThemePreference,
  parseThemePreference,
  THEME_PREF_KEY,
} from "./shell/theme.js";
import {
  renderSidebar,
  renderTopBar,
  renderNotificationsDrawer,
  renderFooter,
} from "./shell/chrome.js";
import {
  renderGlobalDashboard,
  renderDashboard,
  renderStub,
} from "./shell/pages.js";
import {
  renderLockScreen,
  renderUnlockGate,
} from "./shell/auth.js";
import {
  renderProfileManager,
  blankProfile,
  parseTin,
} from "./shell/profile_manager.js";
import { renderSettings } from "./shell/settings.js";
import { renderFormView, FORM_SPECS } from "./shell/form_view.js";

export default class BirApp extends View {
  init(_props, cx) {
    /** @type {import("bir").SystemInfo} */
    this.info = system_info();

    try {
      /** @type {import("bir").ProfileSummary[]} */
      this.profiles = list_profiles();
    } catch (e) {
      this.profiles = [];
      this.profilesError = String(e);
    }
    this.forms = forms_catalog();

    // Painted AppState starts on the global dashboard when there is no
    // profile, on the first profile's dashboard when there is.
    this.view = "global_dashboard";
    this.activeProfileTin = null;
    this.activeSessionTin = null;

    this.isSidebarHidden = false;
    this.isMiniSidebar = false;
    this.showArchived = false;
    this.profileFilter = "";
    this.notificationsOpen = false;
    this.notificationCount = 0;
    this.formLaunchNotice = null;

    this.searchInput = InputState.new({ placeholder: "Search profiles" });
    this.searchInput.on("change", (_event, cx) => {
      this.profileFilter = this.searchInput.value();
      cx.notify();
    });

    this.themePref = parseThemePreference(get_setting(THEME_PREF_KEY));
    applyThemePreference(cx, this.themePref);

    // App-lock + per-profile credential state (mirrors the painted app's
    // startup reads at app.rs:268).
    try {
      /** @type {import("bir").AuthState} */
      this.auth = auth_state();
    } catch (_e) {
      this.auth = {
        enable_profile_pins: false,
        has_admin_totp: false,
        has_admin_pin: false,
        hide_tax_profiles: false,
      };
    }
    this.appLocked =
      (this.auth.enable_profile_pins && this.auth.has_admin_pin) ||
      this.auth.has_admin_totp;
    this.authError = null;

    this.adminPinOtp = OtpState.new(4, { masked: true });
    this.adminTotpOtp = OtpState.new(6);
    this.profilePinOtp = OtpState.new(4, { masked: true });
    this.profileTotpOtp = OtpState.new(6);
    // OTP callbacks get their own cx at fire time — never capture init's.
    for (const [otp, handler] of [
      [this.adminPinOtp, (cx) => this.tryUnlockApp(cx)],
      [this.adminTotpOtp, (cx) => this.tryUnlockApp(cx)],
      [this.profilePinOtp, (cx) => this.tryUnlockProfile(cx)],
      [this.profileTotpOtp, (cx) => this.tryUnlockProfile(cx)],
    ]) {
      otp.on("complete", (_event, cx) => handler(cx));
    }

    this.pendingUnlockTin = null;
    this.profileAuthCache = null;
    this.editing = null;
    this.editingTinDisplay = "";
    this.editorInputs = {};
    this.editingAuth = null;
    this.editorError = null;
    this.editorNotice = null;
    this.deleteArmed = false;
    this.settingsNotice = null;
    this.newTotpSecret = null;

    this.focusRoot = cx.focus_handle();

    // Mirrors the painted app's platform keymap (platform/windows.rs;
    // cmd- variants are added by the host on macOS).
    cx.bind_keys([
      { keystroke: "ctrl-b", action: "toggle-sidebar" },
      { keystroke: "ctrl-shift-b", action: "toggle-sidebar-mini" },
      { keystroke: "ctrl-f", action: "focus-search" },
      { keystroke: "ctrl-n", action: "create-profile" },
      { keystroke: "ctrl-shift-t", action: "toggle-theme" },
      { keystroke: "ctrl-shift-x", action: "open-cron-tasks" },
      { keystroke: "ctrl-,", action: "open-settings" },
      { keystroke: "escape", action: "close-drawer" },
    ]);
  }

  navigate(view, cx) {
    this.view = view;
    // Every page change closes the drawer (top_bar.rs drawer_on_page_change).
    this.notificationsOpen = false;
    if (cx) cx.notify();
  }

  selectProfile(profile, cx) {
    this.activeProfileTin = profile.tin;
    if (profile.is_archived) {
      // Archived profiles can't hold a session — straight to the editor.
      this.activeSessionTin = null;
      this.openProfileEditor(profile, cx);
      return;
    }
    // The painted app gates on enable_profile_pins plus this profile
    // actually carrying credentials (app.rs needs_auth).
    let auth = { has_pin: false, has_totp: false };
    try {
      auth = profile_auth(profile.tin);
    } catch (_e) {}
    if (this.auth.enable_profile_pins && (auth.has_pin || auth.has_totp)) {
      this.pendingUnlockTin = profile.tin;
      this.profileAuthCache = auth;
      this.profilePinOtp.set_value("");
      this.profileTotpOtp.set_value("");
      this.authError = null;
      this.navigate("unlock_profile", cx);
      return;
    }
    this.activeSessionTin = profile.tin;
    this.navigate("dashboard", cx);
  }

  // ---------- app lock ----------

  tryUnlockApp(cx) {
    let ok = false;
    try {
      if (this.auth.has_admin_pin && this.adminPinOtp.value().length === 4) {
        ok = verify_admin_pin(this.adminPinOtp.value());
      }
      if (!ok && this.auth.has_admin_totp && this.adminTotpOtp.value().length === 6) {
        ok = verify_admin_totp(this.adminTotpOtp.value());
      }
    } catch (e) {
      this.authError = String(e);
    }
    if (ok) {
      this.appLocked = false;
      this.authError = null;
      this.adminPinOtp.set_value("");
      this.adminTotpOtp.set_value("");
    } else {
      this.authError = "Wrong PIN or code — try again.";
      this.adminPinOtp.set_value("");
      this.adminTotpOtp.set_value("");
    }
    cx.notify();
  }

  // ---------- profile unlock ----------

  tryUnlockProfile(cx) {
    const tin = this.pendingUnlockTin;
    if (!tin) return;
    const auth = this.profileAuthCache || {};
    let ok = false;
    try {
      if (auth.has_pin && this.profilePinOtp.value().length === 4) {
        ok = verify_profile_pin(tin, this.profilePinOtp.value());
      }
      if (!ok && auth.has_totp && this.profileTotpOtp.value().length === 6) {
        ok = verify_profile_totp(tin, this.profileTotpOtp.value());
      }
    } catch (e) {
      this.authError = String(e);
    }
    if (ok) {
      this.activeSessionTin = tin;
      this.pendingUnlockTin = null;
      this.profileAuthCache = null;
      this.authError = null;
      this.navigate("dashboard", cx);
    } else {
      this.authError = "Wrong PIN or code — try again.";
      this.profilePinOtp.set_value("");
      this.profileTotpOtp.set_value("");
      cx.notify();
    }
  }

  cancelUnlock(cx) {
    this.pendingUnlockTin = null;
    this.profileAuthCache = null;
    this.authError = null;
    this.navigate("global_dashboard", cx);
  }

  // ---------- profile editor ----------

  openProfileEditor(profile, cx) {
    let record;
    try {
      record = profile_detail(profile.tin);
    } catch (e) {
      this.editorError = String(e);
      record = null;
    }
    this.editing = record || blankProfile();
    this.editingTinDisplay = profile.tin;
    this.editingAuth = { has_pin: false, has_totp: false };
    try {
      this.editingAuth = profile_auth(profile.tin);
    } catch (_e) {}
    this.deleteArmed = false;
    this.editorError = null;
    this.editorNotice = null;
    this.buildEditorInputs(this.editing);
    this.navigate("profile_manager", cx);
  }

  openNewProfile(cx) {
    this.editing = blankProfile();
    this.editingTinDisplay = "";
    this.editingAuth = null;
    this.deleteArmed = false;
    this.editorError = null;
    this.editorNotice = null;
    this.buildEditorInputs(this.editing);
    this.navigate("profile_manager", cx);
  }

  buildEditorInputs(profile) {
    const tinStr =
      profile.tin && profile.tin.segment1
        ? `${profile.tin.segment1}${profile.tin.segment2}${profile.tin.segment3}${profile.tin.branch}`
        : "";
    const values = {
      full_name: profile.full_name || "",
      tin: tinStr,
      rdo_code: profile.rdo_code || "",
      email: profile.email || "",
      phone: profile.phone || "",
      line_of_business: profile.line_of_business || "",
      registered_address: profile.registered_address || "",
      zip_code: profile.zip_code || "",
    };
    this.editorInputs = {};
    for (const [key, value] of Object.entries(values)) {
      this.editorInputs[key] = InputState.new({ value });
    }
  }

  saveEditingProfile(cx) {
    const tin = parseTin(this.editorInputs.tin.value());
    if (!tin) {
      this.editorError = "TIN must be 12–14 digits (dashes optional).";
      cx.notify();
      return;
    }
    const payload = {
      ...this.editing,
      full_name: this.editorInputs.full_name.value().trim(),
      tin,
      rdo_code: this.editorInputs.rdo_code.value().trim(),
      email: this.editorInputs.email.value().trim(),
      phone: this.editorInputs.phone.value().trim(),
      line_of_business: this.editorInputs.line_of_business.value().trim(),
      registered_address: this.editorInputs.registered_address.value().trim(),
      zip_code: this.editorInputs.zip_code.value().trim(),
    };
    if (!payload.full_name) {
      this.editorError = "Full name is required.";
      cx.notify();
      return;
    }
    try {
      const saved = save_profile(payload);
      this.reloadProfiles();
      const tinFull = `${tin.segment1}${tin.segment2}${tin.segment3}${tin.branch}`;
      this.activeProfileTin = tinFull;
      if (!this.editing.is_archived) {
        this.activeSessionTin = this.activeSessionTin || tinFull;
      }
      this.editing = null;
      this.navigate("dashboard", cx);
      void saved;
    } catch (e) {
      this.editorError = String(e);
      cx.notify();
    }
  }

  toggleArchiveProfile(cx) {
    try {
      set_profile_archived(this.editingTin(), !this.editing.is_archived);
      this.editing.is_archived = !this.editing.is_archived;
      this.reloadProfiles();
      this.editorNotice = this.editing.is_archived
        ? "Profile archived."
        : "Profile unarchived.";
      cx.notify();
    } catch (e) {
      this.editorError = String(e);
      cx.notify();
    }
  }

  editingTin() {
    const t = this.editing.tin;
    return `${t.segment1}${t.segment2}${t.segment3}${t.branch}`;
  }

  deleteProfile(cx) {
    if (!this.deleteArmed) {
      this.deleteArmed = true;
      cx.notify();
      return;
    }
    try {
      delete_profile(this.editingTin());
      this.deleteArmed = false;
      this.editing = null;
      this.activeProfileTin = null;
      this.activeSessionTin = null;
      this.reloadProfiles();
      this.navigate("global_dashboard", cx);
    } catch (e) {
      this.editorError = String(e);
      cx.notify();
    }
  }

  setProfilePin(cx) {
    const pin = this.profilePinOtp.value();
    if (pin.length !== 4) {
      this.editorError = "Enter the new 4-digit PIN first.";
      cx.notify();
      return;
    }
    try {
      set_profile_pin(this.editingTin(), pin);
      this.profilePinOtp.set_value("");
      this.editingAuth = profile_auth(this.editingTin());
      this.editorError = null;
      this.editorNotice = "PIN saved.";
      cx.notify();
    } catch (e) {
      this.editorError = String(e);
      cx.notify();
    }
  }

  clearProfilePin(cx) {
    try {
      set_profile_pin(this.editingTin(), null);
      this.editingAuth = profile_auth(this.editingTin());
      this.editorNotice = "PIN cleared.";
      cx.notify();
    } catch (e) {
      this.editorError = String(e);
      cx.notify();
    }
  }

  // ---------- settings ----------

  setThemePreference(pref, cx) {
    this.themePref = pref;
    set_setting(THEME_PREF_KEY, pref[0].toUpperCase() + pref.slice(1));
    applyThemePreference(cx, pref);
    cx.notify();
  }

  setAuthFlag(key, checked, cx) {
    try {
      set_setting(key, String(checked));
      this.auth[key] = checked;
      cx.notify();
    } catch (e) {
      this.settingsNotice = String(e);
      cx.notify();
    }
  }

  setAdminPin(cx) {
    const pin = this.adminPinOtp.value();
    if (pin.length !== 4) {
      this.settingsNotice = "Enter the new 4-digit PIN first.";
      cx.notify();
      return;
    }
    try {
      set_admin_pin(pin);
      this.adminPinOtp.set_value("");
      this.auth.has_admin_pin = true;
      this.settingsNotice = "Admin PIN saved.";
      cx.notify();
    } catch (e) {
      this.settingsNotice = String(e);
      cx.notify();
    }
  }

  clearAdminPin(cx) {
    try {
      set_admin_pin(null);
      this.auth.has_admin_pin = false;
      this.settingsNotice = "Admin PIN cleared.";
      cx.notify();
    } catch (e) {
      this.settingsNotice = String(e);
      cx.notify();
    }
  }

  toggleAdminTotp(cx) {
    try {
      if (this.auth.has_admin_totp) {
        delete_setting("app_totp_secret");
        this.auth.has_admin_totp = false;
        this.newTotpSecret = null;
        this.settingsNotice = "Authenticator removed.";
      } else {
        const { secret } = generate_totp_secret("e-BIRForms admin");
        set_setting("app_totp_secret", secret);
        this.auth.has_admin_totp = true;
        this.newTotpSecret = secret;
        this.settingsNotice =
          "Authenticator enrolled — add the secret to your app now.";
      }
      cx.notify();
    } catch (e) {
      this.settingsNotice = String(e);
      cx.notify();
    }
  }

  reloadProfiles() {
    try {
      this.profiles = list_profiles();
      this.profilesError = null;
    } catch (e) {
      this.profilesError = String(e);
    }
  }

  logout(cx) {
    this.activeSessionTin = null;
    this.activeProfileTin = null;
    this.navigate("global_dashboard", cx);
  }

  toggleMiniSidebar(cx) {
    this.isMiniSidebar = !this.isMiniSidebar;
    cx.notify();
  }

  toggleSidebarHidden(cx) {
    this.isSidebarHidden = !this.isSidebarHidden;
    cx.notify();
  }

  toggleArchived(cx) {
    this.showArchived = !this.showArchived;
    cx.notify();
  }

  toggleDrawer(cx) {
    this.notificationsOpen = !this.notificationsOpen;
    cx.notify();
  }

  expandSidebarAndFocusSearch(cx) {
    this.isMiniSidebar = false;
    this.isSidebarHidden = false;
    cx.notify();
  }

  cycleTheme(cx) {
    this.themePref = nextThemePreference(this.themePref);
    // Stored as the painted app's serde variant spelling so the two
    // binaries read each other's preference.
    set_setting(THEME_PREF_KEY, this.themePref[0].toUpperCase() + this.themePref.slice(1));
    applyThemePreference(cx, this.themePref);
    cx.notify();
  }

  openFormCatalogEntry(form, cx) {
    const base = FORM_SPECS[form.code];
    if (!base) {
      // Form specs grow in S4 — announce instead of faking a view.
      this.formLaunchNotice = `Form ${form.code} — view spec ports in the form-views slice (S4)`;
      cx.notify();
      return;
    }
    if (!this.activeProfileTin) {
      this.formLaunchNotice = "Pick a taxpayer profile first.";
      cx.notify();
      return;
    }
    // Extracted specs carry only fields/periods — the catalog row supplies
    // title + period cadence.
    this.openFormView({ title: form.title, ...base }, cx);
  }

  openDraftSummary(draft, cx) {
    const form = this.forms.find((f) => f.code === draft.form_code);
    const base = FORM_SPECS[draft.form_code];
    if (!form || !base) return;
    this.openFormView(
      { title: form.title, ...base },
      cx,
      draft.taxable_year,
      draft.quarter ?? draft.month ?? null,
    );
  }

  formFieldGet(obj, key) {
    return key.split(".").reduce((o, k) => (o == null ? o : o[k]), obj);
  }

  formFieldSet(obj, key, value) {
    const parts = key.split(".");
    let node = obj;
    for (const part of parts.slice(0, -1)) {
      if (node[part] == null || typeof node[part] !== "object") node[part] = {};
      node = node[part];
    }
    node[parts[parts.length - 1]] = value;
  }

  openFormView(spec, cx, year, period) {
    this.formSpec = spec;
    const now = new Date();
    this.formYear = year ?? now.getFullYear();
    this.formQuarter =
      period ??
      (spec.periods === "quarterly"
        ? Math.ceil((now.getMonth() + 1) / 3)
        : spec.periods === "monthly"
          ? now.getMonth() + 1
          : null);
    this.formError = null;
    this.formNotice = null;
    let draft = null;
    this.formDraftStatus = "Draft";
    try {
      draft = get_form_draft(
        this.activeProfileTin,
        spec.code,
        this.formYear,
        this.formQuarter,
      );
      const summaries = list_drafts(this.activeProfileTin, this.formYear);
      const existing = summaries.find(
        (s) => s.form_code === spec.code && s.quarter === this.formQuarter,
      );
      if (existing) this.formDraftStatus = existing.status;
    } catch (e) {
      this.formError = String(e);
    }
    const profile = this.activeProfile() || {};
    this.formDraft = draft || {};
    this.formInputs = {};
    for (const field of spec.fields) {
      if (field.kind === "bool") continue;
      let value = this.formFieldGet(this.formDraft, field.key);
      if ((value === undefined || value === null || value === "") && spec.profileDefaults?.[field.key]) {
        value = profile[spec.profileDefaults[field.key]] || "";
      }
      if (field.kind === "money" || field.kind === "number") {
        value = value === undefined || value === null ? "" : String(value);
      }
      this.formInputs[field.key] = InputState.new({
        value: value == null ? "" : String(value),
      });
    }
    this.formLaunchNotice = null;
    this.navigate("form_view", cx);
  }

  saveFormDraft(cx) {
    const spec = this.formSpec;
    const data = { ...this.formDraft };
    for (const field of spec.fields) {
      if (field.kind === "bool") {
        this.formFieldSet(
          data,
          field.key,
          Boolean(this.formFieldGet(data, field.key)),
        );
      } else if (field.kind === "money") {
        const raw = this.formInputs[field.key].value().trim();
        const value = raw === "" ? 0 : Number(raw);
        if (Number.isNaN(value)) {
          this.formError = `${field.label} must be a number.`;
          cx.notify();
          return;
        }
        this.formFieldSet(data, field.key, value);
      } else if (field.kind === "number") {
        const raw = this.formInputs[field.key].value().trim();
        const value = raw === "" ? 0 : Math.trunc(Number(raw));
        if (Number.isNaN(value)) {
          this.formError = `${field.label} must be a whole number.`;
          cx.notify();
          return;
        }
        this.formFieldSet(data, field.key, value);
      } else {
        this.formFieldSet(data, field.key, this.formInputs[field.key].value().trim());
      }
    }
    try {
      save_form_draft(
        this.activeProfileTin,
        spec.code,
        this.formYear,
        this.formQuarter,
        "Draft",
        data,
      );
      this.formDraft = data;
      this.formDraftStatus = "Draft";
      this.formError = null;
      this.formNotice = "Draft saved.";
      cx.notify();
    } catch (e) {
      this.formError = String(e);
      cx.notify();
    }
  }

  draftsForProfile(year) {
    if (!this.activeProfileTin) return [];
    try {
      return list_drafts(this.activeProfileTin, year);
    } catch (_e) {
      return [];
    }
  }

  activeProfile() {
    return this.profiles.find((p) => p.tin === this.activeProfileTin) || null;
  }

  renderActiveView(cx) {
    switch (this.view) {
      case "global_dashboard":
        return renderGlobalDashboard(cx, this);
      case "dashboard": {
        const profile = this.activeProfile();
        return profile
          ? renderDashboard(cx, this, profile)
          : renderStub(cx, "Dashboard", "No taxpayer selected", "S3");
      }
      case "profile_manager":
        return this.editing
          ? renderProfileManager(cx, this)
          : renderStub(cx, "Taxpayer Profile", "No profile loaded", "S2");
      case "unlock_profile": {
        const profile = this.activeProfile();
        return profile
          ? renderUnlockGate(cx, this, profile)
          : renderStub(cx, "Unlock", "No profile selected", "S2");
      }
      case "import_export":
        return renderStub(
          cx,
          "Import Data",
          "Backup import/export — ports in S5",
          "S5",
        );
      case "cron_tasks":
        return renderStub(
          cx,
          "Background Tasks",
          "Queue and receipt-fetching tasks — ports in S6",
          "S6",
        );
      case "admin_calendar":
        return renderStub(
          cx,
          "Tax Calendars",
          "Admin calendar dashboard — ports in S5",
          "S5",
        );
      case "form_view":
        return this.formSpec
          ? renderFormView(cx, this)
          : renderStub(cx, "Form", "No form selected", "S4");
      case "settings":
        return renderSettings(cx, this);
      case "dev_mode":
        return renderStub(
          cx,
          "Dev Mode",
          "Dev switches and diagnostics — debug builds only",
          "S5",
        );
      default:
        return renderGlobalDashboard(cx, this);
    }
  }

  render(cx) {
    if (this.appLocked) {
      return renderLockScreen(cx, this);
    }
    return v_flex()
      .size_full()
      .track_focus(this.focusRoot)
      .bg(cx.theme().colors.background)
      .on_action("toggle-sidebar", (_e, cx) => this.toggleSidebarHidden(cx))
      .on_action("toggle-sidebar-mini", (_e, cx) => this.toggleMiniSidebar(cx))
      .on_action("focus-search", (_e, cx) => this.expandSidebarAndFocusSearch(cx))
      .on_action("create-profile", (_e, cx) => this.openNewProfile(cx))
      .on_action("toggle-theme", (_e, cx) => this.cycleTheme(cx))
      .on_action("open-cron-tasks", (_e, cx) => this.navigate("cron_tasks", cx))
      .on_action("open-settings", (_e, cx) => this.navigate("settings", cx))
      .on_action("close-drawer", (_e, cx) => {
        if (this.notificationsOpen) {
          this.notificationsOpen = false;
          cx.notify();
        }
      })
      .child(
        div()
          .flex()
          .flex_row()
          .flex_1()
          .min_h_0()
          .when(!this.isSidebarHidden, (el) =>
            el.child(
              div()
                .relative()
                .flex_none()
                .h_full()
                .child(renderSidebar(cx, this)),
            ),
          )
          .child(
            div()
              .flex_1()
              .flex()
              .flex_col()
              .h_full()
              .overflow_hidden()
              .relative()
              .child(renderTopBar(cx, this))
              .child(
                div().flex_1().min_h_0().w_full().child(this.renderActiveView(cx)),
              )
              .child(renderNotificationsDrawer(cx, this)),
          ),
      )
      .when(Boolean(this.formLaunchNotice), (el) =>
        el.child(
          div()
            .px(16)
            .py(8)
            .border_t(1)
            .border_color(cx.theme().colors.border)
            .bg(cx.theme().colors.muted)
            .child(
              div()
                .text_size(12)
                .text_color(cx.theme().colors.muted_foreground)
                .child(this.formLaunchNotice),
            ),
        ),
      )
      .child(renderFooter(cx));
  }
}
