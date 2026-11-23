// The updater's state machine (old crates/bir-shell/src/updater.rs, without the feed and the signing: those are nsh/updater's,
// set in nsh.json "updater"). Pure: the network, the download and the install come in through
// UpdaterApi, so a plain test plays every phase.
//
// Phases, as the page and the banner (shell/UpdateSlot.tsx) know them: idle, checking, downloading, ready, failed, up_to_date,
// available, external, installing. With no update source configured the phase is always idle and nothing is ever fetched.
//
// The host functions reach it through the commands Rust queued (src/desktop.rs): start, check, install, dismiss, auto-on,
// auto-off. The menu's "Check for Updates…" is the same "check". Each change is published to Rust, which answers
// bir.update_status() with the last one.

/** What an update check found (nsh/updater's AppUpdate, the part used here). */
export interface FoundUpdate {
  version: string;
  /** This installation needs a package manager or a fresh download instead of an in-place update. */
  external?: boolean;
}

/** What the driver needs of the world: nsh/updater in the app, a fake in a test. */
export interface UpdaterApi {
  /** Whether the build has an update source (nsh.json "updater"): false means nothing is ever checked. */
  configured(): boolean;
  /** Asks the source. Resolves null when the app is current. Rejects when the source cannot be reached or its manifest is refused. */
  check(): Promise<FoundUpdate | null>;
  /** Downloads and verifies what the last check found. */
  download(): Promise<void>;
  /** Stages the swap, flushes unsaved work and restarts the app; a refusal rejects. */
  install(): Promise<void>;
}

/** The phases the page knows. */
export type UpdatePhase = "idle" | "checking" | "downloading" | "ready" | "failed" | "up_to_date" | "available" | "external" | "installing";

/** What the driver publishes to Rust (src/desktop.rs UpdateReport). */
export interface UpdateReport {
  configured: boolean;
  phase: UpdatePhase;
  version: string | null;
  message: string | null;
  autoDownload: boolean;
}

/** The commands the host functions queue. */
export type UpdateCommand = "start" | "check" | "install" | "dismiss" | "auto-on" | "auto-off";

/** Said while no update source is configured (the same sentence Rust answers). */
export const NOT_CONFIGURED = "Automatic updates are not configured in this build, so nothing is checked.";
/** Said when an update could not be checked or verified. */
export const VERIFY_REJECTED = "The update could not be verified and was not installed.";

/** The first check after launch, and the gap between checks (the old updater's 2 s and 4 hours). */
export const LAUNCH_DELAY_MS = 2000;
export const CHECK_INTERVAL_MS = 4 * 60 * 60 * 1000;

/** What a driver offers its owner. */
export interface Updater {
  /** Runs one command; resolves when it is done (a check or a download takes a while). */
  handle(command: UpdateCommand): Promise<void>;
  /** The state now. */
  report(): UpdateReport;
  /** Cancels the periodic check (the app is closing). */
  stop(): void;
}

/** Options of createUpdater. */
export interface UpdaterOptions {
  api: UpdaterApi;
  /** Where each change goes (Rust's mailbox in the app). */
  publish: (report: UpdateReport) => void;
  /** The stored "Download updates automatically", read when a check finishes. */
  autoDownload: () => boolean;
  /** The version this app is, for "is up to date". */
  currentVersion: () => string;
  /** Timers: setTimeout and clearTimeout in the app, controllable in a test. */
  timers?: { set: (fn: () => void, ms: number) => unknown; clear: (id: unknown) => void };
}

/** The error text of a thrown value. */
function said(error: unknown): string {
  if (error instanceof Error && error.message) return error.message;
  const text = String(error ?? "").trim();
  return text && text !== "[object Object]" ? text : VERIFY_REJECTED;
}

