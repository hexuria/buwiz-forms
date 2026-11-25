// A phone's and a tablet's pages in the platform's navigation stack (shell/NarrowStack.tsx): the system's back (a swipe from the
// edge on iOS, Android's back) pops the top page and the app's history follows; the tax profile's tabs are one page; a return keeps
// what was typed when its page is popped. The harness's back() is the system's back: it pops the top page as the gesture does (it
// cannot drag a finger; the Simulator recording of the change shows the page following one). Against ShellDemo.tsx's demo taxpayer.
import { expect, open, test } from "nsh/test";

const ENTRY = "../ShellDemo.tsx";
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const DEVICES = [
  { name: "iPhone", size: {} },
  { name: "Android phone", size: { platform: "android" as const } },
  { name: "iPad", size: { width: 1032, height: 1376, platform: "ios" as const } },
];

for (const { name, size } of DEVICES) {
  test(`${name}: the system's back pops Settings > Account, then Settings, and the header follows`, async () => {
    const app = await open(ENTRY, { ...size, ...asking() });
    await app.waitFor("id:narrow-stack");
    await app.press("id:nb-avatar");
    await app.press("id:nb-menu-settings");
    await app.waitFor("id:settings-nav-account");
    await app.press("id:settings-nav-account");
    await app.waitFor("id:settings-security");
    expect(app.texts()).toContain("Account");
    await app.back();
    await app.waitFor("id:settings-nav-account");
    expect(app.shows("id:settings-security")).toBe(false);
    expect(app.shows("id:nb-back")).toBe(true);
    await app.back();
    await app.waitFor("id:home");
    expect(app.shows("id:nb-back")).toBe(false);
    // The app's own history went back with the pages: nothing is left to go back to from Home.
    expect(app.shows("id:settings-nav-account")).toBe(false);
  }, 60_000);

  test(`${name}: the tax profile's three tabs are one page: back from Forms leaves the profile, not the tab`, async () => {
    const app = await open(ENTRY, { ...size, ...asking() });
    await app.waitFor("id:narrow-stack");
    await app.press("id:nb-avatar");
    await app.press("id:nb-menu-profile");
    await app.waitFor("id:sheet-extras");
    await app.press("id:panel-open-profile");
    await app.waitFor("id:tp-page-profile");
    await app.press("id:tp-tabs >> text:Forms");
    await app.waitFor("id:tp-page-forms");
    await app.press("id:tp-tabs >> text:Templates");
    await app.waitFor("id:tp-page-templates");
    await app.back();
    await app.waitFor("id:home");
  }, 60_000);
}

test("iPhone: a saved return popped by the system's back keeps what was typed", async () => {
  // The start route opens the return over Home (a pushed page).
  const app = await open(ENTRY, asking({ start: "form=2551Q/2026/3" }));
  await app.waitFor("id:fp-pill", { timeout: 30_000 });
  await app.press("id:fp-pill-open");
  await app.waitFor("id:fp-section-2", { timeout: 15_000 });
  await app.press("id:fp-section-2");
  await app.wait(800);
  await app.type("id:fp-frm2551Qv2018:txt15", "100");
  await app.waitFor("text:✓ Saved", { timeout: 5000 });
  await app.back();
  await app.waitFor("id:home");
  // The return again, from Filings: the amount is there.
  await app.press("id:tab-filings");
  await app.waitFor("id:filings-rows");
  await app.press("id:filings-rows >> role:button >> nth=0");
  await app.waitFor("id:fp-pill", { timeout: 30_000 });
  await app.press("id:fp-pill-open");
  await app.waitFor("id:fp-section-2", { timeout: 15_000 });
  await app.press("id:fp-section-2");
  await app.wait(800);
  expect(app.value("id:fp-frm2551Qv2018:txt15")).toMatch(/^100(\.00)?$/);
}, 90_000);

// The same return left by the header's Back (the app's own move, which waits for the page's save): if this keeps the amount and the
// system's back does not, the loss is the stack's; if neither keeps it, it is how the fixture reopens the return.
test("iPhone: a saved return left by the header's Back keeps what was typed", async () => {
  const app = await open(ENTRY, asking({ start: "form=2551Q/2026/3" }));
  await app.waitFor("id:fp-pill", { timeout: 30_000 });
  await app.press("id:fp-pill-open");
  await app.waitFor("id:fp-section-2", { timeout: 15_000 });
  await app.press("id:fp-section-2");
  await app.wait(800);
  await app.type("id:fp-frm2551Qv2018:txt15", "100");
  await app.waitFor("text:✓ Saved", { timeout: 5000 });
  await app.press("id:nb-back");
  await app.waitFor("id:home");
  await app.press("id:tab-filings");
  await app.waitFor("id:filings-rows");
  await app.press("id:filings-rows >> role:button >> nth=0");
  await app.waitFor("id:fp-pill", { timeout: 30_000 });
  await app.press("id:fp-pill-open");
  await app.waitFor("id:fp-section-2", { timeout: 15_000 });
  await app.press("id:fp-section-2");
  await app.wait(800);
  expect(app.value("id:fp-frm2551Qv2018:txt15")).toMatch(/^100(\.00)?$/);
}, 90_000);

test("a desktop window under 600 points keeps the one view: no navigation stack", async () => {
  const app = await open(ENTRY, { width: 480, height: 800, platform: "macos", ...asking() });
  await app.waitFor("id:tab-bar");
  expect(app.shows("id:narrow-stack")).toBe(false);
});
