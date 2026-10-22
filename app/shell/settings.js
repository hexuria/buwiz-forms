// Settings: a full-height sidebar and a centered column of cards, opened
// with Ctrl+, / Cmd+, or the application menu. Esc and Back to app close it.
//
// Every control writes a setting this build already has. Accent, language,
// spellcheck, and launch-at-login are not here: nothing stores them.
// Tray behaviour is described, not switched: close hides when a tray exists
// and quits when it does not, and that is not a preference.

import { div } from "gpui-kit";
import { Popup } from "gpui-base";
import {
  tokens,
  SPACE,
  RADIUS,
  SIZE,
  Text,
  Line,
  Icon,
  IconButton,
  AppButton,
  Switch,
  Card,
  SettingRow,
  PageTitle,
  NavRow,
  GroupLabel,
  Avatar,
  TextField,
} from "./ui.js";
import { SecurityCard } from "./security.js";
import { SetupCard } from "./setup_change.js";
import { profileExportCard } from "./taxpayer.js";
import { renderJobsBody } from "./cron_tasks.js";
import {
  THEME_OPTIONS,
  settingsView,
  shownSection,
  sectionById,
  hotkeyOptions,
  retentionLabel,
  accountSummary,
} from "./settings_model.js";

const SIDEBAR = 220;
const COLUMN = 420;
const JOBS_COLUMN = 680;

function SearchBox(cx, state) {
  const t = tokens(cx);
  // Same chrome as the library search: the field's own border is off so the
  // icon and the placeholder sit in one rounded box.
  return div()
    .id("settings-search")
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[2])
    .h(SIZE.control)
    .mt(SPACE[3])
    .pl(10)
    .pr(SPACE[2])
    .rounded(RADIUS.row)
    .border(1)
    .border_color(t.borderStrong)
    .bg(t.surface)
    .child(Icon(cx, "search", { color: "fg3" }))
    .child(TextField(cx, state, { id: "settings-search-input", label: "Search settings" }).border(0).px(0).h(30).flex_1().bg(t.surface));
}

function BackLink(cx, app) {
  const t = tokens(cx);
  return div()
    .id("settings-back")
    .flex()
    .flex_row()
    .items_center()
    .gap(6)
    .h(SIZE.control)
    .px(SPACE[2])
    .rounded(RADIUS.row)
    .cursor_pointer()
    .accessibility_label("Back to app")
    .hover((s) => s.bg(t.hover))
    .on_click((_e, cx) => app.closeSettings(cx))
    .child(Icon(cx, "chevron-left", { size: SIZE.iconSmall, color: "fg2" }))
    .child(Text(cx, "control", "Back to app", "fg2"));
}

function sidebar(cx, app, view, shownId) {
  const t = tokens(cx);
  const scroll = div().id("settings-nav").flex().flex_col().flex_1().min_h(0).mt(SPACE[2]).overflow_y_scroll().overflow_x_hidden();
  let group = "";
  for (const section of view) {
    if (section.group !== group) {
      group = section.group;
      scroll.child(GroupLabel(cx, group));
    }
    scroll.child(
      NavRow(cx, `settings-nav-${section.id}`, section.icon, section.label, {
        current: section.id === shownId,
        onClick: (_e, cx) => app.openSettingsSection(section.id, cx),
      }),
    );
  }
  if (!view.length) scroll.child(Text(cx, "meta", "No matching settings.", "fg3").px(SPACE[2]).mt(SPACE[4]).whitespace_normal());
  return div()
    .id("settings-sidebar")
    .flex()
    .flex_col()
    .flex_none()
    .w(SIDEBAR)
    .h_full()
    .min_h(0)
    .pt(SPACE[3])
    .px(SPACE[3])
    .pb(SPACE[2])
    .border_r(1)
    .border_color(t.border)
    .child(BackLink(cx, app))
    .child(SearchBox(cx, app.settingsSearch))
    .child(scroll);
}

