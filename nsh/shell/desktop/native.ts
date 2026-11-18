// The one place the desktop shell reaches its Rust module (src/desktop.rs). Everything else in this folder imports these.
import { Desktop } from "ebirforms/native";
import type { DesktopPoll, InstanceStatus } from "ebirforms/native";
import type { UpdateReport } from "./updater";

export type { DesktopPoll, InstanceStatus };

/** What changed since the last call: a second launch asking for the window, the updater commands the host functions queued, the menu refresh counter. */
export function poll(): DesktopPoll {
  return Desktop.poll();
}

/** Says what the updater is doing: bir.update_status() answers it. */
export function publishUpdate(report: UpdateReport): void {
  Desktop.publishUpdate({ configured: report.configured, phase: report.phase, version: report.version, message: report.message });
}

/** "Download updates automatically": the stored setting (on when never set). The page cannot read that setting; the host can. */
export function autoDownload(): boolean {
  return Desktop.autoDownload();
}

/** The keystroke GlobalShortcut takes for a stored combo ("control+alt+E" is "ctrl+alt+e"), by the rules the Settings page uses; null when it cannot be registered. */
export function hotkeyShortcut(combo: string): string | null {
  return Desktop.hotkeyShortcut(combo) ?? null;
}

/** How the one-copy guard stands in this process. */
export function instance(): InstanceStatus {
  return Desktop.instance();
}

/** macOS: shows the Dock icon (the app is a regular app again and comes to the front) or takes it away (a menu-bar app while hidden). Does nothing elsewhere. */
export function setDockIcon(shown: boolean): void {
  Desktop.setDockIcon(shown);
}
