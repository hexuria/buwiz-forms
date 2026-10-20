// Taxpayer page profile export, driven through the real
// app/shell/taxpayer.js with a mocked `bir` host: PIN cells appear only when
// taxpayer_auth reports has_pin; Export sends export_profile(branch tin,
// pin|null); a typed PIN is cleared from the cells after every attempt and
// never kept in page state; success toasts "Profile exported.", a host
// refusal toasts its message, and a cancelled dialog is quiet.
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
uiExports.Icon = () => el();
uiExports.AppButton = (_cx, id, label, opts = {}) => el({ button: id, label, onClick: opts.onClick });
uiExports.Avatar = () => el();
uiExports.Chip = (_cx, chip) => el({ chip });
uiExports.Segmented = (_cx, id) => el({ segmented: id });
uiExports.Switch = (_cx, id) => el({ switch: id });
uiExports.CheckboxLabel = () => el({ checkboxLabel: true });
uiExports.Checkbox = (_cx, id) => el({ checkbox: id });
uiExports.TextField = (_cx, _s, o = {}) => el({ field: o.id });
uiExports.Dropdown = (_cx, id) => el({ dropdown: id });
uiExports.Field = (_cx, label, opts = {}) => kidsOf(el({ field: label }), [opts.input, opts.inheritSwitch]);
uiExports.FieldGrid = (_cx, _w, cells) => kidsOf(el({ fieldGrid: true }), cells.map((c) => c && c.el));
uiExports.Disclosure = (_cx, id, opts = {}) => kidsOf(el({ disclosure: id, title: opts.title }), [opts.body]);
uiExports.DisclosureGroup = (_cx, id, sections) => kidsOf(el({ group: id }), sections);
uiExports.FormActions = (_cx, id) => el({ formActions: id });
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
  "gpui-kit": mock("gpui-kit", { div: () => el() }),
  "gpui-base": mock("gpui-base", { InputState, OtpState }),
  "./ui.js": mock("./ui.js", uiExports),
  "./store.js": mock("./store.js", storeExports),
  "./library.js": mock("./library.js", libraryExports),
};
await taxpayerMod.link((s) => linked[s]);
await taxpayerMod.evaluate();
const { TaxpayerActions, renderTaxpayerPage } = taxpayerMod.namespace;

const TIN = "12345678800000";
const ROOT = "123456788";
const cx = { notify: () => {}, write_to_clipboard: () => {} };

/**
 * An app showing one head-office taxpayer; `export_profile` is recorded and
 * answers `exportResult`, or throws `exportError`.
 */
function makeApp({ hasPin, exportResult, exportError }) {
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
    yearForms: () => [],
    templates: () => [],
    go: () => {},
    startNewReturn: () => {},
    store: {
      inboxState: () => ({ auth_method: "GoogleOAuth", oauth_connected: false, tracking_enabled: false, last_check_at: null }),
      write: (name, ...args) => {
        calls.push([name, ...args]);
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

// No PIN configured: a direct Export button, no PIN cells.
{
  const { app, calls, toasts } = makeApp({ hasPin: false, exportResult: { kind: "exported", path: "/tmp/p.zip" } });
  const tree = renderTaxpayerPage(cx, app, "profile", 900);
  assert.equal(pinCells(tree).length, 0, "no PIN cells without a configured PIN");
  const btn = exportBtn(tree);
  assert.ok(btn, "Export button rendered");
  click(btn);
  assert.deepEqual(calls, [["export_profile", TIN, null]]);
  assert.deepEqual(toasts.map((x) => x.msg), ["Profile exported."]);
}

// PIN configured: PIN cells render, and Export sends the typed digits.
{
  const { app, calls, toasts } = makeApp({ hasPin: true, exportResult: { kind: "exported", path: "/tmp/p.zip" } });
  const tree = renderTaxpayerPage(cx, app, "profile", 900);
  assert.equal(pinCells(tree).length, 1, "PIN cells render when has_pin");
  app.taxpayerUi.exportOtp.set_value("4321");
  click(exportBtn(tree));
  assert.deepEqual(calls, [["export_profile", TIN, "4321"]]);
  assert.equal(app.taxpayerUi.exportOtp.value(), "", "PIN cells cleared after the attempt");
  assert.deepEqual(toasts.map((x) => x.msg), ["Profile exported."]);
}

// PIN configured: the host's refusal is toasted and the cells still clear.
{
  const { app, calls, toasts } = makeApp({ hasPin: true, exportError: new Error("Wrong PIN.") });
  const tree = renderTaxpayerPage(cx, app, "profile", 900);
  assert.equal(pinCells(tree).length, 1);
  app.taxpayerUi.exportOtp.set_value("0000");
  click(exportBtn(tree));
  assert.deepEqual(calls, [["export_profile", TIN, "0000"]]);
  assert.equal(app.taxpayerUi.exportOtp.value(), "", "PIN cells cleared after a refusal");
  assert.deepEqual(toasts.map((x) => x.msg), ["Wrong PIN."]);
}

// PIN configured: fewer than four digits never reaches the host (a partial
// PIN would still burn a rate-limited attempt); the cells clear.
{
  const { app, calls, toasts } = makeApp({ hasPin: true, exportResult: { kind: "exported" } });
  renderTaxpayerPage(cx, app, "profile", 900);
  app.taxpayerUi.exportOtp.set_value("12");
  app.taxpayerUi.exportProfile(cx);
  assert.equal(calls.length, 0, "partial PIN never reaches the host");
  assert.equal(app.taxpayerUi.exportOtp.value(), "");
  assert.deepEqual(toasts.map((x) => x.msg), ["Type all 4 digits of the profile PIN first."]);
}

// A cancelled save dialog is quiet: no toast, no export claim.
{
  const { app, calls, toasts } = makeApp({ hasPin: false, exportResult: { kind: "cancelled" } });
  const tree = renderTaxpayerPage(cx, app, "profile", 900);
  click(exportBtn(tree));
  assert.deepEqual(calls, [["export_profile", TIN, null]]);
  assert.equal(toasts.length, 0, "cancelled dialog makes no toast");
}

console.log("test_taxpayer_export: ok");
