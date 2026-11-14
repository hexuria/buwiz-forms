// 2551Q's annual income-tax election on a later quarter (pick 1 B), driven
// through the real app/shell/form_page.js and app/shell/ui.js with a mocked
// kit and host: the host's message (keyed to Item 13 by bir-core's
// ERROR_ALIASES) sits under Item 13, never Item 16, and wraps instead of
// being cut; Item 13 asks the question; picking an answer redraws only
// Item 13; Save to tax profile records the election for the return's own
// year, re-syncs the draft from the profile, ticks Item 13 to the same
// answer, saves and checks again, and the message, the part's badge and the
// attention line go, with only the Item 13 row rebuilt.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods, strictElement } from "./kit_surface.mjs";

const root = new URL("../", import.meta.url);
const KIT = kitMethods(root);
const schema = JSON.parse(fs.readFileSync(new URL("app/forms/forms-schema.json", root))).find((f) => f.c === "2551Q");
const K = (tail) => `frm2551Qv2018:${tail}`;
const R1 = K("taxRate1");
const R2 = K("taxRate2");
const at = (no) => {
  for (const [pi, p] of schema.parts.entries()) for (const [ii, it] of p.items.entries()) if (it.no === no) return `${pi}:${ii}`;
  throw Error(`no item ${no}`);
};
const ITEM13 = at("13");
const ITEM16 = at("16");
assert.ok(schema.parts[Number(ITEM13.split(":")[0])].items[Number(ITEM13.split(":")[1])].f.some((f) => f.k === R2), "Item 13 holds the 8% radio");

// ---- a mocked kit that keeps the element tree and redraws retained views when notified
const meta = new WeakMap();
const traces = [];
const entities = [];
const tasks = [];
let owner = null;
let rootDirty = true;
class View {}
function context(entity = null) {
  return {
    notify() {
      if (entity) entity.dirty = true;
      else rootDirty = true;
    },
    new(Class, props) {
      const e = { dirty: true, released: false, tree: null, set_props(p) { e.props = p; e.view.update?.(p); e.dirty = true; }, release() { e.released = true; return true; } };
      e.view = new Class(props);
      e.props = props;
      entities.push(e);
      const old = owner;
      owner = e;
      e.view.init?.(props, context(e));
      owner = old;
      return e;
    },
    spawn(fn) {
      const p = Promise.resolve(fn(context(entity)));
      tasks.push(p);
      return { cancel() {} };
    },
    timer: { after() { return { cancel() {} }; }, every() { return { cancel() {} }; } },
    focus_handle() { return { focus() {}, is_focused: () => false, release() { return true; } }; },
    stop_propagation() {},
    theme: () => ({ appearance: "light", is_dark: false, colors: {} }),
  };
}
function el() {
  const creator = owner;
  const e = strictElement(KIT, (k, args, self) => {
    const m = meta.get(self);
    if (k === "child") m.kids.push(args[0]);
    else if (k === "children") m.kids.push(...args[0]);
    else m.calls.push([k, ...args]);
    if (k === "id") {
      m.id = String(args[0]);
      if (m.id.startsWith("fp-item-")) traces.push(m.id);
    }
    if (k === "on_click") m.click = { fn: args[0], entity: creator };
    if (k === "when" && args[0]) args[1](self);
    if (k === "when_some" && args[0] != null) args[1](self, args[0]);
  });
  meta.set(e, { id: null, calls: [], kids: [] });
  return e;
}
function draw(e) {
  if (e.dirty && !e.released) {
    e.dirty = false;
    const old = owner;
    owner = e;
    e.tree = e.view.render(context(e));
    owner = old;
  }
}
/** Every node under `tree`, a retained view's latest drawing in its place. */
function nodes(tree, out = []) {
  if (tree?.view) return tree.tree ? nodes(tree.tree, out) : out;
  // An element, or a node already found.
  const m = meta.get(tree) || (tree?.calls && tree?.kids ? tree : null);
  if (!m) return out;
  out.push(m);
  for (const k of m.kids) nodes(k, out);
  return out;
}
const byId = (tree, id) => nodes(tree).filter((m) => m.id === id);
const one = (tree, id) => {
  const all = byId(tree, id);
  assert.equal(all.length, 1, `exactly one #${id}`);
  return all[0];
};
const has = (m, name, ...args) => m.calls.some((c) => c[0] === name && args.every((a, i) => c[i + 1] === a));
const texts = (m, out = []) => {
  for (const k of m.kids) {
    if (typeof k === "string") out.push(k);
    else if (k?.view) { if (k.tree) texts(meta.get(k.tree), out); }
    else if (meta.get(k)) texts(meta.get(k), out);
  }
  return out;
};
const click = (tree, id) => {
  const m = one(tree, id);
  assert.ok(m.click, `#${id} has a click handler`);
  const old = owner;
  owner = m.click.entity;
  m.click.fn({}, context(m.click.entity));
  owner = old;
};
const settle = async () => { while (tasks.length) await Promise.all(tasks.splice(0)); };

