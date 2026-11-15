// The F3 fixes of the wave 2 review that show on the page (FormPageAny.tsx, the demo taxpayer, a scratch database).
import { expect, open, test } from "nsh/test";

const ENTRY = "./FormPageAny.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };

test("F3-4 desktop: an autosave drops the frame's cached reads (the saved returns re-read)", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 1 } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.waitFor("text:saved returns 0");
  await app.press("id:fp-parts-toggle");
  await app.type("id:fp-frm2551Qv2018:txt15", "100");
  await app.waitFor("text:✓ Saved", { timeout: 5000 });
  await app.waitFor("text:saved returns 1", { timeout: 3000 });
});

test("F3-11 desktop: Cmd+Right inside a text box stays in the box; outside one it steps", async () => {
  const app = await open(ENTRY, { ...DESKTOP, host: { params: { formCode: "2551Q", year: 2025, period: 2 } } });
  await app.waitFor("text:BIR Form 2551Q");
  await app.press("id:fp-check");
  await app.waitFor("id:fp-pill-count");
  await app.press("id:fp-parts-toggle");
  await app.type("id:fp-frm2551Qv2018:txt15", "5");
  await app.keyboard.press("mod+right");
  await app.wait(300);
  expect(app.texts().some((t) => /^Item 13 · /.test(t))).toBe(false);
  // The return key leaves the box: now the shortcut steps.
  await app.submit("id:fp-frm2551Qv2018:txt15");
  await app.keyboard.press("mod+right");
  await app.waitFor("text:/^Item 13 · /");
});
