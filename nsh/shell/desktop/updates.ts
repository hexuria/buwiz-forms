// The app's updater: the pure driver (updater.ts) over nsh/updater, publishing to Rust (native.ts). In this build nsh.json has no
// "updater", so shellInfo().version is null: nothing is configured, nothing is checked, nothing touches the network, and the status
// says so. To turn updates on the owner puts an "updater" source and a signing key in nsh.json (nsh/AGENTS.md, "Updates").
import { checkForUpdate, downloadUpdate, installAndRestart, shellInfo } from "nsh/updater";
import { app } from "../app";
import { store } from "../store";
import { autoDownload, publishUpdate } from "./native";
import { createUpdater, NOT_CONFIGURED, type Updater, type UpdateCommand, type UpdaterApi } from "./updater";

/** nsh/updater, as the driver uses it. `restart` is a test's stand-in for the swap and the restart. */
export function realApi(restart: () => Promise<void> = installAndRestart): UpdaterApi {
  return {
    configured: () => (shellInfo().version ?? null) !== null,
    async check() {
      const found = await checkForUpdate();
      if (!found) return null;
      if (!found.canInstall) {
        throw new Error("Version " + found.version + " is available, but this copy cannot update itself. Get it from " + found.url + ".");
      }
      return { version: found.version };
    },
    download: () => downloadUpdate(),
    // The frame's quit path first (an open form flushes its unsaved edits), then the door's shutdown (the worker stops, the database
    // is checkpointed), then the swap and the restart. It settles only on failure.
    install: () =>
      new Promise<void>((resolve, reject) => {
        app
          .quit(() => app.endRun(() => void restart().then(resolve, reject)))
          .catch(reject);
      }),
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
    if (r.phase === "ready" || r.phase === "failed" || r.phase === "up_to_date") return null;
    return r.message ?? "Checked for updates.";
  },
  stop(): void {
    driver?.stop();
  },
};
