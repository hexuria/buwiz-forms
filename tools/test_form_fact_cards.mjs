// Decisions 3, 5 and 8 on the form page, driven through the real
// app/shell/form_page.js and app/shell/ui.js with a mocked kit and host:
// a tax fact the profile has not set is asked by a card at the item that
// uses it (the host's `asks`), and its Save writes the right host function,
// brings the return up to date and removes the card with only that row
// redrawn; a profile box changed with Edit offers "Save to profile too",
// off by default, and only on writes it back; Queue warns once in a
// confirmation and is stopped by a blocking message on its row; a filed
// return says the profile changed after it; and a return whose facts are
// all set shows none of it. The taxpayer is the demo Juana Santos.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods, strictElement } from "./kit_surface.mjs";

const root = new URL("../", import.meta.url);
const KIT = kitMethods(root);
const schemas = JSON.parse(fs.readFileSync(new URL("app/forms/forms-schema.json", root)));
const schemaOf = (code) => schemas.find((f) => f.c === code);
const at = (code, no) => {
  for (const [pi, p] of schemaOf(code).parts.entries()) for (const [ii, it] of p.items.entries()) if (it.no === no) return `${pi}:${ii}`;
  throw Error(`${code}: no item ${no}`);
};
const rowOf = (code, key) => {
  for (const [pi, p] of schemaOf(code).parts.entries()) for (const [ii, it] of p.items.entries()) if ((it.f || []).some((f) => f.k === key)) return `${pi}:${ii}`;
  throw Error(`${code}: no ${key}`);
};

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
const { FormPageActions, renderFormPage, renderQueueConfirm } = form.namespace;
const { PALETTES } = ui.namespace;

