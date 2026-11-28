// The migration pages (screens/auth/migration.tsx, through MigrationDemo.tsx): the controls exist and answer,
// on the phone and on the desktop. The look is shots/migration.test.tsx's.
import { expect, open, test } from "nsh/test";

const DEMO = "../../MigrationDemo.tsx";

test("moving with no state: no heading in the first second, then neutral Opening", async () => {
  const app = await open(DEMO, { clock: {}, host: { params: { page: "moving" } } });
  await app.waitFor("id:migration-moving");
  expect(app.texts()).not.toContain("Moving your returns…");
  expect(app.texts()).not.toContain("Opening your account…");
  await (app.clock as unknown as { runFor(ms: number): Promise<void> }).runFor(1100);
  expect(app.texts()).toContain("Opening your account…");
  expect(app.texts()).not.toContain("Moving your returns…");
});

test("moving: the progress page shows, phone", async () => {
  const app = await open(DEMO, { host: { params: { page: "moving" } } });
  await app.waitFor("id:migration-moving");
  expect(app.shows("id:migration-moving-title")).toBe(false);
});

test("moving: the progress page shows, desktop", async () => {
  const app = await open(DEMO, { width: 1440, height: 900, platform: "macos", host: { params: { page: "moving" } } });
  await app.waitFor("id:migration-moving");
  expect(app.shows("id:migration-moving-title")).toBe(false);
});

test("transient before step 3: nothing was changed, no copy line, phone", async () => {
  const app = await open(DEMO, { host: { params: { page: "transient" } } });
  await app.waitFor("id:migration-transient");
  expect(app.texts()).toContain("Nothing was changed: they are still where they were.");
  expect(app.texts()).not.toContain("Nothing was changed: they are still where they were, and a copy is saved in this app's storage.");
  await app.waitFor("id:migration-try-again");
  await app.waitFor("id:migration-transient-sign-out");
});

test("transient after step 3: the copy line shows, desktop", async () => {
  const app = await open(DEMO, { width: 1440, height: 900, platform: "macos", host: { params: { page: "transient-copy" } } });
  await app.waitFor("id:migration-transient");
  expect(app.texts()).toContain("Nothing was changed: they are still where they were, and a copy is saved in this app's storage.");
});

test("moved, not opened: support and sign out, phone", async () => {
  const app = await open(DEMO, { host: { params: { page: "moved" } } });
  await app.waitFor("id:migration-moved-not-opened");
  expect(app.texts()).toContain("Your tax profiles were moved, but could not be opened.");
  expect(app.shows("id:migration-try-again")).toBe(false);
  await app.waitFor("id:migration-moved-sign-out");
});

test("the offer: both answers, desktop", async () => {
  const app = await open(DEMO, { width: 1440, height: 900, platform: "macos", host: { params: { page: "offered" } } });
  await app.waitFor("id:migration-offer");
  expect(app.texts()).toContain("Tax profiles from an older version of eBIRForms were found on this device.");
  await app.waitFor("id:migration-offer-add");
  await app.waitFor("id:migration-offer-not-now");
});
