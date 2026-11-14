// The form page without its taxpayer card and with the section pill (picks
// 8 C and 9 C), driven through the real app/main.js paint, app/shell/chrome.js,
// app/shell/form_page.js and app/shell/ui.js with a mocked kit and host:
// opening a return selects its taxpayer (the title-bar pill names it); the
// card is gone and its Sample, Opening… and assistant chips sit in the chip
// row; the pill floats over the form at 500 px (no chip row) and at 1200 and
// 1240 px (beside the kept side list); its list is a bottom sheet narrow and
// a popover wide, and opening it redraws only the pill; choosing a section
// opens it; ‹ and › visit the rows still to do (messages, empty required
// boxes, stops), in form order, and with none step by section; past the last
// row › is Check; a step opens the section, tints the row, scrolls it into
// view through the host scroll module and puts the keyboard in its box;
// "Stop here" has three states and rebuilds only its row; copied answers are
// marked until edited; ⌘→ / ⌘← step on the form page.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods, strictElement } from "./kit_surface.mjs";

const root = new URL("../", import.meta.url);
const KIT = kitMethods(root);
const schema = JSON.parse(fs.readFileSync(new URL("app/forms/forms-schema.json", root))).find((f) => f.c === "1601C");

// ---- a mocked kit that keeps the element tree and redraws retained views when notified
const meta = new WeakMap();
const traces = [];
const entities = [];
const tasks = [];
// Timers wait until the test runs them (runTimers).
const timers = [];
// Focus handles, by the order they were made, and what was focused.
const focused = [];
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
    timer: { after(_ms, fn) { timers.push([fn, entity]); return { cancel() {} }; }, every() { return { cancel() {} }; } },
    focus_handle() { const h = { focus() { focused.push(h); }, is_focused: () => false, release() { return true; } }; return h; },
    stop_propagation() {},
    propagate() {},
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
    if (k === "on_mouse_down_out") m.out = { fn: args[0], entity: creator };
    if (k === "on_mouse_down") m.down = { fn: args[1], entity: creator };
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
function nodes(tree, out = []) {
  if (tree?.view) return tree.tree ? nodes(tree.tree, out) : out;
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
const texts = (tree) => nodes(tree).flatMap((m) => m.kids.filter((k) => typeof k === "string"));
const fire = (handler) => {
  const old = owner;
  owner = handler.entity;
  handler.fn({}, context(handler.entity));
  owner = old;
};
const click = (tree, id) => {
  const m = one(tree, id);
  assert.ok(m.click, `#${id} has a click handler`);
  fire(m.click);
};
const plain = (v) => JSON.parse(JSON.stringify(v));
const settle = async () => { while (tasks.length) await Promise.all(tasks.splice(0)); };
/** Run the timers queued so far, and the ones they queue, `rounds` deep. */
const runTimers = (rounds = 4) => {
  for (let i = 0; i < rounds && timers.length; i++) for (const [fn, entity] of timers.splice(0)) fn(context(entity));
};

// ---- the real app
let viewport = { width: 1200, height: 800 };
const storage = new Map();
// The host window: what the form asks of it after a step.
const windowCalls = [];
const windowStub = {
  viewport_size: () => viewport,
  localStorage: { getItem: (k) => storage.get(k) ?? null, setItem: (k, v) => storage.set(k, v) },
  refresh: () => windowCalls.push("refresh"),
  focus_next: () => windowCalls.push("focus_next"),
};
const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Object, Array, Promise, Proxy, WeakMap, window: windowStub });
const mock = (name, exports) => new vm.SyntheticModule(Object.keys(exports), function () { for (const [k, v] of Object.entries(exports)) this.setExport(k, v); }, { context: contextVM, identifier: name });
const source = (path) => new vm.SourceTextModule(fs.readFileSync(new URL(path, root), "utf8"), { context: contextVM, identifier: path });
const stub = () => el();
const stubClass = class {
  constructor() {
    return new Proxy(this, { get: (o, k) => (k in o ? o[k] : () => null) });
  }
};
// The Calendar and Filings filter bars as main.js uses them: Esc closes an
// open combo first, and Filings takes a Home tile's preset once.
class CalendarFilters {
  constructor() {
    this.open = false;
  }
  dismiss(cx) {
    if (!this.open) return false;
    this.open = false;
    cx.notify();
    return true;
  }
}
class FilingsFilters extends CalendarFilters {
  applyPreset(app) {
    if (!app.filingsPreset) return;
    this.preset = app.filingsPreset;
    app.filingsPreset = null;
  }
}
const InputState = { new({ value = "" } = {}) { let text = value; return { value: () => text, set_value: (v) => { text = v; }, on() {}, release() { return true; } }; } };
const real = {
  "./shell/store.js": source("app/shell/store.js"),
  "./shell/theme.js": source("app/shell/theme.js"),
  "./shell/ui.js": source("app/shell/ui.js"),
  "./shell/chrome.js": source("app/shell/chrome.js"),
  "./shell/facts.js": source("app/shell/facts.js"),
  "./shell/chrome_gate.js": source("app/shell/chrome_gate.js"),
  "./shell/recent.js": source("app/shell/recent.js"),
  "./shell/form_page.js": source("app/shell/form_page.js"),
};
const linked = {
  "gpui-kit": mock("gpui-kit", { View, div: el, svg: el }),
  "gpui-base": mock("gpui-base", { Input: { new: stub }, InputState, CalendarState: {}, Popup: { new: stub }, OtpInput: { new: stub }, OtpState: InputState, set_theme() {} }),
  "gpui-component": mock("gpui-component", { Spinner: { new: stub } }),
  process: mock("process", { default: { env: {} } }),
  bir: mock("bir", {}),
  "./shell/home.js": mock("home", { renderHome: stub }),
  "./shell/library.js": mock("library", { renderLibrary: stub }),
  "./shell/calendar_page.js": mock("calendar", { renderCalendar: stub, CalendarFilters }),
  "./shell/filings.js": mock("filings", { renderFilings: stub, FilingsFilters }),
  "./shell/settings.js": mock("settings", { renderSettings: stub }),
  "./shell/taxpayer.js": mock("taxpayer", { renderTaxpayerPage: stub, renderNewTaxpayer: stub, TaxpayerActions: stubClass }),
  "./shell/auth.js": mock("auth", { renderSignIn: stub, renderSetup: stub }),
  "./shell/gate.js": mock("gate", { renderAccountGate: stub }),
  "./shell/update.js": mock("update", { installReadyUpdate() {}, pollUpdateStatus: () => false, renderUpdatePrompt: (_cx, _app, page) => page, setAutoUpdateDownload() {} }),
  "./shell/security.js": mock("security", { SecurityActions: stubClass }),
  "./shell/account.js": mock("account", { AccountActions: stubClass }),
  "./shell/setup_change.js": mock("setup_change", { SetupChange: stubClass }),
  "./shell/cron_tasks.js": mock("cron_tasks", { renderCronTasksPage: stub }),
};
for (const [k, v] of Object.entries(real)) {
  linked[k] = v;
  linked[k.replace("./shell/", "./")] = v;
}
linked["./periods.js"] = source("app/shell/periods.js");
linked["./todo.js"] = source("app/shell/todo.js");
const main = source("app/main.js");
await main.link((s) => {
  if (!linked[s]) throw new Error(`unlinked import ${s}`);
  return linked[s];
});
await main.evaluate();
let BirApp = null;
new main.namespace.default().init({}, { new: (Class) => (BirApp = Class) });
const { FormPageActions } = real["./shell/form_page.js"].namespace;
const { PALETTES } = real["./shell/ui.js"].namespace;

