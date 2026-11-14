// The Tax profile page (owner decisions 2, 3, 6, 7, 13, 14), driven through
// the real app/shell/taxpayer.js and app/shell/facts.js with a mocked kit
// and host: the sections in the Certificate of Registration's order; a fact
// shows only when a form on the year's list reads it, flagged when not set,
// with the forms that read it and where it is held; the income-tax rate row
// with and without the lock; the tax-type chips from the forms list; a fact
// row's edit redraws that row only; the birth date refuses a future date;
// setups on the Forms tab, "Save as a setup", and the New taxpayer page's
// "Start from". The demo taxpayer is Juana Santos, TIN 000-123-453-00000.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods, strictElement } from "./kit_surface.mjs";

const root = new URL("../", import.meta.url);
const KIT = kitMethods(root);

const meta = new WeakMap();
function el(info = {}) {
  const e = strictElement(KIT, (k, args, self) => {
    const m = meta.get(self);
    if (k === "child") m.kids.push(args[0]);
    if (k === "children") m.kids.push(...args[0]);
    if (k === "id") m.info.id = String(args[0]);
    if (k === "when" && args[0]) args[1](self);
  });
  meta.set(e, { info, kids: [] });
  return e;
}
/** Every recorded node under an element (or under a node `one` found). */
function nodes(e, out = []) {
  const m = meta.get(e) || (e && Array.isArray(e.kids) ? e : null);
  if (!m) return out;
  out.push(m);
  for (const k of m.kids) nodes(k, out);
  return out;
}
const find = (tree, pred) => nodes(tree).filter((m) => pred(m.info));
const byId = (tree, id) => find(tree, (i) => [i.id, i.button, i.segmented, i.field, i.banner, i.dropdown].includes(id));
const one = (tree, id) => {
  const all = byId(tree, id);
  assert.equal(all.length, 1, `exactly one ${id}`);
  return all[0];
};
const texts = (tree) => find(tree, (i) => typeof i.text === "string").map((m) => m.info.text);
const chips = (tree) => find(tree, (i) => i.chip).map((m) => m.info.chip);
const host = (v) => JSON.parse(JSON.stringify(v));
const kidsOf = (e, kids) => {
  meta.get(e).kids.push(...kids.filter(Boolean));
  return e;
};

const ui = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24, 28, 32], RADIUS: { lg: 8, md: 6, sm: 4 } };
ui.Text = (_cx, _v, text) => el({ text: String(text) });
ui.Line = (_cx, _v, text) => el({ text: String(text) });
ui.Code = (_cx, code) => el({ code });
ui.Icon = () => el();
ui.IconButton = (_cx, id, _icon, opts = {}) => el({ button: id, onClick: opts.onClick });
ui.AppButton = (_cx, id, label, opts = {}) => el({ button: id, label, onClick: opts.onClick });
ui.Avatar = () => el();
ui.Chip = (_cx, chip) => el({ chip });
ui.Segmented = (_cx, id, options, selected, onChange) => el({ segmented: id, options, selected, onChange });
ui.Switch = (_cx, id) => el({ switch: id });
ui.CheckboxLabel = () => el();
ui.Checkbox = (_cx, id) => el({ checkbox: id });
ui.TextField = (_cx, state, o = {}) => el({ field: o.id, state });
ui.Dropdown = (_cx, id, opts) => el({ dropdown: id, options: opts.options, value: opts.value, onChange: opts.onChange });
ui.Field = (_cx, label, opts = {}) => kidsOf(el({ label, value: opts.value, error: opts.error }), [opts.input, opts.inheritSwitch]);
ui.FieldGrid = (_cx, _w, cells) => kidsOf(el(), cells.map((c) => c && c.el));
ui.Disclosure = (_cx, id, opts = {}) => kidsOf(el({ disclosure: id, title: opts.title, meta: opts.meta }), [opts.open ? opts.body : null]);
ui.DisclosureGroup = (_cx, id, sections) => kidsOf(el({ group: id }), sections);
ui.FormActions = (_cx, id, onCancel, onSave) => el({ formActions: id, onCancel, onSave });
ui.ListGroup = (_cx, id, rows) => kidsOf(el({ listGroup: id }), rows);
ui.ListRow = (_cx, id) => el({ row: id });
ui.NameCell = (_cx, title, sub) => el({ title, sub });
ui.EmptyRow = (_cx, id, text, action) => kidsOf(el({ empty: id, text }), [action]);
ui.SettingRow = (_cx, id, title, desc, control) => kidsOf(el({ setting: id }), [control]);
ui.Card = (_cx, id, rows) => kidsOf(el({ card: id }), rows);
ui.SectionTitle = (_cx, label) => el({ section: label });
ui.PageHeader = (_cx, opts) => kidsOf(el({ header: opts.title }), [opts.meta, ...(opts.actions || [])]);
ui.MetaLine = (_cx, parts) => kidsOf(el(), parts);
ui.Tabs = (_cx, id) => el({ tabs: id });
ui.Banner = (_cx, id, kids) => kidsOf(el({ banner: id }), kids);
ui.DigitCells = (_cx, _e, id) => el({ digitCells: id });
ui.PinCells = (_cx, _s, id) => el({ pinCells: id });
ui.ComboField = (_cx, _s, o = {}) => el({ field: o.id });
ui.DigitEntry = class {
  constructor(_shape, v = "") {
    this.v = v;
  }
  value() {
    return this.v;
  }
  isComplete() {
    return this.v.length === 9;
  }
};
ui.ComboState = class {
  constructor(_o, v = "") {
    this.v = v;
    this.input = { on() {} };
  }
  value() {
    return this.v;
  }
  isKnown() {
    return true;
  }
};