// ---- the real modules
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Object, Array, Promise, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
const mock = (name, exports) => new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name });
const source = (path) => new vm.SourceTextModule(fs.readFileSync(new URL(path, root), "utf8"), { context: contextVM, identifier: path });
const InputState = { new({ value = "" } = {}) { let text = value; return { value: () => text, set_value: (v) => { text = v; }, on() {}, release() { return true; } }; } };
const stub = () => el();
const ui = source("app/shell/ui.js");
const form = source("app/shell/form_page.js");
const linked = {
  "gpui-kit": mock("gpui-kit", { View, div: el, svg: el }),
  "gpui-base": mock("gpui-base", { Input: { new: stub }, InputState, CalendarState: {}, Popup: { new: stub }, OtpInput: { new: stub }, OtpState: InputState }),
  "gpui-component": mock("gpui-component", { Spinner: { new: stub } }),
  "./ui.js": ui,
  "./todo.js": source("app/shell/todo.js"),
  "./store.js": mock("./store.js", { formatTin: (root9, code) => `${root9}-${code}`, errorText: (e) => String(e && e.message ? e.message : e) }),
};
await ui.link((s) => linked[s]);
await form.link((s) => linked[s]);
await form.evaluate();
const { FormPageActions, renderFormPage } = form.namespace;
const { PALETTES } = ui.namespace;

