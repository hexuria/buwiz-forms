// The heaviest form (1702RT) on the page: opens with the first part, Expand all mounts every part, typing re-renders
// its own box. Run with --trace to see the mount batches per step (numbers in the B1 report).
import { expect, open, test } from "nsh/test";

test("desktop: 1702RT opens, Expand all mounts every part, typing an amount computes", async () => {
  const app = await open("./FormPageHeavy.tsx", { width: 1440, height: 900, platform: "macos" });
  await app.waitFor("text:BIR Form 1702RT");
  await app.waitFor("id:fp-parts-toggle");
  await app.press("id:fp-parts-toggle");
  await app.waitFor("id:fp-part-11");
  await app.waitFor("text:12 parts");
  expect(app.count("id:fp-frm1702RT:txtPg2Pt4I27Sales")).toBe(1);
  await app.type("id:fp-frm1702RT:txtPg2Pt4I27Sales", "1000");
  await app.wait(400);
  await app.waitFor("text:✓ Saved", { timeout: 5000 });
});
