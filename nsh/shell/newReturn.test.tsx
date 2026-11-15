// The Start a return dialog's header names who it is for: "name ·", the TIN with its branch code, the branch's name (shell/palette.test.tsx reads it
// with a taxpayer). With no taxpayer there is no name, and the dot must not stand alone. NewReturnDemo.tsx mounts the dialog by itself
// (the frame never opens it with no taxpayer).
import { expect, open, test } from "nsh/test";

const ENTRY = "../NewReturnDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };

test("desktop: with no taxpayer the header has no dot of its own", async () => {
  const app = await open(ENTRY, WIDE);
  await app.waitFor("id:new-return-who");
  expect(app.texts()).toContain("Start a return");
  expect(app.texts().some((t) => t.includes("·"))).toBe(false);
});

test("phone: the same", async () => {
  const app = await open(ENTRY);
  await app.waitFor("id:new-return-who");
  expect(app.texts().some((t) => t.includes("·"))).toBe(false);
});