/** Counts every change handler a text box gets: typing must not redraw the page. */
let changeHandlers = 0;
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
  on(event) {
    if (event === "change") changeHandlers += 1;
  }
}
class OtpState {
  static new() {
    const s = new OtpState();
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

const context = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Proxy, WeakMap, Boolean, Object, Array });
const mock = (name, exports) =>
  new vm.SyntheticModule(Object.keys(exports), function () {
    for (const [k, v] of Object.entries(exports)) this.setExport(k, v);
  }, { context, identifier: name });
const source = (path) => new vm.SourceTextModule(fs.readFileSync(new URL(path, root), "utf8"), { context, identifier: path });
const taxpayerMod = source("app/shell/taxpayer.js");
const linked = {
  "gpui-kit": mock("gpui-kit", { div: () => el(), View: class View {} }),
  "gpui-base": mock("gpui-base", { InputState, OtpState }),
  "./ui.js": mock("./ui.js", ui),
  "./store.js": mock("./store.js", {
    errorText: (e) => (e && e.message) || String(e),
    formatTin: (r, c) => `${r}${c ? `-${c}` : ""}`,
    shortDate: (d) => String(d),
    daysUntil: () => 0,
  }),
  "./library.js": mock("./library.js", { searchText: (f) => `${f.c} ${f.t}`.toLowerCase() }),
  "./facts.js": source("app/shell/facts.js"),
};
await taxpayerMod.link((s) => linked[s]);
await taxpayerMod.evaluate();
const { TaxpayerActions, renderTaxpayerPage, renderNewTaxpayer } = taxpayerMod.namespace;

const ROOT = "000123453";
const HEAD = "00012345300000";
const BRANCH = "00012345300001";
const FACTS = ["birth_date", "tax_classification", "income_tax_election", "eopt_tier", "business_start_date", "line_of_business", "is_government_withholding_entity", "atc_codes"];

/** Every redraw: "page" for the page, else the retained entity notified. */
let redraws = [];
const made = [];
const cx = {
  notify: (target) => redraws.push(target ?? "page"),
  new(Cls, props) {
    const view = new Cls();
    view.init(props);
    const entity = { view, released: false, release() { this.released = true; } };
    made.push(entity);
    return entity;
  },
  write_to_clipboard() {},
  spawn() {},
};

/**
 * Juana Santos with a head office and branch 00001, one year (2026), the
 * forms `forms`, and the year view `view(overrides)` answers.
 */
