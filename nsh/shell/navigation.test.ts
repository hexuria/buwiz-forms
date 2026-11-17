// The navigation in the app state: Back and Forward keep the taxpayer each stop was left on, an amendment saved while it is in the
// history opens as the saved return, the screens' beforeLeave hooks run (3 s at most) before any move and before quit, and the
// persisted keys follow the database path. A plain test (no mounted frame): it drives the app state on a scratch database.
import { expect, test } from "nsh/test";
import { bir } from "../bir";
import { createDemoTaxpayer } from "../bir/testing";
import type { FormTarget } from "../screens/form/FormPage";
import { app, LEAVE_HOOKS_CAP_MS, storageKey } from "./app";
import { store } from "./store";

const pause = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));

const SECOND_ROOT = "000111229";
const SECOND_TIN = "00011122900000";

/** The demo taxpayer (Juana Santos, selected) and a second one (Sample Trading Corp), the frame on the first. */
function twoTaxpayers() {
  const juana = createDemoTaxpayer();
  if (!bir.list_taxpayers().some((t) => t.tin_root === SECOND_ROOT)) {
    bir.create_taxpayer({ tin_root: "000-111-229", name: "Sample Trading Corp", taxpayer_type: "Corporation", head_office: { rdo_code: "039", registered_address: "Quezon City", zip_code: "1100" } });
  }
  store.bump();
  app.selectTaxpayer(juana.tin_root, { quiet: true });
  return juana;
}

const target = () => app.params().target as FormTarget;

test("the persisted keys end in the database path; with no path there is no key, and nothing is shared", () => {
  expect(storageKey("redesign.recent", "/data/a/bir_data.db")).toBe("redesign.recent@/data/a/bir_data.db");
  expect(storageKey("redesign.selection", "/data/b/bir_data.db")).toBe("redesign.selection@/data/b/bir_data.db");
  expect(storageKey("redesign.read_notifications", "/data/a/bir_data.db")).toBe("redesign.read_notifications@/data/a/bir_data.db");
  // The preferences belong to the app, not to a database.
  expect(storageKey("redesign.prefs", "/data/a/bir_data.db")).toBe("redesign.prefs");
  expect(storageKey("redesign.prefs", "")).toBe("redesign.prefs");
  // An unknown path: these three are not kept at all (before, every database without a path shared "redesign.recent@").
  for (const key of ["redesign.recent", "redesign.selection", "redesign.read_notifications"]) expect(storageKey(key, "")).toBeNull();
});

test("Back goes onto the taxpayer the screen was left on, and Forward onto the one it came from", () => {
  const juana = twoTaxpayers();
  app.go("calendar");
  expect(app.taxpayer()?.tin_root).toBe(juana.tin_root);
  // A return of the second taxpayer opens: the frame is on her while it shows.
  app.openReturn({ tin: SECOND_TIN, code: "1601C", year: 2025, period: 3 });
  expect(app.view()).toBe("form");
  expect(app.taxpayer()?.tin_root).toBe(SECOND_ROOT);

  expect(app.back()).toBe(true);
  expect(app.view()).toBe("calendar");
  expect(app.taxpayer()?.tin_root).toBe(juana.tin_root);
  expect(app.canForward()).toBe(true);

  expect(app.forward()).toBe(true);
  expect(app.view()).toBe("form");
  expect(app.taxpayer()?.tin_root).toBe(SECOND_ROOT);

  // A taxpayer picked from search: Back returns to the page, on the taxpayer it was on.
  app.go("home");
  app.selectTaxpayer(juana.tin_root, { quiet: true });
  app.openTaxpayer(SECOND_ROOT);
  expect(app.view()).toBe("profile");
  expect(app.taxpayer()?.tin_root).toBe(SECOND_ROOT);
  app.back();
  expect(app.view()).toBe("home");
  expect(app.taxpayer()?.tin_root).toBe(juana.tin_root);
});

test("an amendment saved on its page is a stored return in the history: Back does not start it again", () => {
  const juana = twoTaxpayers();
  const tin = juana.branches[0].tin;
  app.go("filings");
  app.openReturn({ tin, code: "2551Q", year: 2025, period: 1, mode: "amend" });
  expect(target().mode).toBe("amend");

  // The page saves the amendment (its first save is a save of the return: the frame bumps Recent).
  app.bumpRecent({ tin, code: "2551Q", year: 2025, period: 1, returnKey: null });
  expect(target().mode).toBe("return");

  app.go("home");
  app.back();
  expect(app.view()).toBe("form");
  expect(target().mode).toBe("return");
  expect(target().formCode).toBe("2551Q");
});