/** A compact dropdown pill: the current value and a chevron. */
function PillSelect(cx, id, opts) {
  const t = tokens(cx);
  const current = opts.options.find((option) => option.value === opts.value);
  const trigger = div()
    .id(`${id}-trigger`)
    .flex()
    .flex_row()
    .items_center()
    .gap(6)
    .h(SIZE.controlSm)
    .pl(12)
    .pr(8)
    .rounded(RADIUS.full)
    .border(1)
    .border_color(t.borderStrong)
    .bg(t.bg)
    .cursor_pointer()
    .accessibility_label(opts.label || id)
    .on_click((_e, cx) => opts.onOpenChange(!opts.open, cx))
    .child(Text(cx, "body", current ? current.label : opts.placeholder || "Choose"))
    .child(Icon(cx, "chevron-down", { size: SIZE.iconSmall, color: "fg3" }));
  const popup = Popup.new(id, trigger).anchor("top_right");
  if (opts.open) {
    const list = div()
      .id(`${id}-list`)
      .flex()
      .flex_col()
      .gap(2)
      .mt(SPACE[1])
      .p(SPACE[1])
      .min_w(140)
      .rounded(RADIUS.card)
      .border(1)
      .border_color(t.border)
      .bg(t.raised)
      .shadow_lg()
      .on_mouse_down_out((_e, cx) => opts.onOpenChange(false, cx));
    for (const option of opts.options) {
      const on = option.value === opts.value;
      const valueId = option.value === "" ? "off" : String(option.value).replace(/[^A-Za-z0-9]+/g, "-");
      list.child(
        div()
          .id(`${id}-opt-${valueId}`)
          .flex()
          .items_center()
          .h(SIZE.controlSm)
          .flex_shrink(0)
          .px(SPACE[2])
          .rounded(RADIUS.ctl)
          .cursor_pointer()
          .when(on, (el) => el.bg(t.selected))
          .hover((s) => s.bg(t.hover))
          .on_click((_e, cx) => {
            opts.onChange(option.value, cx);
            opts.onOpenChange(false, cx);
          })
          .child(Text(cx, "body", option.label)),
      );
    }
    popup.content(list);
  }
  return popup;
}

function menuOpen(app, id) {
  return app.settingsMenu === id;
}

function setMenu(app, id, open, cx) {
  app.settingsMenu = open ? id : null;
  cx.notify();
}

function labeled(cx, label, card) {
  const block = div().flex().flex_col().gap(SPACE[2]);
  if (label) block.child(Text(cx, "metaStrong", label, "fg3"));
  return block.child(card);
}

function noteRow(cx, id, title, note) {
  return div()
    .id(id)
    .flex()
    .flex_col()
    .gap(4)
    .py(14)
    .child(Text(cx, "control", title))
    .child(Text(cx, "meta", note, "fg3").whitespace_normal());
}

function valueEdit(cx, id, value, onEdit) {
  return div()
    .flex()
    .flex_row()
    .items_center()
    .gap(2)
    .child(Line(cx, "body", value || "Not set", "fg2").max_w(160))
    .child(IconButton(cx, id, "pencil", { tooltip: "Edit", onClick: onEdit }));
}

function generalCards(cx, app, rows) {
  const appearance = [];
  if (rows.includes("theme")) {
    appearance.push(
        SettingRow(
        cx,
        "settings-theme-row",
        "Theme",
        "System follows this computer",
        PillSelect(cx, "settings-theme", {
          label: "Theme",
          options: THEME_OPTIONS,
          value: app.themePref,
          open: menuOpen(app, "theme"),
          onOpenChange: (open, cx) => setMenu(app, "theme", open, cx),
          onChange: (value, cx) => app.setThemePreference(value, cx),
        }),
      ),
    );
  }
  if (rows.includes("recent")) {
    appearance.push(
      SettingRow(
        cx,
        "settings-recent",
        "Show recent returns",
        "The last four returns in the sidebar",
        Switch(cx, "settings-recent-switch", app.prefs.showRecent, (v, cx) => app.setPref("showRecent", v, cx), "Show recent returns"),
      ),
    );
  }
  if (rows.includes("compact")) {
    appearance.push(
      SettingRow(
        cx,
        "settings-compact-form-nav",
        "Compact form sections",
        "One line per part; point at a part for its name",
        Switch(cx, "settings-compact-form-nav-switch", app.prefs.compactFormNav, (v, cx) => app.setPref("compactFormNav", v, cx), "Compact form sections"),
      ),
    );
  }
  if (rows.includes("updates")) {
    appearance.push(
      SettingRow(
        cx,
        "settings-auto-update",
        "Download updates automatically",
        "A newer release downloads in the background. You choose when to install.",
        Switch(
          cx,
          "settings-auto-update-switch",
          !app.updateStatus || app.updateStatus.auto_download !== false,
          (v, cx) => app.setAutoUpdateDownload(v, cx),
          "Download updates automatically",
        ),
      ),
    );
  }
  return appearance.length ? [labeled(cx, "Appearance", Card(cx, "settings-appearance", appearance))] : [];
}

