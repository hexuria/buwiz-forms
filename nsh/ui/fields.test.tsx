// Text entry at phone and desktop size: plain text, the TIN, money, percent and integer masks, the date, PIN cells.
import { expect, open, test } from "nsh/test";
import { openKit, readout, SIZES } from "./harness";

const commaHint = " Use a point for centavos, and commas only between thousands.";
const leftSentence = (typed: string, kept: string) =>
  `${typed} is not an amount. ${kept === "" ? "The box was left blank." : `The box kept ${kept}.`}${typed.includes(",") ? commaHint : ""}`;

test("android: going to the background settles a focused amount before input restarts", async () => {
  const app = await open("./masked.fixture.tsx", { platform: "android" });
  await app.type("id:m-cb", "");
  expect(app.texts()).toContain("Cb blurs: 0");
  await app.setLifecycle("background");
  expect(app.texts()).toContain("Cb blurs: 1");
  expect(app.value("id:m-cb")).toBe("2,000.00");
  await app.setLifecycle("active");
  expect(app.value("id:m-cb")).toBe("2,000.00");
  expect(app.texts()).toContain("Cb blurs: 1");
});

for (const size of SIZES) {
  test(`${size.name}: touch date sheet validates Done, discards dismissal and clears`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(app.shows("id:kit-one-date")).toBe(true);
    await app.press("id:kit-one-date");
    await app.waitFor("id:kit-one-date-sheet-input");
    await app.type("id:kit-one-date-sheet-input", "08/17/88");
    await app.press("id:kit-one-date-sheet-done");
    expect(app.shows("id:kit-one-date-sheet-input")).toBe(true);
    expect(app.texts()).toContain("Type the four-digit year, for example 08/17/1988.");
    await app.clear("id:kit-one-date-sheet-input");
    await app.type("id:kit-one-date-sheet-input", "08171988");
    await app.press("id:kit-one-date-sheet-done");
    await app.waitFor("id:kit-one-date-sheet-input", { state: "gone" });
    expect(app.texts()).toContain("August 17, 1988");
    await app.press("id:kit-one-date");
    expect(app.value("id:kit-one-date-sheet-input")).toBe("08/17/1988");
    await app.clear("id:kit-one-date-sheet-input");
    await app.type("id:kit-one-date-sheet-input", "01/01/2000");
    await app.press("role:button[Close]");
    await app.waitFor("id:kit-one-date-sheet-input", { state: "gone" });
    expect(app.texts()).toContain("August 17, 1988");
    await app.press("id:kit-one-date");
    await app.press("id:kit-one-date-sheet-clear");
    await app.waitFor("id:kit-one-date-sheet-input", { state: "gone" });
    expect(app.texts()).not.toContain("August 17, 1988");
  });
}

