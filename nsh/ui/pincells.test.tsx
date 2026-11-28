// PinCells (the sign-in code boxes and the PINs): the testID's node is the text input itself (the whole point of #608 —
// gpui-agent's `type` needs an input AT the id, hexuria/gpui-agent#135); the kit's edit rule (a whole code at once replaces,
// fewer digits append); the caret (steady with reduced motion, solid for 300 ms after an edit); the masked mode; disabled.
import { expect, open, test } from "nsh/test";

const ENTRY = "./pincells.fixture.tsx";

for (const size of [
  { name: "phone", options: {} },
  { name: "desktop", options: { width: 1440, height: 900, platform: "macos" as const } },
] as const) {
  test(`${size.name}: the testID's node is the text input itself`, async () => {
    const app = await open(ENTRY, size.options);
    await app.waitFor("id:p");
    // The input carries the accessibility label. It is the node the testID names, not one inside it: nothing labelled
    // sits UNDER id:p (an input has no views in it). With OtpInput's composition the labelled input was a descendant
    // of the testID's root and this count was 1.
    expect(app.count("label:Sign-in code, 6 digits")).toBe(1);
    expect(app.count("id:p >> label:Sign-in code, 6 digits")).toBe(0);
    // Positive control: the driver reaches the input by the id.
    await app.type("id:p", "12");
    expect(app.value("id:p")).toBe("12");
  });

  test(`${size.name}: a whole code at once REPLACES the code (paste and autofill); fewer digits append`, async () => {
    const app = await open(ENTRY, size.options);
    await app.waitFor("id:p");
    // Paste over empty boxes.
    await app.paste("id:p", { text: "987654" });
    expect(app.value("id:p")).toBe("987654");
    expect(app.texts()).toContain("Done: 987654");
    // A paste of a whole code over a partial one replaces, never joins: "12" + paste "987654" is "987654",
    // sent once — the kit's edit() (otp_input.rs), or a wrong "129876" goes out as one of five tries.
    await app.clear("id:p");
    await app.type("id:p", "12");
    await app.paste("id:p", { text: "987654" });
    expect(app.value("id:p")).toBe("987654");
    // Over a full box kept after a "No connection" error, a paste replaces too (before the fix it did NOTHING).
    await app.paste("id:p", { text: "135790" });
    expect(app.value("id:p")).toBe("135790");
    // The autofill's shape (one-time-code, separators): six digits at once replace as well.
    await app.clear("id:p");
    await app.type("id:p", "12");
    await app.paste("id:p", { text: "987-654" });
    expect(app.value("id:p")).toBe("987654");
    // Fewer digits than the length are appended to what the boxes hold.
    await app.clear("id:p");
    await app.type("id:p", "12");
    await app.paste("id:p", { text: "34" });
    expect(app.value("id:p")).toBe("1234");
  });

  test(`${size.name}: keys append digits up to the length and Backspace removes the last`, async () => {
    const app = await open(ENTRY, size.options);
    await app.waitFor("id:p");
    await app.type("id:p", "123456");
    expect(app.value("id:p")).toBe("123456");
    expect(app.texts()).toContain("Done: 123456");
    await app.type("id:p", "7");
    expect(app.value("id:p")).toBe("123456");
    await app.keyboard.press("Backspace");
    expect(app.value("id:p")).toBe("12345");
  });

  test(`${size.name}: the digits are 14 pt semibold tabular (the kit cells' look before #608)`, async () => {
    const app = await open(ENTRY, size.options);
    await app.waitFor("id:p");
    await app.type("id:p", "12");
    const style = app.spanStyle("id:p-cells >> text:1");
    expect(style.fontSize).toBe(14);
    expect(style.fontWeight).toBe(600);
  });

  test(`${size.name}: masked cells show bullets, not digits`, async () => {
    const app = await open(ENTRY, size.options);
    await app.waitFor("id:p-mask");
    await app.type("id:p-mask", "12");
    expect(app.texts()).not.toContain("1");
    expect(app.texts()).not.toContain("2");
    expect(app.texts().filter((t) => t === "\u{2022}").length).toBe(2);
    expect(app.texts()).toContain("Pin: 12");
  });

  test(`${size.name}: disabled takes no keys and no focus`, async () => {
    const app = await open(ENTRY, size.options);
    await app.waitFor("id:p-off");
    expect(app.enabled("id:p-off")).toBe(false);
    await app.press("id:p-off").catch(() => undefined);
    await app.keyboard.press("1");
    expect(app.texts()).toContain("Off: empty");
    expect(app.texts()).not.toContain("Focus: off");
  });
}

test("the caret is steady with reduced motion", async () => {
  const app = await open(ENTRY, { reducedMotion: true, clock: {} });
  await app.waitFor("id:p");
  await app.type("id:p", "1");
  // Never blinked: solid at every sample, well past a blink interval.
  for (let i = 0; i < 6; i++) {
    expect(app.viewStyle("id:p-caret")?.opacity ?? 1).toBe(1);
    await app.wait(300);
  }
});

test("the caret is solid for 300 ms after an edit, then blinks", async () => {
  const app = await open(ENTRY, { clock: {} });
  await app.waitFor("id:p");
  await app.type("id:p", "1");
  expect(app.viewStyle("id:p-caret")?.opacity ?? 1).toBe(1);
  await app.wait(200);
  expect(app.viewStyle("id:p-caret")?.opacity ?? 1).toBe(1);
  // Past the hold and deep into the first hidden half (500 ms), the caret is off at some point.
  let sawOff = false;
  for (let i = 0; i < 12; i++) {
    await app.wait(150);
    if ((app.viewStyle("id:p-caret")?.opacity ?? 1) === 0) sawOff = true;
  }
  expect(sawOff).toBe(true);
});
