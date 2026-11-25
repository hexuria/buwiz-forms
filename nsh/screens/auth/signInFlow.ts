// The Buwiz sign-in, one flow for the account gate and Settings > Account. One owner: this module's signals. The screens draw
// its state (signInCards.tsx) and call its actions; neither keeps a copy.
//
// The flow is: the email page (one page for signing in and creating an account), the code page (six boxes that check by
// themselves), Home. Nothing else, and nothing switches to a browser. The first tokens come through EmailPath (the address,
// then the emailed code; the host keeps the pending request, so a relaunch opens on the code page). TokenPath is the seam
// for a second way to the first tokens (a later change); nothing implements or draws one now. Every way ends in `approve`.
import { batch, signal } from "nsh";
import type { CloudAccountStatus, CloudEmailCheck, CloudEmailStep } from "../../bir";
import { errorText } from "../../lib/errors";
import { app, store } from "../../shell";

/** A sentence under the email field or the code boxes. */
export interface Said {
  text: string;
  danger: boolean;
}

/** The code page's request: the address, the code's length, and when the code expires and Resend unlocks (ms, Date.now). */
export interface CodeRequest {
  email: string;
  length: number;
  expiresAt: number;
  resendAt: number;
}

/**
 * Every state of the sign-in. `idle` is the email page before anything happened (on the gate; in Settings the account row);
 * `email` is the email page asked for (Settings' Sign in, Change email, a request that ended) with a sentence or none;
 * `sending` is its button's request. `code` is the code page: `checking` while its request runs, `resending` while Resend's
 * does, `said` a wrong code, expiry or the like, `tooMany` when the boxes are spent until Resend. `approved` lasts no longer
 * than the screen takes to move on (Home, or the account in Settings).
 */
export type SignInState =
  | { kind: "idle" }
  | { kind: "email"; said: Said | null }
  | { kind: "sending" }
  | { kind: "code"; request: CodeRequest; checking: boolean; resending: boolean; said: Said | null; tooMany: boolean }
  | { kind: "approved"; status: CloudAccountStatus };

/** The email-and-code path: what the host's calls answer (crates/bir-app-api/src/cloud_email.rs). */
export interface EmailPath {
  start(email: string): Promise<CloudEmailStep>;
  verify(code: string): Promise<CloudEmailCheck>;
  resend(): Promise<CloudEmailStep>;
  /** Ends the pending request (Change email). */
  cancel(): void;
  /** The request kept from before a relaunch, or null. */
  pending(): CloudEmailStep | null;
}

/** The email path through the host. */
export const hostEmailPath: EmailPath = {
  start: (email) => store.call("cloud_account_email_start", email) as Promise<CloudEmailStep>,
  verify: (code) => store.call("cloud_account_email_verify", code) as Promise<CloudEmailCheck>,
  resend: () => store.call("cloud_account_email_resend") as Promise<CloudEmailStep>,
  cancel: () => void store.call("cloud_account_email_cancel"),
  pending: () => store.call("cloud_account_email_pending") as CloudEmailStep | null,
};

/**
 * The seam for a second way to the first tokens (a later change: a sign-in in the browser that comes back by itself, with no
 * code). It answers once the host has saved them; `signInWith` then ends it as the email path ends. Nothing implements or draws
 * one in this version.
 */
export interface TokenPath {
  obtain(): Promise<{ outcome: "approved" | "cancelled" | "expired"; status: CloudAccountStatus }>;
  cancel(): void;
}

const state = signal<SignInState>({ kind: "idle" });
/** The address as typed on the email page (kept for Change email). */
const emailText = signal("");
/** The code as typed in the boxes. */
const codeText = signal("");
/** The clock the code page counts down by (ms), moved each second while the code page shows. */
const now = signal(Date.now());
/** Each request is a new run; an answer of an older run (after Change email, a reset) changes nothing. */
let run = 0;
let email: EmailPath = hostEmailPath;
let restored = false;
let ticker: ReturnType<typeof setInterval> | null = null;
const approvedHooks: ((status: CloudAccountStatus) => void)[] = [];

/** The state, with the code page's clock running only while it shows. */
function setState(next: SignInState): void {
  state.set(next);
  if (next.kind === "code") now.set(Date.now());
  if (next.kind === "code" && !ticker) {
    ticker = setInterval(() => now.set(Date.now()), 1000);
  } else if (next.kind !== "code" && ticker) {
    clearInterval(ticker);
    ticker = null;
  }
}

/** Signed in: the session opens (the gate goes Home) and whoever listens hears the account. */
function approve(status: CloudAccountStatus): void {
  setState({ kind: "approved", status });
  for (const hook of approvedHooks) hook(status);
}