function accountCards(cx, app, rows) {
  const blocks = [];
  const summary = accountSummary({
    isAdmin: app.isAdmin(),
    isHelpdesk: app.isHelpdesk(),
    adminRoot: app.adminRoot(),
    sessionKind: app.session.kind,
    sessionRoot: app.session.root,
    taxpayers: app.taxpayers(),
    branch: app.branch(),
    canAddTaxpayer: app.canAddTaxpayer(),
  });
  if (rows.includes("identity")) {
    const t = tokens(cx);
    const hero = div()
      .id("settings-account-card")
      .flex()
      .flex_col()
      .gap(SPACE[4])
      .p(SPACE[4])
      .rounded(RADIUS.card)
      .border(1)
      .border_color(t.border)
      .bg(t.surface)
      .child(
        div()
          .flex()
          .flex_row()
          .items_center()
          .gap(12)
          .child(Avatar(cx, summary.name, 40, summary.name))
          .child(
            div()
              .flex()
              .flex_col()
              .min_w(0)
              .flex_1()
              .child(Line(cx, "emph", summary.name))
              .child(Line(cx, "meta", summary.email || summary.detail, "fg3"))
              .child(summary.email ? Line(cx, "meta", summary.detail, "fg3") : div()),
          ),
      );
    const actions = div().flex().flex_row().flex_wrap().gap(SPACE[2]);
    if (summary.showAdd) {
      actions.child(
        AppButton(cx, "settings-add-account", "Add account", {
          size: "sm",
          icon: "plus",
          onClick: (_e, cx) => app.taxpayerUi.openNewTaxpayer(cx),
        }),
      );
    }
    actions.child(AppButton(cx, "settings-sign-out", "Sign out", { size: "sm", icon: "log-out", onClick: (_e, cx) => app.logOut(cx) }));
    hero.child(actions);
    const profile = app.taxpayer();
    const branch = app.branch();
    if (profile && branch) {
      blocks.push(
        labeled(
          cx,
          null,
          div()
            .flex()
            .flex_col()
            .gap(SPACE[4])
            .child(hero)
            .child(
              Card(cx, "settings-profile", [
                SettingRow(
                  cx,
                  "settings-profile-name",
                  "Registered name",
                  profile.tin_display ? `TIN ${profile.tin_display}` : summary.detail,
                  valueEdit(cx, "settings-profile-name-edit", profile.name, (_e, cx) => app.editTaxpayerSection("taxpayer", cx)),
                ),
                SettingRow(
                  cx,
                  "settings-profile-email",
                  "Email",
                  "Printed on this profile's returns",
                  valueEdit(cx, "settings-profile-email-edit", branch.facts.email, (_e, cx) => app.editTaxpayerSection("address", cx)),
                ),
              ]),
            ),
        ),
      );
    } else {
      blocks.push(hero);
    }
  }
  if (rows.includes("google")) blocks.push(googleBlock(cx, app));
  if (rows.includes("security")) blocks.push(labeled(cx, "Security", SecurityCard(cx, app)));
  return blocks.filter(Boolean);
}

