// The updater's state machine and the banner's words, played against a fake nsh/updater: no network, no window.
import { expect, test } from "nsh/test";
import { promptFor, promptKey } from "./banner";
import { CHECK_INTERVAL_MS, createUpdater, LAUNCH_DELAY_MS, NOT_CONFIGURED, VERIFY_REJECTED, type UpdateReport, type UpdaterApi } from "./updater";

/** A fake nsh/updater that records what was asked, and answers what the test says. */
function fake(over: Partial<UpdaterApi> & { found?: string | null } = {}) {
  const calls: string[] = [];
  const api: UpdaterApi = {
    configured: () => true,
    check: async () => {
      calls.push("check");
      return over.found === null ? null : { version: over.found ?? "0.3.0" };
    },
    download: async () => {
      calls.push("download");
    },
    install: async () => {
      calls.push("install");
    },
    ...over,
  };
  return { api, calls };
}

/** Timers a test moves by hand. */
function clock() {
  const pending: { id: number; at: number; fn: () => void }[] = [];
  let now = 0;
  let next = 1;
  return {
    timers: {
      set: (fn: () => void, ms: number) => {
        const id = next++;
        pending.push({ id, at: now + ms, fn });
        return id;
      },
      clear: (id: unknown) => {
        const i = pending.findIndex((t) => t.id === id);
        if (i >= 0) pending.splice(i, 1);
      },
    },
    /** Moves time by ms and runs what comes due, in order. */
    async advance(ms: number) {
      const to = now + ms;
      for (;;) {
        pending.sort((a, b) => a.at - b.at);
        const due = pending[0];
        if (!due || due.at > to) break;
        pending.shift();
        now = due.at;
        due.fn();
        await new Promise<void>((done) => setTimeout(done, 0));
      }
      now = to;
    },
    waiting: () => pending.length,
  };
}

function driver(api: UpdaterApi, options: { auto?: boolean; timers?: ReturnType<typeof clock>["timers"] } = {}) {
  const published: UpdateReport[] = [];
  const updater = createUpdater({
    api,
    publish: (r) => published.push(r),
    autoDownload: () => options.auto ?? true,
    currentVersion: () => "0.2.0",
    timers: options.timers,
  });
  return { updater, published, phases: () => published.map((r) => r.phase) };
}

test("no update source: every command answers the same honest idle, and nothing is ever fetched", async () => {
  const { api, calls } = fake({ configured: () => false });
  const { updater, published } = driver(api);
  for (const command of ["start", "check", "install", "dismiss", "auto-on", "auto-off"] as const) await updater.handle(command);
  expect(calls).toEqual([]);
  expect(published.length > 0).toBe(true);
  for (const r of published) expect(r).toMatchObject({ configured: false, phase: "idle", version: null, message: NOT_CONFIGURED });
  expect(updater.report()).toMatchObject({ configured: false, phase: "idle", message: NOT_CONFIGURED });
});

test("a manual check that finds a release downloads it (even with the automatic download off) and reaches ready", async () => {
  const { api, calls } = fake();
  const { updater, phases } = driver(api, { auto: false });
  await updater.handle("check");
  expect(calls).toEqual(["check", "download"]);
  expect(phases()).toEqual(["checking", "downloading", "ready"]);
  expect(updater.report()).toMatchObject({ configured: true, phase: "ready", version: "0.3.0", message: null });
});

test("a manual check that finds nothing says the app is up to date; Later puts it away", async () => {
  const { api, calls } = fake({ found: null });
  const { updater, phases } = driver(api);
  await updater.handle("check");
  expect(calls).toEqual(["check"]);
  expect(phases()).toEqual(["checking", "up_to_date"]);
  expect(updater.report().message).toBe("eBIRForms 0.2.0 is up to date.");
  await updater.handle("dismiss");
  expect(updater.report()).toMatchObject({ phase: "idle", message: null });
});

test("the launch check runs after the delay and then every four hours; it downloads only with the automatic download on", async () => {
  const time = clock();
  const { api, calls } = fake();
  const on = driver(api, { auto: true, timers: time.timers });
  await on.updater.handle("start");
  await on.updater.handle("start"); // a second start changes nothing
  expect(time.waiting()).toBe(1);
  await time.advance(LAUNCH_DELAY_MS - 1);
  expect(calls).toEqual([]);
  await time.advance(2);
  expect(calls).toEqual(["check", "download"]);
  expect(on.updater.report().phase).toBe("ready");
  await time.advance(CHECK_INTERVAL_MS);
  expect(calls.filter((c) => c === "check").length).toBe(2);
  on.updater.stop();
  expect(time.waiting()).toBe(0);

  const quietTime = clock();
  const quiet = fake();
  const off = driver(quiet.api, { auto: false, timers: quietTime.timers });
  await off.updater.handle("start");
  await quietTime.advance(LAUNCH_DELAY_MS + 1);
  expect(quiet.calls).toEqual(["check"]);
  expect(off.updater.report()).toMatchObject({ phase: "available", version: "0.3.0" });
  // Switching the automatic download on while a release waits downloads it.
  await off.updater.handle("auto-on");
  expect(quiet.calls).toEqual(["check", "check", "download"]);
  expect(off.updater.report()).toMatchObject({ phase: "ready", autoDownload: true });
  off.updater.stop();
});

