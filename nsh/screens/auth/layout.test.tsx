// Layout C on the pages before the app (parts.tsx OpenPage): the back control sits in the fixed back row at the top (never in
// the scrolling block), the same row on every page; and the block (the logo, the page, its button and links) fits the window
// at the smallest sizes by the logo stepping down, or scrolls with its button in the scroll view. The headless test API reads
// no positions, so the page's own measurement is read from the fixture's readout ("fit: <page> logo=.. fits=.. rest=..
// room=..", AuthDemo.tsx); the step rule itself is signInLook.test.ts's.
import { type App, expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const PHONE = { width: 390, height: 844 };
const SMALL_PHONE = { width: 375, height: 667 };
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };
const SHORT_DESKTOP = { width: 1280, height: 640, platform: "macos" as const };
const SMALL_DESKTOP = { width: 800, height: 500, platform: "macos" as const };
const SMALLEST_DESKTOP = { width: 420, height: 480, platform: "macos" as const };
/** Android 360x640 dp with the three-button bar: the status bar's 24 over the page, the bar's 48 under it. */
const SMALL_ANDROID = { width: 360, height: 640, platform: "android" as const, safeArea: { top: 24, bottom: 48 } };

interface Fit {
  page: string;
  logo: number;
  fits: boolean;
  rest: number;
  room: number;
}

/** The open page's measurement, from the readout. */
function fit(app: App): Fit | null {
  const line = app.texts().find((t) => t.startsWith("fit: "));
  const m = line?.match(/^fit: (\S+) logo=(\d+) fits=(true|false) rest=(\d+) room=(\d+)$/);
  if (!m) return null;
  return { page: m[1], logo: Number(m[2]), fits: m[3] === "true", rest: Number(m[4]), room: Number(m[5]) };
}

/** The page has measured its block, and the whole of it (the logo, the page, its button and links) is inside the window. */
async function expectFits(app: App, page: string): Promise<Fit> {
  await app.waitUntil(() => fit(app)?.page === page && (fit(app)?.rest ?? 0) > 0, { timeout: 5000 });
  const now = fit(app)!;
  expect(now.fits).toBe(true);
  expect(now.logo + now.rest).toBeLessThanOrEqual(now.room);
  return now;
}

async function toEmail(app: App): Promise<void> {
  await app.waitFor("id:gate-start");
  await app.press("id:gate-start-continue");
  await app.waitFor("id:gate-email-page");
}

async function toCode(app: App): Promise<void> {
  await toEmail(app);
  await app.type("id:gate-email", "juana@example.com");
  await app.press("id:gate-continue");
  await app.waitFor("id:gate-code-page", { timeout: 15000 });
}

for (const [name, size] of [["phone", PHONE], ["desktop", DESKTOP]] as const) {
  test(`${name}: the back control is in the fixed back row on the email page and the code page, never in the scrolling block`, async () => {
    const app = await open(ENTRY, { ...size, host: { params: { gate: true, readout: true } } });
    await app.waitFor("id:gate-start");
    // The row is there on the first page too, empty, so the block is centred under the same row on every page.
    expect(app.shows("id:open-back-row")).toBe(true);
    expect(app.shows("id:auth-scroll >> id:open-back-row")).toBe(false);
    await toEmail(app);
    expect(app.shows("id:open-back-row >> id:gate-back")).toBe(true);
    expect(app.shows("id:auth-scroll >> id:gate-back")).toBe(false);
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-page", { timeout: 15000 });
    expect(app.shows("id:open-back-row >> id:gate-change-email")).toBe(true);
    expect(app.shows("id:auth-scroll >> id:gate-change-email")).toBe(false);
    expect(app.count("id:open-back-row")).toBe(1);
  }, 60_000);
}

