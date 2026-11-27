// F8-9: Back and Forward after a TIN move, a branch-code move and a deleted taxpayer. A move rewrites every history and Forward stop
// (the return a form stop names and the selection it was left on), not only Recent; a stop whose taxpayer is gone is skipped, so Back
// never opens a form on a TIN that no longer exists or the profile of whoever is first now. Through the real door (scratch database,
// the demo taxpayer Juana Santos and a generated second one).
import { expect, test } from "nsh/test";
import { bir } from "../bir";
import { createDemoTaxpayer, DEMO } from "../bir/testing";
import { app, back, boot, forwardOne, go } from "./app";
import { store } from "./store";
import type { FormTarget } from "../screens/form/FormPage";

const YEAR = new Date().getFullYear();
const MOVED = "000123461";
const B = "000111225";

const target = () => app.route().params.target as FormTarget | undefined;

test("after a TIN move, a branch-code move and a delete, Back and Forward open only what still exists", async () => {
  createDemoTaxpayer();
  bir.create_taxpayer({ tin_root: "000-111-225", name: "Sample Trading Corp", taxpayer_type: "Corporation", head_office: { rdo_code: "039", registered_address: "Quezon City", zip_code: "1100" } });
  bir.create_branch(DEMO.tin.slice(0, 9), { branch_code: "00001", name: "Cebu branch" });
  store.bump();
  await boot(undefined, { testEnvironment: true, firstRun: false });
  expect(app.appLocked()).toBe(false);

  // Each stop keeps the selection the frame was on when it was left. A's 2551Q (head office); A's profile, left on B (B's profile is
  // what that stop shows); Home, left on A's branch 00001; that branch's profile; Home.
  app.openReturn({ tin: DEMO.tin, code: "2551Q", year: YEAR, period: 1 });
  await app.settled();
  expect(target()?.tin).toBe(DEMO.tin);
  go("profile");
  await app.settled();
  app.selectTaxpayer(B, { quiet: true });
  go("home");
  await app.settled();
  app.selectTaxpayer(DEMO.tin.slice(0, 9), { quiet: true, code: "00001" });
  go("profile");
  await app.settled();
  go("home");
  await app.settled();

  // A's TIN moves (what the profile's Save does), then A's branch 00001 becomes 00002.
  store.write("move_taxpayer_tin", DEMO.tin.slice(0, 9), MOVED);
  app.remapRecent((tin) => (tin.startsWith(DEMO.tin.slice(0, 9)) ? MOVED + tin.slice(9) : tin));
  store.write("move_branch_code", MOVED + "00001", "00002");
  app.remapRecent((tin) => (tin === MOVED + "00001" ? MOVED + "00002" : tin));
  // B is deleted.
  store.write("delete_taxpayer", B);
  store.bump();

  // Back: the branch profile, on the moved TIN and branch; then Home; then B's stop is skipped (B is gone) and the 2551Q opens on
  // the moved TIN.
  expect(back()).toBe(true);
  await app.settled();
  expect(app.view()).toBe("profile");
  expect(app.taxpayer()?.tin_root).toBe(MOVED);
  expect(app.branch()?.code).toBe("00002");
  expect(back()).toBe(true);
  await app.settled();
  expect(app.view()).toBe("home");
  expect(app.canBack()).toBe(true);
  expect(back()).toBe(true);
  await app.settled();
  expect(app.view()).toBe("form");
  expect(target()?.tin).toBe(MOVED + "00000");
  expect(app.taxpayer()?.tin_root).toBe(MOVED);

  // Forward: Home, the branch profile, Home; B's stop never comes back.
  expect(forwardOne()).toBe(true);
  await app.settled();
  expect(app.view()).toBe("home");
  expect(forwardOne()).toBe(true);
  await app.settled();
  expect(app.view()).toBe("profile");
  expect(app.taxpayer()?.tin_root).toBe(MOVED);
  expect(app.branch()?.code).toBe("00002");
  expect(forwardOne()).toBe(true);
  await app.settled();
  expect(app.view()).toBe("home");
  expect(app.canForward()).toBe(false);
});
