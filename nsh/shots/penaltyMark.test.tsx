import { expect, open, test } from "nsh/test";
import { shoot } from "./shoot";

// Real-window proof for the penalty computed mark on 1701: untouched, Items 27 to 29
// are all visible and no box carries a mark (the boxes are shut — their own locks and
// reasons say the amounts are computed); after Edit, the filer types the surcharge
// (Item 28) alone, its info mark lifts and the interest (27) and compromise (29)
// boxes keep theirs.
test("penalty mark typed surcharge only", async () => {
  const app = await open("../screens/form/FormPageAny.tsx", {
    width: 1440, height: 900, platform: "macos", clock: {},
    host: { params: { formCode: "1701", year: 2025, period: 0, openParts: "all" } },
  });
  await app.waitFor("text:BIR Form 1701");
  const box = "id:fp-frm1701:txtPg1I28A";
  for (let i = 0; i < 40 && !app.shows(box); i++) await app.scroll("id:form-scroll", 800);
  expect(app.count(box)).toBe(1);
  // The proof shows all three penalty rows: rows mount ahead of the viewport, so
  // scroll past their mounting until Item 29's row is well inside the window.
  for (let i = 0; i < 20 && !(app.shows("id:fp-frm1701:txtPg1I27A") && app.shows("id:fp-frm1701:txtPg1I29A")); i++) await app.scroll("id:form-scroll", 200);
  expect(app.shows("id:fp-frm1701:txtPg1I27A")).toBe(true);
  expect(app.shows("id:fp-frm1701:txtPg1I29A")).toBe(true);
  await app.scroll("id:form-scroll", 420);
  await app.wait(300);
  await shoot(app, "penalty-untouched");
  await app.press("id:fp-penalty-2:6");
  await app.clear(box);
  await app.type(box, "1000");
  await app.waitFor("text:✓ Saved", { timeout: 15_000 }).catch(() => {});
  for (let i = 0; i < 40 && app.count("id:fp-mark-info-frm1701:txtPg1I28A") !== 0; i++) await app.wait(250);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I28A")).toBe(0);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I27A")).toBe(1);
  expect(app.count("id:fp-mark-info-frm1701:txtPg1I29A")).toBe(1);
  await shoot(app, "penalty-surcharge-typed");
});
