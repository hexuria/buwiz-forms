// Recent keeps its order until a return is changed and saved. Opening a
// listed return never moves it; a return not listed joins at the top; a
// changed and saved one moves to the top. What is stored reads back in the
// same order, also when an earlier build wrote it. Driven through the real
// app/main.js, app/shell/chrome.js and app/shell/recent.js.
import assert from "node:assert/strict";
import { makeApp, cx, one, ids, click, storage, recent, TIN } from "./shell_mount.mjs";

const ret = (code, period) => ({ tin: TIN, code, year: 2026, period });
const A = ret("1601C", 1);
const B = ret("1601C", 2);
const C = ret("2551Q", 1);
const D = ret("1601EQ", 1);
const E = ret("1701Q", 2);
const codeOf = (key) => key.split("|").slice(1).join(" ");
// Values made in the app's realm compare as plain JSON here.
const host = (v) => JSON.parse(JSON.stringify(v));
const order = (app) => host(app.recent.map((r) => codeOf(r.key)));
const rowId = (r) => `recent-${String(r.key).replace(/[^A-Za-z0-9]/g, "-")}`;
const STORED = "redesign.recent@/demo/buwiz.db";

/** The Recent rows the sidebar draws, top to bottom. */
function rows(app) {
  return host(ids(one(app.paint(cx), "sb-nav")).filter((id) => id.startsWith("recent-")));
}
/** The Recent row drawn as the current one. */
function current(app) {
  const tree = app.paint(cx);
  return host(app.recent.map(rowId)).filter((id) => {
    // NavRow marks the current row with the selected background.
    const row = one(tree, id);
    return row.calls.some((c) => c[0] === "bg") && !row.calls.some((c) => c[0] === "hover");
  });
}

const app = makeApp();
for (const r of [D, C, B, A]) app.openForm(r, cx);
assert.deepEqual(order(app), ["1601C 2026 1", "1601C 2026 2", "2551Q 2026 1", "1601EQ 2026 1"], "four new returns, the last opened on top");

// Opening any listed return, from the sidebar or elsewhere, moves nothing.
const listed = order(app);
const drawn = rows(app);
click(app.paint(cx), rowId(app.recent[3]), cx);
assert.equal(app.view, "form");
assert.deepEqual(order(app), listed, "opening the bottom row keeps the order");
assert.deepEqual(rows(app), drawn, "the sidebar draws the same order");
assert.deepEqual(current(app), [rowId(app.recent[3])], "the open return is the current row, where it was");
app.openForm(B, cx);
app.openForm(A, cx);
assert.deepEqual(order(app), listed, "opening other listed returns keeps the order");

// A changed and saved return moves to the top.
app.openForm(D, cx);
app.bumpRecent(D);
assert.deepEqual(order(app), ["1601EQ 2026 1", "1601C 2026 1", "1601C 2026 2", "2551Q 2026 1"], "save d: d, a, b, c");
assert.deepEqual(current(app), [rowId(app.recent[0])], "the saved return stays the current row");
app.bumpRecent(D);
assert.deepEqual(order(app)[0], "1601EQ 2026 1", "saving the top return again keeps it on top");

// A new return joins at the top and the oldest goes.
app.openForm(E, cx);
assert.deepEqual(order(app), ["1701Q 2026 2", "1601EQ 2026 1", "1601C 2026 1", "1601C 2026 2"], "a new return joins at the top");
// One saved after it fell off joins at the top again.
app.bumpRecent(C);
assert.deepEqual(order(app), ["2551Q 2026 1", "1701Q 2026 2", "1601EQ 2026 1", "1601C 2026 1"]);

// A listed return opened with its exact key (a Filings card) is the same
// return: the entry takes the key where it is, and nothing is listed twice.
const before = order(app);
app.openForm({ ...C, returnKey: 7 }, cx);
assert.equal(app.recent[0].key, `${TIN}|2551Q|2026|1|7`, "the entry learned its exact key");
assert.equal(app.recent.length, 4);
assert.deepEqual(order(app).map((k) => k.replace(/ 7$/, "")), before, "in place: no second row, no move");
assert.deepEqual(current(app), [rowId(app.recent[0])], "and it is the current row");
// Opened again without the key (the form list): still that one row.
app.openForm(C, cx);
assert.equal(app.recent.filter((r) => r.code === "2551Q").length, 1, "one return, one row");
assert.deepEqual(current(app), [rowId(app.recent[0])], "the row is current whichever way it was opened");
app.bumpRecent(C);
assert.equal(app.recent[0].key, `${TIN}|2551Q|2026|1|7`, "a save keeps the exact key");
assert.equal(app.recent.filter((r) => r.code === "2551Q").length, 1);
// A different exact key for the same period is another return.
app.openForm({ ...C, returnKey: 8 }, cx);
assert.deepEqual(host(app.recent.slice(0, 2).map((r) => r.key)), [`${TIN}|2551Q|2026|1|8`, `${TIN}|2551Q|2026|1|7`], "two exact keys, two rows");
// A list an earlier build stored with one return twice reads back once.
const twice = [recent.recentEntry({ ...A, returnKey: 3 }), recent.recentEntry(A), recent.recentEntry(B), recent.recentEntry(A)];
assert.deepEqual(host(recent.loadRecent(twice).map((r) => r.key)), [`${TIN}|1601C|2026|1|3`, `${TIN}|1601C|2026|2`], "a doubled return keeps its first place");
assert.deepEqual(host(recent.loadRecent([recent.recentEntry(A), recent.recentEntry({ ...A, returnKey: 3 })]).map((r) => r.key)), [`${TIN}|1601C|2026|1|3`], "and its exact key");

// What is stored is the list as shown; a restart reads the same order.
const stored = JSON.parse(storage.get(STORED));
assert.deepEqual(host(recent.loadRecent(stored).map((r) => r.key)), host(app.recent.map((r) => r.key)), "restart keeps the order");

// An earlier build's list: entries with no text id are rebuilt in place.
const old = [
  { tin: TIN, code: "1601C", year: 2026, period: 3 },
  { key: 12, tin: TIN, code: "2551Q", year: 2026, period: 2 },
  { key: `${TIN}|1701Q|2026|1`, tin: TIN, code: "1701Q", year: 2026, period: 1 },
  null,
];
const restored = recent.loadRecent(old);
assert.deepEqual(
  host(restored.map((r) => r.key)),
  [`${TIN}|1601C|2026|3`, `${TIN}|2551Q|2026|2`, `${TIN}|1701Q|2026|1`],
  "an old list keeps its order and gets text ids",
);
assert.deepEqual(host(recent.loadRecent({ not: "a list" })), []);
assert.deepEqual(host(recent.loadRecent(null)), []);
const later = makeApp({ recentList: restored });
later.openForm(ret("1701Q", 1), cx);
later.openForm(ret("1601C", 3), cx);
assert.deepEqual(order(later), ["1601C 2026 3", "2551Q 2026 2", "1701Q 2026 1"], "opening restored entries moves nothing");
later.bumpRecent(ret("1701Q", 1));
assert.deepEqual(order(later), ["1701Q 2026 1", "1601C 2026 3", "2551Q 2026 2"], "saving a restored entry moves it up");

// A library sample is no return: it never joins Recent.
const sampled = makeApp({ recentList: restored });
sampled.openForm({ ...ret("0619E", 1), sample: true }, cx);
assert.equal(sampled.recent.length, 3);

console.log("test_recent_order: ok");
