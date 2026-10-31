// The form page's shared mechanisms (docs/form-findings rules page, rules
// 4-8), driven through the real app/shell/form_page.js with a mocked host
// and a small form that has one box of each kind. Which boxes are money,
// dates, conditional or penalties is the host's to say (crates/bir-shell
// screen_form_module tests prove that on all 43 forms); this proves what the
// page does with it:
//  - rule 7: only a money box gets pesos; a whole-peso box shows no
//    centavos; what is typed reaches the host as typed.
//  - rule 4: 12102020, 12-10-2020 and 12/10/2020 reach the draft as
//    12/10/2020 and the box shows "Dec 10, 2020" (locked and table boxes
//    too); the calendar's month and year lists move its grid.
//  - rule 5: a stacked address is one box over its lines — the whole text
//    goes to the host, and what the host holds is what the box shows.
//  - rule 6: a box the host reports off is faded, takes no typing, says
//    what turns it on, and comes alive when the next check says so.
//  - rule 8: penalties show the computed amounts, locked; one Edit opens
//    all three, only a typed box is sent (the others stay computed);
//    totals follow; "Use the computed amounts" clears them.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
const root = new URL("../", import.meta.url);

const K = (tail) => `frmTEST:${tail}`;
const schema = {
  c: "TEST", t: "Test return", rev: "2026", freq: "Monthly", pages: 1, n: 4, q: 1,
  parts: [
    {
      n: "Part I · Background Information",
      items: [
        { no: "1", l: "Contact Number", f: [{ k: K("phone"), t: "text", l: "Contact Number" }] },
        { no: "2", l: "Date of Transaction", f: [{ k: K("date"), t: "date", l: "Date of Transaction" }] },
        { no: "3", l: "Date of Incorporation", f: [{ k: K("incorporated"), t: "date", l: "Date of Incorporation" }] },
        { no: "4", l: "Registered Name", f: [{ k: K("name"), t: "text", slots: 40, l: "Registered Name" }] },
        {
          no: "5", l: "Registered Address",
          f: [
            { k: K("addr1"), t: "text", slots: 40, l: "Registered Address", pool: { keys: [K("addr1"), K("addr2")], slots: [40, 31], write: [K("addr1")] } },
            { k: K("zip"), t: "text", slots: 4, l: "ZIP Code" },
          ],
        },
        {
          no: "6", l: "Registered Address (three lines)",
          f: [{ k: K("line1"), t: "text", slots: 38, l: "Registered Address", pool: { keys: [K("line1"), K("line2"), K("line3")], slots: [38, 38, 30], write: [K("line1"), K("line2"), K("line3")] } }],
        },
        { no: "7", l: "Email Address", f: [{ k: K("email"), t: "email", l: "Email Address" }] },
        { no: "25", l: "Payments", f: [{ k: K("cashDate"), t: "date", l: "Cash date" }, { k: K("checkDate"), t: "date", l: "Check date" }] },
        { no: "", l: "Tax Agent Accreditation No.", f: [{ k: K("agentNo"), t: "text", l: "Tax Agent Accreditation No." }] },
        { no: "27", l: "Cash/Bank Debit Advice", f: [], note: "Filled in when the return is paid." },
      ],
    },
    {
      n: "Part II · Computation of Tax",
      items: [
        { no: "14", l: "Sales", f: [{ k: K("sales"), t: "money", l: "Sales" }] },
        { no: "15", l: "Gross receipts (whole pesos)", f: [{ k: K("whole"), t: "money", w: true, l: "Gross receipts" }] },
        { no: "16", l: "Tax due", f: [{ k: K("due"), t: "money", c: true, l: "Tax due" }] },
        { no: "17", l: "Amended Return?", f: [{ k: K("amendY"), t: "radio", l: "Yes" }, { k: K("amendN"), t: "radio", l: "No" }] },
        { no: "18", l: "Tax remitted in previously filed return", f: [{ k: K("prev"), t: "money", l: "Tax remitted in previously filed return" }] },
        { no: "20", l: "Surcharge", f: [{ k: K("sur"), t: "money", l: "Surcharge" }] },
        { no: "21", l: "Interest", f: [{ k: K("int"), t: "money", l: "Interest" }] },
        { no: "22", l: "Compromise", f: [{ k: K("comp"), t: "money", l: "Compromise" }] },
        { no: "23", l: "Total Penalties", f: [{ k: K("totpen"), t: "money", c: true, l: "Total Penalties" }] },
        { no: "24", l: "Total Amount Payable", f: [{ k: K("payable"), t: "money", c: true, l: "Total Amount Payable" }] },
      ],
    },
    {
      n: "Schedule 1 · Payments",
      items: [
        { no: "", l: "Payments", rows: 2, calendar: true, cols: [{ l: "Date paid", t: "date" }, { l: "Amount", t: "money" }], keys: [[K("paid0"), K("amount0")], [K("paid1"), K("amount1")]] },
      ],
    },
  ],
};
const partOf = (key) => schema.parts.findIndex((p) => p.items.some((it) => (it.f || []).some((f) => f.k === key) || (it.keys || []).flat().includes(key)));
const itemOf = (key) => schema.parts[partOf(key)].items.findIndex((it) => (it.f || []).some((f) => f.k === key) || (it.keys || []).flat().includes(key));
const idOf = (key) => `${partOf(key)}:${itemOf(key)}`;

