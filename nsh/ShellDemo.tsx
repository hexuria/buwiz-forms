// A test fixture, not a screen: the frame over the demo taxpayer (Juana Santos), for shell/*.test.tsx and shots/shell.test.tsx.
// What a test asks for goes in `open(entry, { host: { params } })`:
//   demo   default true: create the demo taxpayer; false: an empty database
//   years  default true: give her the current tax year with 1601C and 2551Q; false: no tax year yet
//   branch true: add a second branch (00001, Cebu branch)       second true: add a second taxpayer (generated: Sample Trading Corp)
//   theme  the stored theme setting before the app starts ("Dark")
//   start  the dev route ("view=library;theme=dark")             poll true: run the polls
//   filed  true: two filed returns appear after the start (one submitted, one confirmed): the notifications
//   show   "setting" prints the stored theme setting; "update" prints the update status phase (both live)
// It sits at the nsh root because a real-window run serves assets (fonts, icons) from the entry's folder down.
import { effect, host, mount, Text, untracked, View } from "nsh";
import "./theme/fonts";
import { bir, type ReturnRow } from "./bir";
import { createDemoTaxpayer, warmForms } from "./bir/testing";
import { app } from "./shell/app";
import { Shell } from "./shell/Shell";
import { store } from "./shell/store";

interface Params {
  demo?: boolean;
  years?: boolean;
  branch?: boolean;
  second?: boolean;
  theme?: string;
  start?: string;
  poll?: boolean;
  filed?: boolean;
  show?: "setting" | "update";
}

// A start route that opens the library, a form or the palette builds every form's schema in the frame's first render (the host does it lazily,
// once per process): more than the 2 s a headless run gives one JS call on a slow machine, where a bundle's load has 10 s. Any other start
// leaves it to whoever needs it, so the frame is not made to wait for what it does not use.
if (/view=library|form=|palette=/.test(String((host.params as { start?: string } | null)?.start ?? ""))) warmForms();

function seed(p: Params) {
  const taxpayer = createDemoTaxpayer();
  const head = taxpayer.branches[0];
  if (p.years !== false && bir.list_tax_years(head.tin).length === 0) {
    const year = new Date().getFullYear();
    bir.add_tax_year(head.tin, year, false);
    bir.set_yearly_forms(head.tin, year, ["1601C", "2551Q"]);
  }
  if (p.branch && !taxpayer.branches.some((b) => b.code === "00001")) bir.create_branch(taxpayer.tin_root, { branch_code: "00001", name: "Cebu branch" });
  if (p.second && !bir.list_taxpayers().some((t) => t.tin_root === "000111225")) {
    bir.create_taxpayer({ tin_root: "000-111-225", name: "Sample Trading Corp", taxpayer_type: "Corporation", head_office: { rdo_code: "039", registered_address: "Quezon City", zip_code: "1100" } });
  }
}

/**
 * Four filed returns of the demo taxpayer (one sent to the mock relay, one only simulated: the worker's dry run, `send.stage`
 * "test_sent") (generated rows): the list the notifications are made of.
 */
const FILED: ReturnRow[] = [
  { id: 9001, tin: "00012345300000", tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 1, form_code: "2551Q", taxable_year: 2025, period: 1, period_label: "Q1 2025", status: "Submitted", updated_at: "2025-04-20 09:30:00", return_key: null, submitted_via_relay: false, send: null },
  { id: 9002, tin: "00012345300000", tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 1, form_code: "1601C", taxable_year: 2025, period: 3, period_label: "March 2025", status: "Confirmed", updated_at: "2025-04-10 09:30:00", return_key: null, submitted_via_relay: false, send: null },
  { id: 9003, tin: "00012345300000", tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 1, form_code: "1601C", taxable_year: 2025, period: 4, period_label: "April 2025", status: "Submitted", updated_at: "2025-05-10 09:30:00", return_key: null, submitted_via_relay: true, send: null },
  { id: 9004, tin: "00012345300000", tin_root: "000123453", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Juana Santos", branch_count: 1, form_code: "2551Q", taxable_year: 2025, period: 2, period_label: "Q2 2025", status: "Submitted", updated_at: "2025-07-20 09:30:00", return_key: null, submitted_via_relay: false, send: { stage: "test_sent", line: "Test send. Nothing reached the BIR.", code: null, next_try_at: null, can_resolve: false } },
];

function Demo() {
  const p = host.params as Params;
  if (p.demo !== false) seed(p);
  // The filed list is put in the store's cache once the frame is up, so they are news (the first look at a database is not).
  if (p.filed) {
    effect(() => {
      if (!app.booted()) return;
      untracked(() => {
        store.forget("returns:filed");
        store.read("returns:filed", () => FILED, []);
      });
    });
  }
  if (p.theme) bir.set_setting("theme_preference", p.theme);
  return (
    <View class="flex-1">
      <Shell start={p.start} polling={p.poll === true} />
      <If when={p.show === "setting"}>
        <View class="absolute bottom-0 left-0">
          <Text testID="demo-readout">{`theme setting: ${store.setting("theme_preference")}`}</Text>
        </View>
      </If>
      <If when={p.show === "update"}>
        <View class="absolute bottom-0 left-0">
          <Text testID="demo-readout">{`update: ${app.updateStatus()?.phase ?? "none"}`}</Text>
        </View>
      </If>
    </View>
  );
}

mount(Demo);
