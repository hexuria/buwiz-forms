// Layers at phone and desktop size: dialog, sheet, popover, menu, toasts with undo, disclosures.
import { expect, test } from "nsh/test";
import { openKit, readout, SIZES } from "./harness";

for (const size of SIZES) {
  test(`${size.name}: dialog opens and closes by its button and by back`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(app.texts()).not.toContain("Delete this return?");
    await app.press("id:kit-open-dialog");
    await app.waitFor("text:Delete this return?");
    await app.press("id:kit-dialog-cancel");
    await app.waitFor("text:Delete this return?", { state: "gone" });
    await app.press("id:kit-open-dialog");
    await app.waitFor("text:Delete this return?");
    await app.back();
    await app.waitFor("text:Delete this return?", { state: "gone" });
  });

  test(`${size.name}: sheet opens with its title and closes`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.press("id:kit-open-sheet");
    await app.waitFor("text:Schedule 1, row 3");
    expect(app.texts()).toContain("Save row");
    await app.press("label:Close");
    await app.waitFor("text:Schedule 1, row 3", { state: "gone" });
  });

  test(`${size.name}: popover and menu`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.press("text:Popover");
    await app.waitFor("text:Where this number comes from");
    await app.back();
    await app.waitFor("text:Where this number comes from", { state: "gone" });
    await app.press("text:More");
    await app.waitFor("text:Duplicate");
    await expect(app.press("text:Send to BIR")).rejects.toMatchObject({ code: "ERR_NSH_TEST_DISABLED" });
    await app.press("text:Duplicate");
    await app.waitFor("text:Duplicated");
    expect(app.texts()).not.toContain("Delete return");
  });

  test(`${size.name}: undo toast undoes, plain toast times out`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.press("id:kit-undo");
    await app.waitFor("text:Return deleted");
    await app.press("id:toast-action");
    expect(readout(app, "Undone:")).toBe("Undone: 1");
    await app.waitFor("text:Return deleted", { state: "gone" });
    await app.press("id:kit-toast");
    await app.waitFor("text:Draft saved");
    await app.waitFor("text:Draft saved", { state: "gone", timeout: 8000 });
  });

  test(`${size.name}: disclosures open and close`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(app.texts()).toContain("Business income, 1,250,000.00");
    expect(app.texts()).not.toContain("40% of gross sales");
    await app.press("id:kit-disc-ded-toggle");
    expect(app.texts()).toContain("40% of gross sales");
    await app.press("id:kit-disc-income-toggle");
    expect(app.texts()).not.toContain("Business income, 1,250,000.00");
  });
}