for (const size of SIZES) {
  test(`${size.name}: text field types, TIN groups while typing`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.clear("id:kit-name");
    await app.type("id:kit-name", "Juana Santos");
    expect(app.value("id:kit-name")).toBe("Juana Santos");
    await app.clear("id:kit-tin");
    expect(readout(app, "Digits:")).toBe("Digits: ");
    await app.type("id:kit-tin", "0001");
    expect(app.value("id:kit-tin")).toBe("000-1");
    await app.type("id:kit-tin", "23453");
    expect(app.value("id:kit-tin")).toBe("000-123-453");
    await app.type("id:kit-tin", "00000999");
    expect(app.value("id:kit-tin")).toBe("000-123-453-00000");
    expect(readout(app, "Digits:")).toBe("Digits: 00012345300000");
    await app.keyboard.press("backspace");
    expect(app.value("id:kit-tin")).toBe("000-123-453-0000");
  });

  test(`${size.name}: TIN paste with dashes keeps every digit`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.clear("id:kit-tin");
    await app.paste("id:kit-tin", { text: "000-123-453-00000" });
    expect(app.value("id:kit-tin")).toBe("000-123-453-00000");
    expect(readout(app, "Digits:")).toBe("Digits: 00012345300000");
  });

  test(`${size.name}: money stays plain while typing, plain value out, two decimals on leaving`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    expect(app.value("id:kit-money")).toBe("1,250,000.00");
    await app.clear("id:kit-money");
    expect(readout(app, "Value:")).toBe("Value: empty");
    await app.type("id:kit-money", "1250000");
    expect(app.value("id:kit-money")).toBe("1250000");
    expect(readout(app, "Value:")).toBe("Value: 1250000");
    // Real key events are separate turns; type the refused letter and the keys around it one at a time.
    await app.type("id:kit-money", ".5");
    await app.type("id:kit-money", "x");
    await app.type("id:kit-money", ".");
    await app.type("id:kit-money", "7");
    expect(app.value("id:kit-money")).toBe("1250000.5x.7");
    expect(readout(app, "Value:")).toBe("Value: 1250000");
    await app.keyboard.press("backspace");
    await app.keyboard.press("backspace");
    await app.keyboard.press("backspace");
    expect(app.value("id:kit-money")).toBe("1250000.5");
    expect(readout(app, "Value:")).toBe("Value: 1250000.5");
    // Leaving the box: another field takes the keys.
    await app.type("id:kit-name", " ");
    expect(app.value("id:kit-money")).toBe("1,250,000.50");
    expect(readout(app, "Value:")).toBe("Value: 1250000.5");
    await app.clear("id:kit-money");
    await app.paste("id:kit-money", { text: "₱ 2,500.75" });
    expect(readout(app, "Value:")).toBe("Value: 2500.75");
  });

  test(`${size.name}: percent, integer, date, PIN`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    await app.clear("id:kit-pct");
    await app.type("id:kit-pct", "12.5.0a");
    expect(app.value("id:kit-pct")).toBe("12.50");
    await app.clear("id:kit-int");
    await app.type("id:kit-int", "0123456");
    expect(app.value("id:kit-int")).toBe("0123");
    await app.clear("id:kit-date");
    await app.type("id:kit-date", "12102020");
    expect(readout(app, "Stored:")).toBe("Stored: 12/10/2020");
    await app.type("id:kit-name", " ");
    expect(app.value("id:kit-date")).toBe("December 10, 2020");
    await app.clear("id:kit-date");
    await app.type("id:kit-date", "13/45");
    expect(app.texts().some((t) => t.startsWith("Type the date as MM/DD/YYYY"))).toBe(false);
    await app.type("id:kit-name", " ");
    expect(app.texts().some((t) => t.startsWith("Type the date as MM/DD/YYYY"))).toBe(true);
    await app.type("id:kit-code", "123456");
    expect(readout(app, "PIN digits:")).toBe("PIN digits: 0 | Code complete: 123456");
    await app.type("id:kit-pin", "4321");
    expect(readout(app, "PIN digits:")).toBe("PIN digits: 4 | Code complete: 123456");
  });

  test(`${size.name}: a date box is MM/DD/YYYY while typed, the full month when left, and its calendar is named for it`, async () => {
    const app = await openKit("../BasePreview.tsx", size.options);
    // The calendar trigger is named for the box, not a fixed "Open the calendar"
    // (the picker's trigger is a combobox to assistive tech).
    expect(app.shows("role:combobox[Pick Date of birth]")).toBe(true);
    await app.clear("id:kit-date");
    // Eight digits rewrite to MM/DD/YYYY while the box has the keys.
    await app.type("id:kit-date", "12102020");
    expect(app.value("id:kit-date")).toBe("12/10/2020");
    expect(readout(app, "Stored:")).toBe("Stored: 12/10/2020");
    // Other shapes stay as typed while typing and take the long form on blur.
    await app.clear("id:kit-date");
    await app.type("id:kit-date", "12-10-2020");
    expect(app.value("id:kit-date")).toBe("12-10-2020");
    await app.type("id:kit-name", " ");
    expect(app.value("id:kit-date")).toBe("December 10, 2020");
    await app.clear("id:kit-date");
    await app.type("id:kit-date", "Dec 10 2020");
    await app.type("id:kit-name", " ");
    expect(app.value("id:kit-date")).toBe("December 10, 2020");
    // Focusing the long text shows the numeric date (an empty type takes the keys).
    await app.type("id:kit-date", "");
    await app.wait(200);
    expect(app.value("id:kit-date")).toBe("12/10/2020");
  });
}

