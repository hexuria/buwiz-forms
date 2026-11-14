import { describe, expect, test } from "nsh/test";
import { addRecent, bumpRecent, loadRecent, recentEntry, recentKey, recentOpenOptions, sameReturn } from "./recent";
import type { RecentEntry, RecentOpenOptions } from "./recent";

// Translated from tools/test_recent_order.mjs: opening keeps the order, saving moves to the top, a doubled list reads
// back once, and an earlier build's list is rebuilt. Pure functions only: the app is not mounted here.
const TIN = "12345678800000";
const ret = (code: string, period: number): RecentOpenOptions => ({ tin: TIN, code, year: 2026, period });
const A = ret("1601C", 1);
const B = ret("1601C", 2);
const C = ret("2551Q", 1);
const D = ret("1601EQ", 1);
const E = ret("1701Q", 2);
const withKey = (r: RecentOpenOptions, returnKey: number): RecentOpenOptions => ({ ...r, returnKey });
const keys = (list: RecentEntry[]) => list.map((r) => r.key);
const entries = (...list: RecentOpenOptions[]) => list.map((r) => recentEntry(r));

describe("recentEntry", () => {
  test("a return's key is its taxpayer, form, year and period, joined with bars", () => {
    expect(recentEntry(A).key).toBe(`${TIN}|1601C|2026|1`);
  });
  test("the exact key joins after a bar; a null one adds nothing", () => {
    expect(recentEntry(withKey(A, 3)).key).toBe(`${TIN}|1601C|2026|1|3`);
    expect(recentEntry({ ...A, returnKey: null }).key).toBe(`${TIN}|1601C|2026|1`);
  });
  test("an exact key of zero is still a key", () => {
    expect(recentEntry(withKey(A, 0)).key).toBe(`${TIN}|1601C|2026|1|0`);
  });
  test("the entry drops newKey and newKeyFrom and keeps the return's own fields", () => {
    const entry = recentEntry({ ...A, newKey: "k", newKeyFrom: "2026-01-01" });
    expect("newKey" in entry).toBe(false);
    expect("newKeyFrom" in entry).toBe(false);
    expect(entry).toEqual({ tin: TIN, code: "1601C", year: 2026, period: 1, key: `${TIN}|1601C|2026|1` });
  });
});

describe("recentOpenOptions and recentKey", () => {
  test("opening a Recent entry asks for the same return, exactly", () => {
    expect(recentOpenOptions(A)).toEqual(A);
    expect("returnKey" in recentOpenOptions(A)).toBe(false);
  });
  test("an exact key is carried; a null one is dropped", () => {
    expect(recentOpenOptions(withKey(C, 7))).toEqual(withKey(C, 7));
    expect("returnKey" in recentOpenOptions({ ...C, returnKey: null })).toBe(false);
  });
  test("the key of a return is the key of its entry", () => {
    expect(recentKey(withKey(C, 7))).toBe(`${TIN}|2551Q|2026|1|7`);
    expect(recentKey(C)).toBe(`${TIN}|2551Q|2026|1`);
  });
});

describe("sameReturn", () => {
  test("the same taxpayer, form and period, with exact keys that do not disagree, are one return", () => {
    expect(sameReturn(A, A)).toBe(true);
    expect(sameReturn(A, withKey(A, 3))).toBe(true);
    expect(sameReturn(withKey(A, 3), withKey(A, 3))).toBe(true);
  });
  test("two different exact keys are two returns", () => {
    expect(sameReturn(withKey(A, 3), withKey(A, 8))).toBe(false);
  });
  test("any other field that differs makes two returns", () => {
    expect(sameReturn(A, B)).toBe(false);
    expect(sameReturn(A, D)).toBe(false);
    expect(sameReturn(A, { ...A, year: 2025 })).toBe(false);
    expect(sameReturn(A, { ...A, tin: "1" })).toBe(false);
  });
});

