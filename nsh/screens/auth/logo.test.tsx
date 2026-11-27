// Every page a person sees before they are inside the app carries the whole Buwiz Forms logo (brand.tsx, one id), once, at its
// top: the Buwiz sign-in's first, email and code pages (gate.test.tsx), and here the PIN sign-in, its authenticator code,
// Forgot PIN (a taxpayer's and the admin's recovery key), each first-run step (the choice, the help desk's PIN, the recovery
// key, "Just me"'s first taxpayer) and the locked page. At phone size and at desktop size.
import { type App, expect, open, test } from "nsh/test";
import { generatedSecret } from "./testTotp";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const SIZES = [
  { name: "phone", size: {} },
  { name: "desktop", size: WIDE },
] as const;

function expectLogo(app: App): void {
  expect(app.shows("id:brand-logo")).toBe(true);
  expect(app.count("id:brand-logo")).toBe(1);
}

for (const { name, size } of SIZES) {
  const at = (params: Record<string, unknown>) => open(ENTRY, { ...size, host: { params } });

  test(`${name}: the PIN sign-in, its authenticator code and Forgot PIN carry the logo; the glyph is gone`, async () => {
    const secret = generatedSecret();
    const app = await at({ demo: true, mode: "helpdesk", adminPin: "1234", pins: { "000123453": "4321" }, totps: { "000123453": secret }, readout: true });
    await app.waitFor("id:auth-card");
    expectLogo(app);
    for (const id of ["auth-tin", "auth-tp-pin", "auth-submit", "auth-method", "auth-forgot"]) expect(app.shows(`id:${id}`)).toBe(true);
    await app.press("id:auth-method");
    await app.waitFor("id:auth-tp-totp");
    expectLogo(app);
    await app.press("id:auth-method");
    await app.press("id:auth-forgot");
    await app.waitFor("text:The admin can open your account with their PIN.");
    expectLogo(app);
    await app.type("id:auth-tin", "000000000");
    await app.waitFor("id:auth-recovery");
    expectLogo(app);
  }, 60_000);

  test(`${name}: each first-run step carries the logo: the choice, the help desk's PIN, the recovery key, Just me's taxpayer`, async () => {
    const app = await at({ readout: true });
    await app.waitFor("id:setup-card");
    expectLogo(app);
    await app.press("id:setup-helpdesk");
    await app.waitFor("id:setup-pin-card");
    expectLogo(app);
    await app.type("id:setup-admin-pin", "1234");
    await app.press("id:setup-pin-submit");
    await app.waitFor("id:setup-key-card");
    expectLogo(app);
    const solo = await at({ readout: true });
    await solo.waitFor("id:setup-card");
    await solo.press("id:setup-solo");
    await solo.waitFor("id:new-tp-card");
    expectLogo(solo);
  }, 90_000);

  test(`${name}: the locked page (the sign-in could not be checked) carries the logo`, async () => {
    const app = await at({ demo: true, mode: "solo", adminPin: "1234", authFails: "The access state could not be read." });
    await app.waitFor("id:access-error-card");
    expectLogo(app);
  }, 60_000);

  test(`${name}: inside the app there is no logo header`, async () => {
    const app = await at({ demo: true, mode: "solo", firstRun: false });
    await app.waitFor("id:frame", { timeout: 15000 });
    expect(app.shows("id:brand-logo")).toBe(false);
  }, 60_000);
}
