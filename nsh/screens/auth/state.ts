// The state of the account gate, first-run setup and the sign-in screen (old BirApp gate*, setup, signIn, the OTP states and
// their actions in app/main.js), one owner: this module. The frame's session (who is signed in, locked or not) is
// shell/app.ts; these screens only change it through app.startSession, app.finishUnlock and app.setSetup.
import { batch, signal } from "nsh";
import { errorText } from "../../lib/errors";
import { app, go, store, toast, type Session } from "../../shell";
import { codeLengthError, signInTarget, wrongCode } from "./logic";

// ---------------------------------------------------------------- sign-in

const tin = signal("");
const method = signal<"pin" | "totp">("pin");
const forgot = signal(false);
const busy = signal(false);
const error = signal<string | null>(null);
const tpPin = signal("");
const tpTotp = signal("");
const adminPin = signal("");
const adminTotp = signal("");
const recovery = signal("");

function clearCodes(): void {
  batch(() => {
    for (const s of [tpPin, tpTotp, adminPin, adminTotp, recovery]) s.set("");
  });
}

function fail(message: string): void {
  batch(() => {
    error.set(message);
    clearCodes();
  });
}

/** Signed in: the frame takes over; the screen is left clean for the next lock. */
function begin(session: Session): void {
  batch(() => {
    error.set(null);
    forgot.set(false);
    method.set("pin");
    tin.set("");
    clearCodes();
  });
  app.startSession(session);
}

function target() {
  return signInTarget(
    tin(),
    app.authState().admin_root ?? null,
    store.taxpayers().map((t) => t.tin_root),
  );
}

/** TIN plus that account's own PIN or authenticator code. */
function submitSignIn(): void {
  const t = target();
  if ("error" in t) return fail(t.error);
  const totp = method() === "totp";
  const code = totp ? tpTotp() : tpPin();
  const short = codeLengthError(totp, code);
  if (short) return fail(short);
  let ok = false;
  try {
    if (t.admin) ok = totp ? store.call("verify_admin_totp", code) : store.call("verify_admin_pin", code);
    else ok = totp ? store.call("verify_taxpayer_totp", t.root, code) : store.call("verify_taxpayer_pin", t.root, code);
  } catch (e) {
    // Too many wrong tries (the host's limiter), or the check itself failed: say which.
    return fail(errorText(e));
  }
  if (!ok) {
    let none = false;
    if (!t.admin) {
      const c = store.taxpayerCreds(t.root);
      none = !c.has_pin && !c.has_totp;
    }
    return fail(wrongCode(totp, none));
  }
  begin(t.admin ? { kind: "admin" } : { kind: "taxpayer", root: t.root });
}

/** Why the admin's PIN or authenticator code as typed does not open; null when one matches. */
function adminCodeError(): string | null {
  const a = app.authState();
  try {
    if (a.has_admin_pin && adminPin().length === 4 && store.call("verify_admin_pin", adminPin())) return null;
    if (a.has_admin_totp && adminTotp().length === 6 && store.call("verify_admin_totp", adminTotp())) return null;
  } catch (e) {
    return errorText(e);
  }
  return "Wrong admin PIN or code. Try again.";
}

/** "Forgot PIN? Ask the admin": the admin's code opens the taxpayer typed above. */
function submitOverride(): void {
  const t = target();
  if ("error" in t) return fail(t.error);
  if (t.admin) return fail("That is the admin's TIN. Use the recovery key or your Mac login below.");
  const why = adminCodeError();
  if (why) return fail(why);
  begin({ kind: "taxpayer", root: t.root });
  toast("Opened by the admin. Set a new PIN in Settings.", "lock");
}

function recoveredAdmin(message: string): void {
  begin({ kind: "admin" });
  go("settings");
  toast(message, "lock");
}

