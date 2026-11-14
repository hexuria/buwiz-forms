import { describe, expect, test } from "nsh/test";
import { compareRows, effectiveStops, itemKeys, nextRow, stopCall, todoRows } from "./todo";
import type { TodoPart, TodoRow } from "./todo";

// Translated from tools/test_form_todo.mjs; the cases keep their meaning, and more edges are added after them.
const parts: TodoPart[] = [
  {
    items: [
      { l: "Year", f: [{ k: "y", t: "text", r: true }] },
      {
        l: "Withheld?",
        f: [
          { k: "w1", t: "radio", r: true, grp: "w" },
          { k: "w2", t: "radio", r: true, grp: "w" },
        ],
      },
      { l: "Agree", f: [{ k: "c", t: "chk", r: true }] },
      { l: "Computed", f: [{ k: "t", t: "money", r: true, c: true }] },
      { l: "Locked", f: [{ k: "l", t: "text", r: true }] },
    ],
  },
  {
    items: [
      { l: "Amount", f: [{ k: "a", t: "money" }] },
      {
        l: "Kind",
        f: [
          { k: "k1", t: "radio", grp: "k" },
          { k: "k2", t: "radio", grp: "k" },
        ],
      },
      { l: "Table", rows: 2, cols: [{ l: "x" }], keys: [["r0"], ["r1"]] },
    ],
  },
];

const ids = (rows: TodoRow[]) => rows.map((r) => `${r.id}:${r.reason}`);
const blocked = (k: string) => k === "l";
const answered = { y: "2026", w2: "true", c: "true" };
const stops = new Set(["y", "a", "k1", "r1"]);

describe("effectiveStops", () => {
  test("the automatic stops less the switched-off ones, plus the switched-on ones", () => {
    expect([...effectiveStops(["a", "b", "c"], { b: false, d: true })].sort()).toEqual(["a", "c", "d"]);
  });
  test("absent stops and absent edits are empty, and edits alone count", () => {
    expect([...effectiveStops(undefined, undefined)]).toEqual([]);
    expect([...effectiveStops(null, { x: true })]).toEqual(["x"]);
    expect([...effectiveStops(["a"], {})]).toEqual(["a"]);
  });
});

describe("stopCall (Stop here)", () => {
  test("back to automatic is null; otherwise what the switch says", () => {
    expect(stopCall("a", false, ["a"])).toBe(false);
    expect(stopCall("a", true, ["a"])).toBeNull();
    expect(stopCall("z", true, ["a"])).toBe(true);
    expect(stopCall("z", false, ["a"])).toBeNull();
    expect(stopCall("z", true, undefined)).toBe(true);
  });
});

describe("itemKeys", () => {
  test("a field item's keys are its boxes; a pooled line's are its pool's", () => {
    expect(itemKeys({ f: [{ k: "a", t: "text" }, { k: "p1", t: "text", pool: { keys: ["p1", "p2"] } }] })).toEqual(["a", "p1", "p2"]);
  });
  test("a table's keys are every cell, row by row", () => {
    expect(itemKeys({ rows: 2, keys: [["r0a", "r0b"], ["r1a", "r1b"]] })).toEqual(["r0a", "r0b", "r1a", "r1b"]);
  });
  test("an item with no boxes has no keys", () => {
    expect(itemKeys({})).toEqual([]);
    expect(itemKeys({ f: [] })).toEqual([]);
  });
});