// ---- the host: Juana Santos's Q3 2026 return, Section 116 activity, no
// commencement date on the profile and no 2026 election on it yet.
const TIN = "00012345300000";
const ELECTION = "This later-quarter Section 116 return needs the taxpayer profile's recorded annual income-tax election; save the election for this taxable year before filing";
const ITEM13_BLANK = "Choose the applicable Item 13 income-tax-rate election or explicitly select Not applicable";
let profileElection = null;
let draft = { id: 9, taxable_year: 2026, quarter: 3, taxpayer_type: "Individual", item_13_election: "unanswered", annual_income_tax_election: "unrecorded" };
// What sync_with_profile makes of the draft: the profile's taxpayer type, and
// (bir-core) Item 13 Not applicable for a non-individual unless `syncAnswers` is off.
let profileType = "Individual";
let syncAnswers = true;
let values = { [R1]: "false", [R2]: "false", [K("txt16")]: "0.00" };
const errors = () => {
  const out = [];
  // errors_json: bir-core's ERROR_ALIASES names Item 13's radios for this field.
  // later_period_requires_recorded_annual_election: an individual's later quarter.
  if (draft.taxpayer_type === "Individual" && draft.annual_income_tax_election === "unrecorded") out.push({ field: "annual_income_tax_election", keys: [R1, R2], message: ELECTION });
  if (draft.item_13_election === "unanswered") out.push({ field: "item_13_election", keys: [R1, R2], message: ITEM13_BLANK });
  return out;
};
const calls = [];
const toasts = [];
let refuseElection = null;
const bumped = [];
// bir-core income_tax_election_readiness: the profile can take 2026's election.
const READY = { taxable_year: 2026, gap: "ready", taxpayer_type: "Individual", classification: "SelfEmployed", since: "2026-01-01", new_registration_after: "2026-01-01", issues: [] };
let readiness = READY;
const readinessAsked = [];
const store = {
  templates: () => [],
  electionReadiness(tin, year) {
    readinessAsked.push([tin, year]);
    return readiness;
  },
  call(name, ...args) {
    calls.push([name, ...args]);
    if (name === "screen_form_schema") return schema;
    if (name === "screen_form_open") return Promise.resolve({ draft, status: "Draft", editable: true, values: { ...values }, locks: {}, errors: errors() });
    if (name === "screen_form_use_profile") {
      // sync_with_profile: the profile's election; Item 13 stays for the filer.
      const individual = profileType === "Individual";
      draft = { ...args[3], taxpayer_type: profileType, annual_income_tax_election: individual ? profileElection : "unrecorded", item_13_election: individual || !syncAnswers ? "unanswered" : "not_applicable" };
      return { draft, values: { ...values }, options: {}, rejected: {} };
    }
    if (name === "screen_form_fill") {
      values = { ...values, ...args[2] };
      draft = { ...args[1], item_13_election: values[R2] === "true" ? "eight_percent" : values[R1] === "true" ? "graduated" : "unanswered" };
      return { draft, values: { ...values }, options: {}, rejected: {} };
    }
    if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...values }, options: {}, errors: errors() });
    throw Error(`unexpected host call ${name}`);
  },
  write(name, ...args) {
    calls.push([name, ...args]);
    if (name === "record_income_tax_election") {
      if (refuseElection) throw Error(refuseElection);
      profileElection = args[2];
      return true;
    }
    if (name === "screen_form_save") return { draft: args[1], status: "Draft" };
    throw Error(`unexpected host write ${name}`);
  },
};
const app = {
  prefs: {},
  store,
  layout: () => ({ column: 1056 }),
  taxpayers: () => [{ tin_root: "000123453", name: "Juana Santos", branches: [{ tin: TIN, code: "00000", name: "Head office" }] }],
  branchFor: () => ({ tin: TIN, code: "00000", name: "Head office" }),
  invalidate() { rootDirty = true; },
  toast(text) { toasts.push(text); rootDirty = true; },
  // BirApp.bumpRecent: a changed and saved return goes to the top of Recent.
  bumpRecent(opts) { bumped.push(JSON.parse(JSON.stringify(opts))); },
};
app.formUi = new FormPageActions(app);
const cx = context();
let page = null;
function frame() {
  if (rootDirty) {
    rootDirty = false;
    owner = null;
    page = renderFormPage(cx, app, 1056);
  }
  for (const e of entities) draw(e);
  return page;
}

app.formUi.open({ code: "2551Q", tin: TIN, year: 2026, period: 3 }, cx);
await settle();
const s = app.formUi.state;
app.formUi.jumpTo(Number(ITEM13.split(":")[0]), cx);
frame();

// 1.1: the message is at Item 13, in full.
assert.ok(s.messages[ITEM13].includes(ELECTION), "the election message is Item 13's");
assert.equal(s.messages[ITEM16], undefined, "nothing under Item 16");
const row13 = () => one(frame(), `fp-item-${ITEM13}`);
const line = nodes(row13()).find((m) => m.kids.some((k) => typeof k === "string" && k.includes(ELECTION)));
assert.ok(line, "the message is drawn in Item 13's row");
assert.ok(has(line, "whitespace_normal") && !has(line, "truncate"), "the message wraps, never cut");
const slot = nodes(row13()).find((m) => m.kids.some((k) => meta.get(k) === line));
assert.ok(has(slot, "min_h", 18) && !has(slot, "h", 18) && !has(slot, "overflow_hidden"), "the 18 px line is reserved and grows");
// The empty line keeps its 18 px on a row with no message.
const empty = one(frame(), `fp-item-${at("12")}`);
assert.ok(nodes(empty).some((m) => has(m, "min_h", 18)), "a row without a message keeps the reserved line");
assert.ok(byId(frame(), "fp-nav-attention").length === 1, "the side list says a part needs attention");

