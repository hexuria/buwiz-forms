// Taxpayer page profile export, driven through the real
// app/shell/taxpayer.js with a mocked `bir` host: PIN cells appear only when
// taxpayer_auth reports has_pin; Export sends export_profile(branch tin,
// pin|null); a typed PIN is cleared from the cells after every attempt and
// never kept in page state; success toasts "Profile exported.", a host
// refusal toasts its message, and a cancelled dialog is quiet. The verb
// answers a promise (the save panel opens after the click returns): Export
// is disabled while it is pending and a second click opens nothing.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods, strictElement } from "./kit_surface.mjs";

const root = new URL("../", import.meta.url);
const KIT = kitMethods(root);

const meta = new WeakMap();
function el(info = {}) {
  const e = strictElement(KIT, (k, args, self) => {
    if (k === "child") meta.get(self).kids.push(args[0]);
    if (k === "children") meta.get(self).kids.push(...args[0]);
    if (k === "when" && args[0]) args[1](self);
  });
  meta.set(e, { info, kids: [] });
  return e;
}
function nodes(e, out = []) {
  const m = meta.get(e);
  if (!m) return out;
  out.push(m);
  for (const k of m.kids) nodes(k, out);
  return out;
}
const find = (tree, pred) => nodes(tree).filter((m) => pred(m.info));
const click = (m) => m.info.onClick?.({}, cx);

const kidsOf = (e, kids) => {
  meta.get(e).kids.push(...kids.filter(Boolean));
  return e;
};

const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24, 28, 32], RADIUS: { lg: 8, md: 6, sm: 4 } };
uiExports.Text = (_cx, _v, text) => el({ text: String(text) });
uiExports.Code = (_cx, code) => el({ code });
uiExports.Line = (_cx, _v, text) => el({ text: String(text) });
uiExports.Icon = () => el();
uiExports.IconButton = (_cx, id, _icon, opts = {}) => el({ button: id, onClick: opts.onClick });
uiExports.AppButton = (_cx, id, label, opts = {}) => el({ button: id, label, disabled: Boolean(opts.disabled), onClick: opts.onClick });
uiExports.Avatar = () => el();
uiExports.Chip = (_cx, chip) => el({ chip });
uiExports.Segmented = (_cx, id, options, selected, onChange) => el({ segmented: id, options, selected, onChange });
uiExports.Switch = (_cx, id) => el({ switch: id });
uiExports.CheckboxLabel = () => el({ checkboxLabel: true });
uiExports.Checkbox = (_cx, id) => el({ checkbox: id });
uiExports.TextField = (_cx, _s, o = {}) => el({ field: o.id });
uiExports.Dropdown = (_cx, id) => el({ dropdown: id });
uiExports.Field = (_cx, label, opts = {}) => kidsOf(el({ field: label }), [opts.input, opts.inheritSwitch]);
uiExports.FieldGrid = (_cx, _w, cells) => kidsOf(el({ fieldGrid: true }), cells.map((c) => c && c.el));
uiExports.Disclosure = (_cx, id, opts = {}) => kidsOf(el({ disclosure: id, title: opts.title }), [opts.body]);
uiExports.DisclosureGroup = (_cx, id, sections) => kidsOf(el({ group: id }), sections);
uiExports.FormActions = (_cx, id, onCancel, onSave) => el({ formActions: id, onCancel, onSave });
uiExports.ListGroup = (_cx, id, rows) => kidsOf(el({ listGroup: id }), rows);
uiExports.ListRow = (_cx, id) => el({ row: id });
uiExports.NameCell = (_cx, title, sub) => el({ title, sub });
uiExports.EmptyRow = (_cx, id, text) => el({ empty: id, text });
uiExports.SettingRow = (_cx, id, title, desc, control) => kidsOf(el({ setting: id, title, desc }), [control]);
uiExports.Card = (_cx, id, rows) => kidsOf(el({ card: id }), rows);
uiExports.SectionTitle = (_cx, label) => el({ section: label });
uiExports.PageHeader = (_cx, opts) => kidsOf(el({ header: opts.title }), [opts.meta, ...(opts.actions || [])]);
uiExports.MetaLine = (_cx, parts) => kidsOf(el({ metaLine: true }), parts);
uiExports.Tabs = (_cx, id) => el({ tabs: id });
uiExports.Banner = (_cx, id, kids) => kidsOf(el({ banner: id }), kids);
uiExports.DigitCells = (_cx, _e, id) => el({ digitCells: id });
uiExports.PinCells = (_cx, _state, id) => el({ pinCells: id });
uiExports.ComboField = (_cx, _s, o = {}) => el({ field: o.id });
uiExports.DigitEntry = class DigitEntry {
  constructor(_shape, v = "") {
    this.v = v;
  }
  value() {
    return this.v;
  }
  isComplete() {
    return false;
  }
};
uiExports.ComboState = class ComboState {
  constructor(_opts, v = "") {
    this.v = v;
  }
  value() {
    return this.v;
  }
  isKnown() {
    return false;
  }
};

