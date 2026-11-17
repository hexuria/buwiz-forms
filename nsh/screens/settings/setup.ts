// Settings > Computer > Computer setup (old setup_change.js SetupChange; admin only): change who this computer is set up for
// after first launch. "Just me or my business": hand the owner (the admin account) to another taxpayer, or switch to a help
// desk with a new admin PIN. A help desk: make one of the taxpayers the owner. The host re-checks the current admin's PIN or
// authenticator code and refuses a change that would leave nobody able to sign in. Every change makes a new recovery key,
// shown once, and then signs out so the new admin signs in with their own code. One owner: this file's private signals.
import { batch, signal } from "nsh";
import type { Taxpayer } from "../../bir";
import { errorText } from "../../lib/errors";
import { app, store, toast } from "../../shell";

/** Where the change is: closed, or one of its steps. */
export type SetupStep = null | "choose" | "owner" | "helpdesk" | "confirm" | "key";
/** What the change will do. */
export type SetupTarget = { kind: "owner"; root: string; name: string; tin_display: string } | { kind: "helpdesk" };
/** A taxpayer that could become the owner, and whether they can sign in yet (they have a PIN or an authenticator). */
export type Candidate = Taxpayer & { ready: boolean };

const step = signal<SetupStep>(null);
const target = signal<SetupTarget | null>(null);
const useCode = signal(false);
const recoveryKey = signal<string | null>(null);
const newPin = signal("");
const confirmPin = signal("");
const confirmCode = signal("");

/** The change's state, read-only, and the typed codes with their setters. */
export const setupChange = {
  step: (): SetupStep => step(),
  target: (): SetupTarget | null => target(),
  useCode: (): boolean => useCode(),
  recoveryKey: (): string | null => recoveryKey(),
  newPin: (): string => newPin(),
  setNewPin: (v: string): void => newPin.set(v),
  confirmPin: (): string => confirmPin(),
  setConfirmPin: (v: string): void => confirmPin.set(v),
  confirmCode: (): string => confirmCode(),
  setConfirmCode: (v: string): void => confirmCode.set(v),
};

function clearTyped(): void {
  newPin.set("");
  confirmPin.set("");
  confirmCode.set("");
}

/** Closes the change and forgets everything typed. */
export function resetSetup(): void {
  batch(() => {
    step.set(null);
    target.set(null);
    useCode.set(false);
    recoveryKey.set(null);
    clearTyped();
  });
}

// A change in progress does not outlive the session (a lock, a sign-out or another taxpayer signing in): nobody else sees its codes.
app.onSessionChange(resetSetup);

export function goStep(next: SetupStep): void {
  step.set(next);
}

/** The computer's sign-out after a change: the admin moved, so the old session must not carry on (and nobody keeps their place). */
function signOutAfterChange(): void {
  app.signOutAfterSetupChange();
}

/** Settings was left. A finished change has already moved the admin: sign out even if "I wrote it down" was never pressed. */
export function leaveSetup(): void {
  const changed = step() === "key";
  resetSetup();
  if (changed) signOutAfterChange();
}

/** The current owner's name and TIN, or null on a help desk. */
export function currentOwner(): { tin_root: string; name: string; tin_display: string } | null {
  if (app.isHelpdesk()) return null;
  const root = app.authState().admin_root ?? "";
  return app.taxpayers().find((t) => t.tin_root === root) ?? { tin_root: root, name: "", tin_display: root };
}

/** What "Set up for" says now. */
export function setupSummary(): string {
  const owner = currentOwner();
  if (!owner) return "A help desk. The admin signs in with TIN 000-000-000";
  return `Just me or my business. Owner: ${owner.name ? `${owner.name}, ` : ""}TIN ${owner.tin_display}`;
}

/** Taxpayers that could become the owner, with whether they can sign in yet. */
export function candidates(): Candidate[] {
  const current = app.isHelpdesk() ? null : (app.authState().admin_root ?? null);
  return app
    .taxpayers()
    .filter((t) => t.tin_root !== current && !t.archived)
    .map((t) => {
      const c = store.taxpayerCreds(t.tin_root);
      return { ...t, ready: Boolean(c.has_pin || c.has_totp) };
    });
}

export function pickOwner(t: Taxpayer): void {
  batch(() => {
    target.set({ kind: "owner", root: t.tin_root, name: t.name, tin_display: t.tin_display });
    step.set("confirm");
  });
}

export function pickHelpdeskPin(): void {
  if (newPin().length !== 4) return void toast("Type all 4 digits of the new admin PIN.", "lock");
  batch(() => {
    target.set({ kind: "helpdesk" });
    step.set("confirm");
  });
}

export function toggleCode(): void {
  batch(() => {
    useCode.set(!useCode());
    confirmPin.set("");
    confirmCode.set("");
  });
}

/** The host re-checks the admin's PIN or code, makes the change, and answers a new recovery key (shown once). */
export function confirmChange(): void {
  const t = target();
  if (!t) return;
  const pin = useCode() ? null : confirmPin();
  const code = useCode() ? confirmCode() : null;
  if (!useCode() && (pin ?? "").length !== 4) return void toast("Type all 4 digits of your current admin PIN.", "lock");
  if (useCode() && (code ?? "").length !== 6) return void toast("Type all 6 digits of the code.", "lock");
  try {
    const result = t.kind === "owner" ? store.write("change_owner", t.root, pin, code) : store.write("change_to_helpdesk", pin, code, newPin());
    batch(() => {
      recoveryKey.set(result.recovery_key);
      clearTyped();
      step.set("key");
    });
  } catch (error) {
    batch(() => {
      confirmPin.set("");
      confirmCode.set("");
    });
    toast(errorText(error), "triangle-alert");
  }
}

/** "I wrote it down": the change is over and the computer signs out. */
export function finishChange(): void {
  resetSetup();
  signOutAfterChange();
}