/** Makes the driver. It does nothing until it is handed "start" (the session opened) or "check". */
export function createUpdater(options: UpdaterOptions): Updater {
  const { api, publish } = options;
  const timers = options.timers ?? { set: (fn: () => void, ms: number) => setTimeout(fn, ms), clear: (id: unknown) => clearTimeout(id as ReturnType<typeof setTimeout>) };
  let phase: UpdatePhase = "idle";
  let version: string | null = null;
  let message: string | null = null;
  let dismissed: string | null = null;
  /** The release that is downloaded and verified, waiting for Install now. */
  let downloaded: string | null = null;
  let auto = options.autoDownload();
  let timer: unknown = null;
  let busy = false;
  let started = false;
  let stopped = false;

  const report = (): UpdateReport => {
    if (!api.configured()) return { configured: false, phase: "idle", version: null, message: NOT_CONFIGURED, autoDownload: auto };
    return { configured: true, phase, version, message, autoDownload: auto };
  };
  const set = (next: UpdatePhase, nextVersion: string | null = version, nextMessage: string | null = null) => {
    phase = next;
    version = nextVersion;
    message = nextMessage;
    publish(report());
  };

  /** A check, and the download it leads to. manual: the menu item, or the page's button: it downloads whatever auto says and answers "up to date". */
  async function check(manual: boolean): Promise<void> {
    if (!api.configured()) {
      publish(report());
      return;
    }
    if (busy || phase === "installing") return;
    busy = true;
    try {
      if (manual) dismissed = null;
      // An update already on offer stays there while the periodic check asks again.
      const keepOffer = !manual && (phase === "ready" || phase === "external");
      if (!keepOffer) set("checking", version);
      let found: FoundUpdate | null;
      try {
        found = await api.check();
      } catch (error) {
        if (keepOffer) return;
        set(manual ? "failed" : "idle", null, manual ? said(error) : null);
        return;
      }
      if (!found) {
        downloaded = null;
        if (manual) set("up_to_date", null, "eBIRForms " + options.currentVersion() + " is up to date.");
        else set("idle", null, null);
        return;
      }
      if (!manual && dismissed === found.version) {
        set("idle", found.version, null);
        return;
      }
      if (found.external) {
        set("external", found.version, "Version " + found.version + " is available.");
        return;
      }
      // Found again: what was downloaded is still the newest, so there is nothing more to fetch.
      if (found.version === downloaded) {
        set("ready", found.version, null);
        return;
      }
      // A manual check downloads even with the automatic download off (the old force_download).
      if (!manual && !auto) {
        set("available", found.version, "Version " + found.version + " is available.");
        return;
      }
      set("downloading", found.version, "Downloading version " + found.version + "…");
      try {
        await api.download();
      } catch (error) {
        set("failed", found.version, said(error));
        return;
      }
      downloaded = found.version;
      set("ready", found.version, null);
    } finally {
      busy = false;
    }
  }

  async function install(): Promise<void> {
    if (phase !== "ready" || busy) return;
    busy = true;
    const was = version;
    set("installing", was, "Installing version " + (was ?? "") + "…");
    try {
      await api.install();
    } catch (error) {
      set("failed", was, said(error));
    } finally {
      busy = false;
    }
  }

  function schedule(ms: number): void {
    if (stopped) return;
    if (timer !== null) timers.clear(timer);
    timer = timers.set(() => {
      timer = null;
      void check(false).finally(() => schedule(CHECK_INTERVAL_MS));
    }, ms);
  }

  function dismiss(): void {
    if (phase === "ready" || phase === "available" || phase === "external") dismissed = version;
    if (phase === "ready" || phase === "failed" || phase === "up_to_date" || phase === "available" || phase === "external") set("idle", version, null);
  }

  return {
    async handle(command) {
      switch (command) {
        case "start":
          if (started) return;
          started = true;
          if (!api.configured()) {
            publish(report());
            return;
          }
          schedule(LAUNCH_DELAY_MS);
          return;
        case "check":
          return check(true);
        case "install":
          return install();
        case "dismiss":
          return dismiss();
        case "auto-on":
        case "auto-off": {
          auto = command === "auto-on";
          publish(report());
          // Switching it on while a release waits downloads it, as the old updater did.
          if (auto && phase === "available") await check(true);
          return;
        }
      }
    },
    report,
    stop() {
      stopped = true;
      if (timer !== null) timers.clear(timer);
      timer = null;
    },
  };
}