/** The seam's end (see TokenPath): another way's tokens open the session as the email path's do. */
async function signInWith(path: TokenPath): Promise<SignInState> {
  const mine = ++run;
  const got = await path.obtain();
  if (mine !== run) return state();
  if (got.outcome === "approved" && (await app.finishUnlock())) approve(got.status);
  else setState({ kind: "email", said: got.outcome === "expired" ? { text: "The sign-in expired. Try again.", danger: false } : null });
  return state();
}

/** Why the address as typed is not worth a request, or null. The server has the last word (`invalid_email`). */
export function emailProblem(text: string): string | null {
  const t = text.trim();
  if (!t) return "Enter your email address.";
  const at = t.indexOf("@");
  const domain = t.slice(at + 1);
  if (at < 1 || t.includes(" ") || domain.includes("@") || !domain.includes(".") || domain.startsWith(".") || domain.endsWith(".")) {
    return "That email address does not look right. Check it and try again.";
  }
  return null;
}

/** The code page's request from a host answer, counted from now. */
function requestFrom(step: CloudEmailStep): CodeRequest {
  const at = Date.now();
  return { email: step.email, length: step.code_length || 6, expiresAt: at + step.expires_in * 1000, resendAt: at + step.resend_in * 1000 };
}

function codePage(request: CodeRequest, said: Said | null = null, tooMany = false): SignInState {
  return { kind: "code", request, checking: false, resending: false, said, tooMany };
}

/** Continue on the email page: the code is sent and the code page opens, or the page says why not (and stays). */
async function sendCode(): Promise<void> {
  const current = state().kind;
  if (current === "sending" || current === "code") return;
  const problem = emailProblem(emailText());
  if (problem) return setState({ kind: "email", said: { text: problem, danger: true } });
  const mine = ++run;
  batch(() => {
    setState({ kind: "sending" });
    app.setGateError(null);
  });
  try {
    const step = await email.start(emailText().trim());
    if (mine !== run) return;
    if (step.step === "code") {
      codeText.set("");
      setState(codePage(requestFrom(step)));
    } else setState({ kind: "email", said: { text: step.message ?? "Sign-in is not available right now. Try again in a few minutes.", danger: true } });
  } catch (error) {
    if (mine === run) setState({ kind: "email", said: { text: errorText(error), danger: true } });
  }
}

/** The code as typed; the last box checks it by itself. */
function typeCode(digits: string): void {
  codeText.set(digits);
  const page = state();
  if (page.kind === "code" && page.said && digits.length > 0) setState({ ...page, said: page.tooMany ? page.said : null });
}

/**
 * The sixth digit (typed, pasted or autofilled): the code is checked once ("Checking…" while its request runs, the boxes
 * locked, a second call ignored). Accepted: Home at once.
 */
async function checkCode(digits: string): Promise<void> {
  const current = state();
  if (current.kind !== "code" || current.checking || current.tooMany || digits.length !== current.request.length) return;
  const mine = run;
  codeText.set(digits);
  setState({ ...current, checking: true, said: null });
  try {
    const checked = await email.verify(digits);
    if (mine !== run) return;
    const page = state();
    if (page.kind !== "code") return;
    const said = (fallback: string): Said => ({ text: checked.message ?? fallback, danger: true });
    if (checked.outcome === "approved") {
      if (await app.finishUnlock()) approve(checked.status);
      else setState({ kind: "email", said: { text: app.gateError() ?? "The sign-in could not be opened. Try again.", danger: true } });
      return;
    }
    if (checked.outcome === "restart") {
      setState({ kind: "email", said: said("This sign-in request has ended. Start again.") });
      return;
    }
    // An answer the server never gave (no connection, a challenge page, the outer limit) keeps what was typed.
    if (checked.outcome !== "error") codeText.set("");
    if (checked.outcome === "expired") {
      const at = Date.now();
      const request = { ...page.request, expiresAt: Math.min(page.request.expiresAt, at), resendAt: Math.min(page.request.resendAt, at) };
      setState(codePage(request, said("This code has expired. Press Resend for a new one.")));
    } else if (checked.outcome === "too-many") setState(codePage(page.request, said("Too many wrong codes. Send a new code to try again."), true));
    else setState(codePage(page.request, said(checked.outcome === "wrong" ? "That code is not right. Try again." : "No connection. Check your internet and try again.")));
  } catch (error) {
    const page = state();
    if (mine === run && page.kind === "code") setState(codePage(page.request, { text: errorText(error), danger: true }, page.tooMany));
  }
}

