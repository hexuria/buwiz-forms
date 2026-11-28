// The injected decision interface of the owner lock and Set PIN pages. Its lock-state fields mirror B1's
// owner_lock_state; B3 implements verify() by mapping verify_owner_pin's boolean or wait/lost error.

export type BiometricKind = "none" | "face_id" | "touch_id" | "windows_hello";

/** B1's owner_lock_state fields, exactly. */
export interface LockState {
  room_open: boolean;
  store_lost: boolean;
  has_pin: boolean;
  set_at: string | null;
  unlocked: boolean;
  failures: number;
  wait_secs: number;
}

/** Presentation facts the page cannot know from B1's store state. */
export interface LockPresentation {
  account_masked: string;
  biometric: BiometricKind;
}

export type VerifyResult =
  | { kind: "ok" }
  | { kind: "wrong" }
  | { kind: "wait"; secs: number }
  | { kind: "lost" };

export type MaybePromise<T> = T | Promise<T>;

export interface LockInterface {
  lockState(): LockState & LockPresentation;
  verify(pin: string): MaybePromise<VerifyResult>;
  set(pin: string): MaybePromise<void>;
  biometric(): void;
  forgot(): void;
  signOut(): void;
  clientSignIn(): void;
}

/** The only account rendering the lock page may use: two letters, bullets, and the domain. */
export function maskAccount(account: string): string {
  const at = account.lastIndexOf("@");
  if (at < 1) return "•••";
  const local = account.slice(0, at);
  const domain = account.slice(at + 1);
  const firstTwo = Array.from(local).slice(0, 2).join("");
  return `${firstTwo}•••@${domain}`;
}
