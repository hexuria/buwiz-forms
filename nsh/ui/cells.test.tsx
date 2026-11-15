// DigitCells (the sign-in's and the taxpayer pages' TIN and branch code): one cell per digit, drawn in groups with dashes between, the
// last digit handed to the next box. At phone and desktop size.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";

const ENTRY = "./cells.fixture.tsx";
const SIZES = [
  { name: "phone", options: {} },
  { name: "desktop", options: { width: 1440, height: 900, platform: "macos" as const } },
] as const;
const readout = (app: App, prefix: string) => app.texts().find((t) => t.startsWith(prefix));

for (const size of SIZES) {
  test(`${size.name}: the TIN is nine digits in three groups with two dashes; the ninth digit is reported and moves the keys on`, async () => {
    const app = await open(ENTRY, size.options);
    await app.waitFor("id:c-tin");
    expect(app.shows("id:c-tin-dash-1")).toBe(true);
    expect(app.shows("id:c-tin-dash-2")).toBe(true);
    expect(app.shows("id:c-tin-dash-3")).toBe(false);
    expect(app.shows("id:c-code-dash-1")).toBe(false);
    await app.type("id:c-tin", "00012345");
    expect(readout(app, "Tin:")).toBe("Tin: 00012345");
    expect(readout(app, "Done:")).toBe("Done: none");
    await app.type("id:c-tin", "3");
    expect(readout(app, "Tin:")).toBe("Tin: 000123453");
    expect(readout(app, "Done:")).toBe("Done: 000123453");
    // The keys went to the PIN cells: the next digits are its.
    await app.keyboard.press("2");
    await app.keyboard.press("4");
    expect(readout(app, "Pin:")).toBe("Pin: 24");
    expect(readout(app, "Tin:")).toBe("Tin: 000123453");
  });

  test(`${size.name}: dashes and letters typed or pasted are not digits; a branch code is five cells`, async () => {
    const app = await open(ENTRY, size.options);
    await app.waitFor("id:c-tin");
    await app.type("id:c-tin", "000-123-45x3");
    expect(readout(app, "Tin:")).toBe("Tin: 000123453");
    expect(app.value("id:c-code")).toBe("00001");
    await app.clear("id:c-code");
    expect(readout(app, "Code:")).toBe("Code: empty");
    await app.type("id:c-code", "12");
    expect(readout(app, "Code:")).toBe("Code: 12");
    await app.type("id:c-code", "345");
    expect(readout(app, "Code:")).toBe("Code: 12345");
  });
}