describe("todoRows", () => {
  test("a blank form lists its required boxes, a tick group once, and never a computed or shut one", () => {
    expect(ids(todoRows({ parts, blocked }))).toEqual(["0:0:required", "0:1:required", "0:2:required"]);
  });
  test("answered boxes leave the list", () => {
    expect(ids(todoRows({ parts, values: answered, blocked }))).toEqual([]);
  });
  test("a message beats a required blank, which beats a stop; one row, once", () => {
    expect(
      ids(todoRows({ parts, values: {}, messages: { "0:0": "Enter the year", "1:2": "Row 2 is short" }, stops, blocked })),
    ).toEqual(["0:0:message", "0:1:required", "0:2:required", "1:0:stop", "1:1:stop", "1:2:message"]);
  });
  test("stops leave when filled: an amount by any text, a tick group by any tick, a table cell by text", () => {
    expect(ids(todoRows({ parts, values: { ...answered, a: "0.00", k2: "true", r1: "x" }, stops, blocked }))).toEqual([]);
  });
  test("a radio stop whose group is answered by its other box is filled", () => {
    expect(
      ids(todoRows({ parts, values: { ...answered, k1: "false", k2: "true" }, stops: new Set(["k1"]), blocked })),
    ).toEqual([]);
  });
  test("a stop the user has stepped past on this visit is left alone", () => {
    expect(ids(todoRows({ parts, values: answered, stops, passed: new Set(["1:0", "1:2"]), blocked }))).toEqual(["1:1:stop"]);
  });
  test("a message leaves when fixed, and the row's stop stays until its box is filled", () => {
    expect(ids(todoRows({ parts, values: answered, messages: {}, stops: new Set(["y"]), blocked }))).toEqual([]);
    expect(ids(todoRows({ parts, values: {}, messages: {}, stops: new Set(["y"]), blocked })).slice(0, 1)).toEqual(["0:0:required"]);
  });
  test("with every box shut (a read-only return) only messages remain", () => {
    expect(ids(todoRows({ parts, messages: { "1:0": "x" }, blocked: () => true }))).toEqual(["1:0:message"]);
  });
  test("the stop keys a row holds come back with it", () => {
    const row = todoRows({ parts, values: answered, stops }).find((r) => r.id === "1:2");
    expect(row?.stops).toEqual(["r1"]);
  });
  test("with no stops at all, no row gets a stop reason", () => {
    expect(ids(todoRows({ parts, values: {}, blocked }))).toEqual(["0:0:required", "0:1:required", "0:2:required"]);
  });
  test("no parts, no rows", () => {
    expect(todoRows({ parts: [] })).toEqual([]);
    expect(todoRows({})).toEqual([]);
  });
  test("an empty message is no message", () => {
    expect(ids(todoRows({ parts, values: answered, messages: { "1:0": "" }, blocked }))).toEqual([]);
  });
});

describe("compareRows", () => {
  test("parts first, then items, read as numbers", () => {
    expect(compareRows("1:10", "2:0")).toBeLessThan(0);
    expect(compareRows("2:3", "2:10")).toBeLessThan(0);
    expect(compareRows("3:0", "2:9")).toBeGreaterThan(0);
    expect(compareRows("1:2", "1:2")).toBe(0);
  });
});

describe("nextRow (the pill's steps)", () => {
  const list = ["0:2", "1:0", "2:5", "2:10"];
  test("from a row: the next one, or the previous one", () => {
    expect(nextRow(list, 1, "1:0", 0)).toBe("2:5");
    expect(nextRow(list, -1, "2:5", 0)).toBe("1:0");
  });
  test("past either end there is nothing", () => {
    expect(nextRow(list, 1, "2:10", 0)).toBeNull();
    expect(nextRow(list, -1, "0:2", 0)).toBeNull();
  });
  test("from a row that has left the list: the next after where it was", () => {
    expect(nextRow(list, 1, "2:6", 0)).toBe("2:10");
  });
  test("with no row yet: the first in the open part or after; the last before it going back", () => {
    expect(nextRow(list, 1, null, 2)).toBe("2:5");
    expect(nextRow(list, -1, null, 2)).toBe("1:0");
  });
  test("a part with no open rows steps to the next part's first row", () => {
    expect(nextRow(list, 1, null, 1)).toBe("1:0");
  });
  test("after Check (fromTop): from the top going forward, nothing going back", () => {
    expect(nextRow(list, 1, null, 3, true)).toBe("0:2");
    expect(nextRow(list, -1, null, 3, true)).toBeNull();
  });
  test("an empty list has no step", () => {
    expect(nextRow([], 1, null, 0)).toBeNull();
  });
});
