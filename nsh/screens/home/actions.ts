// What a Home row does when pressed: open a return, cancel a queue, use a filed return as a template, start an as-needed
// return. Each goes through the shell (navigation, selection, toasts) and the store's writes, as the old home.js did through
// app.openForm, app.cancelQueuedEntry, app.taxpayerUi.useAsTemplate and app.startNewReturn.
import type { AsNeededForm, AsNeededReturn, DeadlineRow, ReturnRow } from "../../bir";
import { errorText } from "../../lib/errors";
import { app, cancelQueue, openReturn, store, toast } from "../../shell";

/** Opens the return a deadline row names (a saved one opens exactly: two in one month stay two). */
export function openDeadline(d: DeadlineRow): void {
  openReturn({ tin: d.tin, code: d.form_code, year: d.taxable_year, period: d.period, returnKey: d.return_key });
}

/** Opens a filed or saved return from the Recently filed list. */
export function openListed(r: ReturnRow): void {
  openReturn({ tin: r.tin, code: r.form_code, year: r.taxable_year, period: r.period, returnKey: r.return_key });
}

/** "Cancel queue" on a queued row: the return is an editable draft again, or "This return was already sent." and the lists read again. */
export function cancelQueued(r: ReturnRow): void {
  cancelQueue(r.tin, r.form_code, r.taxable_year, r.period || null);
}

/** "Use as template" on a filed return: its answers become the form's template for the branch. */
export function useAsTemplate(r: ReturnRow): void {
  try {
    store.write("create_template_from_return", r.id);
    toast(`${r.form_code} template saved from ${r.period_label}.`, "layout-template");
  } catch (e) {
    toast(errorText(e), "triangle-alert");
  }
}

/** The frame is on the as-needed row's taxpayer, branch and year (old select()), so what opens next is theirs. */
function select(f: AsNeededForm): void {
  app.selectTaxpayer(f.tin_root, { quiet: true, code: f.branch_code });
  app.selectYear(f.taxable_year);
}

/** "Start a return" on an as-needed form: the start-a-return dialog with the form chosen. */
export function startAsNeeded(f: AsNeededForm): void {
  select(f);
  app.startNewReturn(f.form_code);
}

/** "Open" on an as-needed return that holds no trigger date yet. */
export function openUndated(f: AsNeededForm, r: AsNeededReturn): void {
  select(f);
  openReturn({ tin: f.tin, code: f.form_code, year: f.taxable_year, period: r.period, returnKey: r.return_key });
}