const entities = [], tasks = [], texts = [], timers = [];
const clicks = new Map(), downOut = new Map(), opacity = new Map(), ids = new Set();
let calendarSteps = 0;
const calls = [];
let owner = null;
class View {}
function context(entity = null) {
  return {
    stale: false,
    notify() {
      if (this.stale) throw new Error("stale context: cx used after its handler returned");
      if (entity) entity.dirty = true;
    },
    new(Class, props) {
      const e = { dirty: true, released: false, set_props(p) { e.props = p; e.view.update?.(p); e.dirty = true; }, release() { e.released = true; return true; } };
      e.view = new Class(props); e.props = props; entities.push(e);
      const old = owner; owner = e; e.view.init?.(props, context(e)); owner = old;
      return e;
    },
    spawn(fn) { const p = Promise.resolve(fn(context(entity))); tasks.push(p); return { cancel() {} }; },
    timer: { after(_ms, fn) { timers.push(fn); return { cancel() {} }; }, every() { return { cancel() {} }; } },
    focus_handle() { return { focus() {}, is_focused: () => false, release() { return true; } }; },
    stop_propagation() {},
    theme() { return { appearance: "light", is_dark: false, colors: {} }; },
  };
}
function runTimers() { while (timers.length) act(timers.shift()); }
// A host call's context lives only while the handler runs: one used after
// it returns (from a timer, say) throws, as the shell does.
function act(fn, ...args) { const c = context(); try { return fn(...args, c); } finally { c.stale = true; } }
function element() {
  const creationOwner = owner;
  const target = {};
  const el = new Proxy(target, { get(o, k) {
    if (k === "then") return undefined;
    return (...args) => {
      if (k === "id") { o.id = args[0]; ids.add(o.id); }
      if (k === "on_click") clicks.set(o.id, { fn: args[0], entity: creationOwner });
      if (k === "on_mouse_down_out") downOut.set(o.id, { fn: args[0], entity: creationOwner });
      if (k === "opacity") opacity.set(o.id, args[0]);
      if (k === "when" && args[0]) args[1](el);
      if (k === "when_some" && args[0] != null) args[1](el, args[0]);
      if (k === "child") mount(args[0]);
      if (k === "children") for (const c of args[0]) mount(c);
      return el;
    };
  } });
  return el;
}
function mount(e) { if (e?.view && e.dirty && !e.released) { e.dirty = false; const old = owner; owner = e; e.view.render(context(e)); owner = old; } }
const InputState = { new({ value = "" } = {}) { let text = value; const on = {}; return { value: () => text, set_value: (v) => { text = v; }, on(ev, fn) { on[ev] = fn; }, fire(ev) { if (on[ev]) act(on[ev], {}); }, type(v) { text = v; if (on.change) act(on.change, {}); }, release() { return true; } }; } };
// The calendar's grid: a month that the arrows (and the lists) step.
const calendars = [];
const CalendarState = { new() {
  let value = null, y = 2026, m = 10, onChange = null;
  const cal = {
    set_value(v) { value = v; if (typeof v === "string") { y = Number(v.slice(0, 4)); m = Number(v.slice(5, 7)); } },
    value: () => value, year: () => y, month: () => m, today: () => "2026-10-17", month_days: () => [[]],
    next_month() { calendarSteps += 1; m += 1; if (m > 12) { m = 1; y += 1; } },
    prev_month() { calendarSteps += 1; m -= 1; if (m < 1) { m = 12; y -= 1; } },
    on(_ev, fn) { onChange = fn; }, pick(day) { value = day; if (onChange) act(onChange, day); }, release() { return true; },
  };
  calendars.push(cal);
  return cal;
} };
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Object, Array, window: { viewport_size: () => ({ width: 1400, height: 900 }) } });
function mock(name, exports) { return new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name }); }
const kit = mock("gpui-kit", { div: element, svg: element, View, list: (_id, n, _key, render) => { const el = element(); for (let i = 0; i < n; i++) el.child(render(i)); return el; } });
const base = mock("gpui-base", { InputState, CalendarState, Popup: { new: element } });
const components = mock("gpui-component", { Spinner: { new: element } });
const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24], RADIUS: {}, SIZE: { validationLine: 18 }, BREAK: { formNav: 760 } };
for (const n of ["Text", "Line", "Code", "Icon", "AppButton", "IconButton", "Avatar", "Chip", "Switch", "Checkbox", "TextField", "Dropdown", "PageHeader", "MetaLine", "Overlay", "CheckboxLabel"]) uiExports[n] = element;
const dropdowns = new Map();
uiExports.AppButton = (_cx, id, _label, opts = {}) => { const el = element(); el.id(id); if (opts.onClick) el.on_click(opts.onClick); return el; };
uiExports.IconButton = (_cx, id, _icon, opts = {}) => { const el = element(); el.id(id); if (opts.onClick) el.on_click(opts.onClick); return el; };
uiExports.Dropdown = (_cx, id, opts) => { dropdowns.set(id, opts); const el = element(); el.id(id); return el; };
uiExports.Text = (_cx, _variant, text) => { texts.push(String(text)); return element(); };
uiExports.Line = uiExports.Text;
const store = mock("./store.js", { formatTin: (x) => x, errorText: (e) => String(e && e.message ? e.message : e) });
const periods = mock("./periods.js", { periodLabel: () => "October 2026" });
const mod = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/form_page.js", root), "utf8"), { context: contextVM });
await mod.link((s) => ({ "gpui-kit": kit, "gpui-base": base, "gpui-component": components, "./ui.js": mock("./ui.js", uiExports), "./store.js": store, "./periods.js": periods })[s]);
await mod.evaluate();
const { FormPageActions, renderFormPage, formatMoney, formatLongDate, shownDate, moveCalendar, needsText } = mod.namespace;

