// The sidebar, the tax profile and notifications on a narrow window, driven
// through the real app/main.js paint and app/shell/chrome.js with a mocked
// kit and host. Below 860 px opening one never moves or resizes the main
// surface. Below 600 px it fills the window under the title bar, with no
// scrim, and the title-bar toggle, Esc and navigating close it. From 600 to
// 860 px it is the fixed-width overlay over a scrim; at 1000 px it docks.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods, strictElement } from "./kit_surface.mjs";

const root = new URL("../", import.meta.url);
const KIT = kitMethods(root);

// A mock element records every call the screen made on it.
const meta = new WeakMap();
function el() {
  const e = strictElement(KIT, (k, args, self) => {
    const m = meta.get(self);
    if (k === "child") m.kids.push(args[0]);
    else if (k === "children") m.kids.push(...args[0]);
    else m.calls.push([k, ...args]);
    if (k === "id") m.id = String(args[0]);
    if (k === "when" && args[0]) args[1](self);
    if (k === "when_some" && args[0] != null) args[1](self, args[0]);
  });
  meta.set(e, { id: null, calls: [], kids: [] });
  return e;
}
function nodes(e, out = [], parent = null) {
  const m = meta.get(e);
  if (!m) return out;
  m.parent = parent;
  out.push(m);
  for (const k of m.kids) nodes(k, out, m);
  return out;
}
const byId = (tree, id) => nodes(tree).filter((m) => m.id === id);
const one = (tree, id) => {
  const all = byId(tree, id);
  assert.equal(all.length, 1, `exactly one #${id}`);
  return all[0];
};
const has = (m, name, ...args) => m.calls.some((c) => c[0] === name && args.every((a, i) => c[i + 1] === a));
/**
 * A floating element takes the pointer: its left press and release call
 * stop_propagation(), so GPUI (whose hitboxes do not occlude; the bubble
 * phase runs last-painted first until stopped) never arms a click beneath it.
 */
function assertTakesPointer(m, what) {
  for (const name of ["on_mouse_down", "on_mouse_up"]) {
    const c = m.calls.find((x) => x[0] === name && x[1] === "left");
    assert.ok(c, `${what} has ${name}`);
    let stopped = false;
    c[2]({}, { ...cx, notify() {}, stop_propagation() { stopped = true; } });
    assert.ok(stopped, `${what}: ${name} stops propagation`);
  }
}
// A node's own styling, without callbacks (they differ per render).
const style = (m) => JSON.stringify(m.calls.map((c) => c.map((a) => (typeof a === "function" ? "fn" : a))));
const ids = (m, out = []) => {
  if (m.id) out.push(m.id);
  for (const k of m.kids) if (meta.get(k)) ids(meta.get(k), out);
  return out;
};
const click = (tree, id) => {
  const m = one(tree, id);
  const call = m.calls.find((c) => c[0] === "on_click");
  assert.ok(call, `#${id} has a click handler`);
  call[1]({}, cx);
};

// The host window.
let viewport = { width: 1000, height: 800 };
const storage = new Map();
const windowStub = {
  viewport_size: () => viewport,
  localStorage: { getItem: (k) => storage.get(k) ?? null, setItem: (k, v) => storage.set(k, v) },
};

const context = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Proxy, WeakMap, window: windowStub });
const mock = (name, exports) =>
  new vm.SyntheticModule(Object.keys(exports), function () {
    for (const [k, v] of Object.entries(exports)) this.setExport(k, v);
  }, { context, identifier: name });
const source = (path) => new vm.SourceTextModule(fs.readFileSync(new URL(path, root), "utf8"), { context, identifier: path });
const stub = () => el();
const stubClass = class {
  constructor() {
    return new Proxy(this, { get: (o, k) => (k in o ? o[k] : () => null) });
  }
};
const InputState = { new: () => ({ value: () => "", set_value() {}, on() {} }) };
class View {}