// ---- the host: Juana Santos (the demo taxpayer) and a second demo taxpayer
const TIN = "00012345300000";
const OTHER = "00055566600000";
const taxpayers = [
  { tin_root: "000123453", name: "Juana Santos", tin_display: "000-123-453", branches: [{ code: "00000", name: "Head office", tin: TIN, tin_root: "000123453", is_head_office: true, facts: { registered_address: "", rdo_code: "" } }] },
  { tin_root: "000555666", name: "Demo Store", tin_display: "000-555-666", branches: [{ code: "00000", name: "Head office", tin: OTHER, tin_root: "000555666", is_head_office: true, facts: { registered_address: "", rdo_code: "" } }] },
];
const K = (name) => `frm1601c:${name}`;
const STOP_A = K("CatAgent_P");
const STOP_B = K("txtTax14");
/**
 * Every required box answered (a tick group by its first box), so a return
 * opens with nothing to do unless a test says otherwise.
 */
const FILLED = {};
for (const p of schema.parts) {
  for (const it of p.items) {
    const radios = (it.f || []).filter((f) => f.r && f.t === "radio");
    if (radios.length) FILLED[radios[0].k] = "true";
    for (const f of it.f || []) if (f.r && !f.c && f.t !== "radio") FILLED[f.k] = f.t === "sel" ? "01" : "1";
  }
}
let openErrors = [];
// What screen_form_open answers beyond the draft (I-2 and the rest); null fields are left out.
let opened = {};
let openValues = FILLED;
let validateValues = {};
// The host scroll module (ScrollArea, ScrollMark, scroll_*): on or off.
let hostScroll = false;
const hostCalls = [];
const writes = [];
let stopEdits = {};
const component = (name) => ({
  new: (id, props) => {
    const e = el().id(id);
    meta.get(e).component = { name, props };
    return e;
  },
});
const store = {
  taxpayers: () => taxpayers,
  authState: () => ({ mode: "solo" }),
  returns: () => [],
  taxYears: () => [2026],
  yearlyForms: () => ["1601C"],
  templates: () => [],
  screenIndex: () => [{ c: "1601C", freq: "Monthly" }],
  logScreen() {},
  component: (name) => (hostScroll && ["ScrollArea", "ScrollMark"].includes(name) ? component(name) : null),
  call(name, ...args) {
    if (name === "screen_form_schema") return schema;
    if (name === "screen_form_open" || name === "screen_form_sample") {
      return Promise.resolve({ draft: { id: 1, taxable_year: 2026 }, status: "Draft", editable: true, values: { ...openValues }, locks: {}, errors: openErrors, sample: name === "screen_form_sample", ...opened });
    }
    if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: { ...validateValues }, options: {}, off: opened.off || {}, errors: openErrors });
    if (name === "screen_form_fill") return { draft: args[1], values: { ...openValues, ...args[2] }, options: {} };
    if (name === "template_stops") throw Error("the page no longer asks for template stops");
    if (hostScroll && ["scroll_reveal", "scroll_forget", "scroll_offset"].includes(name)) {
      hostCalls.push([name, ...args]);
      return name === "scroll_reveal" ? true : null;
    }
    throw Error(`unexpected host call ${name}`);
  },
  write(name, ...args) {
    writes.push([name, ...args]);
    if (name === "set_template_stop") {
      const [, , key, on] = args;
      stopEdits = { ...stopEdits };
      if (on === null) delete stopEdits[key];
      else stopEdits[key] = on;
      return { stop_edits: stopEdits };
    }
    throw Error(`unexpected host write ${name}`);
  },
};