// The masked boxes against a fixture of their own (masked.fixture.tsx): what a box shows is what it reports.
for (const size of SIZES) {
  test(`${size.name}: money rounds on the digits when the box is left, and the draft takes the rounded value`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-amount", "1.255");
    expect(app.value("id:m-amount")).toBe("1.255");
    expect(readout(app, "Amount:")).toBe("Amount: 1.255");
    await app.type("id:m-other", " "); // leaves the amount
    expect(app.value("id:m-amount")).toBe("1.26"); // toFixed showed 1.25 while the draft kept 1.255
    expect(readout(app, "Amount:")).toBe("Amount: 1.26");
    await app.clear("id:m-amount");
    await app.type("id:m-amount", "2.675");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-amount")).toBe("2.68");
    expect(readout(app, "Amount:")).toBe("Amount: 2.68");
    await app.clear("id:m-amount");
    await app.type("id:m-amount", "1.005");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-amount")).toBe("1.01");
    expect(readout(app, "Amount:")).toBe("Amount: 1.01");
  });

  test(`${size.name}: an invalid key stays visible and the report rolls back to the value at focus`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-amount", "1250000.5");
    expect(readout(app, "Amount:")).toBe("Amount: 1250000.5");
    await app.type("id:m-amount", "x");
    expect(app.value("id:m-amount")).toBe("1250000.5x"); // invalid text stays exactly as typed
    expect(readout(app, "Amount:")).toBe("Amount: empty"); // the valid prefix is rolled back, not saved
    expect(app.texts().some((t) => t.includes(" is not an amount. The box kept "))).toBe(false);
    await app.type("id:m-other", " ");
    expect(app.value("id:m-amount")).toBe("");
    expect(readout(app, "Amount:")).toBe("Amount: empty");
    expect(app.texts()).toContain(leftSentence("1250000.5x", ""));
  });

  test(`${size.name}: negatives read by sign or accounting parentheses only where allowed`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    expect(app.value("id:m-stored")).toBe("-250.00"); // a stored negative is shown negative even where no minus may be typed
    // Typed minus where it is not allowed stays visible; the whole text is invalid and falls back to empty.
    await app.type("id:m-amount", "-");
    await app.type("id:m-amount", "5");
    expect(app.value("id:m-amount")).toBe("-5");
    expect(readout(app, "Amount:")).toBe("Amount: empty");
    // Pasted accounting parentheses are not an amount where negatives are not allowed.
    await app.clear("id:m-amount");
    await app.paste("id:m-amount", { text: "(1,234.00)" });
    expect(app.value("id:m-amount")).toBe("(1,234.00)");
    expect(readout(app, "Amount:")).toBe("Amount: empty");
    // Where negatives are allowed, both a typed minus and pasted parentheses read as negatives.
    await app.type("id:m-free", "-");
    await app.type("id:m-free", "5");
    expect(app.value("id:m-free")).toBe("-5");
    expect(readout(app, "Free:")).toBe("Free: -5");
    await app.clear("id:m-free");
    await app.paste("id:m-free", { text: "(1,234.00)" });
    expect(app.value("id:m-free")).toBe("(1,234.00)");
    expect(readout(app, "Free:")).toBe("Free: -1234.00");
  });

  test(`${size.name}: pasted text is strict: valid grouped amounts report, ambiguous text stays visible invalid and unreported`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.paste("id:m-amount", { text: "1,234,567" });
    expect(app.value("id:m-amount")).toBe("1,234,567");
    expect(readout(app, "Amount:")).toBe("Amount: 1234567");
    await app.clear("id:m-amount");
    await app.paste("id:m-amount", { text: "1,234.56" });
    expect(app.value("id:m-amount")).toBe("1,234.56");
    expect(readout(app, "Amount:")).toBe("Amount: 1234.56");
    for (const bad of ["1.234.567", "1.234,56", "1.2.3"]) {
      await app.clear("id:m-amount");
      await app.paste("id:m-amount", { text: bad });
      expect(app.value("id:m-amount")).toBe(bad);
      expect(readout(app, "Amount:")).toBe("Amount: empty");
      expect(app.texts().some((t) => t.includes(" is not an amount. The box kept "))).toBe(false);
    }
  });

  test(`${size.name}: a stored value is shown as it is and leaving its box unedited changes nothing; an edit is rounded on leaving`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    expect(app.value("id:m-long")).toBe("1.255");
    await app.type("id:m-long", ""); // takes the keys
    await app.type("id:m-other", " "); // leaves
    expect(app.value("id:m-long")).toBe("1.255");
    expect(readout(app, "Long:")).toBe("Long: 1.255");
    await app.type("id:m-long", "9");
    expect(app.value("id:m-long")).toBe("1.2559");
    await app.type("id:m-other", "x");
    expect(app.value("id:m-long")).toBe("1.26");
    expect(readout(app, "Long:")).toBe("Long: 1.26");
  });

  test(`${size.name}: long amounts keep every digit, odd stored values show as they are, partial TINs keep every digit`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-amount", "123456789012345678");
    expect(app.value("id:m-amount")).toBe("123456789012345678");
    expect(readout(app, "Amount:")).toBe("Amount: 123456789012345678");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-amount")).toBe("123,456,789,012,345,678.00");
    expect(app.value("id:m-odd")).toBe("1e21"); // was "121"
    expect(readout(app, "Odd:")).toBe("Odd: 1e21");
    expect(app.value("id:m-whole")).toBe("1234.5"); // a typed fraction in a whole-peso box stays as it is
    expect(app.value("id:m-tin")).toBe("000-123-453-0");
    expect(app.value("id:m-rate")).toBe("12.5");
    expect(app.value("id:m-code")).toBe("0123");
  });

  test(`${size.name}: a letter, a second point, a digit past the limit and a pasted minus leave percent and integer boxes as they were`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-rate", "x");
    expect(app.value("id:m-rate")).toBe("12.5");
    await app.type("id:m-rate", ".");
    expect(app.value("id:m-rate")).toBe("12.5");
    await app.type("id:m-code", "9");
    expect(app.value("id:m-code")).toBe("0123");
    expect(readout(app, "Code:")).toBe("Code: 0123");
    await app.paste("id:m-code", { text: "-5" });
    expect(app.value("id:m-code")).toBe("0123");
    await app.paste("id:m-rate", { text: "-5" });
    expect(app.value("id:m-rate")).toBe("12.5");
    expect(readout(app, "Rate:")).toBe("Rate: 12.5");
  });
}