// 1.3: the question card at Item 13.
assert.equal(byId(row13(), "fp-election").length, 1, "Item 13 asks the question");
assert.equal(byId(frame(), "fp-election").length, 1, "only Item 13 asks it");
assert.ok(texts(one(frame(), "fp-election")).includes("Which income-tax rate did Juana Santos choose for 2026?"));
assert.ok(has(one(frame(), "fp-election-save"), "disabled", true), "Save waits for an answer");

// Picking an answer redraws only Item 13 (rule 1).
traces.length = 0;
click(frame(), "fp-election-eight_percent");
assert.equal(rootDirty, false, "picking does not redraw the page");
frame();
assert.deepEqual([...new Set(traces)], [`fp-item-${ITEM13}`], "picking rebuilds only Item 13");
assert.ok(has(one(frame(), "fp-election-eight_percent"), "border_color", PALETTES.light.accent), "the answer shows chosen");
assert.ok(!has(one(frame(), "fp-election-save"), "disabled", true), "Save is ready");

// Save to tax profile: recorded for the return's year, re-synced, Item 13 ticked, saved and checked.
traces.length = 0;
calls.length = 0;
click(frame(), "fp-election-save");
await settle();
frame();
const names = calls.map((c) => c[0]);
assert.deepEqual(names, ["record_income_tax_election", "screen_form_use_profile", "screen_form_fill", "screen_form_save", "screen_form_validate"], `host calls: ${names}`);
assert.deepEqual(calls[0], ["record_income_tax_election", TIN, 2026, "eight_percent"], "the election for the return's own taxable year");
assert.deepEqual(calls[1].slice(1, 4), ["2551Q", 2026, 3], "the open draft re-syncs from the profile");
assert.equal(JSON.stringify(calls[2][3]), JSON.stringify({ [R1]: "false", [R2]: "true" }), "Item 13 takes the same answer");
assert.equal(s.messages[ITEM13], undefined, "the message is gone");
assert.equal(s.electionAt, null);
assert.equal(byId(frame(), "fp-election").length, 0, "the question is gone");
assert.ok(texts(one(frame(), "fp-election-saved")).includes("Saved to the tax profile for 2026. Later returns reuse it."));
assert.equal(byId(frame(), "fp-nav-attention").length, 0, "the attention line is gone");
assert.equal(app.formUi.partErrors(Number(ITEM13.split(":")[0])), 0, "the part's badge is gone");
assert.ok(toasts.includes("Saved to Juana Santos's tax profile for 2026."), `toasts: ${toasts}`);
assert.deepEqual(bumped, [{ tin: TIN, code: "2551Q", year: 2026, period: 3 }], "the saved return goes to the top of Recent");
assert.deepEqual([...new Set(traces)], [`fp-item-${ITEM13}`], "the save rebuilds only Item 13");
// The pill's red count clears too.
const pill = app.formUi.pillView();
draw(pill);
assert.equal(byId(pill.tree, "fp-pill-count").length, 0, "no section needs attention");
console.log(JSON.stringify({ item13: ITEM13, item16: ITEM16, rowBuildsOnSave: traces.length, hostCalls: names }));