function googleBlock(cx, app) {
  const branch = app.branch();
  if (!branch) {
    return labeled(cx, "Google", Card(cx, "settings-google", [noteRow(cx, "settings-google-empty", "No taxpayer yet", "Add a taxpayer before connecting a Gmail account.")]));
  }
  let state = null;
  try {
    state = app.store.inboxState(branch.tin);
  } catch (e) {
    return labeled(cx, "Google", Card(cx, "settings-google", [noteRow(cx, "settings-google-error", "Google account", String(e))]));
  }
  const connected = Boolean(state.oauth_connected);
  const who = state.oauth_inbox_email || state.inbox_email || "";
  const ui = app.taxpayerUi;
  const rows = [
    SettingRow(
      cx,
      "settings-google",
      "Gmail",
      connected ? `Connected as ${who}` : `Not connected. Reads BIR confirmation mail for ${branch.name || "this profile"}.`,
      AppButton(cx, "settings-google-connect", connected ? "Reconnect" : "Connect", {
        size: "sm",
        icon: "key-round",
        disabled: ui.inboxBusy,
        onClick: (_e, cx) => ui.inboxConnect(cx),
      }),
    ),
    SettingRow(
      cx,
      "settings-google-tracking",
      "Receipt tracking",
      "Watch this inbox for BIR confirmation mail",
      Switch(cx, "settings-google-tracking-switch", Boolean(state.tracking_enabled), (v, cx) => ui.inboxToggleTracking(v, cx), "Receipt tracking"),
    ),
  ];
  const blocks = [labeled(cx, "Google", Card(cx, "settings-google", rows))];
  if (connected) {
    blocks.push(
      Card(cx, "settings-google-disconnect", [
        SettingRow(
          cx,
          "settings-google-disconnect-row",
          "Disconnect Google",
          "Clears the grant for every profile that shares this inbox",
          AppButton(cx, "settings-google-disconnect", "Disconnect", {
            variant: "danger",
            size: "sm",
            onClick: (_e, cx) => ui.inboxDisconnect(cx),
          }),
        ),
      ]),
    );
  }
  if (ui.inboxMsg) {
    blocks.push(Text(cx, "meta", ui.inboxMsg.text, ui.inboxMsg.ok ? "success" : "danger").whitespace_normal());
  }
  return div()
    .flex()
    .flex_col()
    .gap(SPACE[4])
    .children(blocks);
}

function transferCards(cx, app, rows) {
  const blocks = [];
  if (rows.includes("import")) {
    blocks.push(
      labeled(
        cx,
        "Import",
        Card(cx, "settings-data", [
          SettingRow(
            cx,
            "settings-import",
            "Import data",
            "Restore a taxpayer or database export (.zip)",
            AppButton(cx, "settings-import-btn", "Import", { size: "sm", icon: "upload", onClick: (_e, cx) => app.runImport(cx) }),
          ),
        ]),
      ),
    );
  }
  if (rows.includes("export") && app.taxpayer()) {
    blocks.push(labeled(cx, "Export", profileExportCard(cx, app, app.taxpayer())));
  }
  if (!blocks.length) {
    blocks.push(Text(cx, "body", "Add a taxpayer before exporting a profile.", "fg3").whitespace_normal());
  }
  return blocks;
}