for (const size of SIZES) {
  test(`${size.name}: focused money typing sends no text or caret back, at 8, 10 and 12 digits`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    for (const digits of ["12345678", "1234567890", "123456789012", "100000"]) {
      await app.clear("id:m-amount");
      const textWrites = readout(app, "Text writes:");
      const caretWrites = readout(app, "Caret writes:");
      await app.type("id:m-amount", digits);
      expect(app.value("id:m-amount")).toBe(digits);
      expect(readout(app, "Amount:")).toBe(`Amount: ${digits}`);
      expect(readout(app, "Text writes:")).toBe(textWrites);
      expect(readout(app, "Caret writes:")).toBe(caretWrites);
      await app.type("id:m-other", " ");
      expect(app.value("id:m-amount")).toBe(formatGroupedExpectation(digits));
    }
    for (const amount of ["12345678.90", "1234567890.12", "123456789012.34", "99999.99", "1000000.005", "99999999.99", "15000.50"]) {
      await app.clear("id:m-amount");
      const textWrites = readout(app, "Text writes:");
      const caretWrites = readout(app, "Caret writes:");
      await app.type("id:m-amount", amount);
      expect(app.value("id:m-amount")).toBe(amount);
      expect(readout(app, "Amount:")).toBe(`Amount: ${amount}`);
      expect(readout(app, "Text writes:")).toBe(textWrites);
      expect(readout(app, "Caret writes:")).toBe(caretWrites);
    }
  });

  test(`${size.name}: focus on a filled grouped amount writes no text or caret, and typing keeps separators until blur`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    expect(app.value("id:m-grouped")).toBe("1,234.56");
    const textWrites = readout(app, "Text writes:");
    const caretWrites = readout(app, "Caret writes:");
    await app.type("id:m-grouped", ""); // focus only
    await app.wait(100);
    expect(app.value("id:m-grouped")).toBe("1,234.56");
    expect(readout(app, "Text writes:")).toBe(textWrites);
    expect(readout(app, "Caret writes:")).toBe(caretWrites);
    const beforeTypeText = readout(app, "Text writes:");
    const beforeTypeCaret = readout(app, "Caret writes:");
    await app.type("id:m-grouped", "9");
    expect(app.value("id:m-grouped")).toBe("1,234.569");
    expect(readout(app, "Grouped:")).toBe("Grouped: 1234.569");
    expect(readout(app, "Text writes:")).toBe(beforeTypeText);
    expect(readout(app, "Caret writes:")).toBe(beforeTypeCaret);
    await app.type("id:m-other", " ");
    expect(app.value("id:m-grouped")).toBe("1,234.57");
    expect(readout(app, "Grouped:")).toBe("Grouped: 1234.57");
  });

  test(`${size.name}: leaving a box that only gains zeros reports nothing`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-amount", "1250000.5");
    expect(readout(app, "Amount:")).toBe("Amount: 1250000.5");
    const reports = readout(app, "Reports:");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-amount")).toBe("1,250,000.50");
    expect(readout(app, "Amount:")).toBe("Amount: 1250000.5");
    expect(readout(app, "Reports:")).toBe(reports);
  });

  test(`${size.name}: a zero box is empty before and after an untouched focus, without a new report`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    expect(app.value("id:m-host")).toBe("");
    await app.type("id:m-host", "");
    await app.waitUntil(() => app.value("id:m-host") === "", { timeout: 2000 });
    await app.type("id:m-other", " ");
    expect(app.value("id:m-host")).toBe("");
    expect(readout(app, "Host:")).toBe("Host: 0.00");
  });

  test(`${size.name}: select-all, Backspace, then 15000.50 is not rewritten by the host's zero`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-host"); // the harness's select-all and Backspace gesture
    await app.wait(100);
    expect(app.value("id:m-host")).toBe("");
    await app.type("id:m-host", "15000.50");
    expect(app.value("id:m-host")).toBe("15000.50");
    expect(readout(app, "Host:")).toBe("Host: 15000.50");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-host")).toBe("15,000.50");
    expect(readout(app, "Host:")).toBe("Host: 15000.50");
  });

  test(`${size.name}: a plain paste, a third decimal and a refused key send no focused write`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-amount");
    const beforePasteText = readout(app, "Text writes:");
    const beforePasteCaret = readout(app, "Caret writes:");
    await app.paste("id:m-amount", { text: "99999999.99" });
    expect(app.value("id:m-amount")).toBe("99999999.99");
    expect(readout(app, "Amount:")).toBe("Amount: 99999999.99");
    expect(readout(app, "Text writes:")).toBe(beforePasteText);
    expect(readout(app, "Caret writes:")).toBe(beforePasteCaret);
    const beforeText = readout(app, "Text writes:");
    const beforeCaret = readout(app, "Caret writes:");
    await app.type("id:m-amount", "5");
    expect(app.value("id:m-amount")).toBe("99999999.995");
    expect(readout(app, "Amount:")).toBe("Amount: 99999999.995");
    expect(readout(app, "Text writes:")).toBe(beforeText);
    expect(readout(app, "Caret writes:")).toBe(beforeCaret);
    await app.type("id:m-amount", "x.");
    expect(app.value("id:m-amount")).toBe("99999999.995x.");
    expect(readout(app, "Amount:")).toBe("Amount: empty");
    expect(app.texts().some((t) => t.includes(" is not an amount. The box kept "))).toBe(false);
    await app.type("id:m-other", " ");
    expect(app.value("id:m-amount")).toBe("");
    expect(readout(app, "Amount:")).toBe("Amount: empty");
    expect(app.texts()).toContain(leftSentence("99999999.995x.", ""));
  });

  test(`${size.name}: typed commas and letters stay exact, invalid and unreported`, async () => {
    for (const bad of ["15000,500", "150a00.50"]) {
      const app = await open("./masked.fixture.tsx", size.options);
      await app.clear("id:m-amount");
      const textWrites = readout(app, "Text writes:");
      const caretWrites = readout(app, "Caret writes:");
      await app.type("id:m-amount", bad);
      expect(app.value("id:m-amount")).toBe(bad);
      expect(readout(app, "Amount:")).toBe("Amount: empty");
      expect(app.texts().some((t) => t.includes(" is not an amount. The box kept "))).toBe(false);
      expect(readout(app, "Text writes:")).toBe(textWrites);
      expect(readout(app, "Caret writes:")).toBe(caretWrites);
      await app.type("id:m-other", " ");
      expect(app.value("id:m-amount")).toBe("");
      expect(readout(app, "Amount:")).toBe("Amount: empty");
      expect(app.texts()).toContain(leftSentence(bad, ""));
    }
  });

  test(`${size.name}: invalid focused money text is never written back, reported or saved`, async () => {
    for (const bad of ["1500,500", "1.234,56", "1,2,3", "1e5", "0x10", "1 234,50"]) {
      const app = await open("./masked.fixture.tsx", size.options);
      await app.clear("id:m-amount");
      const textWrites = readout(app, "Text writes:");
      const caretWrites = readout(app, "Caret writes:");
      await app.paste("id:m-amount", { text: bad });
      expect(app.value("id:m-amount")).toBe(bad);
      expect(readout(app, "Amount:")).toBe("Amount: empty");
      expect(app.texts().some((t) => t.includes(" is not an amount. The box kept "))).toBe(false);
      expect(readout(app, "Text writes:")).toBe(textWrites);
      expect(readout(app, "Caret writes:")).toBe(caretWrites);
      await app.type("id:m-other", " ");
      expect(app.value("id:m-amount")).toBe("");
      expect(readout(app, "Amount:")).toBe("Amount: empty");
      expect(app.texts()).toContain(leftSentence(bad, ""));
    }
  });

  test(`${size.name}: a fast 15000..50 burst stays exact, invalid and unreported`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-amount");
    const textWrites = readout(app, "Text writes:");
    const caretWrites = readout(app, "Caret writes:");
    await app.type("id:m-amount", "15000..50");
    expect(app.value("id:m-amount")).toBe("15000..50");
    expect(app.texts().some((t) => t.includes(" is not an amount. The box kept "))).toBe(false);
    expect(readout(app, "Amount:")).toBe("Amount: empty");
    expect(readout(app, "Text writes:")).toBe(textWrites);
    expect(readout(app, "Caret writes:")).toBe(caretWrites);
    await app.type("id:m-other", " ");
    expect(app.value("id:m-amount")).toBe("");
    expect(readout(app, "Amount:")).toBe("Amount: empty");
    expect(app.texts()).toContain(leftSentence("15000..50", ""));
  });

  test(`${size.name}: invalid typing falls back to the focus value, and correcting it reports again`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-two"); // the box held 2,000.00 when this focus began
    await app.type("id:m-two", "1");
    await app.type("id:m-two", ",");
    await app.type("id:m-two", "5");
    expect(app.value("id:m-two")).toBe("1,5");
    expect(readout(app, "Two:")).toBe("Two: 2000");
    expect(app.texts().some((t) => t.includes(" is not an amount. The box kept "))).toBe(false);
    await app.type("id:m-two", "0");
    await app.type("id:m-two", "0");
    expect(app.value("id:m-two")).toBe("1,500");
    expect(readout(app, "Two:")).toBe("Two: 1500");
  });

  test(`${size.name}: leaving unreadable text keeps the fallback amount and says what was typed`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-two");
    await app.type("id:m-two", "1500,500");
    expect(readout(app, "Two:")).toBe("Two: 2000");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-two")).toBe("2,000.00");
    expect(readout(app, "Two:")).toBe("Two: 2000");
    expect(app.texts()).toContain(leftSentence("1500,500", "2,000.00"));
    // Refocusing clears the sentence; leaving again without typing does not raise it again.
    await app.type("id:m-two", "");
    expect(app.texts()).not.toContain(leftSentence("1500,500", "2,000.00"));
    await app.type("id:m-other", " ");
    expect(app.value("id:m-two")).toBe("2,000.00");
    expect(app.texts()).not.toContain(leftSentence("1500,500", "2,000.00"));
  });

  test(`${size.name}: a programmatic blur of unreadable text keeps the fallback and says it once`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-two");
    await app.type("id:m-two", "1500x");
    await app.press("id:m-blur-two");
    expect(app.value("id:m-two")).toBe("2,000.00");
    expect(app.texts()).toContain(leftSentence("1500x", "2,000.00"));
  });

  test(`${size.name}: with onLeftUnreadable the box calls back and draws no sentence of its own`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-cb");
    await app.type("id:m-cb", "1500x");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-cb")).toBe("2,000.00");
    expect(readout(app, "Invalid:")).toBe("Invalid: 1500x|2,000.00");
    expect(readout(app, "Invalid count:")).toBe("Invalid count: 1");
    expect(app.texts()).not.toContain(leftSentence("1500x", "2,000.00"));
    await app.type("id:m-cb", "");
    await app.type("id:m-other", " ");
    expect(readout(app, "Invalid count:")).toBe("Invalid count: 1");
  });

  test(`${size.name}: [comma] a lone comma read as the decimal point on leaving is said through onCommaRead, once; a thousands comma is not`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-cb");
    await app.type("id:m-cb", "1500,50");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-cb")).toBe("1,500.50");
    expect(readout(app, "Cb:")).toBe("Cb: 1500.50");
    expect(readout(app, "Comma:")).toBe("Comma: 1500,50|1,500.50");
    expect(readout(app, "Comma count:")).toBe("Comma count: 1");
    // Back in and out with no edit: the box's own grouping is no comma of the filer's.
    await app.type("id:m-cb", "");
    await app.type("id:m-other", " ");
    expect(readout(app, "Comma count:")).toBe("Comma count: 1");
    // "1,5" is 1.50; "2,500" is two thousand five hundred, read strictly: no call.
    await app.clear("id:m-cb");
    await app.type("id:m-cb", "1,5");
    await app.type("id:m-other", " ");
    expect(readout(app, "Comma:")).toBe("Comma: 1,5|1.50");
    expect(readout(app, "Comma count:")).toBe("Comma count: 2");
    await app.clear("id:m-cb");
    await app.type("id:m-cb", "2,500");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-cb")).toBe("2,500.00");
    expect(readout(app, "Comma count:")).toBe("Comma count: 2");
  });

  test(`${size.name}: [F1] leaveRef's leave reads and reports the box at once, with the keys still in it, and the blur after it adds nothing`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-cb");
    await app.type("id:m-cb", "1500,50");
    expect(readout(app, "Cb:")).not.toBe("Cb: 1500.50");
    // Return's key press asks for the leave, in its own turn, before the blur Return brings headless.
    await app.keyboard.press("enter");
    expect(readout(app, "Cb at Return:")).toBe("Cb at Return: 1500.50");
    expect(readout(app, "Comma:")).toBe("Comma: 1500,50|1,500.50");
    // The blur after it is the same leave: it runs once.
    await app.type("id:m-other", " ");
    expect(readout(app, "Cb:")).toBe("Cb: 1500.50");
    expect(readout(app, "Cb blurs:")).toBe("Cb blurs: 1");
    expect(readout(app, "Comma count:")).toBe("Comma count: 1");
  });

  test(`${size.name}: a held outside value is the fallback for unreadable text`, async () => {
    const app = await open("./masked.fixture.tsx", { ...size.options, clock: {} });
    await app.press("id:m-arm-held");
    await app.type("id:m-two", "x");
    expect(readout(app, "Two:")).toBe("Two: 3000");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-two")).toBe("3,000.00");
    expect(app.texts()).toContain(leftSentence("2,000.00x", "3,000.00"));
  });

  test(`${size.name}: a key then Backspace is no edit, so a held outside value still wins`, async () => {
    const app = await open("./masked.fixture.tsx", { ...size.options, clock: {} });
    await app.press("id:m-arm-held");
    await app.type("id:m-two", "");
    await app.keyboard.press("4");
    await app.keyboard.press("backspace");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-two")).toBe("3,000.00");
    expect(readout(app, "Two:")).toBe("Two: 3000.00");
  });

  test(`${size.name}: [B3] an outside value that wins over typing back to the shown amount is said through onOutsideKept; with no typing it is not`, async () => {
    const app = await open("./masked.fixture.tsx", { ...size.options, clock: {} });
    // The same amount typed again over the 2,000.00 the box showed, while 3000 arrived from outside.
    await app.press("id:m-arm-held");
    await app.clear("id:m-two");
    await app.type("id:m-two", "2000");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-two")).toBe("3,000.00");
    expect(readout(app, "Outside:")).toBe("Outside: 2000|3,000.00");
    // A key and its Backspace: no edit, and the number still changed under the filer's keys.
    const again = await open("./masked.fixture.tsx", { ...size.options, clock: {} });
    await again.press("id:m-arm-held");
    await again.type("id:m-two", "");
    await again.keyboard.press("4");
    await again.keyboard.press("backspace");
    await again.type("id:m-other", " ");
    expect(again.value("id:m-two")).toBe("3,000.00");
    expect(readout(again, "Outside:")).toBe("Outside: 2,000.00|3,000.00");
  });

  test(`${size.name}: [B3] focus and leave with no typing while an outside value arrives: the box shows it and says nothing`, async () => {
    const app = await open("./masked.fixture.tsx", { ...size.options, clock: {} });
    await app.press("id:m-arm-held");
    await app.type("id:m-two", "");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-two")).toBe("3,000.00");
    expect(readout(app, "Outside:")).toBe("Outside: none");
  });

  test(`${size.name}: M2-2: an outside refill after the empty report wins when it is left unanswered`, async () => {
    const app = await open("./masked.fixture.tsx", { ...size.options, clock: {} });
    await app.clear("id:m-refill");
    await app.clock?.runFor(100);
    expect(app.value("id:m-refill")).toBe("");
    expect(readout(app, "Refill:")).toBe("Refill: 1250");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-refill")).toBe("1,250.00");
    expect(readout(app, "Refill:")).toBe("Refill: 1250");
  });

  test(`${size.name}: M2-2: any edit after the refill is the filer's, even back to empty`, async () => {
    const app = await open("./masked.fixture.tsx", { ...size.options, clock: {} });
    await app.clear("id:m-refill");
    await app.clock?.runFor(100);
    await app.keyboard.press("7");
    await app.keyboard.press("backspace");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-refill")).toBe("");
    expect(readout(app, "Refill:")).toBe("Refill: empty");
  });

  test(`${size.name}: M2-2: an outside value before the emptying does not win`, async () => {
    const app = await open("./masked.fixture.tsx", { ...size.options, clock: {} });
    await app.press("id:m-arm-refill-before");
    await app.type("id:m-refill", "");
    await app.clear("id:m-refill");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-refill")).toBe("");
    expect(readout(app, "Refill:")).toBe("Refill: empty");
  });

  test(`${size.name}: a valid money key reports in the same turn, with no timer`, async () => {
    const app = await open("./masked.fixture.tsx", { ...size.options, clock: {} });
    await app.clear("id:m-amount");
    await app.type("id:m-amount", "12");
    expect(readout(app, "Amount:")).toBe("Amount: 12");
  });
}