class InputState {
  static new(o = {}) {
    const s = new InputState();
    s.v = o.value || "";
    return s;
  }
  value() {
    return this.v;
  }
  set_value(v) {
    this.v = v;
  }
  on() {}
}
class OtpState {
  static new(len) {
    const s = new OtpState();
    s.len = len;
    s.v = "";
    return s;
  }
  value() {
    return this.v;
  }
  set_value(v) {
    this.v = v;
  }
}

const storeExports = {
  errorText: (e) => (e && e.message) || String(e),
  formatTin: (root, code) => `${root}${code ? `-${code}` : ""}`,
  shortDate: (d) => String(d),
  daysUntil: () => 0,
};
const libraryExports = { searchText: () => "" };

const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Proxy, WeakMap, Boolean });
const mock = (name, exports) =>
  new vm.SyntheticModule(Object.keys(exports), function () {
    for (const [k, v] of Object.entries(exports)) this.setExport(k, v);
  }, { context: contextVM, identifier: name });
const source = (path) => new vm.SourceTextModule(fs.readFileSync(new URL(path, root), "utf8"), { context: contextVM, identifier: path });
const taxpayerMod = source("app/shell/taxpayer.js");
const linked = {
  "gpui-kit": mock("gpui-kit", { div: () => el(), View: class View {} }),
  "gpui-base": mock("gpui-base", { InputState, OtpState }),
  "./ui.js": mock("./ui.js", uiExports),
  "./store.js": mock("./store.js", storeExports),
  "./library.js": mock("./library.js", libraryExports),
  "./facts.js": source("app/shell/facts.js"),
};
await taxpayerMod.link((s) => linked[s]);
await taxpayerMod.evaluate();
const { TaxpayerActions, renderTaxpayerPage, profileExportCard } = taxpayerMod.namespace;

const TIN = "12345678800000";
const ROOT = "123456788";
/** Tasks `cx.spawn` started; `settle()` waits for them all. */
const tasks = [];
const cx = { notify: () => {}, write_to_clipboard: () => {}, spawn: (f) => tasks.push(f(cx)) };
const settle = async () => {
  while (tasks.length) await tasks.shift();
};

/**
 * An app showing one head-office taxpayer; `export_profile` is recorded and
 * answers `exportResult` (or the promise `exportPending`), or rejects with
 * `exportError`.
 */
function makeApp({ hasPin, exportResult, exportError, exportPending }) {
  const calls = [];
  const toasts = [];
  const t = { name: "Example Trading Corp", tin_root: ROOT, taxpayer_type: "Corporation", archived: false, branches: [{ is_head_office: true }] };
  const b = {
    tin: TIN,
    is_head_office: true,
    code: "00000",
    name: "Head office",
    facts: { rdo_code: "050", registered_address: "1 Main St", zip_code: "1226", phone: "", email: "a@b.c" },
    inherited: [],
  };
  const app = {
    taxpayer: () => t,
    branch: () => b,
    taxpayerCreds: (r) => (r === ROOT ? { has_pin: hasPin, has_totp: false } : { has_pin: false, has_totp: false }),
    isAdmin: () => false,
    adminRoot: () => "",
    toast: (msg, _cx, icon) => toasts.push({ msg, icon }),
    year: () => 2026,
    yearForms: () => [],
    templates: () => [],
    go: () => {},
    years: () => [],
    startNewReturn: () => {},
    store: {
      inboxState: () => ({ auth_method: "GoogleOAuth", oauth_connected: false, tracking_enabled: false, last_check_at: null }),
      sendGrants: () => [],
      yearlyForms: () => [],
      yearFacts: () => null,
      taxTypes: () => [],
      screenIndex: () => [],
      writeAsync: async (name, ...args) => {
        calls.push([name, ...args]);
        if (exportPending) return exportPending;
        if (exportError) throw exportError;
        return exportResult;
      },
      call: () => null,
      read: () => null,
    },
  };
  app.taxpayerUi = new TaxpayerActions(app);
  return { app, calls, toasts };
}