// The host: values as the field map writes them; a fill takes edits as given.
let hostValues = {
  [K("phone")]: "0281234567",
  [K("sales")]: "1234567.5",
  [K("whole")]: "1234567",
  [K("due")]: "0",
  [K("date")]: "12/10/2020",
  [K("incorporated")]: "03/05/2021",
  [K("paid0")]: "01/31/2026",
  [K("amount0")]: "100",
  [K("paid1")]: "",
  [K("amount1")]: "",
  [K("name")]: "EXAMPLE TRADING CORP",
  [K("addr1")]: "1 EXAMPLE STREET, QUEZON CITY",
  [K("addr2")]: "",
  [K("zip")]: "1100",
  [K("line1")]: "UNIT 5 BLDG A, 123 AYALA AVENUE CORNER",
  [K("line2")]: "PASEO DE ROXAS",
  [K("line3")]: "",
  [K("email")]: "books@example.com",
  [K("amendY")]: "false",
  [K("amendN")]: "true",
  [K("prev")]: "0.00",
  [K("cashDate")]: "",
  [K("checkDate")]: "",
  [K("agentNo")]: "",
};
// The host reads a pooled address from the draft; its field map cuts the
// first line at 40 characters, which the page must never show.
const poolStore = { [K("addr1")]: "1 EXAMPLE STREET, QUEZON CITY", [K("line1")]: "UNIT 5 BLDG A, 123 AYALA AVENUE CORNER PASEO DE ROXAS" };
function poolValues() { hostValues[K("addr1")] = poolStore[K("addr1")].slice(0, 40); hostValues[K("addr2")] = poolStore[K("addr1")].slice(40); }
poolValues();
// The host says a box is off while what turns it on does not hold.
// The penalty model: computed amounts, the filer's overrides, the totals.
const PENALTY = [K("sur"), K("int"), K("comp")];
const computedPenalty = { [K("sur")]: 125, [K("int")]: 31.5, [K("comp")]: 1000 };
let overrides = {};
function penaltyValues() {
  const used = Object.fromEntries(PENALTY.map((k) => [k, overrides[k] ?? computedPenalty[k]]));
  const total = PENALTY.reduce((n, k) => n + used[k], 0);
  for (const k of PENALTY) hostValues[k] = used[k].toFixed(2);
  hostValues[K("totpen")] = total.toFixed(2);
  hostValues[K("payable")] = (5000 + total).toFixed(2);
}
// A group kept as entered before penalties were computed (PR 11: an excise
// return stored before decision D8; 1701Q's column in PR 5).
let kept = false;
const penaltyState = () => [{ keys: PENALTY, edited: Object.keys(overrides).length > 0, applies: true, ...(kept ? { kept: true } : {}) }];
penaltyValues();
const offNow = () => (hostValues[K("amendY")] === "true" ? {} : { [K("prev")]: "Needs Amended Return = Yes" });
let opened = () => ({
  draft: { id: 7 }, status: "Draft", editable: true, values: { ...hostValues }, options: {},
  locks: { ...Object.fromEntries(PENALTY.map((k) => [k, "computed from the due date; choose Edit penalties to enter the amounts BIR agreed to"])), [K("due")]: "computed by BIR's own formulas", [K("incorporated")]: "comes from the taxpayer profile; change it in the profile", [K("agentNo")]: "cannot be filled here: written by the app without a draft field" },
  profile: [], profile_edit: [], sample: false, blank: [], needs: {}, whys: { [K("agentNo")]: "This app files it blank" }, off: offNow(), penalties: penaltyState(), pools: { ...poolStore }, errors: [],
});
const app = {
  prefs: {}, branchFor: () => null, taxpayers: () => [], layout: () => ({ column: 1056 }),
  invalidate() {}, toast() {}, startNewReturn() {},
  store: {
    templates: () => [],
    write(name, ...args) { calls.push([name, ...args]); return { id: 7, status: "Draft", draft: args[1] }; },
    call(name, ...args) {
      calls.push([name, ...args]);
      if (name === "screen_form_schema") return schema;
      if (name === "screen_form_open") return Promise.resolve(opened());
      if (name === "screen_form_fill") {
        // Sales of 999 stands for a typed answer whose recompute clears
        // boxes the filer typed (the host's `removes`).
        if (String(args[2]?.[K("sales")]) === "999") return { draft: args[1], values: { ...hostValues }, options: {}, rejected: {}, removes: { message: "Changing item 14 clears 1 box you typed (Part II). Continue?", done: "Cleared 1 box", rows: 1 } };
        const edits = { ...args[2] }, rejected = {};
        const edited = Object.keys(overrides).length > 0;
        let relocks;
        for (const k of PENALTY) {
          if (!(k in edits)) continue;
          if (!edited && !(args[3] || []).includes(k)) rejected[k] = "computed; choose Edit penalties";
          else if (String(edits[k]).trim() === "") {
            delete overrides[k];
            // A kept group's blank joins the computed amounts; the host
            // relocks its boxes as computed at once.
            if (kept) {
              kept = false;
              relocks = Object.fromEntries(PENALTY.map((p) => [p, "computed from the due date; choose Edit penalties to enter the amounts BIR agreed to"]));
            }
          } else overrides[k] = Number(String(edits[k]).replace(/,/g, ""));
          delete edits[k];
        }
        for (const k of Object.keys(poolStore)) if (k in edits) { poolStore[k] = String(edits[k]).trim(); delete edits[k]; }
        hostValues = { ...hostValues, ...edits };
        penaltyValues();
        poolValues();
        // A second schedule row folds Sales (the host's `relocks`); "778"
        // unfolds it; a kept penalty group's blanks relock it as computed.
        const folded = edits[K("amount1")] === "777" ? { [K("sales")]: "computed from its (Add more...) rows: OTHERS and their subtotal" } : edits[K("amount1")] === "778" ? { [K("sales")]: null } : {};
        const allRelocks = { ...folded, ...(relocks || {}) };
        const relock_notes = Object.fromEntries(Object.entries(allRelocks).map(([k, v]) => [k, v ? "Computed by the form." : null]));
        return { draft: args[1], values: { ...hostValues }, options: {}, rejected, penalties: penaltyState(), pools: { ...poolStore }, relocks: allRelocks, relock_notes };
      }
      if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...hostValues }, options: {}, errors: [], off: offNow(), penalties: penaltyState(), pools: { ...poolStore } });
      if (name === "screen_form_preview_sync" || name === "screen_form_print") return true;
      throw Error(name);
    },
  },
};
app.formUi = new FormPageActions(app);
const ui = app.formUi;
const cx = context();
async function settle() { while (tasks.length) await Promise.all(tasks.splice(0)); }
function frame() { owner = null; texts.length = 0; ids.clear(); opacity.clear(); for (const e of entities) e.dirty = true; renderFormPage(cx, app, 1056); }
const lastFill = () => calls.filter((c) => c[0] === "screen_form_fill").at(-1);
async function openAll() {
  ui.open({ code: "TEST", tin: "12345678800000", year: 2026, period: 10 }, cx);
  await settle();
  for (let i = 0; i < schema.parts.length; i++) ui.ensureInputs(i, cx);
  ui.state.openParts = new Set(schema.parts.map((_p, i) => i));
  frame();
}

