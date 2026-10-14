// Settings: app-wide preferences for this computer. Taxpayer details live in
// each tax profile. Every control the pre-redesign page had is kept (theme,
// admin PIN, authenticator, profile PINs, import, background tasks); rows the
// prototype shows for features this build does not have yet are left out
// rather than drawn as dead switches (see docs/plans/gpui-shell-migration.md).

import { div } from "gpui-kit";
import { OtpInput } from "gpui-base";
import {
  tokens,
  SPACE,
  RADIUS,
  SIZE,
  Text,
  AppButton,
  Segmented,
  Switch,
  Chip,
  Card,
  SettingRow,
  PageHeader,
  SectionTitle,
} from "./ui.js";

function pinCells(cx, state, id) {
  const t = tokens(cx);
  return OtpInput.new(state)
    .id(id)
    .flex()
    .gap(6)
    .cell_style((cell) =>
      cell
        .size(SIZE.control)
        .flex()
        .items_center()
        .justify_center()
        .border(1)
        .border_color(t.borderStrong)
        .bg(t.surface)
        .rounded(RADIUS.row),
    )
    .cell_active_style((cell) => cell.border_color(t.accent))
    .caret_style((caret) => caret.w(2).h(16).bg(t.fg));
}

function jobChip(cx, status) {
  const s = String(status || "").toLowerCase();
  if (s === "running" || s === "active" || s === "scheduled") return Chip(cx, "Running", "success");
  if (s === "failed" || s === "error") return Chip(cx, "Failed", "danger");
  if (s === "queued" || s === "pending") return Chip(cx, "Waiting", "accent");
  return Chip(cx, status ? status[0].toUpperCase() + status.slice(1).toLowerCase() : "Paused");
}

export function renderSettings(cx, app, _width) {
  const auth = app.store.authState();
  const os = app.isMac ? "macOS" : app.info.os === "windows" ? "Windows" : "system";

  const appearance = Card(cx, "settings-appearance", [
    SettingRow(
      cx,
      "settings-theme",
      "Theme",
      `System follows your ${os} setting`,
      Segmented(
        cx,
        "settings-theme-seg",
        [
          { value: "light", label: "Light" },
          { value: "dark", label: "Dark" },
          { value: "system", label: "System" },
        ],
        app.themePref,
        (v, cx) => app.setThemePreference(v, cx),
        { label: "Theme" },
      ),
    ),
    SettingRow(
      cx,
      "settings-recent",
      "Show recent returns in the sidebar",
      "The last four returns you opened",
      Switch(cx, "settings-recent-switch", app.prefs.showRecent, (v, cx) => app.setPref("showRecent", v, cx), "Show recent returns"),
    ),
  ]);

  const data = Card(cx, "settings-data", [
    SettingRow(
      cx,
      "settings-import",
      "Import data",
      "Restore a taxpayer or database export (.zip)",
      AppButton(cx, "settings-import-btn", "Import", { size: "sm", icon: "upload", onClick: (_e, cx) => app.runImport(cx) }),
    ),
  ]);

  const jobs = app.backgroundJobs().filter((j) => j.kind === "job").slice(0, 5);
  const jobRows = jobs.map((j, i) =>
    SettingRow(cx, `settings-job-${i}`, j.name, j.cron_expr ? `Schedule: ${j.cron_expr}` : j.job_type, jobChip(cx, j.status)),
  );
  jobRows.push(
    SettingRow(
      cx,
      "settings-jobs-open",
      jobs.length ? "All background tasks" : "No background tasks yet",
      "Jobs, submissions and the task log",
      AppButton(cx, "settings-jobs-btn", "Open", { size: "sm", onClick: (_e, cx) => app.go("cron_tasks", cx) }),
    ),
  );
  const tasks = Card(cx, "settings-tasks", jobRows);

  const pinControls = div()
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap(SPACE[2])
    .child(pinCells(cx, app.adminPinOtp, "settings-admin-pin"))
    .child(
      AppButton(cx, "settings-admin-pin-save", auth.has_admin_pin ? "Change PIN" : "Set PIN", {
        size: "sm",
        onClick: (_e, cx) => app.setAdminPin(cx),
      }),
    );
  if (auth.has_admin_pin) {
    pinControls.child(
      AppButton(cx, "settings-admin-pin-clear", "Remove", { variant: "ghost", size: "sm", onClick: (_e, cx) => app.clearAdminPin(cx) }),
    );
  }
  const totpHint = app.newTotpSecret
    ? `Secret: ${app.newTotpSecret}`
    : auth.has_admin_totp
      ? "Codes from your authenticator app unlock the app"
      : "Use a 6-digit code from an authenticator app";
  const security = Card(cx, "settings-security", [
    SettingRow(cx, "settings-admin-pin-row", "Admin PIN", auth.has_admin_pin ? "Asks for this PIN to unlock the app" : "Not set", pinControls),
    SettingRow(
      cx,
      "settings-totp",
      "Authenticator",
      totpHint,
      AppButton(cx, "settings-totp-btn", auth.has_admin_totp ? "Remove" : "Add", {
        variant: auth.has_admin_totp ? "ghost" : "secondary",
        size: "sm",
        onClick: (_e, cx) => app.toggleAdminTotp(cx),
      }),
    ),
    SettingRow(
      cx,
      "settings-profile-pins",
      "Taxpayer PINs",
      "Ask for a taxpayer's PIN before opening it",
      Switch(cx, "settings-profile-pins-switch", auth.enable_profile_pins, (v, cx) => app.setAuthFlag("enable_profile_pins", v, cx), "Taxpayer PINs"),
    ),
    SettingRow(
      cx,
      "settings-hide-taxpayers",
      "Hide other taxpayers",
      "Leave them out of Switch taxpayer and Search",
      Switch(cx, "settings-hide-taxpayers-switch", auth.hide_tax_profiles, (v, cx) => app.setAuthFlag("hide_tax_profiles", v, cx), "Hide other taxpayers"),
    ),
  ]);

  return div()
    .flex()
    .flex_col()
    .child(PageHeader(cx, { title: "Settings", lede: "App-wide preferences for this computer. Taxpayer details live in each tax profile." }))
    .child(div().mt(28).child(SectionTitle(cx, "Appearance", true)))
    .child(appearance)
    .child(SectionTitle(cx, "Data"))
    .child(data)
    .child(SectionTitle(cx, "Background tasks"))
    .child(tasks)
    .child(SectionTitle(cx, "Security"))
    .child(security)
    .child(Text(cx, "meta", `eBIRForms ${app.info.app_version || ""} · ${app.info.database_path || ""}`, "fg3").mt(SPACE[6]).whitespace_normal());
}