const exportBtn = (tree) => find(tree, (i) => i.button === "tp-export-btn")[0];
const pinCells = (tree) => find(tree, (i) => i.pinCells === "tp-export-pin");
const exportTree = (app) => profileExportCard(cx, app, app.taxpayer());

// The profile page no longer draws export; Settings does.
{
  const { app } = makeApp({ hasPin: false, exportResult: { kind: "exported" } });
  const page = renderTaxpayerPage(cx, app, "profile", 900);
  assert.equal(exportBtn(page), undefined, "export moved off the tax profile page");
}

// No PIN configured: a direct Export button, no PIN cells.
{
  const { app, calls, toasts } = makeApp({ hasPin: false, exportResult: { kind: "exported", path: "/tmp/p.zip" } });
  const tree = exportTree(app);
  assert.equal(pinCells(tree).length, 0, "no PIN cells without a configured PIN");
  const btn = exportBtn(tree);
  assert.ok(btn, "Export button rendered");
  click(btn);
  await settle();
  assert.deepEqual(calls, [["export_profile", TIN, null]]);
  assert.deepEqual(toasts.map((x) => x.msg), ["Profile exported."]);
}

// PIN configured: PIN cells render, and Export sends the typed digits.
{
  const { app, calls, toasts } = makeApp({ hasPin: true, exportResult: { kind: "exported", path: "/tmp/p.zip" } });
  const tree = exportTree(app);
  assert.equal(pinCells(tree).length, 1, "PIN cells render when has_pin");
  app.taxpayerUi.exportOtp.set_value("4321");
  click(exportBtn(tree));
  await settle();
  assert.deepEqual(calls, [["export_profile", TIN, "4321"]]);
  assert.equal(app.taxpayerUi.exportOtp.value(), "", "PIN cells cleared after the attempt");
  assert.deepEqual(toasts.map((x) => x.msg), ["Profile exported."]);
}

// PIN configured: the host's refusal is toasted and the cells still clear.
{
  const { app, calls, toasts } = makeApp({ hasPin: true, exportError: new Error("Wrong PIN.") });
  const tree = exportTree(app);
  assert.equal(pinCells(tree).length, 1);
  app.taxpayerUi.exportOtp.set_value("0000");
  click(exportBtn(tree));
  await settle();
  assert.deepEqual(calls, [["export_profile", TIN, "0000"]]);
  assert.equal(app.taxpayerUi.exportOtp.value(), "", "PIN cells cleared after a refusal");
  assert.deepEqual(toasts.map((x) => x.msg), ["Wrong PIN."]);
}

// PIN configured: fewer than four digits never reaches the host (a partial
// PIN would still burn a rate-limited attempt); the cells clear.
{
  const { app, calls, toasts } = makeApp({ hasPin: true, exportResult: { kind: "exported" } });
  exportTree(app);
  app.taxpayerUi.exportOtp.set_value("12");
  app.taxpayerUi.exportProfile(cx);
  await settle();
  assert.equal(calls.length, 0, "partial PIN never reaches the host");
  assert.equal(app.taxpayerUi.exportOtp.value(), "");
  assert.deepEqual(toasts.map((x) => x.msg), ["Type all 4 digits of the profile PIN first."]);
}

// A cancelled save dialog is quiet: no toast, no export claim.
{
  const { app, calls, toasts } = makeApp({ hasPin: false, exportResult: { kind: "cancelled" } });
  const tree = exportTree(app);
  click(exportBtn(tree));
  await settle();
  assert.deepEqual(calls, [["export_profile", TIN, null]]);
  assert.equal(toasts.length, 0, "cancelled dialog makes no toast");
  assert.equal(exportBtn(exportTree(app)).info.disabled, false, "Export enabled again after a cancel");
}