let invalidations = 0;
const cx = context();
function makeApp(width) {
  viewport = { width, height: 800 };
  const app = Object.create(BirApp.prototype);
  Object.assign(app, {
    store, booted: true, cloudLocked: false, appLocked: false, setup: { step: "choose" }, setupPending: null,
    view: "home", history: [], forward: [], sidebarOpen: width >= 860, panel: null, lastPanel: "profile",
    isMac: false, isLinux: false, platformKnown: true, recent: [], prefs: { showRecent: true },
    accountMenu: false, settingsMenu: null, overlay: null, toastState: null, themePref: "system", readIds: new Set(),
    session: { kind: "open" }, selRoot: "000555666", selCode: "00000", selYear: 2026, wasNarrow: width < 860,
    focusRoot: { focus() {} }, taxpayerUi: { panelEdit: null, reset() {}, leaveView() {}, leaveYear() {} },
    security: { leave() {} }, setupChange: { leave() {} },
    calendarUi: new CalendarFilters(), filingsUi: new FilingsFilters(),
    invalidate() { invalidations += 1; rootDirty = true; },
  });
  app.formUi = new FormPageActions(app);
  return app;
}
let tree = null;
function frame(app) {
  if (rootDirty) {
    rootDirty = false;
    owner = null;
    tree = app.paint(cx);
  }
  for (const e of entities) draw(e);
  return tree;
}
const pillEntity = (app) => app.formUi.state.pill.entity;

/**
 * A press and release as GPUI dispatches them (gpui-pre 0.3.9, the same in
 * gpui-fast: window.rs `dispatch_mouse_event`, elements/div.rs
 * `Interactivity::paint` / `paint_mouse_listeners`):
 * - listeners are registered in paint order; an element registers its own
 *   (its `on_mouse_down`s, then its click's arming) before its children paint;
 * - hitboxes do not occlude one another (a script cannot call `occlude()`),
 *   so every element the pointer is over hears the press;
 * - the bubble phase runs the listeners last-registered first and ends when
 *   one calls `stop_propagation()`;
 * - a click arms on the bubble-phase press of an element under the pointer
 *   and fires on the release.
 * `over` lists the elements the pointer is over, by id: the one pressed on
 * the pill and one painted beneath it; their ancestors are over it too.
 * Returns the ids whose clicks fired.
 */
function press(tree, over) {
  const walk = [];
  const visit = (node, parent) => {
    if (node?.view) return node.tree ? visit(node.tree, parent) : undefined;
    const m = meta.get(node);
    if (!m) return;
    walk.push([m, parent]);
    for (const k of m.kids) visit(k, m);
  };
  visit(tree, null);
  const parentOf = new Map(walk);
  const under = new Set();
  for (const id of over) {
    const hits = walk.filter(([m]) => m.id === id);
    assert.equal(hits.length, 1, `exactly one #${id} under the pointer`);
    for (let m = hits[0][0]; m; m = parentOf.get(m)) under.add(m);
  }
  const listeners = [];
  for (const [m] of walk) {
    for (const c of m.calls) if (c[0] === "on_mouse_down" && c[1] === "left") listeners.push([m, "down", c[2]]);
    if (m.click) listeners.push([m, "arm"]);
  }
  let stopped = false;
  // A handler gets the context of the view that drew its element.
  const pcx = (m) => ({ ...context(m.down?.entity ?? null), stop_propagation() { stopped = true; } });
  const armed = [];
  for (const [m, kind, fn] of listeners.reverse()) {
    if (!under.has(m)) continue;
    if (kind === "arm") armed.push(m);
    else fn({}, pcx(m));
    if (stopped) break;
  }
  for (const m of armed) fire(m.click);
  return armed.map((m) => m.id);
}
/** A floating element takes the pointer: its press and release stop there. */
function assertTakesPointer(m, what) {
  for (const name of ["on_mouse_down", "on_mouse_up"]) {
    const c = m.calls.find((x) => x[0] === name && x[1] === "left");
    assert.ok(c, `${what} has ${name}`);
    let stopped = false;
    c[2]({}, { ...context(null), stop_propagation() { stopped = true; } });
    assert.ok(stopped, `${what}: ${name} stops propagation`);
  }
}
/**
 * A toast on the form page clears the pill: the pill's top (its bottom
 * inset plus its 56 px height, over the main surface's 8 px margin) is
 * under the toast's bottom. Elsewhere the toast keeps its 20 px.
 */
function assertToastClearsPill(app) {
  app.toast("Saved", cx);
  rootDirty = true;
  const t = frame(app);
  const toast = one(t, "toast");
  const bottom = toast.calls.find((c) => c[0] === "bottom")[1];
  const layer = one(t, "fp-pill-layer");
  const inset = layer.calls.find((c) => c[0] === "bottom")?.[1] ?? 0;
  const pill = one(t, "fp-pill");
  const height = pill.calls.find((c) => c[0] === "h")[1];
  assert.ok(bottom >= 8 + inset + height + 8, `the toast (bottom ${bottom}) sits above the pill (top ${8 + inset + height})`);
  // And centred on what the pill is centred on: the layer's span, in window px.
  const L = app.layout();
  const left = layer.calls.find((c) => c[0] === "left")[1];
  const span = layer.calls.find((c) => c[0] === "w")?.[1] ?? L.mainWidth;
  assert.ok(has(toast, "left", L.mainLeft + 1 + left) && has(toast, "w", span) && !has(toast, "right", 0), "the toast is centred over the pill's span, not the window");
  app.toastState = null;
}
/**
 * The sheet or popover lays out in the real toolkit: it has no height of its
 * own (it sizes to its content), so its scrolled list needs a definite max
 * height and must not be a flex_1 / zero-basis item, which the toolkit gives
 * no height there (the first build drew the header and no row). Every
 * section is a row directly in that list, in the pill's own view, and a row
 * never shrinks.
 */