function makeApp({ forms = ["2551Q", "1701Q", "1701"], usedBy = {}, notSet = [], facts = {}, owner = {}, locked = false, source = null, readiness = { gap: "ready" }, branch = "00000", type = "Individual", birth = null } = {}) {
  const writes = [];
  const toasts = [];
  const went = [];
  const head = { tin: HEAD, code: "00000", name: "Head office", is_head_office: true, facts: { rdo_code: "039", registered_address: "Quezon City", zip_code: "1100", phone: "", email: "" }, inherited: [] };
  const second = { ...head, tin: BRANCH, code: "00001", name: "Cebu branch", is_head_office: false, inherited: ["registered_address"] };
  const t = { name: "Juana Santos", tin_root: ROOT, taxpayer_type: type, birth_date: birth, archived: false, branches: [head, second] };
  const view = {
    tin: branch === "00000" ? HEAD : BRANCH,
    year: 2026,
    facts: { tax_classification: null, income_tax_election: null, eopt_tier: null, business_start_date: null, line_of_business: null, is_government_withholding_entity: false, atc_codes: [], ...facts },
    owner: { tax_classification: "head_office", income_tax_election: "head_office", eopt_tier: "head_office", business_start_date: "branch", line_of_business: "branch", is_government_withholding_entity: "branch", atc_codes: "branch", ...owner },
    used_by: Object.fromEntries(FACTS.map((f) => [f, usedBy[f] || []])),
    not_set: notSet,
    election_locked: locked,
    election_source: source,
  };
  const setups = [
    { id: "non_vat_professional", name: "Non-VAT professional", taxpayer_types: ["Individual"], forms: ["2551Q", "1701Q", "1701"], built_in: true, adds_forms: false },
    { id: "employer", name: "Employer", taxpayer_types: ["Individual"], forms: ["1601C", "1604C"], built_in: true, adds_forms: true },
    { id: "user-3", name: "Bookkeeper", taxpayer_types: ["Individual"], forms: ["2551Q"], built_in: false, adds_forms: false },
  ];
  let answer = {};
  const app = {
    selCode: branch,
    selYear: 2026,
    history: ["home"],
    taxpayer: () => t,
    branch: () => (app.selCode === "00000" ? head : second),
    taxpayers: () => [t, { name: "Pedro Reyes", tin_root: "000123454", taxpayer_type: "Individual", branches: [{ tin: "00012345400000", name: "Head office" }] }],
    taxpayerCreds: () => ({ has_pin: false }),
    isAdmin: () => false,
    adminRoot: () => "",
    toast: (msg) => toasts.push(msg),
    years: () => [2026],
    year: () => app.selYear,
    yearForms: () => forms,
    templates: () => [],
    go: (v) => {
      went.push(v);
      app.view = v;
    },
    goBack: () => went.push("back"),
    selectYear() {},
    branchFor: () => head,
    finishSoloSetup() {},
    writeLocal() {},
    store: {
      yearFacts: () => view,
      taxTypes: () => [{ type: "percentage_tax", label: "Percentage tax", forms: ["2551Q"] }, { type: "income_tax", label: "Income tax", forms: ["1701", "1701Q"] }],
      electionReadiness: () => readiness,
      // The host's rule (`annual_return_conflicts`): two or more annual income tax returns of one kind.
      annualConflicts: (codes) =>
        [["1700", "1701", "1701A", "1701MS"], ["1702", "1702RT", "1702EX", "1702MX"]]
          .map((group) => [...codes].filter((c) => group.includes(c)).sort())
          .filter((hits) => hits.length > 1),
      setups: (ty) => setups.filter((s) => s.taxpayer_types.includes(ty)),
      screenIndex: () => [
        { c: "2551Q", t: "Quarterly Percentage Tax Return", freq: "Quarterly" },
        { c: "1701Q", t: "Quarterly Income Tax Return", freq: "Quarterly" },
        { c: "1701", t: "Annual Income Tax Return", freq: "Annual" },
        { c: "1601C", t: "Monthly Remittance on Compensation", freq: "Monthly" },
      ],
      inboxState: () => ({ auth_method: "GoogleOAuth", oauth_connected: false, tracking_enabled: false }),
      sendGrants: () => [],
      yearlyForms: () => forms,
      formDeadlines: () => [],
      taxYears: (tin) => (tin === "00012345400000" ? [2025] : [2026]),
      taxpayers: () => app.taxpayers(),
      rdoOptions: () => [],
      zipOptions: () => [],
      yearBounds: () => ({ min: 2010, max: 2027 }),
      write: (name, ...args) => {
        writes.push(host([name, ...args]));
        return answer[name] ?? { forms: [], set: [], kept: [], election_locked: false, templates_copied: [], templates_kept: [] };
      },
      call: () => null,
    },
  };
  app.taxpayerUi = new TaxpayerActions(app);
  return { app, writes, toasts, went, view, answer: (a) => (answer = a) };
}