// ---- Rule 7: only money looks like money.
assert.equal(formatMoney("1234567.5"), "1,234,567.50");
assert.equal(formatMoney("1234567", true), "1,234,567", "whole pesos: no centavos");
assert.equal(formatMoney("-1234", true), "-1,234");
assert.equal(formatMoney("12.5", true), "12.5", "a fraction in a whole-peso box stays as typed");
assert.equal(formatMoney("", true), "");
await openAll();
const input = (k) => ui.state.inputs.get(k);
assert.equal(input(K("sales")).value(), "1,234,567.50", "a peso amount shows pesos and centavos");
assert.equal(input(K("whole")).value(), "1,234,567", "a whole-peso amount shows no centavos");
assert.equal(input(K("phone")).value(), "0281234567", "a contact number shows as typed");
assert.ok(texts.includes("0.00"), "a computed amount shows 0.00");
input(K("whole")).type("2500000");
input(K("whole")).fire("blur");
assert.equal(input(K("whole")).value(), "2,500,000", "leaving a whole-peso box groups the digits only");
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [K("whole")]: "2500000" }), "the host gets what was typed");
input(K("phone")).type("+639171234567");
input(K("phone")).fire("blur");
assert.equal(input(K("phone")).value(), "+639171234567", "a text box keeps exactly what was typed");
console.log("PASS rule 7: only peso amounts look like money.");