// ---- a profile that cannot take the election (a taxpayer added in the app:
// no income type confirmed for 2026). The card knows before any press: no
// rate, no Save, what is missing and for which year, and the way to supply it.
draft = { ...draft, annual_income_tax_election: "unrecorded" };
readiness = { ...READY, gap: "not_business_income", classification: null, since: "2020-01-01", new_registration_after: "2020-01-01" };
const writesBefore = calls.filter((c) => c[0] === "record_income_tax_election").length;
app.formUi.validate(cx);
await settle();
frame();
assert.equal(byId(frame(), "fp-election").length, 1, "Item 13 still has its card");
assert.deepEqual(readinessAsked.at(-1), [TIN, 2026], "readiness is asked for the return's own TIN and year");
assert.equal(byId(frame(), "fp-election-save").length, 0, "no Save the profile would refuse");
assert.equal(byId(frame(), "fp-election-graduated").length + byId(frame(), "fp-election-eight_percent").length, 0, "no rate to pick");
const blocked = texts(one(frame(), "fp-election")).join(" ");
assert.ok(blocked.includes("Juana Santos's tax profile does not say whether Juana Santos is self-employed or has mixed income in 2026."), blocked);
assert.ok(blocked.includes("Certificate of Registration"), "it says where the fact comes from");
assert.ok(!/Other error/.test(blocked), "no raw host text");
// recordElection itself refuses to send it, even if called.
s.electionPick = "graduated";
app.formUi.recordElection(cx);
assert.equal(calls.filter((c) => c[0] === "record_income_tax_election").length, writesBefore, "nothing is sent while the profile cannot take it");
s.electionPick = null;
// No confirmed registration at all, and a compensation-only one, say so too.
for (const [gap, classification, words] of [
  ["no_confirmed_registration", null, "has no confirmed registration for 2026"],
  ["not_business_income", "PurelyCompensation", "registered for compensation income only in 2026"],
]) {
  readiness = { ...readiness, gap, classification };
  app.formUi.refreshRows();
  assert.ok(texts(one(frame(), "fp-election")).join(" ").includes(words), `${gap}: ${words}`);
}
// "Confirm registration" opens the taxpayer's Registration section for 2026.
const opened = [];
app.taxpayerUi = { startRegistration: (opts) => opened.push(opts) };
click(frame(), "fp-election-register");
assert.equal(JSON.stringify(opened), JSON.stringify([{ tin: TIN, year: 2026, fromForm: true }]), "the Registration section, for the return's branch and year");
// The filer confirms it there and comes back: the card asks again, and only Item 13 rebuilds.
readiness = READY;
traces.length = 0;
app.formUi.reenter();
frame();
assert.deepEqual([...new Set(traces)], [`fp-item-${ITEM13}`], "coming back rebuilds only Item 13");
assert.equal(byId(frame(), "fp-election-register").length, 0);
assert.equal(byId(frame(), "fp-election-graduated").length, 1, "the rates are offered now");
assert.ok(texts(one(frame(), "fp-election")).includes("Which income-tax rate did Juana Santos choose for 2026?"));

// A refusal that still comes (the profile changed in between): plain words,
// and the card falls back to what the profile lacks.
refuseElection = "Other error: The tax profile has no confirmed self-employed or mixed-income registration for 2026";
readiness = { ...READY };
const realElectionWrite = store.write;
store.write = (name, ...args) => {
  if (name === "record_income_tax_election") readiness = { ...READY, gap: "no_confirmed_registration", classification: null };
  return realElectionWrite.call(store, name, ...args);
};
click(frame(), "fp-election-graduated");
frame();
click(frame(), "fp-election-save");
await settle();
frame();
assert.ok(toasts.at(-1).startsWith("The tax profile has no confirmed self-employed"), `the refusal is shown, plainly: ${toasts.at(-1)}`);
assert.equal(byId(frame(), "fp-election").length, 1, "the card stays");
assert.equal(byId(frame(), "fp-election-save").length, 0, "and stops offering the Save");
assert.equal(byId(frame(), "fp-election-register").length, 1, "it offers the registration instead");
store.write = realElectionWrite;
refuseElection = null;

