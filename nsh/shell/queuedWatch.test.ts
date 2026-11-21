// The Queued watch's pace and what it drops. A return that stays Queued (the worker is off, SFTP is down, the computer is offline) used to
// make the watch drop the returns, deadlines, forms and jobs reads every 3 s for good, so every mounted screen read again each time.
// Now it looks every 3 s for about 30 s after the Queued set last changed (or a queue action), then every 30 s even without the events poll;
// a change of the set or a new queue action starts the fast phase again. The pace is shortened here (50 ms looks, a 500 ms fast phase).
// The watch drops WORKER_KEYS (the drafts count and the as-needed forms too); the plain 30 s poll keeps its narrower POLL_KEYS, so the
// drafts count stays held past it (Home draws at once). The re-reads of a queue action end with the session (log out, sign out of the account).
// Plain test (no window): the host's list_returns is faked, the app is booted with first-run setup off.
import { untracked } from "nsh";
import { expect, test } from "nsh/test";
import { bir, type ReturnRow } from "../bir";
import { createDemoTaxpayer, DEMO } from "../bir/testing";
import { app, boot, pollSteps, startQueuedWatch } from "./app";
import { POLL_KEYS, SAVED_RETURNS_KEY, store, WORKER_KEYS } from "./store";

const pause = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));
/** Waits for what a test proves to have happened, not for a count of looks in a fixed sleep: a slow machine makes fewer looks in the same time. */
async function until(condition: () => boolean, what: string, cap = 15_000): Promise<void> {
  const end = Date.now() + cap;
  while (!condition()) {
    if (Date.now() > end) throw new Error(`never happened in ${cap} ms: ${what}`);
    await pause(5);
  }
}
const ROW: ReturnRow = {
  id: 9101, tin: DEMO.tin, tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 1,
  form_code: "1601C", taxable_year: 2026, period: 10, period_label: "October 2026", status: "Queued", updated_at: "2026-11-14 09:30:00", return_key: 10,
};
const TIMING = { firstMs: 20, tickMs: 50, fastMs: 500 };

let filed: ReturnRow[] = [ROW];
/** How often the host was asked for the filed list: each drop of "returns:" is read again at once by the frame's own reads. */
let reads = 0;
const fake = bir as unknown as Record<string, unknown>;
fake.list_returns = (which: string) => {
  if (which !== "filed") return [];
  reads += 1;
  return filed;
};
const peek = () => untracked(() => store.returns("filed"));

test("the key sets: the watch and the queue actions drop WORKER_KEYS, the plain poll POLL_KEYS (the drafts count stays held past it)", async () => {
  await boot(undefined, { firstRun: false });
  for (const key of ["returns:filed", "returns:saved", "deadlines:2026-01-01:2026-12-31", "forms:00012345300000:2026", "jobs"]) {
    expect([key, POLL_KEYS.test(key), WORKER_KEYS.test(key)]).toEqual([key, true, true]);
  }
  for (const key of [SAVED_RETURNS_KEY, "as_needed:2026"]) expect([key, POLL_KEYS.test(key), WORKER_KEYS.test(key)]).toEqual([key, false, true]);

  const held = () => store.has(SAVED_RETURNS_KEY) && store.has("as_needed:2026");
  const hold = () => {
    untracked(() => store.savedReturns());
    untracked(() => store.asNeeded(2026));
    expect(held()).toBe(true);
  };
  hold();
  pollSteps.events(); // the plain 30 s poll
  expect(held()).toBe(true);
  pollSteps.queued(); // one look of the Queued watch
  expect(store.has(SAVED_RETURNS_KEY)).toBe(false);
  expect(store.has("as_needed:2026")).toBe(false);

  // The running watch drops the wider set as well.
  hold();
  const stop = startQueuedWatch(TIMING);
  try {
    await pause(250);
    expect(store.has(SAVED_RETURNS_KEY)).toBe(false);
    expect(store.has("as_needed:2026")).toBe(false);
  } finally {
    stop();
  }
});