const real = {
  "./shell/store.js": source("app/shell/store.js"),
  "./shell/theme.js": source("app/shell/theme.js"),
  "./shell/ui.js": source("app/shell/ui.js"),
  "./shell/chrome.js": source("app/shell/chrome.js"),
  "./shell/facts.js": source("app/shell/facts.js"),
  "./shell/chrome_gate.js": source("app/shell/chrome_gate.js"),
  "./shell/recent.js": source("app/shell/recent.js"),
};
const periods = source("app/shell/periods.js");
const linked = {
  "gpui-kit": mock("gpui-kit", { View, div: el, svg: el }),
  "gpui-base": mock("gpui-base", { Input: { new: stub }, InputState, Popup: { new: stub }, OtpInput: { new: stub }, OtpState: InputState, set_theme() {} }),
  process: mock("process", { default: { env: {} } }),
  bir: mock("bir", {}),
  "./shell/home.js": mock("home", { renderHome: stub }),
  "./shell/library.js": mock("library", { renderLibrary: stub }),
  "./shell/calendar_page.js": mock("calendar", { renderCalendar: stub, CalendarFilters: stubClass }),
  "./shell/filings.js": mock("filings", { renderFilings: stub, FilingsFilters: stubClass }),
  "./shell/settings.js": mock("settings", { renderSettings: stub }),
  "./shell/taxpayer.js": mock("taxpayer", { renderTaxpayerPage: stub, renderNewTaxpayer: stub, TaxpayerActions: stubClass }),
  "./shell/form_page.js": mock("form_page", {
    renderFormPage: stub,
    renderFormSplit: () => null,
    formColumnBox: () => null,
    formScrollArea: (_app, id, plain) => plain(stub().id(id)).overflow_y_scroll().overflow_x_hidden(),
    renderExistingOffer: stub,
    renderRemovalConfirm: stub,
    renderQueueConfirm: stub,
    renderSamplePeriod: stub,
    FormPageActions: stubClass,
  }),
  "./shell/auth.js": mock("auth", { renderSignIn: stub, renderSetup: stub }),
  "./shell/gate.js": mock("gate", { renderAccountGate: stub }),
  "./shell/update.js": mock("update", { installReadyUpdate() {}, pollUpdateStatus: () => false, renderUpdatePrompt: (_cx, _app, page) => page, setAutoUpdateDownload() {} }),
  "./shell/security.js": mock("security", { SecurityActions: stubClass }),
  "./shell/account.js": mock("account", { AccountActions: stubClass }),
  "./shell/setup_change.js": mock("setup_change", { SetupChange: stubClass }),
  "./shell/cron_tasks.js": mock("cron_tasks", { renderCronTasksPage: stub }),
};
// chrome.js and its neighbours import each other as "./x.js".
for (const [k, v] of Object.entries(real)) {
  linked[k] = v;
  linked[k.replace("./shell/", "./")] = v;
}
linked["./periods.js"] = periods;
linked["./todo.js"] = source("app/shell/todo.js");
const main = source("app/main.js");
await main.link((s) => {
  if (!linked[s]) throw new Error(`unlinked import ${s}`);
  return linked[s];
});
await main.evaluate();

// BirRoot builds BirApp through cx.new; take the class from there.
let BirApp = null;
new main.namespace.default().init({}, { new: (Class) => (BirApp = Class) });
assert.ok(BirApp, "BirApp class");

const cx = {
  notify() {},
  theme: () => ({ is_dark: false }),
  timer: { after() {}, every() {} },
};

// The demo taxpayer and one submitted return (one notification).
const TIN = "00012345300000";
const taxpayer = {
  tin_root: "000123453",
  name: "Juana Santos",
  tin_display: "000-123-453-00000",
  branches: [{ code: "00000", name: "Head office", tin: TIN, is_head_office: true, facts: { registered_address: "", rdo_code: "" } }],
};
const filed = { id: 7, status: "Submitted", form_code: "1601C", period_label: "March 2026", branch_count: 1, taxpayer_name: "Juana Santos", branch_name: "Head office", updated_at: "2026-04-10 08:00:00" };
const store = {
  taxpayers: () => [taxpayer],
  authState: () => ({ mode: "solo" }),
  returns: (kind) => (kind === "filed" ? [filed] : []),
  taxYears: () => [2026],
  yearlyForms: () => ["1601C"],
  templates: () => [],
  taxTypes: () => [],
  yearFacts: () => null,
  electionReadiness: () => null,
  screenIndex: () => [{ c: "1601C", freq: "Monthly" }],
  logScreen() {},
  call() {},
};

