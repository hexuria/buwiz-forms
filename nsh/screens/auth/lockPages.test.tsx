// The owner lock and Set PIN components against a recording stub of their injected interface.
import { expect, open, test } from "nsh/test";
import type { LockInterface } from "./lockInterface";

const WIDE = { width: 1440, height: 900, platform: "macos" as const };

test("phone: empty lock shows the account, PIN, links and no biometric slot", async () => {
  const app = await open("./LockPagesDemo.tsx", { host: { params: { page: "lock", biometric: "none" } } });
  expect(app.texts()).toContain("ju•••@buwiz.com");
  expect(app.shows("id:owner-lock-pin")).toBe(true);
  expect(app.shows("id:lock-biometric")).toBe(false);
  expect(app.texts()).toContain("Forgot PIN?");
  expect(app.texts()).toContain("Not you? Sign out");
  expect(app.texts()).toContain("Client sign-in");
});

test("desktop: wrong PIN, countdown and lost store states come only from the interface", async () => {
  const app = await open("./LockPagesDemo.tsx", { ...WIDE, host: { params: { page: "lock", wrong: true, biometric: "face_id" } } });
  expect(app.texts()).toContain("Unlock with Face ID");
  await app.type("id:owner-lock-pin", "0000");
  await app.waitFor("text:Wrong PIN. 2 tries left.");
  await app.press("id:fx-wait");
  await app.waitFor("text:Try again in 1:05");
  await app.waitFor("text:Try again in 1:04");
  await app.press("id:fx-lost");
  await app.waitFor("text:This device lost its lock store. Contact support.");
});

test("desktop: every lock action calls the interface", async () => {
  const app = await open("./LockPagesDemo.tsx", { ...WIDE, host: { params: { page: "lock", biometric: "touch_id" } } });
  expect(app.texts()).toContain("Unlock with Touch ID");
  await app.press("id:lock-biometric");
  await app.press("id:lock-forgot");
  await app.press("id:lock-sign-out");
  await app.press("id:lock-client-sign-in");
  await app.waitFor("text:calls: biometric,forgot,signOut,clientSignIn");
});

test("desktop: Set PIN mismatch is shown, then matching PINs call set once", async () => {
  const app = await open("./LockPagesDemo.tsx", { ...WIDE, host: { params: { page: "set" } } });
  expect(app.texts()).toContain("Set a PIN for this device");
  await app.type("id:set-pin-first", "1234");
  await app.type("id:set-pin-again", "1235");
  await app.waitFor("text:The PINs don't match. Try again.");
  await app.type("id:set-pin-first", "1234");
  await app.type("id:set-pin-again", "1234");
  await app.press("id:set-pin-done");
  await app.waitFor("text:calls: set:1234");
});

test("desktop: windows hello label is the interface's label", async () => {
  const app = await open("./LockPagesDemo.tsx", { ...WIDE, host: { params: { page: "lock", biometric: "windows_hello" } } });
  expect(app.texts()).toContain("Unlock with Windows Hello");
});