test("the watch looks every tick for fastMs, then waits between slow looks; a changed set or a queue action starts it again", async () => {
  createDemoTaxpayer();
  filed = [ROW];
  store.bump();
  peek();
  // The fast phase lasts fastMs by Date.now(). Counting looks in a fixed sleep asks the machine for a speed (a slow runner makes a few
  // in the time this machine makes six), so the phase is a minute here, which no machine takes, and the test moves the clock past it
  // itself: each phase waits for its looks to come, and "quiet" is the looks that follow the jump.
  const fastMs = 60_000;
  const realNow = Date.now;
  let skew = 0;
  Date.now = () => realNow() + skew;
  const pastFastPhase = () => void (skew += fastMs + 1);
  const stop = startQueuedWatch({ ...TIMING, fastMs, slowMs: 10 * fastMs });
  try {
    const start = reads;
    await until(() => reads - start >= 3, "a read per look in the fast phase"); // fast phase: a read per look

    pastFastPhase();
    const settled = reads; // a look is synchronous: none is half done
    await pause(500);
    expect(reads).toBe(settled); // still Queued, nothing changed: no drops, no reads

    // The 30 s events poll read the list and found another Queued return: the set changed, the fast phase starts again.
    filed = [ROW, { ...ROW, id: 9102, form_code: "2551Q" }];
    store.forget("returns:filed");
    peek();
    const afterChange = reads;
    await until(() => reads - afterChange >= 3, "the fast phase starts again when the Queued set changes");
    pastFastPhase();
    const quiet = reads;
    await pause(500);
    expect(reads).toBe(quiet);

    // A queue action with the same set: the fast phase starts again. Its own re-reads (1, 3 and 10 s) are put off, so only looks read.
    store.afterQueueChange();
    store.cancelRereads();
    const afterAction = reads;
    await until(() => reads - afterAction >= 3, "the fast phase starts again after a queue action");
  } finally {
    Date.now = realNow;
    stop();
    store.cancelRereads();
  }
}, 30_000);

test("a worker submission after the fast phase refreshes the cached filings without the normal events poll", async () => {
  filed = [ROW];
  store.bump();
  peek();
  const realNow = Date.now;
  let skew = Math.max(0, store.lastQueueAction() - realNow());
  Date.now = () => realNow() + skew;
  const fastMs = 60_000;
  const slowMs = 120_000;
  const stop = startQueuedWatch({ ...TIMING, fastMs, slowMs });
  try {
    const start = reads;
    await until(() => reads - start >= 3, "the fast watch reads the queued return");
    skew += fastMs + 1;
    const settled = reads;
    filed = [{ ...ROW, status: "Submitted" }];
    await pause(250);
    expect(reads).toBe(settled);
    expect(peek().filter((r) => r.status === "Queued").length).toBe(1);

    skew += slowMs;
    await until(() => peek()[0]?.status === "Submitted", "the slow watch refreshes the cached filing");
    expect(peek().filter((r) => r.status === "Queued").length).toBe(0);
    expect(peek().filter((r) => r.status === "Submitted").length).toBe(1);
    const submitted = reads;
    await pause(250);
    expect(reads).toBe(submitted);
  } finally {
    Date.now = realNow;
    stop();
  }
}, 30_000);

test("nothing queued: the watch reads nothing", async () => {
  filed = [{ ...ROW, status: "Submitted" }];
  store.bump();
  peek();
  const stop = startQueuedWatch(TIMING);
  try {
    await pause(100);
    const before = reads;
    await pause(400);
    expect(reads).toBe(before);
  } finally {
    stop();
  }
});

test("the re-reads of a queue action end when the session ends: log out, and signing out of the Buwiz account", async () => {
  filed = [{ ...ROW, status: "Submitted" }];
  store.bump();
  peek();

  // Control: left alone, the 1 s re-read comes.
  store.afterQueueChange();
  let before = reads;
  await pause(1250);
  expect(reads).toBeGreaterThan(before);
  store.cancelRereads();

  // Signing out of the account (Settings > Account).
  fake.cloud_account_session_closed = () => {};
  store.afterQueueChange();
  before = reads;
  app.lockCloud();
  const lockedAt = reads;
  await pause(1250);
  expect(reads).toBe(lockedAt);
  expect(lockedAt).toBeGreaterThanOrEqual(before);
  app.setCloudLocked(false);

  // Log out (the owner has a PIN).
  bir.setup_solo("000123453");
  bir.set_admin_pin("1234");
  store.forget("auth_state");
  store.afterQueueChange();
  app.logOut();
  expect(app.appLocked()).toBe(true);
  const loggedOutAt = reads;
  await pause(1250);
  expect(reads).toBe(loggedOutAt);
}, 30_000);
