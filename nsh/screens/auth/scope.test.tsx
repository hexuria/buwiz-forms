// Help-desk sessions: signed in as one taxpayer, nothing of another taxpayer shows anywhere (old store.js scopeRoot and
// canSee): not its returns or deadlines, not in the palette, not in Recent. The admin sees both. Phone and desktop size.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../AuthDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const PARAMS = { demo: true, second: true, years: true, recent: true, mode: "helpdesk", adminPin: "1234", pins: { "000123453": "4321", "000111229": "5555" }, readout: true };

async function signIn(app: Awaited<ReturnType<typeof open>>, tin: string, pin: string) {
  await app.waitFor("id:auth-card");
  await app.type("id:auth-tin", tin);
  await app.type("id:auth-tp-pin", pin);
  await app.waitFor("id:frame");
}

test("desktop: a taxpayer session sees only its own returns, deadlines, palette entries and Recent", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: PARAMS } });
  await signIn(app, "000123453", "4321");
  await app.waitFor("text:session: taxpayer 000123453");
  expect(app.texts()).toContain("returns: 000123453 | deadlines: 000123453 | taxpayers: 000123453");
  // Recent: only her return.
  await app.waitFor("text:/^1601C .* · Juana Santos$/");
  expect(app.texts().some((t) => /Sample Trading Corp/.test(t))).toBe(false);
  // The palette: searching the other name or TIN finds no taxpayer.
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "Sample");
  expect(app.texts().some((t) => /Sample Trading Corp/.test(t))).toBe(false);
  await app.clear("id:palette-input");
  await app.type("id:palette-input", "000111229");
  expect(app.texts().some((t) => /Sample Trading Corp|000-111-229/.test(t))).toBe(false);
  await app.back();
  // Calendar and Filings name only her.
  await app.press("id:nav-calendar");
  await app.waitFor("id:nav-calendar");
  expect(app.texts().some((t) => /Sample Trading Corp/.test(t))).toBe(false);
  await app.press("id:nav-filings");
  expect(app.texts().some((t) => /Sample Trading Corp/.test(t))).toBe(false);
});

test("phone: the other taxpayer signs in to its own data only; the admin sees both", async () => {
  const app = await open(ENTRY, { host: { params: PARAMS } });
  await signIn(app, "000111229", "5555");
  await app.waitFor("text:session: taxpayer 000111229");
  expect(app.texts()).toContain("returns: 000111229 | deadlines: 000111229 | taxpayers: 000111229");
  expect(app.texts().some((t) => /Juana Santos/.test(t))).toBe(false);
  await app.press("id:nb-avatar");
  await app.waitFor("id:sheet-logout");
  await app.press("id:sheet-logout");
  await signIn(app, "000000000", "1234");
  await app.waitFor("text:session: admin");
  expect(app.texts()).toContain("returns: 000111229,000123453 | deadlines: 000111229,000123453 | taxpayers: 000111229,000123453");
});