/** Resend, once the code has expired: a new code for the same request, the countdown again. */
async function resendCode(): Promise<void> {
  const current = state();
  if (current.kind !== "code" || current.resending || current.checking || now() < current.request.resendAt) return;
  const mine = run;
  setState({ ...current, resending: true });
  try {
    const step = await email.resend();
    if (mine !== run) return;
    if (step.step === "code") {
      codeText.set("");
      setState(codePage(requestFrom(step), { text: `We sent a new code to ${step.email}.`, danger: false }));
    } else if (step.step === "restart") setState({ kind: "email", said: { text: step.message ?? "This sign-in request has ended. Start again.", danger: true } });
    else {
      const page = state();
      if (page.kind !== "code") return;
      const resendAt = step.resend_in > 0 ? Date.now() + step.resend_in * 1000 : page.request.resendAt;
      setState(codePage({ ...page.request, resendAt }, { text: step.message ?? "No connection. Check your internet and try again.", danger: true }, page.tooMany));
    }
  } catch (error) {
    const page = state();
    if (mine === run && page.kind === "code") setState(codePage(page.request, { text: errorText(error), danger: true }, page.tooMany));
  }
}

/**
 * "Send a new code", after too many wrong tries: the old code is dead, so a new request for the same address starts at once (no
 * countdown to wait for) and a fresh code page opens; a refusal is said on the page.
 */
async function newCode(): Promise<void> {
  const current = state();
  if (current.kind !== "code" || !current.tooMany || current.resending) return;
  const mine = ++run;
  setState({ ...current, resending: true });
  try {
    const step = await email.start(current.request.email);
    if (mine !== run) return;
    if (step.step === "code") {
      codeText.set("");
      setState(codePage(requestFrom(step), { text: `We sent a new code to ${step.email}.`, danger: false }));
    } else setState({ ...current, resending: false, said: { text: step.message ?? "No connection. Check your internet and try again.", danger: true } });
  } catch (error) {
    if (mine === run) setState({ ...current, resending: false, said: { text: errorText(error), danger: true } });
  }
}

/** Change email: the request ends and the email page comes back with the address to correct. */
function changeEmail(): void {
  run++;
  email.cancel();
  codeText.set("");
  setState({ kind: "email", said: null });
}

/** Once per run of the app: a request kept from before a relaunch opens the code page, with what is left of its countdowns. */
function restore(): void {
  if (restored) return;
  restored = true;
  if (state().kind !== "idle") return;
  try {
    const step = email.pending();
    if (step && step.step === "code") {
      emailText.set(step.email);
      setState(codePage(requestFrom(step)));
    }
  } catch {
    // Nothing kept, or it cannot be read: the email page.
  }
}

/** "Resend in 4:59" while locked; "Resend" once the code has expired. */
export function resendLabel(request: CodeRequest, at: number): string {
  const left = Math.ceil((request.resendAt - at) / 1000);
  if (left <= 0) return "Resend";
  return `Resend in ${Math.floor(left / 60)}:${String(left % 60).padStart(2, "0")}`;
}

/** The code page's sentence under the boxes: what the state says, or the expiry once the clock passes it. */
export function codeSaid(page: Extract<SignInState, { kind: "code" }>, at: number): Said | null {
  if (page.said) return page.said;
  if (at >= page.request.expiresAt) return { text: "This code has expired. Press Resend for a new one.", danger: true };
  return null;
}

/** The flow's state and actions. */
export const signInFlow = {
  state: (): SignInState => state(),
  /** The email page's address and the code boxes' digits. */
  emailText: (): string => emailText(),
  setEmail: (text: string): void => {
    emailText.set(text);
    const current = state();
    if (current.kind === "email" && current.said) setState({ kind: "email", said: null });
  },
  codeText: (): string => codeText(),
  now: (): number => now(),
  /** The email page asked for (Settings' Sign in). */
  openEmail: (): void => {
    if (state().kind === "idle") setState({ kind: "email", said: null });
  },
  sendCode,
  typeCode,
  checkCode,
  resendCode,
  newCode,
  changeEmail,
  restore,
  signInWith,
  /** Hears every approval (Settings shows the account). */
  onApproved: (hook: (status: CloudAccountStatus) => void): void => {
    approvedHooks.push(hook);
  },
  /** Back to the email page before anything happened (a signed-out session, a test). */
  reset: (): void => {
    run++;
    codeText.set("");
    setState({ kind: "idle" });
  },
  /** Another email path (a test). */
  useEmailPath: (next: EmailPath): void => {
    email = next;
  },
};

/** The email page's sentence under the field, for the state: a refusal or a bad address, or nothing. */
export function emailSaid(now: SignInState): Said | null {
  return now.kind === "email" ? now.said : null;
}

/** Which page the state draws. */
export function pageOf(now: SignInState): "email" | "code" {
  return now.kind === "code" ? "code" : "email";
}