// ---- A box the filer cannot fill says why; a printed item with no box says so.
assert.equal(ui.isLocked(K("agentNo")), true);
assert.ok(texts.includes("This app files it blank"), "a locked, blank box on a return shows the host's reason in the box");
const noteItem = schema.parts[0].items.findIndex((it) => it.note);
assert.ok(ids.has(`fp-note-0:${noteItem}`), "a printed item with no box is drawn");
assert.ok(texts.includes("Filled in when the return is paid.") && texts.includes("Cash/Bank Debit Advice"), "with its caption and its note");
console.log("PASS a box the filer cannot fill says why, and a printed item with no box says so.");

// ---- Rule 4: dates read like dates.
assert.equal(formatLongDate({ y: 2020, m: 12, d: 10 }), "Dec 10, 2020");
assert.equal(shownDate("12/10/2020"), "Dec 10, 2020");
assert.equal(shownDate("2020-12-10"), "Dec 10, 2020");
assert.equal(shownDate("3/31/2026"), "Mar 31, 2026", "a date the field map writes unpadded (2550Q Return Period) reads the same way");
assert.equal(shownDate(""), "", "a blank date stays blank");
assert.equal(shownDate("12/2020"), "12/2020", "what is not a whole date shows as it is");
assert.equal(input(K("date")).value(), "Dec 10, 2020", "the draft's 12/10/2020 shows as Dec 10, 2020");
assert.equal(input(K("paid0")).value(), "Jan 31, 2026", "a schedule date shows the same way");
assert.ok(texts.includes("Mar 5, 2021"), "a locked date shows as Mar 5, 2021");
for (const typed of ["12102020", "12-10-2020", "12/10/2020", "Dec 10, 2020"]) {
  input(K("date")).type("01012000");
  input(K("date")).type(typed);
  assert.equal(ui.state.pending[K("date")], "12/10/2020", `${typed} reaches the draft as 12/10/2020`);
  input(K("date")).fire("blur");
  assert.equal(input(K("date")).value(), "Dec 10, 2020", `${typed} shows as Dec 10, 2020`);
  assert.equal(lastFill()[3][K("date")], "12/10/2020", "the host gets the form's format");
}
input(K("date")).type("13312020");
input(K("date")).fire("blur");
assert.equal(input(K("date")).value(), "13312020", "a typed day that is not a date stays as typed");
assert.match(ui.state.dateErrors[idOf(K("date"))], /valid date/, "and says why");
input(K("date")).type("12102020");
input(K("date")).fire("blur");
await settle();

// The calendar: its month and year lists move the grid; a day fills the box.
const pid = idOf(K("date"));
frame();
act(clicks.get(`fp-date-${pid}`).fn, {});
const cal = ui.state.picker.cal;
assert.deepEqual([cal.year(), cal.month()], [2020, 12], "the calendar opens on the box's date");
frame();
const months = dropdowns.get(`fp-date-month-${pid}`), years = dropdowns.get(`fp-date-year-${pid}`);
assert.ok(months && years, "the calendar has a month list and a year list");
assert.equal(months.options.length, 12);
assert.ok(years.options.some((o) => o.value === "1900") && years.options.some((o) => o.value === "2036"), "years run from 1900 to ten years ahead");
act(months.onOpenChange, true);
assert.equal(ui.state.picker.select, "month", "the month list opens over the calendar");
act(months.onChange, "3");
assert.deepEqual([cal.year(), cal.month()], [2020, 3], "a month changes the month");
assert.equal(ui.state.picker.select, null, "and closes its list");
calendarSteps = 0;
act(years.onChange, "1900");
assert.deepEqual([cal.year(), cal.month()], [1900, 3], "a year changes the year");
assert.equal(calendarSteps, 0, "in one jump, not a month at a time");
act(years.onChange, "1985");
assert.deepEqual([cal.year(), cal.month()], [1985, 3]);
const fillsBefore = calls.filter((c) => c[0] === "screen_form_fill").length;
cal.pick(ui.state.picker.jumpedTo);
assert.equal(calls.filter((c) => c[0] === "screen_form_fill").length, fillsBefore, "a jump selects a day but fills nothing");
moveCalendar(cal, 2026, 1);
assert.deepEqual([cal.year(), cal.month()], [2026, 1]);
moveCalendar(cal, 1985, 3);
cal.pick("1985-03-07");
assert.equal(lastFill()[3][K("date")], "03/07/1985", "a picked day reaches the draft as MM/DD/YYYY");
assert.equal(input(K("date")).value(), "Mar 7, 1985", "and shows as Mar 7, 1985");
// A click away while a list is open closes the list and the calendar.
frame();
act(clicks.get(`fp-date-${pid}`).fn, {});
frame();
act(dropdowns.get(`fp-date-year-${pid}`).onOpenChange, true);
act(downOut.get(`fp-date-card-${pid}`).fn, {});
assert.ok(ui.state.picker, "a click in the open list is the calendar's own");
runTimers();
assert.ok(ui.state.picker, "and keeps it open");
// The card hears the click first, then the list closes itself.
act(downOut.get(`fp-date-card-${pid}`).fn, {});
act(dropdowns.get(`fp-date-year-${pid}`).onOpenChange, false);
runTimers();
assert.equal(ui.state.picker, null, "a click outside the list closes it and the calendar");
// Each date of an item has its own calendar.
const payments = idOf(K("cashDate"));
frame();
assert.ok(clicks.has(`fp-date-${payments}-0`) && clicks.has(`fp-date-${payments}-1`), "two dates, two calendars");
act(clicks.get(`fp-date-${payments}-1`).fn, {});
ui.state.picker.cal.pick("2026-03-15");
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [K("checkDate")]: "03/15/2026" }), "the second calendar fills the second date");
// A schedule's date box has its own calendar too, in a table the host marks
// `calendar` (PR 4: 2000's Schedules 2-4).
const paidRow = idOf(K("paid0"));
frame();
assert.ok(clicks.has(`fp-date-${paidRow}-0-0`) && clicks.has(`fp-date-${paidRow}-1-0`), "each table date box has its own calendar");
act(clicks.get(`fp-date-${paidRow}-1-0`).fn, {});
ui.state.picker.cal.pick("2026-02-14");
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [K("paid1")]: "02/14/2026" }), "a table date's calendar fills its own box as MM/DD/YYYY");
assert.equal(input(K("paid1")).value(), "Feb 14, 2026", "and shows it as Feb 14, 2026");
console.log("PASS rule 4: dates show as Dec 10, 2020 and the calendar changes month and year.");

