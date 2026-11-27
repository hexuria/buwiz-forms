// The injected decision interface of the owner lock and Set PIN pages. B3 implements it over the B1 host verbs;
// the pages themselves know nothing about the store, the network or the app's real lock.

export type BiometricKind = "none" | "face_id" | "touch_id" | "windows_hello";

export interface LockState {
  account_masked: string;
  biometric: BiometricKind;
  wait_secs: number;
  tries_left: number;
  lost: boolean;
}

export type VerifyResult =
  | { kind: "ok" }
  | { kind: "wrong"; tries_left: number }
  | { kind: "wait"; secs: number }
  | { kind: "lost" };

export type MaybePromise<T> = T | Promise<T>;

export interface LockInterface {
  lockState(): LockState;
  verify(pin: string): MaybePromise<VerifyResult>;
  set(pin: string): MaybePromise<void>;
  biometric(): void;
  forgot(): void;
  signOut(): void;
  clientSignIn(): void;
}