for (const [name, size, steps] of [
  ["375x667 phone", SMALL_PHONE, [150, 112, 80]],
  ["1280x640 desktop window", SHORT_DESKTOP, [180, 128, 88]],
] as const) {
  test(`${name}: the first, email and code pages fit the window, the logo at one of its steps`, async () => {
    const app = await open(ENTRY, { ...size, host: { params: { gate: true, readout: true } } });
    await app.waitFor("id:gate-start");
    expect(steps).toContain((await expectFits(app, "gate-start")).logo);
    await toEmail(app);
    expect(steps).toContain((await expectFits(app, "gate-email-page")).logo);
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-page", { timeout: 15000 });
    expect(steps).toContain((await expectFits(app, "gate-code-page")).logo);
  }, 60_000);

  test(`${name}: the PIN sign-in, its links and Forgot PIN for the admin fit the window`, async () => {
    const app = await open(ENTRY, { ...size, host: { params: { demo: true, mode: "helpdesk", adminPin: "1234", pins: { "000123453": "4321" }, readout: true } } });
    await app.waitFor("id:auth-card");
    await expectFits(app, "auth-card");
    for (const id of ["auth-submit", "auth-method", "auth-forgot"]) expect(app.shows(`id:${id}`)).toBe(true);
    await app.press("id:auth-forgot");
    await app.type("id:auth-tin", "000000000");
    await app.waitFor("id:auth-recovery");
    await app.wait(100);
    await expectFits(app, "auth-card");
  }, 60_000);

  test(`${name}: each first-run step and the locked page fit the window`, async () => {
    const app = await open(ENTRY, { ...size, host: { params: { readout: true } } });
    await app.waitFor("id:setup-card");
    await expectFits(app, "setup-card");
    await app.press("id:setup-helpdesk");
    await app.waitFor("id:setup-pin-card");
    await expectFits(app, "setup-pin-card");
    await app.type("id:setup-admin-pin", "1234");
    await app.press("id:setup-pin-submit");
    await app.waitFor("id:setup-key-card");
    await expectFits(app, "setup-key-card");
    const locked = await open(ENTRY, { ...size, host: { params: { demo: true, mode: "solo", adminPin: "1234", authFails: "The access state could not be read.", readout: true } } });
    await locked.waitFor("id:access-error-card");
    await expectFits(locked, "access-error-card");
  }, 90_000);
}

for (const [name, size] of [
  ["800x500 desktop window", SMALL_DESKTOP],
  ["420x480 desktop window (the smallest the app allows)", SMALLEST_DESKTOP],
] as const) {
  test(`${name}: the email and code pages fit, drawn tight, the logo at one of its steps`, async () => {
    const app = await open(ENTRY, { ...size, host: { params: { gate: true, readout: true } } });
    await toEmail(app);
    expect([180, 128, 88, 64]).toContain((await expectFits(app, "gate-email-page")).logo);
    await app.type("id:gate-email", "juana@example.com");
    await app.press("id:gate-continue");
    await app.waitFor("id:gate-code-page", { timeout: 15000 });
    await expectFits(app, "gate-code-page");
  }, 60_000);

  test(`${name}: the TIN and PIN sign-in fits, the owner's and the help desk's (whose line is longer)`, async () => {
    const owner = await open(ENTRY, { ...size, host: { params: { demo: true, mode: "solo", adminPin: "1234", readout: true } } });
    await owner.waitFor("id:auth-card");
    await expectFits(owner, "auth-card");
    const desk = await open(ENTRY, { ...size, host: { params: { demo: true, mode: "helpdesk", adminPin: "1234", pins: { "000123453": "4321" }, readout: true } } });
    await desk.waitFor("id:auth-card");
    expect(desk.texts().some((t) => t.includes("help desk admin"))).toBe(true);
    await expectFits(desk, "auth-card");
    for (const id of ["auth-tin", "auth-tp-pin", "auth-submit", "auth-method", "auth-forgot"]) expect(desk.shows(`id:${id}`)).toBe(true);
  }, 60_000);
}

test("Android 360x640 with the three-button bar: the TIN and PIN sign-in fits above the bar, Forgot PIN? included", async () => {
  const owner = await open(ENTRY, { ...SMALL_ANDROID, host: { params: { demo: true, mode: "solo", adminPin: "1234", readout: true } } });
  await owner.waitFor("id:auth-card");
  await expectFits(owner, "auth-card");
  expect(owner.shows("id:auth-forgot")).toBe(true);
  const desk = await open(ENTRY, { ...SMALL_ANDROID, host: { params: { demo: true, mode: "helpdesk", adminPin: "1234", pins: { "000123453": "4321" }, readout: true } } });
  await desk.waitFor("id:auth-card");
  await expectFits(desk, "auth-card");
}, 60_000);

test("a window too short for the block at its smallest step: the logo at 80, and the page scrolls with its field and button in the scroll view", async () => {
  // As a 390x844 phone with its keyboard up leaves about 390 points (the test kit has no keyboard to raise).
  const app = await open(ENTRY, { width: 390, height: 390, host: { params: { gate: true, readout: true } } });
  await toEmail(app);
  await app.waitUntil(() => fit(app)?.page === "gate-email-page" && (fit(app)?.rest ?? 0) > 0, { timeout: 5000 });
  const now = fit(app)!;
  expect(now.logo).toBe(80);
  expect(now.fits).toBe(false);
  expect(app.shows("id:auth-scroll >> id:gate-email")).toBe(true);
  expect(app.shows("id:auth-scroll >> id:gate-continue")).toBe(true);
  // Still a working page: the address goes in and Continue sends it.
  await app.type("id:gate-email", "juana@example.com");
  await app.press("id:gate-continue");
  await app.waitFor("id:gate-code-page", { timeout: 15000 });
}, 60_000);