/** The admin's one-time recovery key: signs the admin in, then a new PIN is set in Settings. */
function submitRecoveryKey(): void {
  const key = recovery().trim();
  if (!key) return fail("Type the recovery key you wrote down when the admin PIN was set.");
  let ok = false;
  try {
    ok = store.write("use_recovery_key", key);
  } catch (e) {
    return fail(errorText(e));
  }
  if (!ok) return fail("That recovery key does not match. Check it and try again.");
  recoveredAdmin("Recovery key used. It works only once: set a new admin PIN and make a new key.");
}

/** Last resort: whoever can log in to this Mac can reset the admin PIN. The OS prompt runs off the engine thread. */
async function submitMacLogin(): Promise<void> {
  if (busy()) return;
  batch(() => {
    busy.set(true);
    error.set(null);
  });
  let ok = false;
  let why: string | null = null;
  try {
    ok = await store.call("mac_login", "reset the eBIRForms admin PIN");
  } catch (e) {
    why = errorText(e);
  }
  busy.set(false);
  if (!ok) return fail(why || "Mac login was cancelled or did not match.");
  recoveredAdmin("Signed in with your Mac login. Set a new admin PIN now.");
}

/** The admin has no code, so there is nothing to sign in with: the owner is at the keyboard. */
function openWithoutSignIn(): void {
  // The host's own "no code" (asked now), never the cached read whose fallback is "no code" too.
  if (app.adminCodeNow() !== false) return;
  begin({ kind: "open" });
}

function toggleMethod(): void {
  batch(() => {
    method.set(method() === "totp" ? "pin" : "totp");
    error.set(null);
    clearCodes();
  });
}

function toggleForgot(): void {
  batch(() => {
    forgot.set(!forgot());
    error.set(null);
    clearCodes();
  });
}

// ---------------------------------------------------------------- first launch

const setupPin = signal("");
const setupKey = signal<string | null>(null);
let chosenHelpdeskPin: string | null = null;

/** "Just me or my business": the owner's taxpayer becomes the admin account; with none yet it is typed first. */
function chooseSolo(): void {
  const first = store.taxpayers()[0];
  if (first) {
    try {
      store.write("setup_solo", first.tin_root);
    } catch (e) {
      toast(errorText(e), "triangle-alert");
    }
    return;
  }
  // No taxpayer yet: New taxpayer, with no chrome; Create finishes the setup (SoloTaxpayer watches for it).
  go("new_taxpayer");
  app.setSetup("solo", "choose");
}

/** The New taxpayer page was left during first launch: back to the choice. */
function cancelSetup(): void {
  batch(() => {
    error.set(null);
    app.setSetup(null, "choose");
  });
}

/** The first taxpayer exists and first launch chose "solo": it is the owner, the admin. */
function finishSoloSetup(root: string): void {
  if (app.setup().pending !== "solo") return;
  try {
    store.write("setup_solo", root);
  } catch (e) {
    toast(errorText(e), "triangle-alert");
  }
  app.setSetup(null, "choose");
}

/** "I help other taxpayers": choose the admin PIN, then the recovery key is shown once. */
function chooseHelpdesk(): void {
  batch(() => {
    setupPin.set("");
    error.set(null);
    app.setSetup(null, "helpdesk-pin");
  });
}

function submitHelpdeskPin(): void {
  const pin = setupPin();
  if (pin.length !== 4) return fail("Type all 4 digits of the admin PIN.");
  try {
    // Back from the recovery key returns here to correct the PIN. The help desk is already written then, so this updates it
    // and goes forward to the same key rather than starting the setup again.
    if (setupKey() !== null) {
      if (chosenHelpdeskPin !== pin) {
        store.write("set_admin_pin", pin);
        chosenHelpdeskPin = pin;
      }
      batch(() => {
        error.set(null);
        app.setSetup(null, "helpdesk-key");
      });
      return;
    }
    const key = store.write("setup_helpdesk", pin).recovery_key;
    chosenHelpdeskPin = pin;
    batch(() => {
      setupKey.set(key);
      error.set(null);
      app.setSetup(null, "helpdesk-key");
    });
  } catch (e) {
    error.set(errorText(e));
  }
}