describe("loadRecent", () => {
  test("anything that is not a list is no list", () => {
    expect(loadRecent({ not: "a list" })).toEqual([]);
    expect(loadRecent(null)).toEqual([]);
    expect(loadRecent("text")).toEqual([]);
  });
  test("entries that are not objects are skipped", () => {
    expect(keys(loadRecent([null, 5, "x", { key: "k1", ...A }]))).toEqual(["k1"]);
  });
  test("an entry with a text key is kept as it was stored", () => {
    expect(loadRecent([{ key: "custom", ...A }])).toEqual([{ key: "custom", ...A }]);
  });
  test("an entry an earlier build wrote without a text id is rebuilt in place", () => {
    const old = [
      { tin: TIN, code: "1601C", year: 2026, period: 3 },
      { key: 12, tin: TIN, code: "2551Q", year: 2026, period: 2 },
      { key: `${TIN}|1701Q|2026|1`, tin: TIN, code: "1701Q", year: 2026, period: 1 },
      null,
    ];
    expect(keys(loadRecent(old))).toEqual([`${TIN}|1601C|2026|3`, `${TIN}|2551Q|2026|2`, `${TIN}|1701Q|2026|1`]);
  });
  test("a return an earlier build listed twice keeps its first place", () => {
    const twice = [recentEntry(withKey(A, 3)), recentEntry(A), recentEntry(B), recentEntry(A)];
    expect(keys(loadRecent(twice))).toEqual([`${TIN}|1601C|2026|1|3`, `${TIN}|1601C|2026|2`]);
  });
  test("and it takes the exact key of its later copy", () => {
    expect(keys(loadRecent([recentEntry(A), recentEntry(withKey(A, 3))]))).toEqual([`${TIN}|1601C|2026|1|3`]);
  });
  test("what is stored reads back in the same order", () => {
    const stored = entries(D, C, withKey(B, 4));
    expect(keys(loadRecent(stored))).toEqual(keys(stored));
  });
});

describe("addRecent (opening a return)", () => {
  test("a return not listed joins at the top", () => {
    expect(keys(addRecent(entries(B, C), A))).toEqual(keys(entries(A, B, C)));
  });
  test("the oldest entry goes once there are four", () => {
    expect(keys(addRecent(entries(B, C, D, E), A))).toEqual(keys(entries(A, B, C, D)));
  });
  test("a listed return opened again keeps its place; nothing moves", () => {
    const list = entries(B, A, C);
    expect(keys(addRecent(list, A))).toEqual(keys(list));
  });
  test("a listed return opened with its exact key takes the key where it is", () => {
    expect(keys(addRecent(entries(A, C), withKey(A, 3)))).toEqual(keys(entries(withKey(A, 3), C)));
  });
  test("an entry with an exact key, opened without it, keeps its key", () => {
    expect(keys(addRecent(entries(withKey(C, 7)), C))).toEqual([`${TIN}|2551Q|2026|1|7`]);
  });
  test("two exact keys are two returns", () => {
    expect(keys(addRecent(entries(withKey(C, 7)), withKey(C, 8)))).toEqual([`${TIN}|2551Q|2026|1|8`, `${TIN}|2551Q|2026|1|7`]);
  });
});

describe("bumpRecent (saving a return)", () => {
  test("a changed and saved return moves to the top", () => {
    expect(keys(bumpRecent(entries(A, B, C), C))).toEqual(keys(entries(C, A, B)));
  });
  test("a return already on top stays on top", () => {
    expect(keys(bumpRecent(entries(A, B), A))).toEqual(keys(entries(A, B)));
  });
  test("a return not listed joins at the top, and the oldest goes past four", () => {
    expect(keys(bumpRecent(entries(B, C, D, E), A))).toEqual(keys(entries(A, B, C, D)));
  });
  test("the more exact of the two stays", () => {
    expect(keys(bumpRecent(entries(withKey(C, 7)), C))).toEqual([`${TIN}|2551Q|2026|1|7`]);
  });
  test("a saved return with its exact key replaces the listed one without it", () => {
    expect(keys(bumpRecent(entries(C), withKey(C, 7)))).toEqual([`${TIN}|2551Q|2026|1|7`]);
  });
  test("two exact keys are two returns", () => {
    expect(keys(bumpRecent(entries(withKey(C, 7)), withKey(C, 8)))).toEqual([`${TIN}|2551Q|2026|1|8`, `${TIN}|2551Q|2026|1|7`]);
  });
});