// ---- a return built while the profile said individual; the profile is now a
// corporation's. The draft's snapshot (taxpayer_type) still says individual,
// so validation asks for the rate, and the profile, rightly, cannot take one.
// The card offers the way out: bring the return up to date with the profile.
profileType = "Corporation";
readiness = { ...READY, gap: "not_individual", taxpayer_type: "Corporation", classification: null, since: "2026-01-01" };
draft = { ...draft, taxpayer_type: "Individual", annual_income_tax_election: "unrecorded", item_13_election: "unanswered" };
s.draft = { ...s.draft, ...draft };
app.formUi.validate(cx);
await settle();
frame();
let card = texts(one(frame(), "fp-election")).join(" ");
assert.ok(card.includes("This return still treats Juana Santos as an individual, but the tax profile registers Juana Santos as a corporation in 2026."), card);
assert.ok(!/Other error/.test(card));
assert.equal(byId(frame(), "fp-election-save").length + byId(frame(), "fp-election-register").length, 0, "no rate, no registration: neither can help");
calls.length = 0;
traces.length = 0;
click(frame(), "fp-election-follow");
await settle();
frame();
const followed = calls.map((c) => c[0]);
const took = followed.indexOf("screen_form_use_profile");
assert.ok(took >= 0, `the return takes the profile: ${followed}`);
assert.ok(followed.indexOf("screen_form_save") > took && followed.lastIndexOf("screen_form_validate") > took, `then it is saved and checked: ${followed}`);
assert.ok(!followed.includes("record_income_tax_election"), "nothing goes on the profile");
assert.ok(!calls.some((c) => c[0] === "screen_form_fill" && (R1 in c[3] || R2 in c[3])), "Item 13 is not ticked by the page");
assert.equal(draft.taxpayer_type, "Corporation");
assert.equal(s.messages[ITEM13], undefined, "the election and Item 13 messages are gone");
assert.equal(byId(frame(), "fp-election").length, 0, "and the card with them");
assert.equal(toasts.at(-1), "This return now follows Juana Santos's tax profile.");

// The same, where the sync leaves Item 13 unanswered: the toast says what it needs.
syncAnswers = false;
s.profileRefreshed = false;
draft = { ...draft, taxpayer_type: "Individual", annual_income_tax_election: "unrecorded", item_13_election: "unanswered" };
s.draft = { ...s.draft, ...draft };
app.formUi.validate(cx);
await settle();
frame();
click(frame(), "fp-election-follow");
await settle();
frame();
assert.equal(toasts.at(-1), "This return now follows Juana Santos's tax profile. Item 13 still needs an answer: choose Not applicable.");
assert.ok(s.messages[ITEM13].includes(ITEM13_BLANK) && !s.messages[ITEM13].includes(ELECTION), "only Item 13's own message is left");
assert.equal(byId(frame(), "fp-election").length, 0);
syncAnswers = true;

// Updated, and the quarter still resolves to an individual: no loop, the card
// says the profile's dates disagree.
profileType = "Individual";
profileElection = "unrecorded";
s.profileRefreshed = false;
draft = { ...draft, taxpayer_type: "Individual", annual_income_tax_election: "unrecorded", item_13_election: "unanswered" };
s.draft = { ...s.draft, ...draft };
app.formUi.validate(cx);
await settle();
frame();
click(frame(), "fp-election-follow");
await settle();
frame();
card = texts(one(frame(), "fp-election")).join(" ");
assert.ok(card.includes("still an individual's registration"), card);
assert.equal(byId(frame(), "fp-election-follow").length, 0, "the update is not offered again");
console.log("PASS: the 2551Q election is asked and answered at Item 13, never offered when the profile would refuse it, a return the profile has moved past is brought up to date, and it clears in place.");