const page = (app) => renderTaxpayerPage(cx, app, "profile", 900);

// ---------------------------------------------------------------- sections and their order
{
  const { app } = makeApp({ usedBy: { eopt_tier: ["2551Q"] } });
  const tree = page(app);
  const sections = find(tree, (i) => i.disclosure).map((m) => [m.info.disclosure, m.info.title]);
  assert.deepEqual(sections, [
    ["tp-sec-taxpayer", "Taxpayer"],
    ["tp-sec-branch", "Registered activity"],
    ["tp-sec-taxtypes", "Tax types · your forms for 2026"],
    ["tp-sec-facts", "Facts your forms use"],
    ["tp-sec-address", "Address and contact"],
    ["tp-sec-inbox", "Confirmation inbox"],
    ["tp-sec-agent", "Agent filing"],
  ]);
  assert.equal(byId(tree, "tp-sec-registration").length, 0, "the Registration section is gone");
  assert.ok(!texts(tree).some((x) => /Not confirmed for/.test(x)), "and its wording");
  const labels = find(tree, (i) => i.label).map((m) => m.info.label);
  for (const l of ["Registered name", "TIN", "Taxpayer type", "Birth date", "Branch code", "Branch name", "RDO", "Line of business", "Business start date"]) {
    assert.ok(labels.includes(l), `${l} shows`);
  }
  one(tree, "tp-profile-years");
}

// ---------------------------------------------------------------- tax types
{
  const { app, went } = makeApp();
  const tree = page(app);
  for (const code of ["2551Q", "1701Q", "1701"]) one(tree, `tp-taxtypes-form-${code}`);
  assert.ok(chips(tree).includes("Quarterly") && chips(tree).includes("Annual"), "each form's frequency");
  assert.ok(chips(tree).includes("Percentage tax · from 2551Q"), "the chips from the forms list");
  assert.ok(chips(tree).includes("Income tax · from 1701, 1701Q"));
  assert.equal(find(tree, (i) => i.checkbox).length, 0, "no tick boxes");
  one(tree, "tp-taxtypes-change").info.onClick({}, cx);
  assert.deepEqual(went, ["forms"], "Change goes to the Forms tab");
}

// ---------------------------------------------------------------- facts: shown only when read, flagged
{
  const { app } = makeApp({
    usedBy: { eopt_tier: ["2551Q", "1701Q", "1701", "0619E", "1601EQ", "2550Q"], income_tax_election: ["2551Q", "1701Q"], birth_date: ["1701Q"], line_of_business: ["2551Q"] },
    notSet: ["eopt_tier", "birth_date"],
    facts: { line_of_business: "Bookkeeping" },
  });
  const tree = page(app);
  one(tree, "tp-fact-eopt_tier");
  one(tree, "tp-fact-income_tax_election");
  for (const f of ["tax_classification", "is_government_withholding_entity", "atc_codes"]) assert.equal(byId(tree, `tp-fact-${f}`).length, 0, `${f} is read by no form`);
  for (const f of ["birth_date", "line_of_business", "business_start_date"]) assert.equal(byId(tree, `tp-fact-${f}`).length, 0, `${f} is not shown twice`);
  assert.ok(texts(tree).includes("3 facts are not shown: no form on your list uses them."));
  const eopt = one(tree, "tp-fact-eopt_tier");
  assert.ok(chips(eopt).includes("Not set"), "a fact not set is flagged");
  assert.deepEqual(chips(one(tree, "tp-fact-eopt_tier-forms")), ["penalties on every form"], "a long list collapses");
  assert.deepEqual(chips(one(tree, "tp-fact-income_tax_election-forms")), ["2551Q", "1701Q"], "the forms that read it");
  assert.ok(!chips(one(tree, "tp-fact-income_tax_election")).includes("Not set"), "a fact that is set is not flagged");
  assert.ok(chips(tree).includes("Needed by 1701Q"), "the birth date in its own section is flagged");
  const meta = find(tree, (i) => i.disclosure === "tp-sec-facts")[0].info.meta;
  assert.equal(meta, "1 not set");
  // Only head-office facts on a branch say where they are held.
  assert.equal(byId(tree, "tp-fact-eopt_tier-owner").length, 0, "the head office holds its own");
}
{
  const { app } = makeApp({ branch: "00001", usedBy: { eopt_tier: ["2551Q"], atc_codes: ["1701Q"] } });
  const tree = page(app);
  one(tree, "tp-fact-eopt_tier-owner");
  assert.ok(texts(tree).includes("Held by the head office. A change here changes it for every branch."));
  assert.equal(byId(tree, "tp-fact-atc_codes-owner").length, 0, "a branch holds its own ATC codes");
}

