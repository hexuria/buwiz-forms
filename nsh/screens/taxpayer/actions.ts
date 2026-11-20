// The taxpayer screens' shared state and the writes that need no form (old TaxpayerActions): which profile sections are
// open, the "Replace what is set?" question, the inbox's progress line, the export in progress; setups, templates,
// archive, delete, send grants and the confirmation inbox. One owner: this module (private signals, read-only accessors).
import { batch, signal } from "nsh";
import type { Setup } from "../../bir";
import { errorText } from "../../lib/errors";
import { app, go, store, toast } from "../../shell";
import { afterApply, checkText, templateErrorText, type ReplaceAsk, type SectionId } from "./logic";

// ---------------------------------------------------------------- state

/** The profile sections open as a first visit finds them. */
const firstVisit = (): Record<SectionId, boolean> => ({ taxpayer: true, branch: true, taxtypes: true, facts: true, address: true, inbox: true, agent: false });
const openSecs = signal<Record<SectionId, boolean>>(firstVisit());
const replaceAsk = signal<ReplaceAsk | null>(null);
const inboxMsg = signal<{ ok: boolean; text: string } | null>(null);
const inboxBusy = signal(false);
const exportBusy = signal(false);

/** Read-only views of the module's state. */
export const shared = {
  open: (id: SectionId) => openSecs()[id],
  replaceAsk: () => replaceAsk(),
  inboxMsg: () => inboxMsg(),
  inboxBusy: () => inboxBusy(),
  exportBusy: () => exportBusy(),
};

export function setSectionOpen(id: SectionId, on: boolean): void {
  openSecs.set({ ...openSecs(), [id]: on });
}

/** Taxpayer or branch switched, or the year: the question and the inbox's line no longer apply. */
export function resetShared(): void {
  batch(() => {
    replaceAsk.set(null);
    inboxMsg.set(null);
    inboxBusy.set(false);
  });
}

/** The year switched: the question was about the other year. */
export function leaveYearShared(): void {
  replaceAsk.set(null);
}

// The next session starts from the defaults (a lock, a sign-out or another taxpayer signing in): the sections as a first visit
// finds them, no question or inbox line about the last taxpayer. (An open edit lives in the page, which goes with the lock.)
app.onSessionChange(() => {
  resetShared();
  openSecs.set(firstVisit());
});

const fail = (e: unknown) => toast(errorText(e), "triangle-alert");

// ---------------------------------------------------------------- setups

/** Applies a setup to the branch's year: its forms and facts. Facts already set are kept; the page then asks once. */
export function applySetup(setup: Setup): boolean {
  const b = app.branch();
  const year = app.year();
  if (!b || !year) return false;
  try {
    const r = store.write("apply_setup", b.tin, year, setup.id, false);
    const done = afterApply(r, { kind: "setup", tin: b.tin, year, id: setup.id, label: setup.name });
    replaceAsk.set(done.ask);
    toast(done.toast, "layout-template");
    return true;
  } catch (e) {
    fail(e);
    return false;
  }
}

/** A new taxpayer's "start from": the setup or the copy, then the same question. Throws what the host throws. */
export function startFromChoice(kind: "setup" | "copy", headTin: string, year: number, opts: { id?: string; from?: string; label: string }): void {
  const r =
    kind === "copy" ? store.write("copy_setup_from", opts.from as string, headTin, year, false) : store.write("apply_setup", headTin, year, opts.id as string, false);
  const done = afterApply(r, { kind, tin: headTin, year, id: opts.id, from: opts.from, label: opts.label });
  replaceAsk.set(done.ask);
  toast(done.toast, "layout-template");
}

/** "Replace what is set?" answered Replace: the same setup or copy again, replacing. */
export function confirmReplace(): void {
  const a = replaceAsk();
  if (!a) return;
  replaceAsk.set(null);
  try {
    if (a.kind === "copy") store.write("copy_setup_from", a.from as string, a.tin, a.year, true);
    else store.write("apply_setup", a.tin, a.year, a.id as string, true);
    toast(`Replaced with ${a.label}.`);
  } catch (e) {
    fail(e);
  }
}

/** "Keep mine". */
export function keepSet(): void {
  replaceAsk.set(null);
}

export function deleteSetup(setup: Setup): void {
  try {
    store.write("delete_setup", setup.id);
    toast(`Setup ${setup.name} deleted.`, "trash");
  } catch (e) {
    fail(e);
  }
}

// ---------------------------------------------------------------- templates

export function makeTemplate(code: string): void {
  const b = app.branch();
  if (!b) return;
  try {
    store.write("create_template_from_latest", b.tin, code);
    toast(`${code} template saved from the newest return.`, "layout-template");
  } catch (e) {
    toast(templateErrorText(code, errorText(e)), "triangle-alert");
  }
}

export function deleteTemplate(code: string): void {
  const b = app.branch();
  if (!b) return;
  try {
    store.write("delete_form_template", b.tin, code);
    toast(`${code} template deleted.`, "trash");
  } catch (e) {
    fail(e);
  }
}

/** "Use as template" on a filed return (Home's and Filings' row action: old home.js). */
export function useAsTemplate(row: { id: number; form_code: string; period_label: string }): void {
  try {
    store.write("create_template_from_return", row.id);
    toast(`${row.form_code} template saved from ${row.period_label}.`, "layout-template");
  } catch (e) {
    fail(e);
  }
}