test("a release that is ready stays on offer through the periodic check, and is not downloaded twice", async () => {
  const time = clock();
  const { api, calls } = fake();
  const { updater, phases } = driver(api, { timers: time.timers });
  await updater.handle("check");
  expect(calls).toEqual(["check", "download"]);
  const seen = phases().length;
  await updater.handle("start");
  await time.advance(LAUNCH_DELAY_MS + 1);
  // It asked again (the banner never flickers to "checking") and found the same release: nothing more to fetch.
  expect(calls).toEqual(["check", "download", "check"]);
  expect(phases().slice(seen)).toEqual(["ready"]);
  expect(updater.report()).toMatchObject({ phase: "ready", version: "0.3.0" });
  updater.stop();
});

test("a periodic check that fails while a release waits leaves the release on offer", async () => {
  const time = clock();
  let fail = false;
  const { api } = fake({
    check: async () => {
      if (fail) throw new Error("offline");
      return { version: "0.3.0" };
    },
  });
  const { updater } = driver(api, { timers: time.timers });
  await updater.handle("check");
  fail = true;
  await updater.handle("start");
  await time.advance(LAUNCH_DELAY_MS + 1);
  expect(updater.report()).toMatchObject({ phase: "ready", version: "0.3.0" });
  updater.stop();
});

test("an automatic check that cannot reach the source stays quiet; a manual one says why", async () => {
  const time = clock();
  const { api } = fake({
    check: async () => {
      throw new Error("The update server did not answer.");
    },
  });
  const { updater, phases } = driver(api, { timers: time.timers });
  await updater.handle("start");
  await time.advance(LAUNCH_DELAY_MS + 1);
  expect(updater.report()).toMatchObject({ phase: "idle", message: null });
  await updater.handle("check");
  expect(updater.report()).toMatchObject({ phase: "failed", message: "The update server did not answer." });
  expect(phases().includes("failed")).toBe(true);
  updater.stop();
});

test("a download that does not verify is a failed update, and Later clears it", async () => {
  const { api } = fake({
    download: async () => {
      throw new Error(VERIFY_REJECTED);
    },
  });
  const { updater } = driver(api);
  await updater.handle("check");
  expect(updater.report()).toMatchObject({ phase: "failed", version: "0.3.0", message: VERIFY_REJECTED });
  await updater.handle("dismiss");
  expect(updater.report().phase).toBe("idle");
});

test("Install now installs only a release that is ready; a failed install says so and the app goes on", async () => {
  const idle = fake();
  const first = driver(idle.api);
  await first.updater.handle("install");
  expect(idle.calls).toEqual([]);

  const { api, calls } = fake({
    install: async () => {
      throw new Error("The app cannot be replaced.");
    },
  });
  const { updater, phases } = driver(api);
  await updater.handle("check");
  await updater.handle("install");
  expect(calls).toEqual(["check", "download"]);
  expect(phases().slice(-2)).toEqual(["installing", "failed"]);
  expect(updater.report()).toMatchObject({ phase: "failed", message: "The app cannot be replaced." });
});

test("Later on a ready release is remembered: the periodic check does not offer that release again, a manual one does", async () => {
  const time = clock();
  const { api, calls } = fake();
  const { updater } = driver(api, { timers: time.timers });
  await updater.handle("check");
  await updater.handle("dismiss");
  expect(updater.report().phase).toBe("idle");
  await updater.handle("start");
  await time.advance(LAUNCH_DELAY_MS + 1);
  expect(calls).toEqual(["check", "download", "check"]);
  expect(updater.report()).toMatchObject({ phase: "idle", version: "0.3.0" });
  await updater.handle("check");
  expect(updater.report().phase).toBe("ready");
  updater.stop();
});

test("the banner: Install now and Later for a ready release, Later alone for a failure or an answer, nothing otherwise", () => {
  const status = (phase: string, version: string | null, message: string | null) => ({ phase, version, message, auto_download: true, current_version: "0.2.0" });
  expect(promptFor(null)).toBeNull();
  expect(promptFor(status("ready", "0.3.0", null))).toEqual({ message: "Version 0.3.0 is ready.", install: true });
  expect(promptFor(status("ready", null, null))).toBeNull();
  expect(promptFor(status("failed", "0.3.0", "It broke."))).toEqual({ message: "It broke.", install: false });
  expect(promptFor(status("failed", null, null))).toEqual({ message: VERIFY_REJECTED, install: false });
  expect(promptFor(status("up_to_date", null, "eBIRForms 0.2.0 is up to date."))).toEqual({ message: "eBIRForms 0.2.0 is up to date.", install: false });
  expect(promptFor(status("up_to_date", null, null))).toBeNull();
  for (const phase of ["idle", "checking", "downloading", "available", "installing"]) expect(promptFor(status(phase, "0.3.0", "x"))).toBeNull();
  // Not configured: the status is idle with a sentence, and the banner never shows.
  expect(promptFor(status("idle", null, NOT_CONFIGURED))).toBeNull();
  expect(promptKey(status("ready", "0.3.0", null))).not.toBe(promptKey(status("ready", "0.4.0", null)));
  expect(promptKey(null)).toBe("");
});
