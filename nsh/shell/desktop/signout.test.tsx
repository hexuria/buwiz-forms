// F8-7: the menu's Sign Out (eBIRForms > Sign Out) is Settings › Account's sign-out: the Buwiz session closes, the account gate comes
// back and every screen's state is put back for the next session (app.lockCloud runs the session-change resets). It used to only set
// the gate flag, so a half-typed Settings search outlived the sign-out. DesktopDemo with a sign-out that needs no network.
import { expect, open, test } from "nsh/test";

const ENTRY = "../../DesktopDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };

test("desktop: the menu's Sign Out brings the gate back and leaves nothing half typed in Settings", async () => {
  const app = await open(ENTRY, { ...WIDE, host: { params: { signOut: true } } });
  await app.waitFor("id:sidebar");
  await app.keyboard.press("mod+,");
  await app.waitFor("id:settings-search");
  await app.type("id:settings-search", "juana");
  await app.waitFor("text:settings search=juana");
  expect(app.shows("text:gate=open")).toBe(true);

  await app.press("id:run:eBIRForms > Sign Out");
  await app.waitFor("text:gate=closed");
  await app.waitFor("text:settings search=");
  expect(app.texts()).not.toContain("settings search=juana");
  // The filing menus went with the gate.
  await app.waitUntil(() => !app.shows("id:run:File > New Return…"));
});
