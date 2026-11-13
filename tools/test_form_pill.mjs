// The form page without its taxpayer card and with the section pill (picks
// 8 C and 9 C), driven through the real app/main.js paint, app/shell/chrome.js,
// app/shell/form_page.js and app/shell/ui.js with a mocked kit and host:
// opening a return selects its taxpayer (the title-bar pill names it); the
// card is gone and its Sample, Opening… and assistant chips sit in the chip
// row; the pill floats over the form at 500 px (no chip row) and at 1200 and
// 1240 px (beside the kept side list); its list is a bottom sheet narrow and
// a popover wide, and opening it redraws only the pill; choosing a section
// opens it; ‹ and › step by section, or with a template by the stops it
// marks (a step opens the section and marks the row); "Stop here" marks a
// box in the template and rebuilds only its row.
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
const settle = async () => { while (tasks.length) await Promise.all(tasks.splice(0)); };

// ---- the real app
let viewport = { width: 1200, height: 800 };
const storage = new Map();
const windowStub = { viewport_size: () => viewport, localStorage: { getItem: (k) => storage.get(k) ?? null, setItem: (k, v) => storage.set(k, v) } };
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
  "./shell/gallery.js": mock("gallery", { renderGallery: stub }),
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
const STOP_A = "frm1601c:CatAgent_P";
const STOP_B = "frm1601c:txtTax14";
const STOP_C = "frm1601c:txtTax15";
let template = null;
let stops = null;
let openErrors = [];
const writes = [];
const store = {
  taxpayers: () => taxpayers,
  authState: () => ({ mode: "solo" }),
  returns: () => [],
  taxYears: () => [2026],
  yearlyForms: () => ["1601C"],
  templates: () => (template ? [template] : []),
  screenIndex: () => [{ c: "1601C", freq: "Monthly" }],
  logScreen() {},
  call(name, ...args) {
    if (name === "screen_form_schema") return schema;
    if (name === "screen_form_open" || name === "screen_form_sample") {
      return Promise.resolve({ draft: { id: 1, taxable_year: 2026 }, status: "Draft", editable: true, values: {}, locks: {}, errors: openErrors, sample: name === "screen_form_sample" });
    }
    if (name === "screen_form_validate") return Promise.resolve({ draft: args[1], values: {}, options: {}, errors: openErrors });
    if (name === "template_stops") return stops;
    throw Error(`unexpected host call ${name}`);
  },
  write(name, ...args) {
    writes.push([name, ...args]);
    if (name === "set_template_stop") {
      const [, , key, on] = args;
      stops = { stops: on ? [...stops.stops, key] : stops.stops.filter((k) => k !== key), markable: stops.markable };
      return stops;
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
  assert.ok(texts(one(t, "fp-pill-count")).includes("1"), "one section needs attention");
  assert.ok(texts(one(t, "fp-pill")).includes("Return details") && texts(one(t, "fp-pill")).includes("Page 1 · 1 of 4"), "the pill names the open section");
  assert.ok(has(one(t, "fp-pill-prev"), "disabled", true), "‹ waits at the first section");

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

  // ‹ and › by section, no template.
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
  click(t, "fp-pill-main");
  t = frame(app);
  assert.equal(byId(t, "fp-sections-scrim").length, 0, "wide: a popover, no scrim");
  assert.equal(byId(one(t, "fp-pill-layer"), "fp-sections").length, 1, "above the pill");
  fire(one(t, "fp-pill-layer").out);
  t = frame(app);
  assert.equal(byId(t, "fp-sections").length, 0, "a click outside closes it");
}

// ---- a template: ‹ and › stop on what it marks; "Stop here" marks a box
{
  template = { form_code: "1601C", updated_at: "", answers_kept: 0, marked: 2, source: "blank" };
  stops = { stops: [STOP_B, STOP_A], markable: [STOP_A, STOP_B, STOP_C] };
  const app = makeApp(1200);
  app.openForm({ tin: TIN, code: "1601C", year: 2026, period: 3 }, cx);
  await settle();
  const ui = app.formUi;
  const s = ui.state;
  const [A, B, C] = [STOP_A, STOP_B, STOP_C].map((k) => ui.itemIdForKey(k));
  assert.ok(s.guided, "template mode starts on when the template marks fields");
  assert.deepEqual([...s.stops.order], [A, B], "stops in page order");
  rootDirty = true;
  let t = frame(app);
  assert.ok(texts(one(t, "fp-pill")).includes("2 template stops · Page 1"), `the pill counts the stops: ${texts(one(t, "fp-pill"))}`);
  click(t, "fp-pill-next");
  t = frame(app);
  assert.equal(s.stopAt, A, "› goes to the first stop");
  assert.deepEqual([...s.openParts], [Number(A.split(":")[0])], "its section opens");
  assert.ok(has(one(t, `fp-item-${A}`), "bg", PALETTES.light.accentSoft), "its row is marked (the page cannot scroll to it)");
  assert.ok(texts(one(t, "fp-pill")).some((x) => x.startsWith("Stop 1 of 2")), "the pill names the stop");
  click(t, "fp-pill-next");
  t = frame(app);
  assert.equal(s.stopAt, B, "› goes to the next stop, in another section");
  assert.ok(has(one(t, `fp-item-${B}`), "bg", PALETTES.light.accentSoft), "the mark moves with the stop");
  assert.ok(has(one(t, "fp-pill-next"), "disabled", true), "› waits at the last stop");
  click(t, "fp-pill-prev");
  t = frame(app);
  assert.equal(s.stopAt, A, "‹ goes back a stop");

  // Stop here on C: the template marks it; only C's row rebuilds.
  click(t, "fp-pill-next");
  t = frame(app);
  traces.length = 0;
  invalidations = 0;
  const sw = one(t, `fp-stop-${C}`);
  assert.ok(has(sw, "bg", PALETTES.light.borderStrong), "C is not a stop yet");
  fire(sw.click);
  t = frame(app);
  assert.deepEqual(writes.at(-1), ["set_template_stop", TIN, "1601C", STOP_C, true], "the template marks C's field");
  assert.deepEqual([...new Set(traces)], [`fp-item-${C}`], "only C's row rebuilds");
  assert.equal(invalidations, 0, "the page does not redraw");
  assert.ok(has(one(t, `fp-stop-${C}`), "bg", PALETTES.light.accent), "C is a stop");
  assert.deepEqual([...s.stops.order], [A, B, C]);
  click(t, "fp-pill-next");
  assert.equal(s.stopAt, C, "› now stops on C");
  // Unmark it again.
  t = frame(app);
  fire(one(t, `fp-stop-${C}`).click);
  assert.deepEqual(writes.at(-1), ["set_template_stop", TIN, "1601C", STOP_C, false]);
  assert.deepEqual([...s.stops.order], [A, B]);

  // Template stops off (the switch the Guided mode switch became): by section again.
  t = frame(app);
  click(t, "fp-pill-list");
  t = frame(app);
  fire(one(t, "fp-guided").click);
  t = frame(app);
  assert.equal(s.guided, false);
  assert.equal(byId(t, `fp-stop-${C}`).length, 0, "no Stop here out of template mode");
  const before = s.current;
  rootDirty = true;
  t = frame(app);
  click(t, "fp-pill-next");
  assert.equal(s.current, before + 1, "› steps by section");
  assert.equal(s.stopAt, null);
  template = null;
  stops = null;
}

console.log("PASS: no taxpayer card; openForm selects the taxpayer; the section pill at 500, 1200 and 1240 px, its list, ‹ › and the template stops.");
