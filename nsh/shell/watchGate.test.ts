// The Queued watch and the library warm-up used to start before the lock state was known: the watch's first look came 1 s in and the
// warm-up's build 600 ms in, checking neither that the app had booted nor that first-run setup was done, and the build's ~1 s synchronous
// host call froze the sign-in screen while someone typed. Both now wait for the frame (app.chromeVisible: booted, the account gate
// open, first-run setup done, no sign-in in front); the warm-up builds once, WARM_DELAY_MS after the frame is first shown.
// Plain test (no window): list_returns and screen_forms_index are faked and counted.
import { expect, test } from "nsh/test";
import { bir, type ReturnRow } from "../bir";
import { createDemoTaxpayer, DEMO } from "../bir/testing";
import { app, boot, startQueuedWatch, warmLibrary, WARM_DELAY_MS } from "./app";
import { store } from "./store";

const pause = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));
const ROW: ReturnRow = {
  id: 9101, tin: DEMO.tin, tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 1,
  form_code: "1601C", taxable_year: 2026, period: 10, period_label: "October 2026", status: "Queued", updated_at: "2026-11-14 09:30:00", return_key: 10,
};

let reads = 0;
let builds = 0;
const fake = bir as unknown as Record<string, unknown>;
fake.list_returns = (which: string) => {
  if (which !== "filed") return [];
  reads += 1;
  return [ROW];
};
fake.screen_forms_index = () => {
  builds += 1;
  return [{ c: "1601C", t: "Monthly Remittance Return", freq: "Monthly" }];
};

test("neither the watch nor the warm-up does anything before boot, behind first-run setup or behind the sign-in; the frame showing starts both, the build once", async () => {
  createDemoTaxpayer();
  const stopWatch = startQueuedWatch({ firstMs: 20, tickMs: 50, fastMs: 5000 });
  const stopWarm = warmLibrary();
  try {
    // Not booted: nothing is known about locks yet.
    await pause(WARM_DELAY_MS + 400);
    expect([app.booted(), reads, builds]).toEqual([false, 0, 0]);

    // Booted, first-run setup (the "who will use this computer" choice) in front of the frame.
    await boot(undefined, { firstRun: true });
    expect([app.booted(), app.chromeVisible()]).toEqual([true, false]);
    const booting = reads; // the boot's own read of the filed list (the notifications)
    await pause(WARM_DELAY_MS + 400);
    expect([reads - booting, builds]).toEqual([0, 0]);

    // Set up, with a PIN, and the sign-in screen in front.
    app.setAppLocked(true);
    bir.setup_solo("000123453");
    bir.set_admin_pin("1234");
    store.forget("auth_state");
    expect(app.chromeVisible()).toBe(false);
    await pause(WARM_DELAY_MS + 400);
    expect([reads - booting, builds]).toEqual([0, 0]);

    // Signed in: the frame shows. The watch looks at once (a Queued return); the library is not built yet (the delay is not over) ...
    app.setAppLocked(false);
    expect(app.chromeVisible()).toBe(true);
    await pause(300);
    expect(reads - booting).toBeGreaterThan(0);
    expect(builds).toBe(0);

    // ... and a lock within the delay: the timer finds the frame gone and builds nothing behind it.
    app.setAppLocked(true);
    const locked = reads;
    await pause(WARM_DELAY_MS + 300);
    expect([reads - locked, builds]).toEqual([0, 0]);

    // The frame shows again: the library is built after the delay, once.
    app.setAppLocked(false);
    await pause(300);
    expect(builds).toBe(0);
    await pause(WARM_DELAY_MS);
    expect(builds).toBe(1);

    // The frame going away and coming back builds nothing more.
    app.setAppLocked(true);
    await pause(100);
    app.setAppLocked(false);
    await pause(WARM_DELAY_MS + 400);
    expect(builds).toBe(1);
  } finally {
    stopWatch();
    stopWarm();
    store.cancelRereads();
  }
}, 30_000);
