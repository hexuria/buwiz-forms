// Home and Calendar with as-needed forms (decision D8c, 2026-10-18), driven
// through the real app/shell/home.js, calendar_page.js and store.js with a
// mocked `bir` host: Home lists the as-needed forms the taxpayers file in an
// "As needed" section, each with "Start a return", and under each the saved
// returns that hold no trigger date yet, with "Open"; a return that holds its
// trigger date is a dated row in "Due in the next 30 days" and on the
// Calendar, with its computed due date, and opens exactly that return. The
// Calendar never shows the undated list. With no as-needed forms, Home has
// no "As needed" section.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";
import { kitMethods, strictElement } from "./kit_surface.mjs";

const root = new URL("../", import.meta.url);
const KIT = kitMethods(root);

// A mock element tree: each element keeps what the screen passed it.
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
/** Every node under an already-found node (its `meta` record), itself first. */
function subtree(m) {
  const out = [m];
  for (const k of m.kids) nodes(k, out);
  return out;
}
const find = (tree, pred) => nodes(tree).filter((m) => pred(m.info));
const one = (tree, pred, what) => {
  const all = find(tree, pred);
  assert.equal(all.length, 1, `exactly one ${what}`);
  return all[0];
};

const uiExports = { tokens: () => new Proxy({}, { get: () => "#000" }), SPACE: [0, 4, 8, 12, 16, 20, 24, 28, 32] };
uiExports.Text = (_cx, _v, text) => el({ text: String(text) });
uiExports.Code = (_cx, code) => el({ code });
uiExports.Icon = () => el();
uiExports.StepCircle = () => el();
uiExports.NameCell = (_cx, title, sub) => el({ title, sub });
uiExports.AppButton = (_cx, id, label, opts = {}) => el({ button: id, label, onClick: opts.onClick });
uiExports.StatusChip = (_cx, chip) => el({ chip });
uiExports.ListRow = (_cx, id, opts = {}) => el({ row: id, onClick: opts.onClick });
uiExports.ListGroup = (_cx, id, rows) => {
  const g = el({ group: id });
  meta.get(g).kids.push(...rows);
  return g;
};
uiExports.EmptyRow = (_cx, id, text) => el({ empty: id, text });
uiExports.PageHeader = (_cx, { title, lede }) => el({ header: title, lede });
uiExports.SectionTitle = (_cx, label) => el({ section: label });
uiExports.Segmented = (_cx, id) => el({ segmented: id });

// The host: what deadlines_between and as_needed_forms answer.
const calls = [];
const host = { deadlines: [], asNeeded: [] };
const birExports = {
  deadlines_between: (from, to) => {
    calls.push(["deadlines_between", from, to]);
    return host.deadlines.filter((d) => d.due_date >= from && d.due_date <= to);
  },
  as_needed_forms: (year) => {
    calls.push(["as_needed_forms", year]);
    return host.asNeeded;
  },
  list_returns: () => [],
};

const contextVM = vm.createContext({ console, Map, Set, JSON, Date, Math, Number, String, Proxy, WeakMap });
const mock = (name, exports) =>
  new vm.SyntheticModule(Object.keys(exports), function () {
    for (const [k, v] of Object.entries(exports)) this.setExport(k, v);
  }, { context: contextVM, identifier: name });
const source = (path) => new vm.SourceTextModule(fs.readFileSync(new URL(path, root), "utf8"), { context: contextVM, identifier: path });
const storeMod = source("app/shell/store.js");
const homeMod = source("app/shell/home.js");
const calendarMod = source("app/shell/calendar_page.js");
const linked = {
  "gpui-kit": mock("gpui-kit", { div: () => el() }),
  bir: mock("bir", birExports),
  "./ui.js": mock("./ui.js", uiExports),
  "./store.js": storeMod,
  "./home.js": homeMod,
};
for (const m of [storeMod, homeMod, calendarMod]) await m.link((s) => linked[s]);
await calendarMod.evaluate();
const { Store, isoDate, addDays } = storeMod.namespace;
const { renderHome } = homeMod.namespace;
const { renderCalendar } = calendarMod.namespace;

const TIN = "12345678800000";
const today = new Date();
const year = today.getFullYear();
const inDays = (n) => isoDate(addDays(today, n));
const who = { tin: TIN, tin_root: "123456788", branch_code: "00000", branch_name: "Head office", taxpayer_name: "Example Trading Corp", branch_count: 1 };
const periodic = { ...who, form_code: "1601C", form_name: "Monthly Remittance", frequency: "Monthly", taxable_year: year, period: 9, period_label: `September ${year}`, due_date: inDays(10), status: "Not started", draft_id: null, return_key: null };
const removal = { ...who, form_code: "2200A", form_name: "As Needed / Special / Event-Based", frequency: "As needed", taxable_year: year, period: 10, period_label: `Removal Oct 7, ${year}`, due_date: inDays(3), status: "Draft", draft_id: 41, return_key: 10 };
const secondRemoval = { ...removal, period_label: `Removal Oct 9, ${year}`, due_date: inDays(5), draft_id: 42, return_key: 11 };
const later = { ...removal, period_label: "Removal later", due_date: inDays(120), draft_id: 43, return_key: 12 };
host.deadlines = [removal, secondRemoval, periodic, later];
host.asNeeded = [
  { ...who, form_code: "2200A", form_name: "Excise Tax Return for Alcohol Products", taxable_year: year, trigger: "Removal", due_rule: "Before removal of the alcohol products", undated: [] },
  { ...who, form_code: "2000OT", form_name: "Documentary Stamp Tax Return (One-Time)", taxable_year: year, trigger: "Transaction", due_rule: "Within five days after the close of the month of the taxable document", undated: [{ draft_id: 51, period: 0, return_key: 3, status: "Draft", updated_at: "2026-10-14 08:00:00" }] },
];

