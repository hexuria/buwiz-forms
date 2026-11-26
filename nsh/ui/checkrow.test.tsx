// The selectable list row: the whole row is the target, one tap toggles once, the keyboard path works,
// a disabled row does nothing. Phone and desktop sizes.
import { expect, open, test } from "nsh/test";
import { SIZES } from "./harness";

for (const size of SIZES) {
  test(`${size.name}: a tap on the row's text toggles it, a second tap back, one tap exactly once`, async () => {
    const app = await open("./checkrow.fixture.tsx", size.options);
    await app.waitFor("id:cr-a");
    expect(app.texts()).toContain("a:off/0 b:on/0");
    // A tap on the row's words selects it — the press lands on the one control the row is.
    await app.press("text:Alpha");
    expect(app.texts()).toContain("a:on/1 b:on/0");
    // A second tap deselects: the count moved once per tap, never twice.
    await app.press("text:Alpha");
    expect(app.texts()).toContain("a:off/2 b:on/0");
    // A tap on the control itself (the box end) toggles exactly once.
    await app.press("id:cr-b");
    expect(app.texts()).toContain("a:off/2 b:off/1");
  });

  test(`${size.name}: a row that is not selectable does nothing`, async () => {
    const app = await open("./checkrow.fixture.tsx", size.options);
    await app.waitFor("id:cr-off");
    await expect(app.press("id:cr-off")).rejects.toMatchObject({ code: "ERR_NSH_TEST_DISABLED" });
    expect(app.texts()).toContain("a:off/0 b:on/0");
  });
}

test("tablet: a tap on the row's text toggles it at 834 wide", async () => {
  const app = await open("./checkrow.fixture.tsx", { width: 834, height: 1112, platform: "ios" });
  await app.waitFor("id:cr-a");
  await app.press("text:Alpha");
  expect(app.texts()).toContain("a:on/1 b:on/0");
});

test("desktop: Space on a focused row toggles it", async () => {
  const app = await open("./checkrow.fixture.tsx", { width: 1440, height: 900, platform: "macos" });
  await app.waitFor("id:cr-a");
  // The row takes the keyboard (its focus ring) and Space toggles it, no tap involved.
  await app.press("id:cr-focus-a");
  await app.keyboard.press("space");
  expect(app.texts()).toContain("a:on/1 b:on/0");
  await app.keyboard.press("space");
  expect(app.texts()).toContain("a:off/2 b:on/0");
});
