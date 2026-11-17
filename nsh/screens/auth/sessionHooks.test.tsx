// app.onSessionChange (shell/app.ts): a screen's reset runs when the app locks, signs out and signs in; a hook that throws does
// not stop the others; the function it returns unregisters it. Over SessionDemo.tsx (two taxpayers, help-desk mode).
import { expect, open, test } from "nsh/test";

const ENTRY = "./SessionDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const ONE = "first,fails,last";

async function signIn(app: Awaited<ReturnType<typeof open>>, tin: string, pin: string) {
  await app.waitFor("id:auth-card");
  await app.type("id:auth-tin", tin);
  await app.type("id:auth-tp-pin", pin);
  await app.waitFor("id:frame");
}

test("desktop: every sign-in, lock and log out runs the hooks once, in order; a failing one does not stop the rest; an unregistered one never runs", async () => {
  const app = await open(ENTRY, { ...WIDE });
  await app.waitFor("text:hooks: none");
  // Sign in as A.
  await signIn(app, "000123453", "4321");
  await app.waitFor("text:hooks: " + ONE);
  // Lock (Cmd+L), then B signs in: the lock and the sign-in each run them.
  await app.keyboard.press("mod+l");
  await app.waitFor("id:auth-card");
  await app.waitFor("text:hooks: " + [ONE, ONE].join(","));
  await signIn(app, "000111229", "5555");
  await app.waitFor("text:hooks: " + [ONE, ONE, ONE].join(","));
  // Log out from the account menu.
  await app.press("id:account-row");
  await app.press("id:menu-logout");
  await app.waitFor("id:auth-card");
  await app.waitFor("text:hooks: " + [ONE, ONE, ONE, ONE].join(","));
  expect(app.texts().some((t) => /never/.test(t))).toBe(false);
});

test("phone: the same on the sheet's Log out", async () => {
  const app = await open(ENTRY);
  await signIn(app, "000123453", "4321");
  await app.waitFor("text:hooks: " + ONE);
  await app.press("id:nb-avatar");
  await app.press("id:nb-menu-profile");
  await app.waitFor("id:sheet-logout");
  await app.press("id:sheet-logout");
  await app.waitFor("id:auth-card");
  await app.waitFor("text:hooks: " + [ONE, ONE].join(","));
});