for (const size of SIZES) {
  test(`${size.name}: group 5 commas: typed decimal commas read only on leaving`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    for (const [typed, shown, saved] of [
      ["1500,50", "1,500.50", "1500.50"],
      ["1500,5", "1,500.50", "1500.50"],
      ["1,500", "1,500.00", "1500"],
      ["1,50", "1.50", "1.50"],
    ] as const) {
      const beforeKeys = readout(app, "Amount:");
      await app.clear("id:m-amount");
      await app.type("id:m-amount", typed);
      expect(app.value("id:m-amount")).toBe(typed);
      if (typed !== "1,500") expect(readout(app, "Amount:")).toBe(beforeKeys);
      await app.type("id:m-other", " ");
      expect(app.value("id:m-amount")).toBe(shown);
      expect(readout(app, "Amount:")).toBe(`Amount: ${saved}`);
    }
  });

  test(`${size.name}: group 5 commas: mid typing 1,5 reports nothing new and shows no error`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-two");
    await app.type("id:m-two", "1,5");
    expect(app.value("id:m-two")).toBe("1,5");
    expect(readout(app, "Two:")).toBe("Two: 2000");
    expect(app.texts().some((t) => t.includes(" is not an amount. The box kept "))).toBe(false);
  });

  test(`${size.name}: group 5 commas: replacing zero and typing a decimal comma saves the point amount`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-host");
    await app.type("id:m-host", "1500,50");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-host")).toBe("1,500.50");
    expect(readout(app, "Host:")).toBe("Host: 1500.50");
  });

  test(`${size.name}: group 5 commas: a comma pasted into text that already had ours is unsure and refused with the hint`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-grouped", "");
    await app.paste("id:m-grouped", { text: "1500,50" });
    await app.type("id:m-other", " ");
    expect(app.value("id:m-grouped")).toBe("1,234.56");
    expect(app.texts()).toContain(`${leftSentence("1,234.561500,50", "1,234.56")}`);
    expect(readout(app, "Grouped:")).toBe("Grouped: 1234.56");
  });

  test(`${size.name}: group 5 commas: a grouped paste into an empty box reads as a grouped amount`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-amount");
    await app.paste("id:m-amount", { text: "1,234" });
    expect(readout(app, "Amount:")).toBe("Amount: 1234");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-amount")).toBe("1,234.00");
    expect(readout(app, "Amount:")).toBe("Amount: 1234");
  });

  test(`${size.name}: group 5 commas: negative and peso-prefixed decimal commas read on leaving`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-free");
    await app.type("id:m-free", "-1500,50");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-free")).toBe("-1,500.50");
    expect(readout(app, "Free:")).toBe("Free: -1500.50");
    await app.clear("id:m-amount");
    await app.type("id:m-amount", "₱ 1500,50");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-amount")).toBe("1,500.50");
    expect(readout(app, "Amount:")).toBe("Amount: 1500.50");
  });

  test(`${size.name}: V1: an emptied nullable amount is blank after leaving`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-amount", "");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-amount")).toBe("");
    expect(readout(app, "Amount:")).toBe("Amount: empty");
  });

  test(`${size.name}: V5: an all-zero integer code keeps its text on focus while a count still empties`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-zcode", "");
    expect(app.value("id:m-zcode")).toBe("000");
  });
}

