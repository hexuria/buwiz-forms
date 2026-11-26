// Settings > Account > Buwiz account (old account.js AccountActions): the cloud account's status, the refresh when Settings
// opens, sign in (the gate) and sign out. The sign-in pages are the gate's only; these are the same host calls the old card
// made, and the gate's seam on the frame: app.finishUnlock and app.lockCloud. One owner: this file's private signals.
import { signal } from "nsh";
import type { CloudAccountStatus } from "../../bir";
import { errorText } from "../../lib/errors";
import { app, store } from "../../shell";
import { signInFlow } from "../auth/signInFlow";

const status = signal<CloudAccountStatus | null>(null);
const busy = signal(false);
const failure = signal<string | null>(null);
let freshStarted = false;

function offlineStatus(detail: string): CloudAccountStatus {
  return { server_url: "", server_url_source: "", account_url: "", signed_in: false, email: null, user_id: null, offline: false, practice: false, detail };
}

/** The local status (no network): the saved sign-in and the server address. */
function load(): void {
  try {
    status.set(store.call("cloud_account_status"));
  } catch (error) {
    status.set(offlineStatus(errorText(error)));
    failure.set(errorText(error));
  }
}

/** The account's state, read-only. */
export const cloud = {
  status: (): CloudAccountStatus => status() ?? offlineStatus(""),
  busy: (): boolean => busy(),
  failure: (): string | null => failure(),
};

/** The card is on screen: read the status, and once per run check a saved sign-in with the server. */
export function openAccount(): void {
  load();
  if (freshStarted || !status()?.signed_in || busy()) return;
  freshStarted = true;
  void refresh();
}

/** Asks the server about the saved sign-in; offline keeps what is saved. */
export async function refresh(): Promise<void> {
  try {
    status.set(await store.call("cloud_account_refresh"));
    failure.set(null);
  } catch (error) {
    failure.set(errorText(error));
  }
}

// Every approval of the sign-in flow shows the account here.
signInFlow.onApproved((approved) => {
  status.set(approved);
  failure.set(null);
});

/** Sign in from Settings (signed out: a development state): the gate, the one place to sign in (screens/auth/Gate.tsx). */
export function signIn(): void {
  if (busy()) return;
  failure.set(null);
  app.lockCloud();
}

export async function signOut(): Promise<void> {
  if (busy()) return;
  busy.set(true);
  failure.set(null);
  try {
    status.set(await store.call("cloud_account_sign_out"));
    failure.set(null);
    signInFlow.reset();
    app.lockCloud();
  } catch (error) {
    failure.set(errorText(error));
  }
  busy.set(false);
}