function assertListLaysOut(app, room) {
  const tree = pillEntity(app).tree;
  const sections = one(tree, "fp-sections");
  assert.ok(!sections.calls.some((c) => c[0] === "h" || c[0] === "flex_1"), "the sheet or popover sizes to its content");
  const list = one(tree, "fp-sections-list");
  const max = list.calls.find((c) => c[0] === "max_h");
  assert.ok(max && typeof max[1] === "number" && max[1] >= 150 && max[1] <= room, `the list has a definite max height within ${room}: ${max}`);
  for (const bad of ["flex_1", "flex_grow", "flex_basis", "min_h", "size_full", "h_full", "overflow_y_scrollbar"]) {
    assert.ok(!has(list, bad), `the list is not ${bad}()`);
  }
  assert.ok(has(list, "flex_none") && has(list, "overflow_y_scroll"), "a fixed item that scrolls, like the combo lists");
  const rows = list.kids.map((k) => meta.get(k)?.id);
  const parts = app.formUi.state.schema.parts;
  assert.deepEqual(rows, parts.map((_p, i) => `fp-section-${i}`), "every section is a row of the list");
  for (const k of list.kids) assert.ok(has(meta.get(k), "flex_shrink", 0), "a row keeps its height");
}

// ---- opening a return selects its taxpayer; the card is gone
{
  const app = makeApp(1200);
  assert.equal(app.taxpayer().name, "Demo Store", "precondition: another taxpayer is selected");
  app.openForm({ tin: TIN, code: "1601C", year: 2026, period: 3 }, cx);
  assert.equal(app.selRoot, "000123453", "openForm selects the return's taxpayer");
  assert.equal(app.selCode, "00000", "and its branch");
  rootDirty = true;
  let t = frame(app);
  assert.ok(texts(one(t, "context-pill")).includes("Juana Santos"), "the title-bar pill names the return's taxpayer");
  // Still opening: the chip sits in the chip row, beside the form's own chips.
  const chipRow = nodes(one(t, "main-surface")).find((m) => texts(m).includes("Monthly") && m.kids.length > 2);
  assert.ok(chipRow && texts(chipRow).includes("Opening…"), "Opening… is in the chip row");
  await settle();
  rootDirty = true;
  t = frame(app);
  const page = texts(one(t, "main-surface"));
  assert.ok(!page.includes("Juana Santos") && !page.includes("Head office") && !page.some((x) => /^For /.test(x) && /2026/.test(x)), "no taxpayer card: no name, branch or period under the buttons");
  assert.ok(!page.includes("Guided mode"), "the Guided mode switch left with the card");
  app.formUi.state.agentReview = true;
  rootDirty = true;
  t = frame(app);
  const chips = nodes(one(t, "main-surface")).find((m) => texts(m).includes("Monthly") && m.kids.length > 2);
  assert.ok(texts(chips).includes("Prepared by the assistant — review, then Queue"), "the assistant chip is in the chip row");
  // Opening a return of the taxpayer already selected keeps the selection quiet.
  app.openForm({ tin: TIN, code: "1601C", year: 2026, period: 4 }, cx);
  assert.equal(app.selRoot, "000123453");
  await settle();
  // A sample says so in the chip row.
  app.openForm({ tin: TIN, code: "1601C", year: 2026, period: 3, sample: true }, cx);
  await settle();
  rootDirty = true;
  t = frame(app);
  const sampleChips = nodes(one(t, "main-surface")).find((m) => texts(m).includes("Monthly") && m.kids.length > 2);
  assert.ok(texts(sampleChips).includes("Sample"), "a sample's chip is in the chip row");
}

// ---- 500 px: the pill over main-scroll, no chip row; the list is a bottom sheet
{
  const app = makeApp(500);
  openErrors = [{ field: "x", keys: [STOP_B], message: "Enter the total amount of compensation" }];
  app.openForm({ tin: TIN, code: "1601C", year: 2026, period: 3 }, cx);
  await settle();
  rootDirty = true;
  let t = frame(app);
  const surface = one(t, "main-surface");
  assert.equal(byId(surface, "fp-pill").length, 1, "the pill is on the narrow page");
  assert.equal(byId(one(t, "main-scroll"), "fp-pill").length, 0, "it floats over main-scroll, not in it");
  assert.equal(byId(t, "fp-split").length, 0, "no side list at 500");
  assert.equal(byId(t, "fp-nav-0").length, 0, "no wrapped chip row");
  assert.equal(byId(t, "fp-nav-attention").length, 0, "no attention line: the pill counts");
  assert.ok(texts(one(t, "fp-pill-count")).includes("1"), "one row needs attention");
  // The message is the one row to do: the pill counts it and names the open section.
  assert.ok(texts(one(t, "fp-pill")).includes("Return details") && texts(one(t, "fp-pill")).includes("1 to do · Page 1"), `the pill counts what is to do: ${texts(one(t, "fp-pill"))}`);
  assert.ok(has(one(t, "fp-pill-prev"), "disabled", true), "‹ waits at the first section");
  assertToastClearsPill(app);
  t = frame(app);

  // The list: only the pill redraws.
  const rows = [...app.formUi.state.rowViews.values()];
  invalidations = 0;
  traces.length = 0;
  click(t, "fp-pill-list");
  assert.equal(invalidations, 0, "opening the list does not redraw the page");
  assert.ok(pillEntity(app).dirty && rows.every((r) => !r.entity.dirty), "only the pill redraws");
  t = frame(app);
  assert.equal(traces.length, 0, "no row rebuilds");
  assert.equal(byId(t, "fp-sections-scrim").length, 1, "narrow: a bottom sheet over a scrim");
  assert.ok(has(one(t, "fp-sections"), "bottom", 0) && has(one(t, "fp-sections"), "rounded_t", 22), "anchored to the bottom, rounded on top");
  assert.ok(texts(one(t, "fp-section-2")).includes("Fix · 1"), "the section to fix says so");
  assertListLaysOut(app, Math.round((800 - 40) * 0.84));
  // The scrim closes it.
  fire(one(t, "fp-sections-scrim").down);
  t = frame(app);
  assert.equal(byId(t, "fp-sections").length, 0, "the scrim closes the sheet");

  // Choosing a section opens it and closes the list.
  click(t, "fp-pill-list");
  t = frame(app);
  invalidations = 0;
  click(t, "fp-section-2");
  assert.deepEqual([...app.formUi.state.openParts], [2], "the chosen section opens");
  assert.equal(invalidations, 1, "the page redraws once to open it");
  t = frame(app);
  assert.equal(byId(t, "fp-sections").length, 0, "the list closes");
  assert.ok(texts(one(t, "fp-pill")).includes("Computation of Tax"), "the pill follows the open section");

  // ‹ and › by section once nothing is to do (the message fixed).
  app.formUi.setErrors([]);
  app.formUi.refreshRows();
  rootDirty = true;
  t = frame(app);
  assert.equal(app.formUi.todo().length, 0, "nothing to do");
  assert.deepEqual(plain(app.formUi.stepTarget(1)), { part: 3, row: null }, "› goes to the next section");
  click(t, "fp-pill-next");
  assert.deepEqual([...app.formUi.state.openParts], [3]);
  t = frame(app);
  assert.ok(has(one(t, "fp-pill-next"), "disabled", true), "› waits at the last section");
  // Jumping back to a part opened before rebuilds none of its rows.
  traces.length = 0;
  click(t, "fp-pill-prev");
  t = frame(app);
  assert.deepEqual([...app.formUi.state.openParts], [2]);
  assert.equal(traces.length, 0, `a jump rebuilds no row: ${traces}`);
  assert.equal(byId(t, "fp-stop-1:6").length, 0, "no Stop here without a template");
  openErrors = [];
}

