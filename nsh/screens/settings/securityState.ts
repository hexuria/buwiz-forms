// Settings > Account > Security (old security.js SecurityActions): the signed-in account's own PIN and authenticator, and the
// admin's recovery key. Whose codes these are follows the session: a taxpayer edits its own (one PIN and one authenticator per
// taxpayer, shared by every branch); the admin edits the admin codes, which in "Just me" mode are the owner taxpayer's own and
// on a help desk belong to the reserved admin account. The host refuses a change that would leave a help desk's admin with no
// way in. Half-typed codes and a recovery key on screen live here and are forgotten when Settings is left (leave()).
import { signal } from "nsh";
import { errorText } from "../../lib/errors";
import { app, go, store, toast } from "../../shell";

const pin = signal("");
const totpCode = signal("");
const totpSecret = signal<string | null>(null);
const recoveryKey = signal<string | null>(null);

/** Whose codes Settings edits, and what they already have. */
export type SecurityTarget = { admin: true; has_pin: boolean; has_totp: boolean } | { admin: false; root: string; has_pin: boolean; has_totp: boolean };

/** The account whose codes Settings edits: the admin, or the signed-in taxpayer. */
export function securityTarget(): SecurityTarget {
  if (app.isAdmin()) {
    const a = app.authState();
    return { admin: true, has_pin: Boolean(a.has_admin_pin), has_totp: Boolean(a.has_admin_totp) };
  }
  const s = app.session();
  const root = s.kind === "taxpayer" ? s.root : "";
  const creds = store.taxpayerCreds(root);
  return { admin: false, root, has_pin: Boolean(creds.has_pin), has_totp: Boolean(creds.has_totp) };
}

/** What the Security card shows of the typing, read-only. */
export const security = {
  pin: (): string => pin(),
  setPin: (v: string): void => pin.set(v),
  totpCode: (): string => totpCode(),
  setTotpCode: (v: string): void => totpCode.set(v),
  /** The authenticator key being added (until it is confirmed or cancelled), or null. */
  totpSecret: (): string | null => totpSecret(),
  /** A recovery key just made: shown until Settings is left, never again. */
  recoveryKey: (): string | null => recoveryKey(),
};

/** Forgets anything half-typed and the recovery key on screen (Settings was left). */
export function leaveSecurity(): void {
  pin.set("");
  totpCode.set("");
  totpSecret.set(null);
  recoveryKey.set(null);
}
// Nor does it outlive the session (a lock, a sign-out or another taxpayer signing in): the next person never sees a half-typed code.
app.onSessionChange(leaveSecurity);

function fail(error: unknown): void {
  toast(errorText(error), "triangle-alert");
}

function makeRecoveryKey(): void {
  recoveryKey.set(store.write("create_recovery_key").recovery_key);
}

/**
 * The admin's first code was just set. Nobody signed in until now (no code existed), so the person at the keyboard becomes
 * the signed-in admin, and gets a recovery key if there is none yet. The frame's session seam is startSession, which also
 * starts from Home: Settings is opened again on the Account section, then the key is made (so it is shown on that page).
 */
async function afterAdminCode(): Promise<void> {
  if (app.session().kind === "open") {
    app.startSession({ kind: "admin" });
    go("settings", { section: "account" });
    await app.settled();
  }
  if (!app.authState().has_recovery_key) makeRecoveryKey();
}

export function setPin(): void {
  const typed = pin();
  if (typed.length !== 4) return void toast("Type all 4 digits of the new PIN first.", "lock");
  const t = securityTarget();
  try {
    if (t.admin) store.write("set_admin_pin", typed);
    else store.write("set_taxpayer_pin", t.root, typed);
    pin.set("");
    if (t.admin) afterAdminCode().catch(fail);
    toast(t.has_pin ? "PIN changed." : "PIN saved. Sign-in asks for it from now on.", "lock");
  } catch (error) {
    fail(error);
  }
}

export function clearPin(): void {
  const t = securityTarget();
  try {
    if (t.admin) store.write("set_admin_pin", null);
    else store.write("set_taxpayer_pin", t.root, null);
    toast("PIN removed.", "lock");
  } catch (error) {
    fail(error);
  }
}

/** Makes a key for the authenticator app; the code it shows confirms it. */
export function startTotp(): void {
  const t = securityTarget();
  const who = t.admin ? (app.isHelpdesk() ? "admin" : app.taxpayer()?.tin_display || "admin") : app.taxpayer()?.tin_display || t.root;
  try {
    totpSecret.set(store.call("generate_totp_secret", `eBIRForms ${who}`).secret);
    totpCode.set("");
  } catch (error) {
    fail(error);
  }
}

export function confirmTotp(): void {
  const t = securityTarget();
  const secret = totpSecret();
  if (!secret) return;
  try {
    if (t.admin) store.write("set_admin_totp", secret, totpCode());
    else store.write("set_taxpayer_totp", t.root, secret, totpCode());
    totpSecret.set(null);
    totpCode.set("");
    if (t.admin) afterAdminCode().catch(fail);
    toast("Authenticator added.", "lock");
  } catch (error) {
    fail(error);
  }
}

export function cancelTotp(): void {
  totpSecret.set(null);
  totpCode.set("");
}

export function clearTotp(): void {
  const t = securityTarget();
  try {
    if (t.admin) store.write("set_admin_totp", null);
    else store.write("set_taxpayer_totp", t.root, null);
    toast("Authenticator removed.", "lock");
  } catch (error) {
    fail(error);
  }
}

/** A new recovery key; the old one stops working. */
export function newRecoveryKey(): void {
  try {
    makeRecoveryKey();
  } catch (error) {
    fail(error);
  }
}