// ---- Rule 5: a stacked address is one box over its lines.
const long = "UNIT 5 BLDG A, 123 AYALA AVENUE CORNER PASEO DE ROXAS, MAKATI"; // 61
frame();
input(K("name")).type("EXAMPLE TRADING AND GENERAL MERCHANDISE CORP");
input(K("name")).fire("blur");
await settle(); frame();
assert.equal(ui.state.values[K("name")], "EXAMPLE TRADING AND GENERAL MERCHANDISE CORP", "the name is kept whole");
// A two-line address is one box over 40 + 31 slots, read from the draft
// (the host's pools), never from the field map's cut lines.
assert.equal(ui.state.inputs.has(K("addr2")), false, "the second line has no box of its own");
assert.equal(input(K("addr1")).value(), "1 EXAMPLE STREET, QUEZON CITY");
input(K("addr1")).type(long);
input(K("addr1")).fire("blur");
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [K("addr1")]: long }), "the whole address goes to the host as one text");
await settle(); frame();
assert.notEqual(ui.state.values[K("addr1")], long, "(the field map cuts it)");
assert.equal(input(K("addr1")).value(), long, "the box shows the whole address");
const longer = `${long} CITY 1226 METRO MANILA`;
input(K("addr1")).type(longer);
input(K("addr1")).fire("blur");
await settle(); frame();
assert.equal(ui.poolText({ pool: { keys: [K("addr1")] } }), longer, "never cut");
// Another box's edit leaves the address as the host holds it.
input(K("phone")).type("0287654321");
input(K("phone")).fire("blur");
await settle(); frame();
assert.equal(input(K("addr1")).value(), longer, "no space is ever put into it");
// Saved straight after typing, the box shows the host's address.
input(K("addr1")).type(long);
ui.save(cx);
assert.equal(ui.state.poolTexts[K("addr1")], long, "save takes the host's pool text");
await settle(); frame();
// Three lines, each a field of its own: the host splits the one text.
assert.equal(input(K("line1")).value(), "UNIT 5 BLDG A, 123 AYALA AVENUE CORNER PASEO DE ROXAS", "the lines read as one address");
input(K("line1")).type("UNIT 5 BLDG A, 123 AYALA AVENUE CORNER PASEO DE ROXAS, MAKATI CITY 1226");
input(K("line1")).fire("blur");
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [K("line1")]: "UNIT 5 BLDG A, 123 AYALA AVENUE CORNER PASEO DE ROXAS, MAKATI CITY 1226" }), "sent as one text");
await settle(); frame();
assert.equal(input(K("line1")).value(), "UNIT 5 BLDG A, 123 AYALA AVENUE CORNER PASEO DE ROXAS, MAKATI CITY 1226", "the box keeps what was typed");
console.log("PASS rule 5: a stacked address is one box over its lines.");