function computerCards(cx, app, rows) {
  const info = app.liveInfo();
  const blocks = [];
  const files = [];
  if (rows.includes("data")) {
    files.push(noteRow(cx, "settings-database", "Database", info.database_path || "Not available"));
    files.push(noteRow(cx, "settings-data-dir", "Data folder", info.data_dir || "Not available"));
  }
  if (rows.includes("logs")) {
    files.push(noteRow(cx, "settings-logs", "Logs folder", info.logs_dir || "Not available"));
    const choices = (info.log_retention_choices || [3, 7, 14, 30, 90]).map((days) => ({
      value: String(days),
      label: retentionLabel(days),
    }));
    const current = String(info.log_retention_days || 7);
    if (!choices.some((option) => option.value === current)) choices.push({ value: current, label: retentionLabel(current) });
    files.push(
      SettingRow(
        cx,
        "settings-retention-row",
        "Keep logs for",
        "Older daily log files are deleted",
        PillSelect(cx, "settings-retention-pill", {
          label: "Keep logs for",
          options: choices,
          value: current,
          open: menuOpen(app, "retention"),
          onOpenChange: (open, cx) => setMenu(app, "retention", open, cx),
          onChange: (value, cx) => app.setLogRetention(Number(value), cx),
        }),
      ),
    );
  }
  if (files.length) blocks.push(labeled(cx, "Local files", Card(cx, "settings-files", files)));
  if (rows.includes("hotkey")) {
    const presets = info.hotkey_presets || [];
    const combo = info.global_hotkey || "";
    blocks.push(
      labeled(
        cx,
        "Shortcuts",
        Card(cx, "settings-hotkey", [
          SettingRow(
            cx,
            "settings-hotkey-row",
            "Global show hotkey",
            "Shows or hides eBIRForms from any app",
            PillSelect(cx, "settings-hotkey-pill", {
              label: "Global show hotkey",
              options: hotkeyOptions(combo, presets),
              value: combo,
              open: menuOpen(app, "hotkey"),
              onOpenChange: (open, cx) => setMenu(app, "hotkey", open, cx),
              onChange: (value, cx) => app.setGlobalHotkey(value, cx),
            }),
          ),
        ]),
      ),
    );
  }
  if (rows.includes("tray")) {
    blocks.push(
      labeled(
        cx,
        "Window",
        Card(cx, "settings-tray", [
          noteRow(
            cx,
            "settings-tray-row",
            "Close window",
            "Closing hides eBIRForms to the menu bar when a tray icon exists. With no tray, closing quits. Quit is always in the application menu.",
          ),
        ]),
      ),
    );
  }
  if (rows.includes("setup")) blocks.push(labeled(cx, "Computer setup", SetupCard(cx, app)));
  if (rows.includes("version")) {
    blocks.push(Text(cx, "meta", `eBIRForms ${info.app_version || ""}`.trim(), "fg3"));
  }
  return blocks;
}

function sectionBody(cx, app, shown, width) {
  if (!shown) return [Text(cx, "body", "No settings match that search.", "fg3").whitespace_normal()];
  if (shown.id === "general") return generalCards(cx, app, shown.rows);
  if (shown.id === "account") return accountCards(cx, app, shown.rows);
  if (shown.id === "transfer") return transferCards(cx, app, shown.rows);
  if (shown.id === "jobs") return [renderJobsBody(cx, app, Math.min(JOBS_COLUMN, width))];
  if (shown.id === "computer") return computerCards(cx, app, shown.rows);
  return [];
}

export function renderSettings(cx, app, width, _height) {
  const t = tokens(cx);
  const keyOnly = app.setupChange.step === "key";
  const view = keyOnly ? [] : settingsView(app.settingsQuery(), app.isAdmin());
  const shownId = keyOnly ? null : shownSection(app.settingsSection, view);
  const shown = shownId ? sectionById(view, shownId) : null;
  const paneWidth = Math.max(240, width - (keyOnly ? 0 : SIDEBAR));
  const columnWidth = Math.min(shown && shown.id === "jobs" ? JOBS_COLUMN : COLUMN, paneWidth - SPACE[6] * 2);
  const column = div().id("settings-column").flex().flex_col().w(columnWidth).gap(SPACE[5]);
  column.child(div().id("settings-title").child(PageTitle(cx, keyOnly ? "Computer" : shown ? shown.label : "Settings")));
  const blocks = keyOnly ? [labeled(cx, "Computer setup", SetupCard(cx, app))] : sectionBody(cx, app, shown, columnWidth);
  for (const block of blocks) column.child(block);

  const pane = div()
    .id("settings-content")
    .flex()
    .flex_col()
    .flex_1()
    .min_w(0)
    .min_h(0)
    .overflow_y_scroll()
    .overflow_x_hidden()
    .child(
      div()
        .flex()
        .flex_col()
        .items_center()
        .w_full()
        .px(SPACE[6])
        .pt(28)
        .pb(56)
        .child(column),
    );

  const shell = div().id("settings-shell").flex().flex_row().flex_1().w_full().h_full().min_w(0).min_h(0).bg(t.bg);
  if (!keyOnly) shell.child(sidebar(cx, app, view, shownId));
  else {
    shell.child(
      div()
        .flex()
        .flex_col()
        .flex_none()
        .w(SIDEBAR)
        .h_full()
        .pt(SPACE[3])
        .px(SPACE[3])
        .border_r(1)
        .border_color(t.border)
        .child(BackLink(cx, app)),
    );
  }
  return shell.child(pane);
}