// ---- 1200 and 1240 px: the side list stays, the pill floats over fp-scroll; the list is a popover
for (const width of [1200, 1240]) {
  const app = makeApp(width);
  app.openForm({ tin: TIN, code: "1601C", year: 2026, period: 3 }, cx);
  await settle();
  rootDirty = true;
  let t = frame(app);
  const split = one(t, "fp-split");
  assert.equal(byId(split, "fp-nav-list").length, 1, `the side list stays at ${width}`);
  assert.equal(byId(split, "fp-pill").length, 1, `the pill shows at ${width}`);
  assert.equal(byId(one(t, "fp-scroll"), "fp-pill").length, 0, "it floats over fp-scroll");
  const layer = one(t, "fp-pill-layer");
  assert.ok(has(layer, "absolute") && has(layer, "bottom", 16), "at the bottom of the form");
  assert.ok(layer.calls.some((c) => c[0] === "left" && c[1] > 208), "beside the side list");
  // Centred on the form's own column, not on the pane: it starts past the
  // side list and the body's 22 px and is as wide as the parts.
  const L = app.layout();
  const edge = Math.max(L.padX, Math.floor((L.mainWidth - L.column) / 2));
  assert.ok(has(layer, "left", edge + 208 + 6 + 22) && has(layer, "w", L.column - 208 - 28) && !has(layer, "right", 0), `${width}: the pill's layer is the form's column`);
  assertToastClearsPill(app);
  t = frame(app);
  click(t, "fp-pill-main");
  t = frame(app);
  assert.equal(byId(t, "fp-sections-scrim").length, 0, "wide: a popover, no scrim");
  assert.equal(byId(one(t, "fp-pill-layer"), "fp-sections").length, 1, "above the pill");
  const pillMax = one(t, "fp-pill").calls.find((c) => c[0] === "max_w")[1];
  assert.ok(has(one(t, "fp-sections"), "w_full") && has(one(t, "fp-sections"), "max_w", pillMax), "the popover is as wide as the pill: their edges line up");
  assertListLaysOut(app, 440);
  fire(one(t, "fp-pill-layer").out);
  t = frame(app);
  assert.equal(byId(t, "fp-sections").length, 0, "a click outside closes it");

  // Expand all / Collapse all, beside "On this form" and in the pill's list.
  const s = app.formUi.state;
  const every = s.schema.parts.map((_p, i) => i);
  const current = s.current;
  assert.ok(has(one(t, "fp-parts-toggle"), "tooltip", "Expand all parts"), "one part open: the toggle expands");
  click(t, "fp-parts-toggle");
  rootDirty = true;
  t = frame(app);
  assert.deepEqual([...s.openParts].sort(), every, "every part opens");
  assert.equal(s.current, current, "the pill keeps its section");
  assert.ok(has(one(t, "fp-parts-toggle"), "tooltip", "Collapse all parts"), "all open: the toggle collapses");
  click(t, "fp-parts-toggle");
  rootDirty = true;
  t = frame(app);
  assert.deepEqual([...s.openParts], [], "every part folds");
  click(t, "fp-pill-main");
  t = frame(app);
  click(t, "fp-sections-toggle");
  rootDirty = true;
  t = frame(app);
  assert.deepEqual([...s.openParts].sort(), every, "the list's toggle opens every part");
  assert.equal(byId(t, "fp-sections").length, 1, "and the list stays open");
  assert.ok(has(one(t, "fp-sections-toggle"), "tooltip", "Collapse all parts"));
  fire(one(t, "fp-pill-layer").out);
}

