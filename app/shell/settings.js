// Settings: preferences for this computer and the signed-in account's own
// PIN and authenticator. A taxpayer sees Appearance and Security; the admin
// also sees Data, Background tasks, Computer setup (who this computer is set
// up for), the recovery key and where the database lives. Rows the prototype shows for features this build does not have yet
// are left out rather than drawn as dead switches
// (see docs/plans/gpui-shell-migration.md).

import { div } from "gpui-kit";
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
import { SecurityCard } from "./security.js";
import { SetupCard } from "./setup_change.js";


function jobChip(cx, status) {
  const s = String(status || "").toLowerCase();
  if (s === "running" || s === "active" || s === "scheduled") return Chip(cx, "Running", "success");
  if (s === "failed" || s === "error") return Chip(cx, "Failed", "danger");
  if (s === "queued" || s === "pending") return Chip(cx, "Waiting", "accent");
  return Chip(cx, status ? status[0].toUpperCase() + status.slice(1).toLowerCase() : "Paused");
}

export function renderSettings(cx, app, _width) {
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
    SettingRow(
      cx,
      "settings-compact-form-nav",
      "Compact form sections",
      "One line per part in On this form; point at a part for its full name",
      Switch(cx, "settings-compact-form-nav-switch", app.prefs.compactFormNav, (v, cx) => app.setPref("compactFormNav", v, cx), "Compact form sections"),
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

  const page = div()
    .flex()
    .flex_col()
    .child(PageHeader(cx, { title: "Settings", lede: "Preferences for this computer, and your PIN. Taxpayer details live in each tax profile." }))
    .child(div().mt(28).child(SectionTitle(cx, "Appearance", true)))
    .child(appearance);
  const version = `eBIRForms ${app.info.app_version || ""}`;
  // A setup change just moved the admin. Until the old session signs out,
  // show only the new recovery key: the rest of Settings already belongs to
  // the new admin.
  if (app.setupChange.step === "key") {
    return page.child(SectionTitle(cx, "Computer setup")).child(SetupCard(cx, app));
  }
  // A taxpayer: appearance and its own PIN. Import, background tasks, the
  // database's location and every other taxpayer belong to the admin.
  if (!app.isAdmin()) {
    return page
      .child(SectionTitle(cx, "Security"))
      .child(SecurityCard(cx, app))
      .child(Text(cx, "meta", version, "fg3").mt(SPACE[6]).whitespace_normal());
  }
  return page
    .child(SectionTitle(cx, "Data"))
    .child(data)
    .child(SectionTitle(cx, "Background tasks"))
    .child(tasks)
    .child(SectionTitle(cx, "Computer setup"))
    .child(SetupCard(cx, app))
    .child(SectionTitle(cx, "Security"))
    .child(SecurityCard(cx, app))
    .child(Text(cx, "meta", `${version} · ${app.info.database_path || ""}`, "fg3").mt(SPACE[6]).whitespace_normal());
}
