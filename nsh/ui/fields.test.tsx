// Text entry at phone and desktop size: plain text, the TIN, money, percent and integer masks, the date, PIN cells.
import { expect, test } from "nsh/test";
import { openKit, readout, SIZES } from "./harness";

for (const size of SIZES) {
  test(`${size.name}: text field types, TIN groups while typing`, async () => {
    const app = await openKit("../App.tsx", size.options);
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
    const app = await openKit("../App.tsx", size.options);
    await app.clear("id:kit-tin");
    await app.paste("id:kit-tin", { text: "000-123-453-00000" });
    expect(app.value("id:kit-tin")).toBe("000-123-453-00000");
    expect(readout(app, "Digits:")).toBe("Digits: 00012345300000");
  });

  test(`${size.name}: money groups while typing, plain value out, two decimals on leaving`, async () => {
    const app = await openKit("../App.tsx", size.options);
    expect(app.value("id:kit-money")).toBe("1,250,000.00");
    await app.clear("id:kit-money");
    expect(readout(app, "Value:")).toBe("Value: empty");
    await app.type("id:kit-money", "1250000");
    expect(app.value("id:kit-money")).toBe("1,250,000");
    expect(readout(app, "Value:")).toBe("Value: 1250000");
    await app.type("id:kit-money", ".5x.7");
    expect(app.value("id:kit-money")).toBe("1,250,000.57");
    expect(readout(app, "Value:")).toBe("Value: 1250000.57");
    await app.keyboard.press("backspace");
    await app.keyboard.press("backspace");
    await app.keyboard.press("backspace");
    expect(app.value("id:kit-money")).toBe("1,250,000");
    // Leaving the box: another field takes the keys.
    await app.type("id:kit-name", " ");
    expect(app.value("id:kit-money")).toBe("1,250,000.00");
    expect(readout(app, "Value:")).toBe("Value: 1250000");
    await app.clear("id:kit-money");
    await app.paste("id:kit-money", { text: "₱ 2,500.75" });
    expect(readout(app, "Value:")).toBe("Value: 2500.75");
  });

  test(`${size.name}: percent, integer, date, PIN`, async () => {
    const app = await openKit("../App.tsx", size.options);
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
    expect(app.value("id:kit-date")).toBe("Dec 10, 2020");
    await app.clear("id:kit-date");
    await app.type("id:kit-date", "13/45");
    expect(app.texts().some((t) => t.startsWith("Enter a valid date"))).toBe(false);
    await app.type("id:kit-name", " ");
    expect(app.texts().some((t) => t.startsWith("Enter a valid date"))).toBe(true);
    await app.type("id:kit-code", "123456");
    expect(readout(app, "PIN digits:")).toBe("PIN digits: 0 | Code complete: 123456");
    await app.type("id:kit-pin", "4321");
    expect(readout(app, "PIN digits:")).toBe("PIN digits: 4 | Code complete: 123456");
  });
}