// ---------------------------------------------------------------- the rate row
{
  const { app } = makeApp({ usedBy: { income_tax_election: ["2551Q"] }, facts: { income_tax_election: "EightPercent" }, source: "2551Q Q1" });
  let tree = page(app);
  const row = one(tree, "tp-fact-income_tax_election");
  assert.ok(find(row, (i) => i.value === "8% for 2026").length, "8% for the year");
  assert.ok(texts(row).includes("Chosen on 2551Q Q1"), "and where it was chosen");
  one(row, "tp-fact-income_tax_election-edit");

  const locked = makeApp({ usedBy: { income_tax_election: ["2551Q"] }, facts: { income_tax_election: "GraduatedOsd" }, locked: true, source: "2551Q Q1" });
  tree = page(locked.app);
  const lockedRow = one(tree, "tp-fact-income_tax_election");
  assert.ok(find(lockedRow, (i) => i.value === "Graduated rates, optional standard deduction for 2026").length);
  assert.equal(byId(lockedRow, "tp-fact-income_tax_election-edit").length, 0, "locked: no Change");
  assert.ok(texts(lockedRow).includes("A 2026 return that carries it was filed, so it cannot change for 2026."));

  for (const [gap, why] of [
    ["not_individual", "Only an individual chooses between 8% and graduated rates."],
    ["not_business_income", "Only a self-employed or mixed-income individual chooses between 8% and graduated rates. Set the income type first."],
  ]) {
    const x = makeApp({ usedBy: { income_tax_election: ["2551Q"] }, notSet: ["income_tax_election"], readiness: { gap } });
    const r = one(page(x.app), "tp-fact-income_tax_election");
    assert.equal(byId(r, "tp-fact-income_tax_election-edit").length, 0, `${gap}: no choices`);
    assert.ok(texts(r).includes(why), gap);
    assert.ok(!chips(r).includes("Not set"), `${gap}: nothing to set`);
  }

  // Change: 8% or graduated; a graduated method is kept when Graduated is picked.
  const { app: a2, writes } = makeApp({ usedBy: { income_tax_election: ["2551Q"] }, facts: { income_tax_election: "GraduatedItemized" } });
  a2.taxpayerUi.startFactEdit("income_tax_election", cx);
  const entity = made.at(-1);
  const edited = entity.view.render(cx);
  const choice = one(edited, "tp-fact-income_tax_election-choice");
  assert.deepEqual(Array.from(choice.info.options, (o) => o.value), ["EightPercent", "GraduatedUnspecified"]);
  assert.equal(choice.info.selected, "GraduatedUnspecified");
  a2.taxpayerUi.pickFact("GraduatedUnspecified", cx);
  a2.taxpayerUi.saveFactEdit(cx);
  assert.deepEqual(writes.at(-1), ["set_year_facts", HEAD, 2026, { income_tax_election: "GraduatedItemized" }]);
}

