import { describe, expect, test } from "nsh/test";
import { shellChromeVisible } from "./chrome_gate";
import type { ChromeState } from "./chrome_gate";

// Translated from tools/test_shell_chrome.mjs; the filing chrome stays hidden until setup is finished and sign-in is done.
const ready: ChromeState = {
  booted: true,
  cloudLocked: false,
  appLocked: false,
  mode: "solo",
  setupPending: null,
  setupStep: "choose",
};

describe("shellChromeVisible", () => {
  test("a finished, signed-in app shows its chrome, solo or help desk", () => {
    expect(shellChromeVisible(ready)).toBe(true);
    expect(shellChromeVisible({ ...ready, mode: "helpdesk" })).toBe(true);
  });
  test("nothing shows before boot, behind the account gate, or on the sign-in screen", () => {
    expect(shellChromeVisible({ ...ready, booted: false })).toBe(false);
    expect(shellChromeVisible({ ...ready, cloudLocked: true })).toBe(false);
    expect(shellChromeVisible({ ...ready, appLocked: true })).toBe(false);
  });
  test("no mode yet (who will use this computer) hides it", () => {
    expect(shellChromeVisible({ ...ready, mode: null })).toBe(false);
    expect(shellChromeVisible({ ...ready, mode: "" })).toBe(false);
  });
  test("a setup still pending (the first taxpayer) hides it; an empty pending step does not", () => {
    expect(shellChromeVisible({ ...ready, mode: null, setupPending: "solo" })).toBe(false);
    expect(shellChromeVisible({ ...ready, setupPending: "solo" })).toBe(false);
    expect(shellChromeVisible({ ...ready, setupPending: "" })).toBe(true);
  });
  test("the recovery key step, and the PIN step Back returns to from it, hide it until the key is written down", () => {
    expect(shellChromeVisible({ ...ready, mode: "helpdesk", setupStep: "helpdesk-key" })).toBe(false);
    expect(shellChromeVisible({ ...ready, mode: "helpdesk", setupStep: "helpdesk-pin" })).toBe(false);
    expect(shellChromeVisible({ ...ready, mode: "helpdesk", setupStep: "choose" })).toBe(true);
  });
});