function formatGroupedExpectation(digits: string): string {
  return `${digits.replace(/\B(?=(\d{3})+(?!\d))/g, ",")}.00`;
}

for (const size of SIZES) {
  test(`${size.name}: U: unmounting a focused unreadable money box is a leave, without a box write`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-cb");
    await app.type("id:m-cb", "1500x");
    await app.press("id:m-unmount-cb");
    expect(readout(app, "Invalid:")).toBe("Invalid: 1500x|2,000.00");
    expect(readout(app, "Invalid count:")).toBe("Invalid count: 1");
    expect(readout(app, "Cb blurs:")).toBe("Cb blurs: 1");
    expect(app.shows("id:m-cb")).toBe(false);
  });

  test(`${size.name}: U: unmounting a focused readable money box reports the blur amount; unfocused unmount says nothing`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.clear("id:m-cb");
    await app.type("id:m-cb", "1500.5");
    await app.press("id:m-unmount-cb");
    expect(readout(app, "Cb:")).toBe("Cb: 1500.5");
    expect(readout(app, "Cb blurs:")).toBe("Cb blurs: 1");
    expect(readout(app, "Invalid count:")).toBe("Invalid count: 0");
  });
}

for (const size of SIZES) {

  test(`${size.name}: U: unmounting an unfocused money box says nothing`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.press("id:m-unmount-cb");
    expect(readout(app, "Cb blurs:")).toBe("Cb blurs: 0");
    expect(readout(app, "Invalid count:")).toBe("Invalid count: 0");
  });
}

