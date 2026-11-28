// Real-window pictures of the debug-mounted lock components: phone, tablet and desktop, light and dark, every state.
import { expect, open, test } from "nsh/test";
import { shoot } from "./shoot";

const LOOKS = [
  { name: "phone-light", width: 390, height: 844, platform: "ios", colorScheme: "light" },
  { name: "phone-dark", width: 390, height: 844, platform: "ios", colorScheme: "dark" },
  { name: "tablet-light", width: 834, height: 1194, platform: "ios", colorScheme: "light" },
  { name: "tablet-dark", width: 834, height: 1194, platform: "ios", colorScheme: "dark" },
  { name: "desktop-light", width: 1440, height: 900, platform: "macos", colorScheme: "light" },
  { name: "desktop-dark", width: 1440, height: 900, platform: "macos", colorScheme: "dark" },
] as const;

for (const look of LOOKS) {
  const opts = { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme } as const;

  test(`lock empty, ${look.name}`, async () => {
    const app = await open("../screens/auth/LockPagesDemo.tsx", { ...opts, host: { params: { page: "lock", biometric: "face_id" } } });
    expect(app.texts()).toContain("an•••@example.com");
    await shoot(app, `lockpages-${look.name}-empty`);
  });

  test(`lock wrong, ${look.name}`, async () => {
    const app = await open("../screens/auth/LockPagesDemo.tsx", { ...opts, host: { params: { page: "lock", wrong: true, biometric: "touch_id" } } });
    await app.type("id:owner-lock-pin", "0000");
    await app.waitFor("text:Wrong PIN.");
    await shoot(app, `lockpages-${look.name}-wrong`);
  });

  test(`lock wait, ${look.name}`, async () => {
    const app = await open("../screens/auth/LockPagesDemo.tsx", { ...opts, host: { params: { page: "lock", wait: 65, biometric: "none" } } });
    await app.waitFor("text:Try again in 1:05");
    await shoot(app, `lockpages-${look.name}-wait`);
  });

  test(`lock lost, ${look.name}`, async () => {
    const app = await open("../screens/auth/LockPagesDemo.tsx", { ...opts, host: { params: { page: "lock", lost: true, biometric: "windows_hello" } } });
    await app.waitFor("text:This device's lock can't be read. Sign out and sign in again, or write to support@buwiz.com.");
    await shoot(app, `lockpages-${look.name}-lost`);
  });

  test(`set PIN, ${look.name}`, async () => {
    const app = await open("../screens/auth/LockPagesDemo.tsx", { ...opts, host: { params: { page: "set" } } });
    expect(app.texts()).toContain("Set a PIN for this device");
    await shoot(app, `lockpages-${look.name}-set-empty`);
    await app.type("id:set-pin-first", "1234");
    await app.type("id:set-pin-again", "1235");
    await app.waitFor("text:The PINs don't match. Try again.");
    await shoot(app, `lockpages-${look.name}-set-mismatch`);
  });
}