/**
 * Android's Back in first-run setup: the PIN page returns to the choice until its setup is written; the recovery-key page
 * returns to the PIN. Once the key exists there is no way back to the choice (the help desk is already written), so Back on
 * the PIN page then only keeps the key page's way forward from leaving the app.
 */
function back(): boolean {
  const step = app.setup().step;
  if (step === "helpdesk-pin") {
    if (setupKey() !== null) return true;
    cancelSetup();
    return true;
  }
  if (step === "helpdesk-key") {
    app.setSetup(null, "helpdesk-pin");
    return true;
  }
  return false;
}

/** The recovery key was written down: the admin starts on an empty Home. */
function finishHelpdeskSetup(): void {
  // Only the key screen right after choosing the PIN signs the admin in.
  if (app.setup().step !== "helpdesk-key") return;
  batch(() => {
    setupKey.set(null);
    setupPin.set("");
    chosenHelpdeskPin = null;
    app.setSetup(null, "choose");
  });
  begin({ kind: "admin" });
}

// ---------------------------------------------------------------- the dev start route

/**
 * Steps of BIR_SHELL_START for these screens (old runAccessStep): "setup=solo|helpdesk", "setuppin=<pin>", "keydone", "lock",
 * "tin=<root>", "forgot", "signin=<root>:<pin>", "override=<admin pin>", "recover=<key>". Each types into the same boxes a person
 * does and presses the same button: every code is still checked by the host. The route is read only in a test environment
 * (shell/app.ts boot), and nothing here logs a PIN, a code or the recovery key: the screen shows the key once, as for a person.
 */
function accessStep(key: string, value: string): void {
  const [a, b = ""] = value.split(":");
  if (key === "setup") return a === "solo" ? chooseSolo() : chooseHelpdesk();
  if (key === "setuppin") {
    setupPin.set(a);
    return submitHelpdeskPin();
  }
  if (key === "keydone") return finishHelpdeskSetup();
  if (key === "lock") return app.lockApp();
  if (key === "tin") return tin.set(a);
  if (key === "forgot") return toggleForgot();
  if (key === "signin") {
    tin.set(a);
    tpPin.set(b);
    return submitSignIn();
  }
  if (key === "override") {
    adminPin.set(a);
    return submitOverride();
  }
  if (key === "recover") {
    recovery.set(value);
    return submitRecoveryKey();
  }
}
app.setAccessSteps(accessStep);

// ---------------------------------------------------------------- the handle

const ro = <T,>(s: () => T) => () => s();

/** What the auth screens read (accessors) and do (actions). */
export const auth = {
  tin: ro(tin),
  method: ro(method),
  forgot: ro(forgot),
  busy: ro(busy),
  error: ro(error),
  tpPin: ro(tpPin),
  tpTotp: ro(tpTotp),
  adminPin: ro(adminPin),
  adminTotp: ro(adminTotp),
  recovery: ro(recovery),
  setupPin: ro(setupPin),
  setupKey: ro(setupKey),
  /** The TIN as typed: up to 9 digits; typing clears the error (the cells below depend on whose TIN it is). */
  setTin(digits: string): void {
    batch(() => {
      tin.set(digits);
      tin.set(digits.slice(0, 9));
      error.set(null);
    });
  },
  setTpPin: (v: string) => tpPin.set(v),
  setTpTotp: (v: string) => tpTotp.set(v),
  setAdminPin: (v: string) => adminPin.set(v),
  setAdminTotp: (v: string) => adminTotp.set(v),
  setRecovery: (v: string) => recovery.set(v),
  setSetupPin: (v: string) => setupPin.set(v),
  submitSignIn,
  submitOverride,
  submitRecoveryKey,
  submitMacLogin,
  openWithoutSignIn,
  toggleMethod,
  toggleForgot,
  chooseSolo,
  cancelSetup,
  back,
  finishSoloSetup,
  chooseHelpdesk,
  submitHelpdeskPin,
  finishHelpdeskSetup,
  accessStep,
};