for (const size of SIZES) {
  test(`${size.name}: S2: unreadable text in an unfocused box falls back to its last value, not a stale focus value`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-two", "");
    await app.type("id:m-other", " "); // focus once, then leave: the focus value is 2000
    await app.press("id:m-set-two"); // while unfocused the value becomes 3000
    await app.press("id:m-insert-x-two");
    expect(readout(app, "Two:")).toBe("Two: 3000");
  });
}

for (const size of SIZES) {
  test(`${size.name}: group 5 commas: End and four Backspaces in a grouped amount read 123`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-grouped", "");
    await app.keyboard.press("end");
    for (let i = 0; i < 4; i++) await app.keyboard.press("backspace");
    expect(app.value("id:m-grouped")).toBe("1,23");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-grouped")).toBe("123.00");
    expect(readout(app, "Grouped:")).toBe("Grouped: 123");
  });

  test(`${size.name}: group 5 commas: End and Backspace in a whole-peso amount read 1,234`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-whole-five", "");
    await app.keyboard.press("end");
    await app.keyboard.press("backspace");
    expect(app.value("id:m-whole-five")).toBe("12,34");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-whole-five")).toBe("1,234");
    expect(readout(app, "WholeFive:")).toBe("WholeFive: 1234");
  });

  test(`${size.name}: group 5 commas: typing a comma into our grouping is unsure and strict`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-grouped", "");
    await app.keyboard.press("end");
    await app.keyboard.press(",");
    expect(app.value("id:m-grouped")).toBe("1,234.56,");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-grouped")).toBe("1,234.56");
    expect(app.texts()).toContain(leftSentence("1,234.56,", "1,234.56"));
  });

  test(`${size.name}: group 5 commas: select all then paste a decimal comma is unsure and never 150050`, async () => {
    const app = await open("./masked.fixture.tsx", size.options);
    await app.type("id:m-grouped", "");
    await app.keyboard.press("ctrl+a");
    await app.paste("id:m-grouped", { text: "1500,50" });
    expect(app.value("id:m-grouped")).not.toBe("150050");
    await app.type("id:m-other", " ");
    expect(app.value("id:m-grouped")).toBe("1,234.56");
    expect(readout(app, "Grouped:")).toBe("Grouped: 1234.56");
    expect(app.texts().some((t) => t.includes("Use a point for centavos"))).toBe(true);
  });
}
