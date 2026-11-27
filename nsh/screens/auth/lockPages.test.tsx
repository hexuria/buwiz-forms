// The owner lock and Set PIN components against a recording stub of their injected interface.
import { expect, open, test } from "nsh/test";
import { maskAccount } from "./lockInterface";

const WIDE = { width: 1440, height: 900, platform: "macos" as const };

test("phone: empty lock shows the account, PIN, links and no biometric slot", async () => {
  const app = await open("./LockPagesDemo.tsx", { host: { params: { page: "lock", biometric: "none" } } });
  expect(app.texts()).toContain("an•••@example.com");
  expect(app.shows("id:owner-lock-pin")).toBe(true);
  expect(app.shows("id:lock-biometric")).toBe(false);
  expect(app.texts()).toContain("Forgot PIN?");
  expect(app.texts()).toContain("Not you? Sign out");
  expect(app.texts()).toContain("Client sign-in");
});

test("desktop: wrong PIN, wait and lost store states come only from the interface", async () => {
  const app = await open("./LockPagesDemo.tsx", { ...WIDE, host: { params: { page: "lock", wrong: true, biometric: "face_id" } } });
  expect(app.texts()).toContain("Unlock with Face ID");
  await app.type("id:owner-lock-pin", "0000");
  await app.waitFor("text:Wrong PIN.");
  expect(app.value("id:owner-lock-pin")).toBe("");
  await app.press("id:fx-wait");
  await app.waitFor("text:Try again in 1:05");
  await app.press("id:fx-lost");
  await app.waitFor("text:This device's lock can't be read. Sign out and sign in again, or write to support@buwiz.com.");
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
  await app.waitFor("text:calls: set");
  expect(app.value("id:set-pin-first")).toBe("");
  expect(app.value("id:set-pin-again")).toBe("");
});

test("desktop: windows hello label is the interface's label", async () => {
  const app = await open("./LockPagesDemo.tsx", { ...WIDE, host: { params: { page: "lock", biometric: "windows_hello" } } });
  expect(app.texts()).toContain("Unlock with Windows Hello");
});

test("masking never renders a full email or TIN-shaped local part", async () => {
  const app = await open("./LockPagesDemo.tsx", { host: { params: { page: "lock", account: "00012345300000@example.com" } } });
  expect(app.texts()).toContain("00•••@example.com");
  expect(app.texts()).not.toContain("00012345300000@example.com");
});

test("maskAccount always renders two letters, bullets, and the domain", () => {
  expect(maskAccount("juana.santos@example.com")).toBe("ju•••@example.com");
  expect(maskAccount("00012345300000@example.com")).toBe("00•••@example.com");
  expect(maskAccount("an•••@example.com")).toBe("an•••@example.com");
});

test("desktop: the PIN entry reopens when the store's wait reaches zero", async () => {
  const app = await open("./LockPagesDemo.tsx", { ...WIDE, host: { params: { page: "lock", wait: 2 } } });
  await app.waitFor("text:Try again in 0:02");
  await app.press("id:fx-lock");
  await app.waitFor("id:owner-lock-pin");
});
