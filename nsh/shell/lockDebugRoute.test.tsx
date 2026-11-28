// The debug-only lock-page route through the shell's dev start route.
import { expect, open, test } from "nsh/test";

const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

test("desktop: view=lockdebug mounts the Set PIN debug page with its route parameters", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, ...asking({ start: "view=lockdebug;page=set" }) });
  await app.waitFor("id:set-pin-page", { timeout: 10_000 });
  expect(app.texts()).toContain("Set a PIN for this device");
});

test("desktop: view=lockdebug mounts the lock debug page", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, ...asking({ start: "view=lockdebug;biometric=none" }) });
  await app.waitFor("id:owner-lock-page", { timeout: 10_000 });
  expect(app.texts()).toContain("an•••@example.com");
  expect(app.shows("id:lock-biometric")).toBe(false);
});

test("desktop: the debug route mounts even with first-run setup on, as the real app's proof entry", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, ...asking({ start: "view=lockdebug;biometric=none", firstRun: true }) });
  await app.waitFor("id:owner-lock-page", { timeout: 10_000 });
  expect(app.texts()).toContain("an•••@example.com");
});

test("desktop: without the debug probe the route lands on home and never mounts", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, ...asking({ start: "view=lockdebug", debugProbe: false }) });
  await app.waitFor("id:frame", { timeout: 10_000 });
  expect(app.shows("id:owner-lock-page")).toBe(false);
  expect(app.shows("id:debug-lock-pages")).toBe(false);
});

test("desktop: with the test environment false the route lands on home and never mounts", async () => {
  const app = await open("../ShellDemo.tsx", { ...WIDE, ...asking({ start: "view=lockdebug", testEnvironment: false }) });
  await app.waitFor("id:frame", { timeout: 10_000 });
  expect(app.shows("id:owner-lock-page")).toBe(false);
  expect(app.shows("id:debug-lock-pages")).toBe(false);
});
