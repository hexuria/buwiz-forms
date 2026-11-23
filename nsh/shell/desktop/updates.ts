// The app's updater: the pure driver over nsh/updater, publishing to the Rust mailbox.
import { Application } from "nsh/desktop";
import { checkForUpdate, downloadUpdate, installOnQuit, shellInfo } from "nsh/updater";
import { app } from "../app";
import { store } from "../store";
import { allowsSelfUpdate, autoDownload, masBuild, publishUpdate } from "./native";
import { createUpdater, NOT_CONFIGURED, type Updater, type UpdateCommand, type UpdaterApi } from "./updater";

/** nsh/updater, as the driver uses it. */
export function realApi(stage = installOnQuit, restart = () => Application.restart()): UpdaterApi {
  return {
    configured: () => !masBuild() && (shellInfo().version ?? null) !== null,
    async check() {
      const found = await checkForUpdate();
      if (!found) return null;
      return { version: found.version, external: !allowsSelfUpdate() || !found.canInstall };
    },
    download: () => downloadUpdate(),
    async install() {
      await stage();
      await app.quit(() => app.endRun(restart));
    },
  };
}

let api: UpdaterApi | null = null;
let driver: Updater | null = null;

function get(): Updater {
  api ??= realApi();
  driver ??= createUpdater({
    api,
    publish: (report) => {
      try {
        publishUpdate(report);
      } catch (error) {
        store.logScreen("warn", "update status: " + String(error));
      }
    },
    autoDownload,
    currentVersion: () => store.systemInfo().app_version ?? "",
  });
  return driver;
}

/** The updater of this app. */
export const updates = {
  /** Replaces the world the driver runs in (a fixture or a test), before anything runs. */
  use(next: UpdaterApi | null): void {
    driver?.stop();
    driver = null;
    api = next;
  },
  /** Runs a command a host function queued. */
  handle(command: UpdateCommand): Promise<void> {
    return get().handle(command);
  },
  /** Publishes the state now (so bir.update_status() is right from the first poll, configured or not). */
  announce(): void {
    const d = get();
    try {
      publishUpdate(d.report());
    } catch (error) {
      store.logScreen("warn", "update status: " + String(error));
    }
  },
  /**
   * "Check for Updates…": checks and downloads, and answers the sentence to show as a toast, or null when the banner says it (a release
   * is ready, the check failed, the app is up to date).
   */
  async checkNow(): Promise<string | null> {
    const d = get();
    if (!d.report().configured) return NOT_CONFIGURED;
    await d.handle("check");
    const r = d.report();
    if (r.phase === "ready" || r.phase === "failed" || r.phase === "up_to_date" || r.phase === "external") return null;
    return r.message ?? "Checked for updates.";
  },
  stop(): void {
    driver?.stop();
  },
};