test("an amendment saved after the person left it (the pending save runs as the page goes) is a stored return in Back and in Forward", () => {
  const juana = twoTaxpayers();
  const tin = juana.branches[0].tin;
  app.go("filings");
  app.openReturn({ tin, code: "1601C", year: 2025, period: 2, mode: "amend" });
  app.go("library");
  app.go("calendar");
  for (let i = 0; i < 3; i++) app.back(); // calendar, library, the amendment, then Filings: the amendment now sits in Forward
  expect(app.view()).toBe("filings");
  // The page's last save lands now, while the amendment sits in Forward.
  app.bumpRecent({ tin, code: "1601C", year: 2025, period: 2, returnKey: null });
  app.forward();
  expect(app.view()).toBe("form");
  expect(target().mode).toBe("return");
});

test("a page that reports it shows the saved return (retarget) settles the amendment in the history too", () => {
  const juana = twoTaxpayers();
  const tin = juana.branches[0].tin;
  app.go("filings");
  app.openReturn({ tin, code: "2551Q", year: 2025, period: 2, mode: "amend" });
  app.go("calendar");
  // The page is gone from the screen; a late report of the saved return still reaches the history.
  app.retarget({ tin, formCode: "2551Q", year: 2025, period: 2, key: null, mode: "return" });
  app.back();
  expect(app.view()).toBe("form");
  expect(target().mode).toBe("return");
  // Another amendment of another period is left alone.
  app.go("home");
  app.openReturn({ tin, code: "2551Q", year: 2025, period: 3, mode: "amend" });
  app.bumpRecent({ tin, code: "2551Q", year: 2025, period: 1, returnKey: null });
  expect(target().mode).toBe("amend");
});

test("a move waits for the screens' beforeLeave hooks, in order, and a screen that goes takes its hook away", async () => {
  twoTaxpayers();
  app.go("home");
  const log: string[] = [];
  const off = app.beforeLeave(async () => {
    await pause(60);
    log.push("saved");
  });
  app.go("filings");
  expect(app.view()).toBe("home"); // not yet: the hook is still saving
  app.go("calendar");
  await app.settled();
  log.push("moved");
  expect(log).toEqual(["saved", "saved", "moved"]);
  expect(app.view()).toBe("calendar");

  // Back and Forward wait too, and still say at once whether there is a step.
  expect(app.back()).toBe(true);
  expect(app.view()).toBe("calendar");
  await app.settled();
  expect(app.view()).toBe("filings");
  expect(app.forward()).toBe(true);
  await app.settled();
  expect(app.view()).toBe("calendar");
  app.replace("library");
  await app.settled();
  expect(app.view()).toBe("library");

  // Nothing registered: a move is at once, as before.
  off();
  app.go("filings");
  expect(app.view()).toBe("filings");
});

test("a hook that throws does not stop the move; one that never answers is waited for 3 seconds, no longer", async () => {
  twoTaxpayers();
  app.go("home");
  let ran = 0;
  const offBroken = app.beforeLeave(() => {
    ran++;
    throw new Error("could not save");
  });
  app.go("filings");
  await app.settled();
  expect(ran).toBe(1);
  expect(app.view()).toBe("filings");
  offBroken();

  const offStuck = app.beforeLeave(() => new Promise<void>(() => {}));
  const started = Date.now();
  app.go("calendar");
  await pause(LEAVE_HOOKS_CAP_MS / 2);
  expect(app.view()).toBe("filings");
  await app.settled();
  const waited = Date.now() - started;
  expect(app.view()).toBe("calendar");
  expect(waited).toBeGreaterThanOrEqual(LEAVE_HOOKS_CAP_MS - 50);
  expect(waited).toBeLessThan(LEAVE_HOOKS_CAP_MS + 1500);
  offStuck();
}, 30_000);

test("quit waits for the hooks first, 3 seconds at most", async () => {
  const order: string[] = [];
  const off = app.beforeLeave(async () => {
    await pause(80);
    order.push("saved");
  });
  await app.quit(() => order.push("exit"));
  expect(order).toEqual(["saved", "exit"]);
  off();

  const offStuck = app.beforeLeave(() => new Promise<void>(() => {}));
  const started = Date.now();
  await app.quit(() => order.push("exit anyway"));
  expect(order).toEqual(["saved", "exit", "exit anyway"]);
  expect(Date.now() - started).toBeLessThan(LEAVE_HOOKS_CAP_MS + 1500);
  offStuck();
}, 30_000);
