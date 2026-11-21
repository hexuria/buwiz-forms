// returnsSending (actions.ts): the quit warning's count. It reads the frame's scoped store, never the door directly: a taxpayer session
// counts its own Queued returns (store.returns applies canSee), and behind a lock it counts nothing (as app.ts queuedKey). A plain
// test: the filed list is put in the store's cache, as FilingsDemo's generated row is.
import { batch } from "nsh";
import { expect, test } from "nsh/test";
import type { ReturnRow } from "../../bir";
import { app } from "../app";
import { store } from "../store";
import { returnsSending } from "./actions";

const row = (id: number, root: string, status: string): ReturnRow => ({
  id, tin: root + "00000", tin_root: root, branch_code: "00000", branch_name: "Head office", taxpayer_name: "T " + root, branch_count: 1,
  form_code: "1601C", taxable_year: 2026, period: 10, period_label: "October 2026", status, updated_at: "2026-11-14 09:30:00", return_key: id, submitted_via_relay: false,
});

test("the quit warning counts the Queued returns the session may see, and none behind a lock", () => {
  const filed = [row(1, "000123453", "Queued"), row(2, "000999991", "Queued"), row(3, "000123453", "Submitted")];
  batch(() => {
    store.forget("returns:filed");
    store.read("returns:filed", () => filed, []);
  });
  // Every taxpayer's (the admin's session): both Queued returns.
  expect(returnsSending()).toBe(2);
  // A taxpayer session narrows the store: only its own.
  store.setScope("000123453");
  expect(returnsSending()).toBe(1);
  store.setScope(null);
  expect(returnsSending()).toBe(2);
  // Behind the account gate nothing is read for nobody.
  app.lockCloud();
  expect(returnsSending()).toBe(0);
});