const opened = [];
const started = [];
function makeApp() {
  return {
    store: new Store(),
    isFresh: () => false,
    taxpayer: () => null,
    branch: () => null,
    taxpayerUi: {},
    openForm(opts) {
      opened.push({ opts: JSON.parse(JSON.stringify(opts)), root: this.selRoot, code: this.selCode, year: this.selYear });
    },
    startNewReturn(_cx, code) {
      started.push({ code, root: this.selRoot, branch: this.selCode, year: this.selYear });
    },
  };
}
const cx = {};

// ---------------------------------------------------------------- Home
{
  const app = makeApp();
  const home = renderHome(cx, app, 1000);
  assert.ok(calls.some((c) => c[0] === "as_needed_forms" && c[1] === year), "asks for this year's as-needed forms");
  const sections = find(home, (i) => i.section).map((m) => m.info.section);
  assert.deepEqual(sections, ["Due in the next 30 days", "As needed", "Recently filed"]);

  // The dated as-needed returns are deadline rows, each its own row.
  const due = one(home, (i) => i.group === "home-due", "home-due group");
  const dueRows = due.kids.map((k) => meta.get(k).info.row);
  assert.deepEqual(dueRows, [
    `due-${TIN}-2200A-${year}-10-41`,
    `due-${TIN}-2200A-${year}-10-42`,
    `due-${TIN}-1601C-${year}-9`,
  ]);
  const titles = subtree(due).filter((m) => m.info.title).map((m) => m.info.title);
  assert.ok(titles.includes(`2200A · Removal Oct 7, ${year}`));
  assert.ok(titles.includes(`1601C · September ${year}`));
  assert.ok(!titles.includes("2200A · Removal later"), "due beyond 30 days: not on Home");
  meta.get(due.kids[0]).info.onClick(null, cx);
  assert.deepEqual(opened.pop().opts, { tin: TIN, code: "2200A", year, period: 10, returnKey: 10 }, "opens exactly that return");

  // The As needed list: one row per form with Start a return; an undated
  // return follows its form with Open.
  const list = one(home, (i) => i.group === "home-as-needed", "home-as-needed group");
  assert.deepEqual(
    list.kids.map((k) => meta.get(k).info.row),
    [`as-needed-${TIN}-2200A`, `as-needed-${TIN}-2000OT`, `as-needed-${TIN}-2000OT-51`],
  );
  const buttons = subtree(list).filter((m) => m.info.button && m.info.button.startsWith("as-needed-"));
  assert.deepEqual(
    buttons.map((m) => [m.info.button, m.info.label]),
    [
      [`as-needed-${TIN}-2200A-start`, "Start a return"],
      [`as-needed-${TIN}-2000OT-start`, "Start a return"],
      [`as-needed-${TIN}-2000OT-51-open`, "Open"],
    ],
  );
  const listTitles = subtree(list).filter((m) => m.info.title).map((m) => [m.info.title, m.info.sub]);
  assert.deepEqual(listTitles, [
    ["2200A · Excise Tax Return for Alcohol Products", "Example Trading Corp · Before removal of the alcohol products"],
    ["2000OT · Documentary Stamp Tax Return (One-Time)", "Example Trading Corp · Within five days after the close of the month of the taxable document"],
    ["No transaction date yet", "Saved Oct 14, 2026"],
  ]);
  buttons[0].info.onClick(null, cx);
  assert.deepEqual(started.pop(), { code: "2200A", root: "123456788", branch: "00000", year }, "starts the return for that branch and year");
  buttons[2].info.onClick(null, cx);
  assert.deepEqual(opened.pop().opts, { tin: TIN, code: "2000OT", year, period: 0, returnKey: 3 });

  // Narrow: the due rule moves out of the row.
  const narrow = renderHome(cx, makeApp(), 480);
  const narrowList = one(narrow, (i) => i.group === "home-as-needed", "narrow list");
  assert.equal(subtree(narrowList).find((m) => m.info.title).info.sub, "Example Trading Corp");
}

// ------------------------------------------------- Home, nothing as needed
{
  const saved = host.asNeeded;
  host.asNeeded = [];
  const home = renderHome(cx, makeApp(), 1000);
  assert.deepEqual(find(home, (i) => i.section).map((m) => m.info.section), ["Due in the next 30 days", "Recently filed"]);
  assert.equal(find(home, (i) => i.group === "home-as-needed").length, 0);
  host.asNeeded = saved;
}

// ------------------------------------------------------------- Calendar
{
  const calendar = renderCalendar(cx, makeApp(), 1000);
  const rowIds = find(calendar, (i) => i.row).map((m) => m.info.row);
  assert.deepEqual(
    rowIds,
    [`due-${TIN}-2200A-${year}-10-41`, `due-${TIN}-2200A-${year}-10-42`, `due-${TIN}-1601C-${year}-9`, `due-${TIN}-2200A-${year}-10-43`],
    "every dated return through the next year, as-needed ones included, in date order",
  );
  assert.ok(!find(calendar, (i) => i.section === "As needed").length, "no As needed list on the Calendar");
  assert.ok(!find(calendar, (i) => i.button && i.button.startsWith("as-needed-")).length);
  // Grouped under the month of the due date.
  const laterMonth = addDays(today, 120);
  const label = `${storeMod.namespace.MONTHS_LONG[laterMonth.getMonth()]} ${laterMonth.getFullYear()}`;
  const group = one(calendar, (i) => i.group === `calendar-${label.replace(" ", "-")}`, "later month group");
  assert.ok(group.kids.some((k) => meta.get(k).info.row === `due-${TIN}-2200A-${year}-10-43`));
}

console.log("home/calendar as-needed: ok");