// ---------------------------------------------------------------- a fact row's edit redraws that row only
{
  const { app, writes, toasts } = makeApp({ usedBy: { eopt_tier: ["2551Q"], atc_codes: ["1701Q"] }, notSet: ["eopt_tier"] });
  const ui = app.taxpayerUi;
  redraws = [];
  changeHandlers = 0;
  ui.startFactEdit("eopt_tier", cx);
  const entity = made.at(-1);
  assert.equal(entity.view.props.fact, "eopt_tier", "the row gets its own view");
  assert.deepEqual(redraws, ["page"], "opening mounts it: one page redraw");
  const tree = page(app);
  assert.ok(meta.get(tree) && nodes(tree).some((m) => m.kids.includes(entity)), "the page shows the retained row");
  redraws = [];
  ui.pickFact("Small", cx);
  ui.pickFact("Micro", cx);
  assert.deepEqual(redraws, [entity, entity], "a pick redraws the row only");
  assert.equal(one(entity.view.render(cx), "tp-fact-eopt_tier-choice").info.selected, "Micro");
  redraws = [];
  ui.saveFactEdit(cx);
  assert.deepEqual(writes.at(-1), ["set_year_facts", HEAD, 2026, { eopt_tier: "Micro" }]);
  assert.ok(entity.released, "the row's view is released when the edit closes");
  assert.equal(ui.factViews.size, 0);
  assert.equal(toasts.at(-1), "EOPT tier saved for 2026.");

  // ATC codes: typing never redraws (no change handler); a bad code is refused in the row.
  ui.startFactEdit("atc_codes", cx);
  const atc = made.at(-1);
  assert.equal(changeHandlers, 0, "typing does not redraw the page");
  ui.factEdit.input.set_value("wc160, 12");
  redraws = [];
  ui.saveFactEdit(cx);
  assert.deepEqual(redraws, [atc], "the error redraws the row only");
  assert.match(ui.factEdit.error, /ATC codes like WC160/);
  ui.factEdit.input.set_value("wc160, WI010");
  ui.saveFactEdit(cx);
  assert.deepEqual(writes.at(-1), ["set_year_facts", HEAD, 2026, { atc_codes: ["WC160", "WI010"] }]);

  // Cancel releases too; a year switch drops the edit.
  ui.startFactEdit("eopt_tier", cx);
  const again = made.at(-1);
  ui.leaveYear();
  assert.ok(again.released && ui.factEdit === null, "a year switch drops the edit");
}

// ---------------------------------------------------------------- birth date and registered activity
{
  const { app, writes } = makeApp({ birth: "1990-05-17" });
  const ui = app.taxpayerUi;
  ui.startEdit("taxpayer", cx);
  ui.inputs.birth_date.set_value("2999-01-01");
  ui.saveSection("taxpayer", cx);
  assert.equal(ui.factErrors.birth_date, "That date is in the future.");
  assert.equal(writes.length, 0, "a future birth date is refused");
  ui.inputs.birth_date.set_value("1990-02-30");
  ui.saveSection("taxpayer", cx);
  assert.match(ui.factErrors.birth_date, /YYYY-MM-DD/);
  ui.inputs.birth_date.set_value("1991-06-01");
  ui.inputs.tin.v = ROOT;
  ui.saveSection("taxpayer", cx);
  assert.deepEqual(writes.at(-1), ["update_taxpayer", ROOT, { name: "Juana Santos", taxpayer_type: "Individual", birth_date: "1991-06-01" }]);

  const corp = makeApp({ type: "Corporation" });
  const labels = find(page(corp.app), (i) => i.label).map((m) => m.info.label);
  assert.ok(!labels.includes("Birth date") && labels.includes("Date of incorporation"), "a corporation has no birth date");
}
{
  const { app, writes } = makeApp({ facts: { line_of_business: "Bookkeeping" } });
  const ui = app.taxpayerUi;
  ui.startEdit("branch", cx);
  ui.inputs.business_start_date.set_value("2020-01-06");
  ui.saveSection("branch", cx);
  assert.deepEqual(writes.at(-1), ["set_year_facts", HEAD, 2026, { business_start_date: "2020-01-06" }], "only what changed, for the year");
  assert.equal(writes.at(-2)[0], "update_branch");
}

// ---------------------------------------------------------------- two annual income tax returns: a warning, never a removal
{
  const { app, writes } = makeApp({ forms: ["2551Q", "1701", "1701A"] });
  const ui = app.taxpayerUi;
  let tree = renderTaxpayerPage(cx, app, "forms", 900);
  const banner = one(tree, "tp-annual-conflict-1701-1701A");
  assert.match(texts(banner).join(" "), /1701 and 1701A are both annual income tax returns\. A taxpayer files only one of them for 2026\./);
  assert.equal(writes.length, 0, "the list is the user's: showing the warning writes nothing");
  // While editing it follows what is ticked, before any save.
  ui.editForms(cx);
  ui.toggleForm("1701A", false, cx);
  tree = renderTaxpayerPage(cx, app, "forms", 900);
  assert.equal(byId(tree, "tp-annual-conflict-1701-1701A").length, 0, "unticked: the warning goes");
  ui.toggleForm("1701A", true, cx);
  tree = renderTaxpayerPage(cx, app, "forms", 900);
  one(tree, "tp-annual-conflict-1701-1701A");
  ui.cancelForms(cx);
  // One annual return and its quarterly returns: nothing to say.
  const fine = makeApp({ forms: ["2551Q", "1701Q", "1701"] });
  assert.equal(find(renderTaxpayerPage(cx, fine.app, "forms", 900), (i) => String(i.banner || "").startsWith("tp-annual-conflict")).length, 0);
}

