// The form page's to-do list (app/shell/todo.js, decision 12), as pure
// functions: effective stops, the three "Stop here" calls, the rows and
// their strongest reason, a stop leaving when filled or passed, a tick group
// counting once, shut boxes never to do, and where ‹ and › go.
import assert from "node:assert/strict";
import { effectiveStops, stopCall, todoRows, nextRow, compareRows } from "../app/shell/todo.js";

// Effective stops: (auto minus switched off) plus switched on; absent is empty.
assert.deepEqual([...effectiveStops(["a", "b", "c"], { b: false, d: true })].sort(), ["a", "c", "d"]);
assert.deepEqual([...effectiveStops(undefined, undefined)], []);
assert.deepEqual([...effectiveStops(null, { x: true })], ["x"]);

// "Stop here": back to automatic is null; otherwise what the switch says.
assert.equal(stopCall("a", false, ["a"]), false, "an automatic stop switched off");
assert.equal(stopCall("a", true, ["a"]), null, "an automatic stop switched on again");
assert.equal(stopCall("z", true, ["a"]), true, "a box the app would not stop on, switched on");
assert.equal(stopCall("z", false, ["a"]), null, "and off again");
assert.equal(stopCall("z", true, undefined), true, "no automatic stops sent");

const parts = [
  {
    items: [
      { l: "Year", f: [{ k: "y", t: "text", r: true }] },
      { l: "Withheld?", f: [{ k: "w1", t: "radio", r: true, grp: "w" }, { k: "w2", t: "radio", r: true, grp: "w" }] },
      { l: "Agree", f: [{ k: "c", t: "chk", r: true }] },
      { l: "Computed", f: [{ k: "t", t: "money", r: true, c: true }] },
      { l: "Locked", f: [{ k: "l", t: "text", r: true }] },
    ],
  },
  {
    items: [
      { l: "Amount", f: [{ k: "a", t: "money" }] },
      { l: "Kind", f: [{ k: "k1", t: "radio", grp: "k" }, { k: "k2", t: "radio", grp: "k" }] },
      { l: "Table", rows: 2, cols: [{ l: "x" }], keys: [["r0"], ["r1"]] },
    ],
  },
];
const ids = (rows) => rows.map((r) => `${r.id}:${r.reason}`);
const blocked = (k) => k === "l";

// A blank form: the required boxes, a tick group once, never a computed or shut one.
assert.deepEqual(ids(todoRows({ parts, blocked })), ["0:0:required", "0:1:required", "0:2:required"]);
// Answered: none left.
const answered = { y: "2026", w2: "true", c: "true" };
assert.deepEqual(ids(todoRows({ parts, values: answered, blocked })), []);
// A message beats a required blank, which beats a stop; one row, once.
const stops = new Set(["y", "a", "k1", "r1"]);
assert.deepEqual(
  ids(todoRows({ parts, values: {}, messages: { "0:0": "Enter the year", "1:2": "Row 2 is short" }, stops, blocked })),
  ["0:0:message", "0:1:required", "0:2:required", "1:0:stop", "1:1:stop", "1:2:message"],
);
// Stops leave when filled: an amount by any text, a tick group by any tick, a table cell by text.
assert.deepEqual(ids(todoRows({ parts, values: { ...answered, a: "0.00", k2: "true", r1: "x" }, stops, blocked })), []);
// A radio stop whose group is answered by its other box is filled.
assert.deepEqual(ids(todoRows({ parts, values: { ...answered, k1: "false", k2: "true" }, stops: new Set(["k1"]), blocked })), []);
// ...or when stepped past on this visit.
assert.deepEqual(ids(todoRows({ parts, values: answered, stops, passed: new Set(["1:0", "1:2"]), blocked })), ["1:1:stop"]);
// A message leaves when fixed (the page's messages change); the row's stop stays.
assert.deepEqual(ids(todoRows({ parts, values: answered, messages: {}, stops: new Set(["y"]), blocked })), []);
assert.deepEqual(ids(todoRows({ parts, values: {}, messages: {}, stops: new Set(["y"]), blocked })).slice(0, 1), ["0:0:required"]);
// Every box shut (a read-only return): only messages.
assert.deepEqual(ids(todoRows({ parts, messages: { "1:0": "x" }, blocked: () => true })), ["1:0:message"]);
// The stop keys a row holds come back with it.
assert.deepEqual(todoRows({ parts, values: answered, stops }).find((r) => r.id === "1:2").stops, ["r1"]);

// Form order, and where ‹ and › go.
assert.ok(compareRows("1:10", "2:0") < 0 && compareRows("2:3", "2:10") < 0 && compareRows("3:0", "2:9") > 0);
const list = ["0:2", "1:0", "2:5", "2:10"];
assert.equal(nextRow(list, 1, "1:0", 0), "2:5", "› from a row: the next");
assert.equal(nextRow(list, -1, "2:5", 0), "1:0", "‹ from a row: the previous");
assert.equal(nextRow(list, 1, "2:10", 0), null, "› past the last: nothing (the page offers Check)");
assert.equal(nextRow(list, -1, "0:2", 0), null);
assert.equal(nextRow(list, 1, "2:6", 0), "2:10", "from a row that left the list: the next after where it was");
assert.equal(nextRow(list, 1, null, 2), "2:5", "no row yet: the first in the open section or after");
assert.equal(nextRow(list, -1, null, 2), "1:0", "‹ with no row yet: the last before the open section");
assert.equal(nextRow(list, 1, null, 3, true), "0:2", "after Check: from the top");
assert.equal(nextRow([], 1, null, 0), null);

console.log("PASS: the to-do list's order, reasons, stops filled or passed, Stop here calls and steps.");