// ---- the to-do list (decision 12): ‹ and › visit, in form order, every row
// that still needs the user; a step scrolls the row into view and puts the
// keyboard in its box; past the last row › is Check.
const parentOf = (tree, m) => nodes(tree).find((x) => x.kids.some((k) => meta.get(k) === m));
const tracking = (tree, handle) => nodes(tree).find((m) => m.calls.some((c) => c[0] === "track_focus" && c[1] === handle));
{
  const without = (...keys) => Object.fromEntries(Object.entries(FILLED).filter(([k]) => !keys.map(K).includes(k)));
  openValues = without("txtYear", "TaxWithheld_1", "txtTelNum", "txtZipCode");
  openErrors = [{ field: "x", keys: [STOP_B], message: "Enter the total amount of compensation" }];
  stopEdits = { [K("txtTax16")]: false, [K("txtTax25")]: true };
  opened = {
    // A required box the profile fills and one another answer turns off are not the user's to do.
    locks: { [K("txtTelNum")]: "profile" },
    off: { [K("txtZipCode")]: "Needs item 9" },
    kept: { [K("txtTax18")]: "last", [K("txtTax19")]: "template" },
    auto_stops: [K("txtTax14"), K("txtTax15"), K("txtTax16")],
    stop_edits: stopEdits,
    stoppable: ["txtTax14", "txtTax15", "txtTax16", "txtTax17", "txtTax18", "txtTax25"].map(K),
  };
  hostScroll = true;
  const app = makeApp(1200);
  app.openForm({ tin: TIN, code: "1601C", year: 2026, period: 3 }, cx);
  await settle();
  const ui = app.formUi;
  const s = ui.state;
  const id = (name) => ui.itemIdForKey(K(name));
  const list = () => plain(ui.todo().map((r) => [r.id, r.reason]));
  assert.deepEqual(list(), [
    [id("txtYear"), "required"],
    // The tick pair is one item.
    [id("TaxWithheld_1"), "required"],
    // A message beats the row's stop.
    [id("txtTax14"), "message"],
    [id("txtTax15"), "stop"],
    // Switched on by the user; txtTax16, switched off, is not a stop.
    [id("txtTax25"), "stop"],
  ], "the rows to do, in form order, each with its strongest reason");
  rootDirty = true;
  let t = frame(app);
  assert.ok(texts(one(t, "fp-pill")).includes("5 to do · Page 1"), `the pill counts them: ${texts(one(t, "fp-pill"))}`);
  assert.ok(texts(one(t, "fp-pill-count")).includes("1"), "red: the one row with a message");
  assert.equal(one(t, "fp-scroll").component?.name, "ScrollArea", "the form scrolls in the host's scroll area");
  assert.ok(has(one(t, "fp-pill-prev"), "disabled", true), "‹ waits before the first row");
  // The list says, per section, how many rows still need the user.
  click(t, "fp-pill-list");
  t = frame(app);
  assert.ok(texts(one(t, "fp-section-0")).includes("2 to do"), "section 1: two rows to do");
  assert.ok(texts(one(t, "fp-section-2")).includes("Fix · 1"), "a section with a message says Fix");
  assert.equal(byId(t, "fp-guided").length, 0, "no Template stops switch: one behaviour");
  fire(one(t, "fp-pill-layer").out);
  t = frame(app);

  // › to the first row: its section opens, the row is tinted, and once drawn
  // it scrolls into view and the keyboard goes into its empty required box.
  hostCalls.length = 0;
  windowCalls.length = 0;
  click(t, "fp-pill-next");
  const first = id("txtYear");
  assert.equal(s.stepAt, first);
  t = frame(app);
  assert.ok(has(one(t, `fp-item-${first}`), "bg", PALETTES.light.accentSoft), "the row is tinted");
  assert.ok(texts(one(t, "fp-pill")).includes("1 of 5 to do · Page 1") && texts(one(t, "fp-pill")).includes("Item 1 · For the Month (MM/YYYY)"), `the pill names the row: ${texts(one(t, "fp-pill"))}`);
  assert.deepEqual(plain(one(t, `fp-row-mark-${first}`).component), { name: "ScrollMark", props: { name: `fp-row-${first}`, area: "fp-scroll" } }, "the row's mark is in the form's area");
  assert.deepEqual(hostCalls, [["scroll_forget", `fp-row-${first}`]], "nothing scrolls before the row is drawn again");
  assert.deepEqual(windowCalls, [], "and nothing is focused while drawing");
  runTimers();
  assert.deepEqual(hostCalls.at(-1), ["scroll_reveal", "fp-scroll", `fp-row-${first}`, 24], "the row is revealed 24 px below the top");
  assert.deepEqual(windowCalls, ["refresh", "focus_next"], "redrawn, then Tab order moves on from the anchor");
  const anchor = s.focusAnchors.get(K("txtYear"));
  assert.equal(focused.at(-1), anchor, "the year box's anchor took focus");
  const anchorEl = tracking(t, anchor);
  assert.ok(has(anchorEl, "tab_stop", false) && has(anchorEl, "size", 0), "the anchor is invisible and not a Tab stop");
  const wrap = parentOf(t, anchorEl);
  const at = wrap.kids.findIndex((k) => meta.get(k) === anchorEl);
  assert.equal(meta.get(wrap.kids[at + 1]).id, `fp-${K("txtYear")}`, "the anchor sits just before the box");

  // › to the tick pair: the group's own handle, no Tab move.
  click(t, "fp-pill-next");
  runTimers();
  const pair = id("TaxWithheld_1");
  assert.equal(s.stepAt, pair);
  assert.equal(focused.at(-1), s.focusAnchors.get(`radio:${pair}`), "the tick group took focus");
  assert.deepEqual(windowCalls, ["refresh", "focus_next", "refresh"], "a choice takes the keyboard itself");
  t = frame(app);
  assert.ok(tracking(t, s.focusAnchors.get(`radio:${pair}`)), "the group tracks its handle");

  // On through the message to the stops; ‹ goes back.
  click(t, "fp-pill-next");
  assert.equal(s.stepAt, id("txtTax14"));
  assert.deepEqual([...s.openParts], [2], "the row's section opens");
  t = frame(app);
  click(t, "fp-pill-next");
  assert.equal(s.stepAt, id("txtTax15"));
  t = frame(app);
  click(t, "fp-pill-prev");
  assert.equal(s.stepAt, id("txtTax14"), "‹ goes back a row");
  t = frame(app);
  click(t, "fp-pill-next");
  t = frame(app);
  click(t, "fp-pill-next");
  assert.equal(s.stepAt, id("txtTax25"));
  // Stepped past, the stop at txtTax15 leaves the list.
  assert.ok(!list().some(([r]) => r === id("txtTax15")), "a stop stepped past leaves the list");
  t = frame(app);
  assert.ok(texts(one(t, "fp-pill")).includes("4 of 4 to do · Part II"), `the last row: ${texts(one(t, "fp-pill"))}`);
  assert.ok(has(one(t, "fp-pill-next"), "tooltip", "Check"), "at the last row › is Check");
  // Filled, the last stop leaves the list too.
  s.values[K("txtTax25")] = "100.00";
  ui.refreshRows();
  assert.ok(!list().some(([r]) => r === id("txtTax25")), "a filled stop leaves the list");
  assert.deepEqual(plain(ui.stepTarget(1)), { check: true });

  // Check: it validates; what it finds is to do, from the top.
  openErrors = [{ field: "tin", keys: [K("txtTIN1")], message: "Enter the TIN" }];
  app.toastState = null;
  t = frame(app);
  click(t, "fp-pill-next");
  await settle();
  // One round: Check's wait; the toast's own clearing waits.
  runTimers(1);
  assert.ok(JSON.stringify(app.toastState).includes("1 item needs attention"), `Check says what it found: ${JSON.stringify(app.toastState)}`);
  assert.deepEqual(list(), [
    [id("txtYear"), "required"],
    [id("TaxWithheld_1"), "required"],
    [id("txtTIN1"), "message"],
    // txtTax14: its message fixed, and its stop stepped past on this visit.
  ], "Check's new message is to do; the fixed one left");
  t = frame(app);
  click(t, "fp-pill-next");
  assert.equal(s.stepAt, id("txtYear"), "after Check › starts from the top");

  // "Stop here": shown on the row a step went to, or under the pointer; three states.
  ui.jumpTo(2, cx);
  t = frame(app);
  const r16 = id("txtTax16");
  const r17 = id("txtTax17");
  assert.equal(byId(t, `fp-stop-${r16}`).length, 0, "not on every row at once");
  s.hoverItem = r16;
  ui.refreshRows();
  t = frame(app);
  let sw = one(t, `fp-stop-${r16}`);
  assert.ok(has(sw, "bg", PALETTES.light.borderStrong), "an automatic stop switched off shows off");
  traces.length = 0;
  invalidations = 0;
  fire(sw.click);
  t = frame(app);
  assert.deepEqual(writes.at(-1), ["set_template_stop", TIN, "1601C", K("txtTax16"), null], "on again: back to automatic");
  assert.deepEqual([...new Set(traces)], [`fp-item-${r16}`], "only its row rebuilds");
  assert.equal(invalidations, 0, "the page does not redraw");
  assert.ok(has(one(t, `fp-stop-${r16}`), "bg", PALETTES.light.accent), "it is a stop");
  assert.ok(list().some(([r, why]) => r === r16 && why === "stop"), "and to do");
  fire(one(t, `fp-stop-${r16}`).click);
  assert.deepEqual(writes.at(-1), ["set_template_stop", TIN, "1601C", K("txtTax16"), false], "an automatic stop switched off");
  s.hoverItem = r17;
  ui.refreshRows();
  t = frame(app);
  assert.equal(byId(t, `fp-stop-${r16}`).length, 0, "the pointer left: the switch goes");
  fire(one(t, `fp-stop-${r17}`).click);
  assert.deepEqual(writes.at(-1), ["set_template_stop", TIN, "1601C", K("txtTax17"), true], "a box the app would not stop on, switched on");
  assert.ok(s.stopKeys.has(K("txtTax17")));
  t = frame(app);
  fire(one(t, `fp-stop-${r17}`).click);
  assert.deepEqual(writes.at(-1), ["set_template_stop", TIN, "1601C", K("txtTax17"), null], "switched off again: back to automatic");
  assert.ok(!s.stopKeys.has(K("txtTax17")));
  s.hoverItem = null;
  ui.refreshRows();

  // Copied answers are marked until edited.
  t = frame(app);
  const r18 = id("txtTax18");
  const r19 = id("txtTax19");
  assert.ok(has(one(t, `fp-mark-history-${r18}`), "tooltip", "Same as last time"), "from the last return");
  assert.ok(has(one(t, `fp-mark-layout-template-${r19}`), "tooltip", "From your template"), "from the template");
  ui.edit(K("txtTax18"), "5", cx);
  ui.refreshRows();
  t = frame(app);
  assert.equal(byId(t, `fp-mark-history-${r18}`).length, 0, "edited: the mark goes");
  assert.equal(byId(t, `fp-mark-layout-template-${r19}`).length, 1, "the other stays");

  // ⌘→ / ⌘← on the form page step (the root's actions).
  rootDirty = true;
  t = frame(app);
  const action = (name) => one(t, "app-root").calls.find((c) => c[0] === "on_action" && c[1] === name)[2];
  ui.jumpTo(0, cx);
  let propagated = 0;
  const kcx = () => ({ ...context(null), propagate() { propagated += 1; } });
  action("next-to-do")({}, kcx());
  assert.equal(s.stepAt, id("txtYear"), "⌘→ steps to the first row");
  action("next-to-do")({}, kcx());
  assert.equal(s.stepAt, id("TaxWithheld_1"), "and on");
  action("previous-to-do")({}, kcx());
  assert.equal(s.stepAt, id("txtYear"), "⌘← steps back");
  assert.equal(propagated, 0, "on the form page the chord stops there");
  // The palette offers both, with their keys.
  app.palette = { input: InputState.new({ value: "" }) };
  const palette = app.paletteItems().filter((x) => /to do/.test(String(x.label)));
  assert.deepEqual(plain(palette.map((x) => [String(x.label), String(x.hint)])), [["Next to do", "Ctrl →"], ["Previous to do", "Ctrl ←"]]);
}