// ---------------------------------------------------------------- archive and delete

/** Archive and delete: the admin only, never on the admin's own account. */
export function canRemove(): boolean {
  const t = app.taxpayer();
  return Boolean(t) && app.isAdmin() && t?.tin_root !== app.authState().admin_root;
}

export function toggleArchive(): void {
  const t = app.taxpayer();
  if (!t) return;
  try {
    store.write("set_taxpayer_archived", t.tin_root, !t.archived);
    toast(t.archived ? `${t.name} is back in new work.` : `${t.name} archived.`, "archive");
  } catch (e) {
    fail(e);
  }
}

/** Deletes the taxpayer (the page arms it first); the frame moves to the next taxpayer and Home. */
export function deleteTaxpayer(): boolean {
  const t = app.taxpayer();
  if (!t) return false;
  try {
    store.write("delete_taxpayer", t.tin_root);
  } catch (e) {
    fail(e);
    return false;
  }
  const next = store.taxpayers()[0];
  if (next) app.selectTaxpayer(next.tin_root, { quiet: true });
  go("home");
  toast(`${t.name} deleted.`, "trash");
  return true;
}

// ---------------------------------------------------------------- export (the host's save dialog)

/**
 * Exports this profile through the host's save dialog. The archive never carries credentials; a taxpayer with a PIN
 * gives it here, and the caller clears its cells after every attempt. A cancelled dialog is quiet.
 */
export async function exportProfile(pin: string | null): Promise<void> {
  if (exportBusy()) return;
  const b = app.branch();
  const t = app.taxpayer();
  if (!b || !t) return;
  const hasPin = Boolean(store.taxpayerCreds(t.tin_root).has_pin);
  if (hasPin && (pin ?? "").length !== 4) {
    toast("Type all 4 digits of the profile PIN first.", "lock");
    return;
  }
  exportBusy.set(true);
  try {
    const r = await store.writeAsync("export_profile", b.tin, hasPin ? pin : null);
    if (r.kind === "exported") toast(r.path?.startsWith("Downloads/eBIRForms/") ? `Saved to ${r.path}` : "Profile exported.", "circle-check");
  } catch (e) {
    fail(e);
  } finally {
    exportBusy.set(false);
  }
}

// ---------------------------------------------------------------- send grants

/** Turns the assistant's standing OK for one form on or off. */
export function setGrant(tin: string, code: string, on: boolean): void {
  try {
    if (on) store.writeFor(`grants:${tin}`, "grant_send", tin, code);
    else store.writeFor(`grants:${tin}`, "revoke_send", tin, code);
    toast(on ? `Assistant may file ${code}.` : `Assistant may no longer file ${code}.`);
  } catch (e) {
    fail(e);
  }
}

// ---------------------------------------------------------------- confirmation inbox

export function setTracking(tin: string, on: boolean): void {
  try {
    store.write("set_inbox_tracking", tin, on);
    toast(on ? "Automated receipt tracking on." : "Automated receipt tracking off.");
  } catch (e) {
    fail(e);
  }
}

export function disconnectInbox(tin: string): void {
  try {
    store.write("disconnect_inbox_oauth", tin);
    toast("Google account disconnected for every profile sharing this inbox.");
  } catch (e) {
    fail(e);
  }
}

/** One inbox job at a time: a line while it runs, its answer after. */
async function inboxJob(start: string, run: () => Promise<{ ok: boolean; text: string; icon?: string }>): Promise<void> {
  if (inboxBusy()) return;
  batch(() => {
    inboxBusy.set(true);
    inboxMsg.set({ ok: true, text: start });
  });
  try {
    const r = await run();
    inboxMsg.set(r.ok && r.icon === "key-round" ? null : { ok: r.ok, text: r.text });
    toast(r.text, r.icon ?? (r.ok ? "circle-check" : "triangle-alert"));
  } catch (e) {
    inboxMsg.set({ ok: false, text: errorText(e) });
    fail(e);
  } finally {
    store.bump();
    inboxBusy.set(false);
  }
}

/** The browser sign-in with Google; the host runs it and answers when the grant is stored. */
export function connectInbox(tin: string): Promise<void> {
  return inboxJob("Google sign-in opened in your browser…", async () => {
    await store.call("connect_inbox_oauth", tin);
    return { ok: true, text: "Google account connected. Email tracking is on.", icon: "key-round" };
  });
}

/** A real login to the mailbox: success turns tracking on. */
export function verifyInbox(tin: string): Promise<void> {
  return inboxJob("Testing connection…", async () => {
    const r = await store.call("test_inbox_connection", tin);
    if (!r.ok) store.logScreen("error", `inbox verify: ${r.message}`);
    return { ok: r.ok, text: r.message };
  });
}

/** One fetch of the mailbox for BIR confirmations. */
export function checkInboxNow(tin: string): Promise<void> {
  return inboxJob("Checking the inbox for BIR confirmations…", async () => {
    const r = await store.call("check_inbox_now", tin);
    const text = checkText(r);
    if (!r.ok) store.logScreen("error", `inbox check: ${text}`);
    return { ok: r.ok, text };
  });
}