// While the save panel is open: Export is disabled, a second click opens
// nothing, and the button comes back once the panel answers.
{
  let answer;
  const exportPending = new Promise((resolve) => (answer = resolve));
  const { app, calls, toasts } = makeApp({ hasPin: false, exportPending });
  assert.equal(exportBtn(exportTree(app)).info.disabled, false, "Export enabled before the click");
  click(exportBtn(exportTree(app)));
  assert.equal(exportBtn(exportTree(app)).info.disabled, true, "Export disabled while the panel is open");
  app.taxpayerUi.exportProfile(cx);
  assert.equal(calls.length, 1, "no second panel while one is open");
  answer({ kind: "exported", path: "/tmp/p.zip" });
  await settle();
  assert.deepEqual(toasts.map((x) => x.msg), ["Profile exported."]);
  assert.equal(exportBtn(exportTree(app)).info.disabled, false, "Export enabled after the panel answers");
}

// ---- The income type: where the 2551Q card sends a filer whose year has no
// self-employed or mixed-income type. The old Registration section is gone;
// the Facts section's income-type row opens in place, nothing is chosen for
// the filer, the save goes through set_year_facts for the return's year and
// then back to the return.
{
  const { app, toasts } = makeApp({ hasPin: false });
  const disclosures = (tree) => find(tree, (i) => i.disclosure).map((m) => m.info.disclosure);
  assert.ok(!disclosures(renderTaxpayerPage(cx, app, "profile", 900)).includes("tp-sec-registration"), "no Registration section");

  app.taxpayer().taxpayer_type = "Individual";
  const writes = [];
  const went = [];
  let refuse = null;
  const view = {
    tin: TIN,
    year: 2026,
    facts: { tax_classification: null, income_tax_election: null, eopt_tier: null, business_start_date: null, line_of_business: null, is_government_withholding_entity: false, atc_codes: [] },
    owner: {},
    used_by: { tax_classification: ["2551Q"], income_tax_election: ["2551Q"], eopt_tier: ["2551Q"], is_government_withholding_entity: [], atc_codes: [], birth_date: [], business_start_date: ["2551Q"], line_of_business: [] },
    not_set: ["tax_classification", "eopt_tier"],
    election_locked: false,
    election_source: null,
  };
  app.years = () => [2026];
  app.store.yearFacts = () => view;
  app.store.taxTypes = () => [];
  app.store.screenIndex = () => [];
  app.store.electionReadiness = () => ({ gap: "not_business_income" });
  app.store.write = (name, ...args) => {
    writes.push([name, ...args]);
    if (refuse) throw new Error(refuse);
    return true;
  };
  app.branchFor = () => app.branch();
  app.history = ["home", "form"];
  app.go = (v) => went.push(["go", v]);
  app.goBack = () => went.push(["back"]);
  app.selectYear = () => {};

  const ui = app.taxpayerUi;
  ui.startRegistration({ tin: TIN, year: 2026, fromForm: true }, cx);
  assert.deepEqual(went, [["go", "profile"]], "the card opens the Profile tab");
  assert.equal(ui.factEdit.fact, "tax_classification", "with the income type open");
  assert.equal(ui.factEdit.value, null, "and nothing chosen for the filer");
  const page = renderTaxpayerPage(cx, app, "profile", 900);
  const choice = find(page, (i) => i.segmented === "tp-fact-tax_classification-choice")[0];
  assert.deepEqual(Array.from(choice.info.options, (o) => o.value), ["SelfEmployed", "MixedIncome", "PurelyCompensation"], "an individual's income types");
  ui.saveFactEdit(cx);
  assert.equal(writes.length, 0, "nothing is sent before a choice");
  assert.equal(ui.factEdit.error, "Choose one first.");
  ui.pickFact("SelfEmployed", cx);
  refuse = "Other error: The tax profile cannot take it.";
  ui.saveFactEdit(cx);
  assert.equal(ui.factEdit.error, "The tax profile cannot take it.", "a refusal in plain words");
  refuse = null;
  ui.saveFactEdit(cx);
  assert.equal(JSON.stringify(writes.at(-1)), JSON.stringify(["set_year_facts", TIN, 2026, { tax_classification: "SelfEmployed" }]));
  assert.ok(!writes.some((w) => w[0] === "confirm_income_registration"), "the old confirmation is not used");
  assert.equal(ui.factEdit, null, "the edit closes");
  assert.equal(toasts.at(-1).msg, "Income type saved for 2026.");
  assert.deepEqual(went.at(-1), ["back"], "back to the return, whose card asks again");
}

console.log("test_taxpayer_export: ok");
