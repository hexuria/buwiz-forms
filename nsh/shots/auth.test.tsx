// Real-window screenshots of every gate and lock screen: the Buwiz account gate (and a refusal), first launch (the choice,
// the help desk's PIN with an error, the recovery key, "Just me" typing its taxpayer), and sign-in (PIN, an error, the
// authenticator, Forgot PIN? for a taxpayer and for the admin, Open without signing in). Light and dark, wide and narrow.
// `just nsh-shots auth` saves ~/BuwizNsh/evidence/auth/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { open, test } from "nsh/test";
import { clearCells, LOOKS, shoot } from "./shoot";

const ENTRY = "../AuthDemo.tsx";

for (const look of LOOKS) {
  const size = { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme };
  const at = (params: Record<string, unknown>) => open(ENTRY, { ...size, host: { params } });
  const name = (step: string) => `auth-${look.name}-${step}`;

  test(`account gate, ${look.name}`, async () => {
    const app = await at({ gate: true, email: "offline" });
    await app.waitFor("id:gate-card");
    await app.wait(400);
    await shoot(app, name("gate"));
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("text:No connection. Check your internet and try again.");
    await app.wait(300);
    await shoot(app, name("gate-refused"));
  }, 120_000);

  test(`first launch, ${look.name}`, async () => {
    const app = await at({});
    await app.waitFor("id:setup-card");
    await app.wait(400);
    await shoot(app, name("setup-choose"));
    await app.press("id:setup-helpdesk");
    await app.waitFor("id:setup-pin-card");
    await app.type("id:setup-admin-pin", "12");
    await app.press("id:setup-pin-submit");
    await app.waitFor("text:Type all 4 digits of the admin PIN.");
    await app.wait(300);
    await shoot(app, name("setup-pin-error"));
    await app.type("id:setup-admin-pin", "1234");
    await app.press("id:setup-pin-submit");
    await app.waitFor("id:setup-key-card");
    await app.wait(300);
    await shoot(app, name("setup-key"));
  }, 120_000);

  test(`just me, ${look.name}`, async () => {
    const app = await at({});
    await app.waitFor("id:setup-card");
    await app.press("id:setup-solo");
    await app.waitFor("id:new-tp-card");
    await app.wait(400);
    await shoot(app, name("setup-solo-taxpayer"));
  }, 120_000);

  test(`sign-in, ${look.name}`, async () => {
    const app = await at({ demo: true, second: true, mode: "helpdesk", adminPin: "1234", pins: { "000123453": "4321" } });
    await app.waitFor("id:auth-card");
    await app.wait(400);
    await shoot(app, name("signin"));
    await app.type("id:auth-tin", "000123453");
    await app.type("id:auth-tp-pin", "0000");
    await app.waitFor("text:Wrong TIN or PIN. Try again.");
    await app.wait(300);
    await shoot(app, name("signin-error"));
    await app.press("id:auth-method");
    await app.waitFor("id:auth-tp-totp");
    await app.wait(300);
    await shoot(app, name("signin-authenticator"));
    await app.press("id:auth-forgot");
    await app.waitFor("id:auth-admin-pin");
    await app.wait(300);
    await shoot(app, name("forgot-taxpayer"));
    await clearCells(app, "id:auth-tin");
    await app.type("id:auth-tin", "000000000");
    await app.waitFor("id:auth-recovery");
    await app.wait(300);
    await shoot(app, name("forgot-admin"));
  }, 120_000);

  test(`open without signing in, ${look.name}`, async () => {
    const app = await at({ demo: true, mode: "solo", lock: true });
    await app.waitFor("id:auth-open");
    await app.wait(400);
    await shoot(app, name("signin-no-code"));
  }, 120_000);
}