function makeApp(width) {
  viewport = { width, height: 800 };
  const app = Object.create(BirApp.prototype);
  Object.assign(app, {
    store,
    booted: true,
    cloudLocked: false,
    appLocked: false,
    setup: { step: "choose" },
    setupPending: null,
    view: "home",
    history: [],
    forward: [],
    sidebarOpen: width >= 860,
    panel: null,
    lastPanel: "profile",
    isMac: false,
    isLinux: false,
    platformKnown: true,
    recent: [{ key: `${TIN}|1601C|2026|3`, tin: TIN, code: "1601C", year: 2026, period: 3 }],
    prefs: { showRecent: true },
    accountMenu: false,
    settingsMenu: null,
    overlay: null,
    toastState: null,
    themePref: "system",
    readIds: new Set(),
    session: { kind: "open" },
    selRoot: taxpayer.tin_root,
    selCode: "00000",
    selYear: 2026,
    wasNarrow: width < 860,
    focusRoot: { focus() {} },
    taxpayerUi: { panelEdit: null, reset() {}, leaveView() {}, leaveYear() {} },
    formUi: { open() {}, leavingPage() {}, askingRemoval: () => false },
    security: { leave() {} },
    setupChange: { leave() {} },
  });
  return app;
}

/** What opens each of the three, from the title bar. */
const OPEN = {
  sidebar: (app) => app.toggleSidebar(cx),
  profile: (app) => app.togglePanel("profile", cx),
  notifs: (app) => app.togglePanel("notifs", cx),
};
const TOGGLE = { sidebar: "tb-sidebar", profile: "context-pill", notifs: "tb-notifications" };
const ROOT_ID = { sidebar: "sidebar", profile: "panel-profile", notifs: "panel-notifications" };
const closed = (app) => !app.sidebarOpen && !app.panel;

/** The overlay container: the absolute box that holds the sidebar or panel. */
function overlayBox(tree, which) {
  const m = one(tree, ROOT_ID[which]);
  assert.ok(m.parent && has(m.parent, "absolute"), `${which} sits in an absolute overlay`);
  return m;
}

/** The main surface, its column and the title bar's controls: what must not move. */
function geometry(app) {
  const tree = app.paint(cx);
  const L = app.layout();
  const main = one(tree, "main-surface");
  const body = main.parent;
  return JSON.stringify({
    main: style(main),
    beside: body.kids.map((k) => meta.get(k).id),
    width: [L.mainWidth, L.padX, L.column],
    titleBar: ids(one(tree, "title-bar")),
  });
}

for (const width of [500, 700]) {
  for (const which of Object.keys(OPEN)) {
    const app = makeApp(width);
    const before = geometry(app);
    OPEN[which](app);
    assert.ok(!closed(app), `${which} opened at ${width}`);
    assert.equal(geometry(app), before, `${which} at ${width} leaves the main surface where it was`);

    const tree = app.paint(cx);
    const box = overlayBox(tree, which);
    const panel = box.parent;
    assert.ok(has(panel, "top", 40) && has(panel, "bottom", 0), `${which} at ${width} sits under the title bar`);
    // A press on the overlay never reaches the scrim or the page beneath it.
    assertTakesPointer(panel, `${which} at ${width}`);
    if (width >= 600) {
      assertTakesPointer(one(tree, "overlay-scrim"), `the scrim at ${width}`);
      OPEN[which](app);
    }
    if (width < 600) {
      assert.equal(byId(tree, "overlay-scrim").length, 0, `no scrim at ${width}`);
      assert.ok(has(panel, "left", 0) && has(panel, "right", 0), `${which} at ${width} spans the window`);
      assert.ok(has(panel, "bg"), `${which} at ${width} has its own background`);
      assert.ok(has(box, "w_full") && !has(box, "w"), `${which} at ${width} fills its box`);
      // No content column stays at the docked width.
      for (const m of nodes(panel.kids[0])) {
        assert.ok(!has(m, "w", 248) && !has(m, "w", 320), `${which} at ${width}: #${m.id || "?"} pinned to a docked width`);
      }
    } else {
      assert.equal(byId(tree, "overlay-scrim").length, 1, `scrim at ${width}`);
      const fixed = which === "sidebar" ? 248 : 320;
      assert.ok(has(box, "w", fixed) && !has(box, "w_full"), `${which} at ${width} is ${fixed} wide`);
      assert.ok(which === "sidebar" ? !has(panel, "right", 0) : !has(panel, "left", 0), `${which} at ${width} floats on one side`);
    }

    // Only one is open at a time.
    for (const other of Object.keys(OPEN)) {
      if (other === which) continue;
      OPEN[other](app);
      assert.equal(byId(app.paint(cx), ROOT_ID[which]).length, 0, `${other} replaces ${which} at ${width}`);
      OPEN[which](app);
    }
  }
}

