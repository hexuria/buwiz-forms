// Real-window screenshots of every card of the Buwiz sign-in flow (screens/auth/signInFlow.ts): on the account gate the start card,
// the pressed Sign in, the browser's waiting card, the no-browser card (address and code), the expired, cancelled and failed start
// cards; in Settings > Account the waiting and no-browser cards. Light and dark, wide and narrow.
// `just nsh-shots signin` saves ~/BuwizNsh/evidence/signin/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const GATE = "../AuthDemo.tsx";
const SETTINGS = "../SettingsDemo.tsx";

for (const look of LOOKS) {
  const size = { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme };
  const gate = (params: Record<string, unknown>) => open(GATE, { ...size, host: { params: { gate: true, begin: "code", finish: "hold", ...params } } });
  const name = (step: string) => `signin-${look.name}-${step}`;

  test(`gate: start, waiting, cancelled, expired, ${look.name}`, async () => {
    const app = await gate({ opened: true });
    await app.waitFor("id:gate-card");
    await app.wait(400);
    await shoot(app, name("gate-start"));
    await app.press("id:gate-sign-in");
    await app.waitFor("id:gate-wait-card");
    await app.wait(400);
    await shoot(app, name("gate-waiting"));
    await app.press("id:gate-cancel");
    await app.waitFor("id:gate-card");
    await app.wait(300);
    await shoot(app, name("gate-cancelled"));
    await app.press("id:gate-sign-in");
    await app.waitFor("id:gate-wait-card");
    await app.press("id:demo-expire");
    await app.waitFor("text:/^The sign-in code expired/");
    await app.wait(300);
    await shoot(app, name("gate-expired"));
  }, 120_000);

  test(`gate: no browser, failed, ${look.name}`, async () => {
    const app = await gate({ reopen: false });
    await app.waitFor("id:gate-card");
    await app.press("id:gate-sign-in");
    await app.waitFor("id:gate-wait-card");
    await app.wait(400);
    await shoot(app, name("gate-no-browser"));
    const refused = await gate({ begin: "refuse" });
    await refused.waitFor("id:gate-card");
    await refused.press("id:gate-sign-in");
    await refused.waitFor("text:/^Could not reach the Buwiz server/");
    await refused.wait(300);
    await shoot(refused, name("gate-failed"));
  }, 120_000);

  test(`settings > account: waiting and no browser, ${look.name}`, async () => {
    for (const [step, opened] of [["settings-waiting", true], ["settings-no-browser", false]] as const) {
      const app = await open(SETTINGS, { ...size, host: { params: { cloud: "out", solo: true, hold: true, opened, start: "section=account" } } });
      await app.waitFor("id:settings-security", { timeout: 30000 });
      await app.press("id:settings-account-sign-in");
      await app.waitFor("id:settings-account-wait");
      await app.wait(400);
      await shoot(app, name(step));
    }
  }, 120_000);
}