// ---- Rule 6: a box another answer turns off looks off and says why.
assert.equal(needsText("Needs Amended Return = Yes"), "Needs Amended Return = Yes", "the host's tag shows as it is");
assert.ok(needsText("x".repeat(300)).endsWith("…") && needsText("x".repeat(300)).length <= 90, "a long tag is cut");
frame();
assert.equal(ui.isOff(K("prev")), "Needs Amended Return = Yes", "the host's tag");
assert.equal(ui.isLocked(K("prev")), true, "an off box takes no typing");
assert.ok(texts.includes("Needs Amended Return = Yes"), "it says what turns it on");
assert.ok(texts.includes("0.00"), "and still shows its value");
// Under the item: an off box says what it needs in its place, never its
// lock note too; an off box the form computes keeps its tag and also says
// "Computed by the form." (PR 0b re-stack check, item a).
{
  const itemFor = (k) => schema.parts[partOf(k)].items[itemOf(k)];
  const kept = { values: ui.state.values, lockNotes: ui.state.lockNotes, off: ui.state.off };
  ui.state.values = { ...kept.values, [K("prev")]: "" };
  ui.state.lockNotes = { [K("prev")]: "Can't be filled on this page yet.", [K("due")]: "Computed by the form." };
  assert.equal(JSON.stringify(ui.lockNotesFor(itemFor(K("prev")))), "[]", "a blank off box shows its tag, not its lock note");
  assert.equal(JSON.stringify(ui.lockNotesFor(itemFor(K("due")))), "[]", "a computed amount on: 0.00, no note");
  ui.state.off = { ...kept.off, [K("due")]: "Needs Amended Return = Yes" };
  assert.equal(JSON.stringify(ui.lockNotesFor(itemFor(K("due")))), JSON.stringify(["Computed by the form."]), "an off computed box says it is computed");
  frame();
  assert.equal(texts.filter((t) => t === "Needs Amended Return = Yes").length, 2, "and keeps its Needs tag");
  assert.ok(texts.includes("Computed by the form."), "both show under the item");
  Object.assign(ui.state, kept);
  frame();
}
ui.choose({ [K("amendY")]: "true", [K("amendN")]: "false" }, cx);
await settle(); frame();
assert.equal(ui.isOff(K("prev")), "", "Amended Yes turns it on");
assert.equal(ui.isLocked(K("prev")), false, "it takes typing");
assert.ok(!texts.some((t) => t.startsWith("Needs ")), "and the tag goes");
input(K("prev")).type("1500");
input(K("prev")).fire("blur");
assert.equal(lastFill()[3][K("prev")], "1500", "the host gets what was typed");
ui.choose({ [K("amendY")]: "false", [K("amendN")]: "true" }, cx);
await settle(); frame();
assert.equal(ui.isLocked(K("prev")), true, "Amended No turns it off again");
console.log("PASS rule 6: an off box is faded, says what turns it on, and comes alive when it holds.");

// ---- Rule 8: penalties are computed; one Edit unlocks all three.
const [SUR, INT, COMP] = PENALTY;
frame();
for (const k of PENALTY) assert.equal(ui.isLocked(k), true, `${k} shows the computed amount, locked`);
assert.ok(texts.includes("125.00") && texts.includes("31.50") && texts.includes("1,000.00"), "the computed amounts show");
assert.equal(ui.isLocked(K("totpen")), false, "the total is a computed box, not a lock of its own");
const editId = (k) => `fp-penalty-edit-${idOf(k)}`;
assert.ok(clicks.has(editId(SUR)) && clicks.has(editId(COMP)), "each penalty item offers Edit penalties");
act(clicks.get(editId(COMP)).fn, {});
frame();
for (const k of PENALTY) assert.equal(ui.isLocked(k), false, `one Edit opens ${k}`);
// Focusing a box and leaving it untouched sends nothing.
{ const fills = calls.filter((c) => c[0] === "screen_form_fill").length;
  input(SUR).fire("focus"); input(SUR).fire("blur");
  assert.equal(calls.filter((c) => c[0] === "screen_form_fill").length, fills, "a merely focused penalty is no override"); }
