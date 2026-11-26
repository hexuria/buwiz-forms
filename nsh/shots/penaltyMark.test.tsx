import { expect, open, test } from "nsh/test";
import { shoot } from "./shoot";

// Real-window proof for the penalty computed mark on 1701: untouched, every penalty
// box carries the lock mark; after the filer types the surcharge (Item 28) alone,
// its mark lifts and the interest (27) and compromise (29) boxes keep theirs.
test("penalty mark typed surcharge only", async () => {
  const app = await open("../screens/form/FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: { params: { formCode: "1701", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1701");
  const box = "id:fp-frm1701:txtPg1I28A";
  for (let i = 0; i < 40 && !app.shows(box); i++) await app.scroll("id:form-scroll", 800);
  expect(app.count(box)).toBe(1);
  await shoot(app, "penalty-untouched");
  await app.press("id:fp-penalty-2:6");
  await app.clear(box);
  await app.type(box, "1000");
  await app.waitFor("text:✓ Saved", { timeout: 15_000 }).catch(() => {});
  for (let i = 0; i < 40 && app.count("id:fp-mark-lock-frm1701:txtPg1I28A") !== 0; i++) await app.wait(250);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I28A")).toBe(0);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I27A")).toBe(1);
  expect(app.count("id:fp-mark-lock-frm1701:txtPg1I29A")).toBe(1);
  await shoot(app, "penalty-surcharge-typed");
});
