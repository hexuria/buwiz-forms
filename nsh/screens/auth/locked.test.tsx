// F7-3: first launch's "Just me" is half done (pending "solo") while the database's access mode is already set and it has no
// taxpayer: the screen in front of the frame is the New taxpayer step, not a blank window, also when it is mounted again.
import { expect, open, test } from "nsh/test";

const ENTRY = "./LockedDemo.tsx";

test("phone: access mode set, first taxpayer not created: the New taxpayer step shows, and again after a re-mount", async () => {
  const app = await open(ENTRY);
  await app.waitFor("text:/^mode: helpdesk \\| pending: solo \\| taxpayers: 0$/");
  await app.waitFor("id:new-tp-card");
  expect(app.shows("id:setup-card")).toBe(false);
  // The screen goes and is mounted again with the same state: it is not blank.
  await app.press("id:fx-hide");
  await app.waitUntil(() => !app.shows("id:locked-screen"));
  await app.press("id:fx-show");
  await app.waitFor("id:new-tp-card");
  expect(app.shows("id:new-tp-tin")).toBe(true);
});

test("desktop: the same at desktop size", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos" });
  await app.waitFor("id:new-tp-card");
  await app.press("id:fx-hide");
  await app.press("id:fx-show");
  await app.waitFor("id:new-tp-card");
  expect(app.shows("id:new-tp-save")).toBe(true);
});