// At 500: the title-bar toggle and Esc close each; navigating closes it.
for (const which of Object.keys(OPEN)) {
  const app = makeApp(500);
  OPEN[which](app);
  click(app.paint(cx), TOGGLE[which]);
  assert.ok(closed(app), `${TOGGLE[which]} closes ${which}`);

  OPEN[which](app);
  app.dismiss(cx);
  assert.ok(closed(app), `Esc closes ${which}`);
}
{
  // The panel button closes whichever panel is open.
  const app = makeApp(500);
  OPEN.notifs(app);
  click(app.paint(cx), "tb-panel");
  assert.ok(closed(app), "tb-panel closes notifications");
}
const navigations = [
  ["sidebar", "nav-calendar", "calendar"],
  ["sidebar", "nav-filings", "filings"],
  ["sidebar", `recent-${TIN}-1601C-2026-3`, "form"],
  ["profile", "panel-open-profile", "profile"],
  ["profile", "panel-open-forms", "forms"],
  ["profile", "panel-open-templates", "templates"],
  ["notifs", "notif-0", "filings"],
];
for (const [which, id, view] of navigations) {
  for (const width of [500, 700]) {
    const app = makeApp(width);
    OPEN[which](app);
    click(app.paint(cx), id);
    assert.equal(app.view, view, `${id} goes to ${view}`);
    assert.ok(closed(app), `${id} closes ${which} at ${width}`);
  }
}

// At 1000 both dock beside the main surface and navigating keeps them.
{
  const app = makeApp(1000);
  for (const which of ["profile", "notifs"]) {
    OPEN[which](app);
    const tree = app.paint(cx);
    assert.equal(byId(tree, "overlay-scrim").length, 0, "no scrim when docked");
    const body = one(tree, "main-surface").parent;
    assert.deepEqual(body.kids.map((k) => meta.get(k).id), ["sidebar", "main-surface", ROOT_ID[which]], `${which} docks at 1000`);
    assert.ok(has(one(tree, ROOT_ID[which]), "w", 320), `${which} is 320 wide docked`);
    assert.ok(has(one(tree, "sidebar"), "w", 248), "sidebar is 248 wide docked");
    app.panel = null;
  }
  OPEN.notifs(app);
  click(app.paint(cx), "notif-0");
  assert.equal(app.view, "filings");
  assert.ok(app.sidebarOpen && app.panel === "notifs", "docked chrome stays open on a wide window");
  assert.ok(app.readIds.has("return:7:submitted"), "opening a notification reads it");
}

// The account menu lies over the sidebar's Recent rows: it takes the pointer.
{
  const app = makeApp(1000);
  app.accountMenu = true;
  assertTakesPointer(one(app.paint(cx), "account-menu"), "the account menu");
  app.accountMenu = false;
  // A dialog over a scrim (Overlay: the palette, New return): the card and
  // the scrim that dismisses both keep their press.
  app.palette = { input: { value: () => "", on() {} }, index: 0 };
  app.newReturn = { code: null, period: null, periodOpen: false };
  app.overlay = "palette";
  const tree = app.paint(cx);
  assertTakesPointer(one(tree, "palette-card"), "the palette card");
  assertTakesPointer(one(tree, "palette"), "the palette's scrim");
}

console.log("test_shell_overlays: ok");
