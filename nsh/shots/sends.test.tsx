// Real-window screenshots of what a return's send says at phone size: the form page on each send stage (FormSendShot.tsx stands the
// host's send in), the unknown outcome's confirm step, and the phone's returns list with the sends a card calls out (FilingsDemo `send`
// and `stage`). `just nsh-shots sends` saves ~/BuwizNsh/evidence/sends/*.png. Headless (plain nsh test) it takes no pictures and passes.
// Demo taxpayer and stood-in sends only: nothing is sent.
import { expect, open, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

const NARROW = LOOKS.filter((l) => l.width < 600);
const STAGES = ["queued", "sending", "checking", "held", "waiting", "needs_attention", "refused", "sent", "test_sent"] as const;
const inAnHour = () => new Date(Date.now() + 60 * 60 * 1000).toISOString();

for (const look of NARROW) {
  const size = { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme };
  test(`sends, the form page on each stage, ${look.name}`, async () => {
    for (const stage of STAGES) {
      const app = await open("../FormSendShot.tsx", { ...size, host: { params: { send: stage, nextTry: inAnHour() } } });
      await app.waitFor("id:fp-header", { timeout: 30000 });
      await app.waitFor(stage === "refused" ? "id:fp-send-notice" : "id:fp-send-line", { timeout: 15000 });
      await app.wait(500);
      await shoot(app, `sends-${look.name}-page-${stage}`);
      if (stage === "needs_attention") {
        await app.press("id:fp-email-came");
        await app.waitFor("id:resolve-continue");
        await app.wait(300);
        await shoot(app, `sends-${look.name}-page-${stage}-confirm`);
      }
      app.dispose();
    }
    expect(true).toBe(true);
  }, 240_000);

  test(`sends, the returns list, ${look.name}`, async () => {
    const app = await open("../FilingsDemo.tsx", { ...size, host: { params: { send: true } } });
    await app.waitFor("id:filings-tabs", { timeout: 30000 });
    await app.press("id:filings-tabs >> label:Queued");
    await app.wait(500);
    await shoot(app, `sends-${look.name}-list-queued`);
    const checking = await open("../FilingsDemo.tsx", { ...size, host: { params: { stage: "checking" } } });
    await checking.waitFor("id:filings-tabs", { timeout: 30000 });
    await checking.press("id:filings-tabs >> label:Queued");
    await checking.wait(500);
    await shoot(checking, `sends-${look.name}-list-checking`);
    expect(app.shows("id:filings")).toBe(true);
  }, 120_000);
}