// ---------------------------------------------------------------- setups on the Forms tab and Save as a setup
{
  const { app, writes, answer } = makeApp();
  const ui = app.taxpayerUi;
  ui.editForms(cx);
  let tree = renderTaxpayerPage(cx, app, "forms", 900);
  one(tree, "tp-setup-non_vat_professional");
  one(tree, "tp-setup-employer");
  one(tree, "tp-setup-user-3-delete");
  assert.equal(byId(tree, "tp-setup-employer-delete").length, 0, "a built-in setup cannot be deleted");
  answer({ apply_setup: { forms: ["1701", "1701Q", "2551Q"], set: ["tax_classification"], kept: ["eopt_tier", "income_tax_election"], election_locked: true, templates_copied: [], templates_kept: [] } });
  one(tree, "tp-setup-non_vat_professional").info.onClick({}, cx);
  assert.deepEqual(writes.at(-1), ["apply_setup", HEAD, 2026, "non_vat_professional", false], "set facts are never replaced unasked");
  assert.deepEqual(host(ui.replaceAsk.kept), ["eopt_tier"], "a locked rate is not asked about");
  tree = renderTaxpayerPage(cx, app, "forms", 900);
  one(tree, "tp-replace");
  assert.ok(texts(tree).includes("EOPT tier is already set. Non-VAT professional has something else. Replace what is set?"));
  one(tree, "tp-replace-yes").info.onClick({}, cx);
  assert.deepEqual(writes.at(-1), ["apply_setup", HEAD, 2026, "non_vat_professional", true]);
  assert.equal(ui.replaceAsk, null, "asked once");

  ui.openSaveSetup(cx);
  tree = page(app);
  one(tree, "tp-save-setup-name");
  ui.saveSetupEdit.name.set_value("  Bookkeeper 2  ");
  ui.saveSetupNow(cx);
  assert.deepEqual(writes.at(-1), ["save_setup", HEAD, 2026, "Bookkeeper 2"]);
  assert.equal(ui.saveSetupEdit, null);
}

// ---------------------------------------------------------------- New taxpayer: Start from
{
  const run = (pick) => {
    const x = makeApp();
    const ui = x.app.taxpayerUi;
    ui.openNewTaxpayer(cx);
    const n = ui.newTp;
    n.tin.v = "000123455";
    n.name.set_value("Ana Cruz");
    pick(n, renderNewTaxpayer(cx, x.app, 900));
    ui.createTaxpayer(cx);
    return x;
  };
  const year = new Date().getFullYear();
  let options;
  let x = run((n, tree) => {
    options = one(tree, "new-tp-start").info.options.map((o) => o.value);
  });
  assert.deepEqual(Array.from(options), ["none", "setup:non_vat_professional", "setup:employer", "setup:user-3", "copy"], "nothing, the type's setups, the user's, a copy");
  assert.deepEqual(x.writes.map((w) => w[0]), ["create_taxpayer"], "none: nothing else is written");

  x = run((n) => (n.start = "setup:non_vat_professional"));
  assert.deepEqual(x.writes.map((w) => w[0]), ["create_taxpayer", "apply_setup"]);
  assert.deepEqual(x.writes[1], ["apply_setup", "00012345500000", year, "non_vat_professional", false]);
  assert.equal(x.app.selYear, year, "the page shows the year it started");
  assert.deepEqual(x.went.at(-1), "forms");

  x = run((n) => {
    n.start = "copy";
  });
  assert.deepEqual(x.writes.map((w) => w[0]), [], "a copy needs a taxpayer to copy");
  x = run((n) => {
    n.start = "copy";
    n.from = "00012345400000";
  });
  assert.deepEqual(x.writes[1], ["copy_setup_from", "00012345400000", "00012345500000", 2025, false], "the source's year when it lacks this one");
  assert.ok(!x.writes.some((w) => w[0] === "update_taxpayer" || w[0] === "update_branch"), "never the name, TIN or address");
}

console.log("test_profile_facts: ok");