// The narrow page reveals in its own scroll area.
{
  openValues = Object.fromEntries(Object.entries(FILLED).filter(([k]) => k !== K("txtYear")));
  opened = {};
  hostScroll = true;
  const app = makeApp(500);
  app.openForm({ tin: TIN, code: "1601C", year: 2026, period: 3 }, cx);
  await settle();
  rootDirty = true;
  let t = frame(app);
  assert.equal(one(t, "main-scroll").component?.name, "ScrollArea", "narrow: main-scroll is the host's scroll area");
  hostCalls.length = 0;
  click(t, "fp-pill-next");
  runTimers();
  assert.deepEqual(hostCalls.at(-1), ["scroll_reveal", "main-scroll", `fp-row-${app.formUi.itemIdForKey(K("txtYear"))}`, 24]);
}

// An older host: no kept, auto_stops, stop_edits or stoppable, and no scroll
// module. Nothing breaks: required boxes and messages are to do, a step tints
// the row and nothing scrolls.
{
  openValues = Object.fromEntries(Object.entries(FILLED).filter(([k]) => k !== K("txtYear")));
  opened = {};
  hostScroll = false;
  const app = makeApp(1200);
  app.openForm({ tin: TIN, code: "1601C", year: 2026, period: 3 }, cx);
  await settle();
  const s = app.formUi.state;
  assert.deepEqual(plain([s.kept, s.autoStops, s.stopEdits, s.stopKeys.size, s.stoppable.size]), [{}, [], {}, 0, 0]);
  rootDirty = true;
  let t = frame(app);
  assert.ok(has(one(t, "fp-scroll"), "overflow_y_scroll"), "a plain scroll area");
  hostCalls.length = 0;
  windowCalls.length = 0;
  click(t, "fp-pill-next");
  runTimers();
  t = frame(app);
  assert.equal(s.stepAt, app.formUi.itemIdForKey(K("txtYear")));
  assert.ok(has(one(t, `fp-item-${s.stepAt}`), "bg", PALETTES.light.accentSoft), "the row is tinted");
  assert.deepEqual([hostCalls, windowCalls], [[], []], "no scroll calls, no focus");
  assert.ok(!nodes(t).some((m) => /^fp-stop-/.test(m.id || "")), "no Stop here anywhere");
  openValues = FILLED;
  openErrors = [];
  stopEdits = {};
}

