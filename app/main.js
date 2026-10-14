// buwiz BIR eFiling — gpui-shell application entry.
//
// The app shell ported from bir-desktop's AppState::render: a collapsible
// profile sidebar, a 36px top strip (bell + theme), a routed page area, the
// right-edge notifications drawer, and the seal footer. Page bodies live in
// shell/pages.js; chrome in shell/chrome.js; theme in shell/theme.js.
//
// Migration plan + tracking: docs/plans/gpui-shell-migration.md, issue #135.

import { View, div } from "gpui-kit";
import { InputState, v_flex } from "gpui-base";
import {
  system_info,
  list_profiles,
  forms_catalog,
  get_setting,
  set_setting,
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
    // S1: selection opens the dashboard directly. The painted app's unlock
    // gate (PIN/session auth) arrives with the auth slice.
    this.activeSessionTin = profile.is_archived ? null : profile.tin;
    this.navigate(profile.is_archived ? "profile_manager" : "dashboard", cx);
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
    // Form views port in S4 — the catalog row is live data now, the view is
    // announced but not mounted yet.
    this.formLaunchNotice = `Form ${form.code} — view ports in the form-views slice (S4)`;
    cx.notify();
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
        return renderStub(
          cx,
          "Taxpayer Profile",
          "Create and edit taxpayer profiles — unlock + editor port in S2",
          "S2",
        );
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
      case "settings":
        return renderStub(cx, "Settings", "App settings — ports in S2", "S2");
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
    return v_flex()
      .size_full()
      .track_focus(this.focusRoot)
      .bg(cx.theme().colors.background)
      .on_action("toggle-sidebar", (_e, cx) => this.toggleSidebarHidden(cx))
      .on_action("toggle-sidebar-mini", (_e, cx) => this.toggleMiniSidebar(cx))
      .on_action("focus-search", (_e, cx) => this.expandSidebarAndFocusSearch(cx))
      .on_action("create-profile", (_e, cx) => this.navigate("profile_manager", cx))
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