// ---- the host: Juana Santos, 000-123-453-00000, tax year 2026.
const TIN = "00012345300000";
const BIRTH = ["frm1701q:txtBirthMonth", "frm1701q:txtBirthDay", "frm1701q:txtBirthYear"];
const RDO = "frm1601c:txtRDOCode";
const SURCHARGE = "frm1601c:txtTax32";
const ITEM11 = at("1701Q", "11");
const ITEM7 = rowOf("1601C", RDO);
const ITEM32 = rowOf("1601C", SURCHARGE);
const EOPT_WARNING = "EOPT tier is not set. It would set the penalty rates if this return were late.";
const BIRTH_ASK = { fact: "birth_date", key: BIRTH[0], label: "Date of birth", control: "date", choices: null, note: "Juana Santos's date of birth is not in the tax profile; what you save here goes there for every later return." };
const EOPT_ASK = {
  fact: "eopt_tier",
  key: SURCHARGE,
  label: "EOPT tier",
  control: "choice",
  choices: [
    { value: "Micro", label: "Micro: under ₱3 million gross sales" },
    { value: "Small", label: "Small: ₱3 million to under ₱20 million" },
  ],
  note: "Juana Santos's EOPT tier (taxpayer size) for 2026 is not in the tax profile; what you save here goes there for every later return.",
};
let opened = {};
let asksNow = [];
let check = { blocks: [], warns: [] };
const calls = [];
const toasts = [];
const store = {
  templates: () => [],
  electionReadiness: () => ({ gap: "ready" }),
  call(name, ...args) {
    calls.push([name, ...args]);
    if (name === "screen_form_schema") return schemaOf(args[0]);
    if (name === "screen_form_open") return Promise.resolve({ ...opened, asks: asksNow });
    if (name === "screen_form_use_profile") {
      const values = { ...opened.values };
      if (args[4]?.includes("birth_date")) Object.assign(values, { [BIRTH[0]]: "02", [BIRTH[1]]: "03", [BIRTH[2]]: "1980" });
      asksNow = asksNow.filter((a) => !args[4]?.includes(a.fact));
      return { draft: { ...args[3], followed: args[4] }, values, options: {}, rejected: {}, asks: asksNow };
    }
    if (name === "screen_form_fill") return { draft: { ...args[1], ...args[2] }, values: { ...opened.values, ...args[2] }, options: {}, rejected: {} };
    if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...opened.values }, options: {}, errors: [], fact_errors: [] });
    if (name === "screen_form_queue_check") return check;
    throw Error(`unexpected host call ${name}`);
  },
  write(name, ...args) {
    calls.push([name, ...args]);
    if (name === "update_taxpayer" || name === "set_year_facts") return {};
    if (name === "screen_form_save") return { draft: { ...args[1], id: 7 }, status: "Draft" };
    if (name === "screen_form_save_to_profile") return { draft: args[3], values: {}, saved: ["RDO"], notes: [] };
    if (name === "screen_form_queue" || name === "queue_form_draft") return 7;
    throw Error(`unexpected host write ${name}`);
  },
};
const app = {
  prefs: {},
  store,
  overlay: null,
  layout: () => ({ column: 1056 }),
  taxpayers: () => [{ tin_root: "000123453", name: "Juana Santos", branches: [{ tin: TIN, code: "00000", name: "Head office" }] }],
  branchFor: () => ({ tin: TIN, code: "00000", name: "Head office" }),
  invalidate() { rootDirty = true; },
  toast(text) { toasts.push(text); rootDirty = true; },
  bumpRecent() {},
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
const openForm = async (code, period, r, asks = []) => {
  opened = { draft: { id: 7, tin: TIN, taxable_year: 2026 }, status: "Draft", editable: true, values: {}, locks: {}, errors: [], ...r };
  asksNow = asks;
  app.formUi.open({ code, tin: TIN, year: 2026, period }, cx);
  await settle();
  const s = app.formUi.state;
  for (const pi of s.schema.parts.keys()) if (!s.openParts.has(pi)) app.formUi.togglePart(pi, cx);
  frame();
  return s;
};
const row = (id) => one(frame(), `fp-item-${id}`);

// ---- 1. A date card at the item that uses it: 1701Q's date of birth.
let s = await openForm("1701Q", 1, {}, [BIRTH_ASK, { ...EOPT_ASK, key: null }]);
assert.equal(byId(row(ITEM11), "fp-ask-birth_date").length, 1, "the birth date is asked at Item 11");
assert.equal(byId(frame(), "fp-ask-birth_date").length, 1, "only there");
assert.equal(byId(row("0:0"), "fp-ask-eopt_tier").length, 1, "a fact the host could not place goes at the top of the first part");
assert.ok(texts(one(frame(), "fp-ask-birth_date")).includes(BIRTH_ASK.note), "one plain sentence");
s.askInputs.get("birth_date").set_value("02/03/1980");
traces.length = 0;
calls.length = 0;
click(frame(), "fp-ask-birth_date-save");
await settle();
frame();
let names = calls.map((c) => c[0]);
assert.deepEqual(names.slice(0, 2), ["update_taxpayer", "screen_form_use_profile"], `host calls: ${names}`);
assert.equal(JSON.stringify(calls[0]), JSON.stringify(["update_taxpayer", "000123453", { birth_date: "1980-02-03" }]), "the birth date goes to the taxpayer");
assert.equal(JSON.stringify(calls[1][5]), JSON.stringify(["birth_date"]), "the return takes it like Update from tax profile");
assert.ok(names.includes("screen_form_save"), "and is saved");
assert.equal(byId(frame(), "fp-ask-birth_date").length, 0, "the card is gone");
assert.ok(texts(one(frame(), `fp-ask-saved-${ITEM11}`)).join(" ").includes("Date of birth saved to the tax profile."));
assert.equal(byId(frame(), "fp-ask-eopt_tier").length, 1, "the other card stays");
assert.deepEqual([...new Set(traces)], [`fp-item-${ITEM11}`], "only the Item 11 row is redrawn");
assert.ok(toasts.includes("Saved to Juana Santos's tax profile."), `toasts: ${toasts}`);
console.log(JSON.stringify({ birthRow: ITEM11, rowBuildsOnSave: traces.length, hostCalls: names }));

// ---- 2. A choice card: 1601C's EOPT tier at its penalties.
s = await openForm("1601C", 2, { profile: [RDO], profile_edit: [RDO], values: { [RDO]: "039" }, profile_save: { [RDO]: { field: "rdo_code", label: "RDO", note: "Saved for this branch." } } }, [EOPT_ASK]);
assert.equal(byId(row(ITEM32), "fp-ask-eopt_tier").length, 1, "EOPT tier is asked at the penalties");
assert.ok(has(one(frame(), "fp-ask-eopt_tier-save"), "disabled", true), "Save waits for an answer");
traces.length = 0;
click(frame(), "fp-ask-eopt_tier-0");
frame();
assert.deepEqual([...new Set(traces)], [`fp-item-${ITEM32}`], "picking rebuilds only its row");
calls.length = 0;
click(frame(), "fp-ask-eopt_tier-save");
await settle();
frame();
assert.equal(JSON.stringify(calls[0]), JSON.stringify(["set_year_facts", TIN, 2026, { eopt_tier: "Micro" }]), "set_year_facts for the return's year");
assert.equal(byId(frame(), "fp-ask-eopt_tier").length, 0);

// ---- 3. "Save to profile too": off by default; off keeps the change on this return.
// The RDO is a filled profile box: it sits in the part's "From tax profile" fold.
s.profileOpen.add(Number(ITEM7.split(":")[0]));
rootDirty = true;
frame();
assert.equal(byId(frame(), `fp-item-${ITEM7}`).length, 1, "the fold is open");
assert.equal(byId(frame(), `fp-to-profile-${ITEM7}`).length, 0, "no switch before Edit");
app.formUi.startProfileEdit([RDO], cx);
frame();
assert.equal(byId(frame(), `fp-to-profile-${ITEM7}`).length, 1, "the edited row shows the switch");
const sw = one(frame(), `fp-to-profile-switch-${ITEM7}`);
assert.ok(texts(one(frame(), `fp-to-profile-${ITEM7}`)).join(" ").includes("Off: the change stays on this return."), "off by default");
app.formUi.edit(RDO, "043", cx);
calls.length = 0;
app.formUi.save(cx);
await settle();
assert.ok(!calls.some((c) => c[0] === "screen_form_save_to_profile"), "off: nothing goes to the profile");
assert.ok(calls.some((c) => c[0] === "screen_form_save"), "the return is saved");
// On: the next save writes it through the host, and the row says so first.
traces.length = 0;
click(frame(), `fp-to-profile-switch-${ITEM7}`);
frame();
assert.deepEqual([...new Set(traces)], [`fp-item-${ITEM7}`], "the switch redraws only its row");
assert.ok(texts(one(frame(), `fp-to-profile-${ITEM7}`)).join(" ").includes("Your next save writes it to the tax profile too."));
calls.length = 0;
app.formUi.save(cx);
await settle();
frame();
const write = calls.find((c) => c[0] === "screen_form_save_to_profile");
assert.ok(write, `on: written back: ${calls.map((c) => c[0])}`);
assert.equal(JSON.stringify(write.slice(1, 4)), JSON.stringify(["1601C", 2026, 2]));
assert.equal(JSON.stringify(write[5]), JSON.stringify([RDO]), "the box switched on");
assert.ok(calls.findIndex((c) => c[0] === "screen_form_save") < calls.indexOf(write), "after the return is saved");
assert.ok(toasts.at(-1).includes("Saved to the tax profile too: RDO."), toasts.at(-1));
assert.equal(s.toProfile.size, 0, "the switch goes back off");
assert.ok(!s.profileEdited.has(RDO), "the box is the profile's again");
void sw;

// ---- 4. Queue: a warning is said once in a confirmation; Continue queues.
check = { blocks: [], warns: [EOPT_WARNING] };
calls.length = 0;
app.formUi.queue(cx);
assert.equal(app.overlay, "queue-confirm");
assert.ok(!calls.some((c) => c[0] === "queue_form_draft"), "nothing queued yet");
const confirm = renderQueueConfirm(cx, app);
assert.ok(nodes(confirm).some((m) => m.kids.includes(EOPT_WARNING)), "the warning is in the confirmation");
app.formUi.confirmQueue(cx);
assert.equal(app.overlay, null);
assert.ok(calls.some((c) => c[0] === "queue_form_draft"), "Continue queues");
// Queued, the page reopens the return.
await settle();
s = app.formUi.state;
for (const pi of s.schema.parts.keys()) if (!s.openParts.has(pi)) app.formUi.togglePart(pi, cx);
frame();
// Cancel queues nothing.
calls.length = 0;
app.formUi.queue(cx);
app.formUi.cancelQueueConfirm(cx);
assert.ok(!calls.some((c) => c[0] === "queue_form_draft"));
// A block stops Queue and reads as a message on its card's row.
const BLOCK = "EOPT tier is not set in the tax profile, and it sets the penalty rates on this return. Answer it in the card at Item 32, then queue.";
check = { blocks: [{ field: "tax_profile.eopt_tier", message: BLOCK, keys: [SURCHARGE] }], warns: [] };
calls.length = 0;
app.formUi.queue(cx);
assert.equal(app.overlay, null, "no confirmation");
assert.ok(!calls.some((c) => c[0] === "queue_form_draft"), "not queued");
assert.equal(s.messages[ITEM32], BLOCK, "the message is on the card's row");
assert.ok(app.formUi.todo().some((r) => r.id === ITEM32), "the pill's to-do list has it");

// ---- 5. A filed return says the profile changed after it, read-only.
s = await openForm("1601C", 2, { status: "Submitted", editable: false, submitted_at: "2026-03-05T01:02:03+00:00", profile_changed: ["RDO 039 (profile now 043)", "EOPT tier"] }, [EOPT_ASK]);
const line = texts(one(frame(), "fp-profile-changed")).join(" ");
assert.ok(line.includes("The tax profile changed after this return was filed. This return keeps what was filed."), line);
assert.ok(line.includes("RDO 039 (profile now 043)") && line.includes("EOPT tier"), line);
assert.equal(byId(frame(), "fp-ask-eopt_tier").length, 0, "a filed return asks nothing");

// ---- 6. A return whose facts are all set: no card, no line, and Queue goes straight on.
s = await openForm("1601C", 2, {}, []);
assert.equal(nodes(frame()).filter((m) => String(m.id).startsWith("fp-ask-")).length, 0, "no card");
assert.equal(byId(frame(), "fp-profile-changed").length, 0, "no filed line");
check = { blocks: [], warns: [] };
calls.length = 0;
app.formUi.queue(cx);
assert.equal(app.overlay, null);
assert.ok(calls.some((c) => c[0] === "queue_form_draft"), "queued at once");
console.log("PASS: a fact not set is asked at its item and saved to the profile in place; Save to profile too is off by default and only on writes back; Queue warns once or blocks on the row; a filed return says the profile changed; a return with every fact set shows none of it.");