assert.ok(clicks.has(`fp-penalty-computed-${idOf(SUR)}`), "and offers the computed amounts back");
input(COMP).type("500");
input(COMP).fire("blur");
const sent = lastFill();
assert.equal(JSON.stringify(sent[3]), JSON.stringify({ [COMP]: "500" }), "only the typed box is sent");
assert.equal(JSON.stringify(sent[4]), JSON.stringify([COMP]), "opened with Edit: sent as unlocked");
assert.equal(JSON.stringify(Object.keys(overrides)), JSON.stringify([COMP]), "the other two stay computed");
await settle(); frame();
assert.equal(ui.state.values[K("totpen")], "656.50", "Total Penalties recomputes from what was entered");
assert.equal(ui.state.values[K("payable")], "5656.50", "and so does the amount payable");
assert.equal(ui.penaltyGroupOf(SUR).edited, true);
assert.equal(ui.isLocked(SUR), false, "an edited group stays open");
// Reopened (no Edit pressed on this visit), an edited return keeps them open.
ui.state.penaltyOpen.clear();
assert.equal(ui.isLocked(INT), false, "entered amounts stay open on a later visit");
input(INT).type("0");
input(INT).fire("blur");
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [INT]: "0" }), "later edits send just that box");
await settle(); frame();
assert.equal(ui.state.values[K("totpen")], "625.00");
act(clicks.get(`fp-penalty-computed-${idOf(SUR)}`).fn, {});
await settle(); frame();
assert.equal(JSON.stringify(lastFill()[3]), JSON.stringify({ [SUR]: "", [INT]: "", [COMP]: "" }), "Use the computed amounts blanks all three");
assert.equal(ui.state.values[K("totpen")], "1156.50", "the computed amounts are back");
for (const k of PENALTY) assert.equal(ui.isLocked(k), true, `${k} is locked again`);
assert.equal(input(COMP).value(), "1,000.00", "its box shows the computed amount again");
console.log("PASS rule 8: penalties computed by default; one Edit unlocks all three; only typed boxes override; totals recompute.");

// ---- A typed box that is itself an answer clearing typed boxes.
{
  frame();
  const shownBefore = input(K("sales")).value();
  const salesBefore = hostValues[K("sales")];
  input(K("sales")).type("999");
  input(K("sales")).fire("blur");
  assert.equal(app.overlay, "removal", "the typed answer asks first");
  const fills = () => calls.filter((c) => c[0] === "screen_form_fill").length;
  const asked = fills();
  act((c) => ui.cancelRemoval(c));
  assert.equal(app.overlay, null, "Cancel closes the question");
  assert.equal(fills(), asked + 1, "and asks the host once, never again in a loop");
  assert.equal(input(K("sales")).value(), shownBefore, "the typed box shows what the draft holds again");
  assert.equal(hostValues[K("sales")], salesBefore, "the draft keeps it");
  await settle();
  console.log("PASS removal: Cancel on a typed answer puts the box back instead of asking again.");
}

// ---- A lock the fill changes takes effect at once.
{
  frame();
  assert.equal(ui.isLocked(K("sales")), false, "Sales starts open");
  input(K("amount1")).type("777");
  input(K("amount1")).fire("blur");
  await settle(); frame();
  assert.equal(ui.isLocked(K("sales")), true, "the fill's relock shuts it without reopening the return");
  assert.equal(ui.state.lockNotes[K("sales")], "Computed by the form.", "with its note");
  input(K("amount1")).type("778");
  input(K("amount1")).fire("blur");
  await settle(); frame();
  assert.equal(ui.isLocked(K("sales")), false, "and a null relock opens it again");
  assert.equal(ui.state.lockNotes[K("sales")], undefined);
  console.log("PASS relock: a fill's lock changes apply at once.");
}
// ---- PR 11 (decision D8): a group kept as entered before penalties were
// computed is locked with Edit; "Use the computed amounts" sends the three
// blanks while Edit still has them open (so the host takes them), and the
// host's relocks take effect at once.
overrides = {};
kept = true;
penaltyValues();
ui.state.penalties = penaltyState();
ui.state.locks = { ...ui.state.locks, ...Object.fromEntries(PENALTY.map((k) => [k, "kept as entered before penalties were computed; choose Edit penalties to change them."])) };
ui.state.penaltyOpen.clear();
frame();
for (const k of PENALTY) assert.equal(ui.isLocked(k), true, `${k} is kept, locked`);
assert.ok(clicks.has(editId(SUR)), "a kept group offers Edit penalties");
act(clicks.get(editId(SUR)).fn, {});
frame();
assert.ok(clicks.has(`fp-penalty-computed-${idOf(SUR)}`), "Edit opened, the computed amounts are offered");
act(clicks.get(`fp-penalty-computed-${idOf(SUR)}`).fn, {});
await settle(); frame();
const keptFill = lastFill();
assert.equal(JSON.stringify(keptFill[3]), JSON.stringify({ [SUR]: "", [INT]: "", [COMP]: "" }), "the kept group's three blanks are sent");
assert.equal(JSON.stringify([...keptFill[4]].sort()), JSON.stringify([...PENALTY].sort()), "sent while Edit has them open");
assert.equal(kept, false, "the host took them");
for (const k of PENALTY) {
  assert.equal(ui.isLocked(k), true, `${k} is computed and locked`);
  assert.equal(ui.state.locks[k], "computed from the due date; choose Edit penalties to enter the amounts BIR agreed to", `${k}: relocked at once`);
  assert.equal(ui.state.lockNotes[k], "Computed by the form.");
}
assert.equal(ui.state.values[K("totpen")], "1156.50", "the computed amounts show");
console.log("PASS PR 11: a kept penalty group goes to the computed amounts in one step, relocked at once.");