// Off the form page the toast keeps its place at the window's edge.
{
  const app = makeApp(1200);
  app.toast("Saved", cx);
  rootDirty = true;
  const t = frame(app);
  assert.equal(byId(t, "fp-pill").length, 0);
  assert.ok(has(one(t, "toast"), "bottom", 20), "no pill: 20 px");
  assert.ok(has(one(t, "toast"), "left", 0) && has(one(t, "toast"), "right", 0), "and the window's middle");
  // The Filings board takes the whole pane; other pages keep the reading column.
  const home = app.layout();
  app.view = "filings";
  const filings = app.layout();
  assert.equal(filings.column, filings.mainWidth - 2 * filings.padX, "Filings: the pane less its padding");
  assert.ok(home.column < filings.column && home.column <= 880, "Home keeps the 880 px column");
  app.view = "home";
}

// ---- a press on the pill does not reach a section header painted beneath it
// (the re-test: a header under the pill toggled with the press).
for (const width of [1200, 500]) {
  const app = makeApp(width);
  app.openForm({ tin: TIN, code: "1601C", year: 2026, period: 3 }, cx);
  await settle();
  rootDirty = true;
  let t = frame(app);
  const s = app.formUi.state;
  const under = "fp-part-3-toggle";
  assertTakesPointer(one(t, "fp-pill"), `${width}: the pill`);
  const before = [...s.openParts];
  // The middle opens the list and nothing else.
  assert.deepEqual(press(t, ["fp-pill-main", under]), ["fp-pill-main"], `${width}: only the pill's own click fires`);
  t = frame(app);
  assert.deepEqual([...s.openParts], before, `${width}: the header beneath did not toggle`);
  assert.equal(byId(t, "fp-sections").length, 1, `${width}: the list is open`);
  assertTakesPointer(one(t, "fp-sections"), `${width}: the ${width >= 760 ? "popover" : "sheet"}`);
  if (width < 760) assertTakesPointer(one(t, "fp-sections-scrim"), `${width}: the scrim`);
  // A row of the list opens its section, and only that.
  const fired = press(t, ["fp-section-1", under, ...(width < 760 ? ["fp-sections-scrim"] : [])]);
  assert.deepEqual(fired, ["fp-section-1"], `${width}: a row's press stays on the row`);
  assert.deepEqual([...s.openParts], [1], `${width}: the chosen section opens, the one beneath does not toggle`);
  t = frame(app);
  // › steps once; the header beneath stays as it is.
  assert.deepEqual(press(t, ["fp-pill-next", under]), ["fp-pill-next"]);
  assert.deepEqual([...s.openParts], [2], `${width}: › opened the next section only`);
  if (width < 760) {
    // The scrim closes the sheet; the header beneath it does not toggle.
    t = frame(app);
    click(t, "fp-pill-list");
    t = frame(app);
    assert.deepEqual(press(t, ["fp-sections-scrim", under]), [], `${width}: the scrim's press reaches nothing beneath`);
    t = frame(app);
    assert.equal(byId(t, "fp-sections").length, 0, "the scrim closed the sheet");
    assert.deepEqual([...s.openParts], [2]);
  }
  // A toast over the form takes the pointer too.
  app.toast("Saved", cx);
  rootDirty = true;
  assertTakesPointer(one(frame(app), "toast-card"), `${width}: the toast`);
  app.toastState = null;
}

console.log("PASS: no taxpayer card; openForm selects the taxpayer; the section pill at 500, 1200 and 1240 px, its list, the to-do steps, Check, Stop here, copied-answer marks, scroll and focus after a step, and ⌘→ / ⌘←.");
