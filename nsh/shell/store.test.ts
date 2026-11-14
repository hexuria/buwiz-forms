// The store: memoised reads, forget and bump, failed reads and their retry, the session's scope. Plain test (no app):
// it reads a scratch database through the door.
import { expect, test } from "nsh/test";
import { bir } from "../bir";
import { createDemoTaxpayer } from "../bir/testing";
import { FAILED_READ_RETRY_MS, store } from "./store";

const pause = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));

test("a read is memoised until its key is forgotten", () => {
  const first = store.taxpayers();
  expect(store.taxpayers()).toBe(first);
  createDemoTaxpayer(); // written behind the store's back
  expect(store.taxpayers()).toBe(first);
  store.forget("taxpayers");
  const second = store.taxpayers();
  expect(second).not.toBe(first);
  expect(second.map((t) => t.name)).toContain("Juana Santos");
});

test("forgetWhere drops the keys it matches and no others; bump drops all", () => {
  let a = 0;
  let b = 0;
  const readA = () => store.read("jobs:one", () => ++a, 0);
  const readB = () => store.read("other:two", () => ++b, 0);
  expect([readA(), readB(), readA(), readB()]).toEqual([1, 1, 1, 1]);
  store.forgetWhere(/^jobs/);
  expect([readA(), readB()]).toEqual([2, 1]);
  store.bump();
  expect([readA(), readB()]).toEqual([3, 2]);
});

test("a write runs the host function, drops the cache, and reports a failure", () => {
  expect(store.setting("theme_preference")).toBeNull();
  store.write("set_setting", "theme_preference", "Dark");
  expect(store.setting("theme_preference")).toBe("Dark");
  store.writeFor("setting:theme_preference", "set_setting", "theme_preference", "Light");
  expect(store.setting("theme_preference")).toBe("Light");
  // The host refuses a key the page may not set, and the write says so.
  expect(() => store.write("set_setting", "not_a_page_setting", "x")).toThrow("not available to the page");
});

test("a failed read answers its fallback, is logged once, and is asked again after two seconds", async () => {
  let calls = 0;
  let fail = true;
  const read = () =>
    store.read(
      "flaky",
      () => {
        calls++;
        if (fail) throw new Error("locked");
        return "fine";
      },
      "fallback",
    );
  expect(read()).toBe("fallback");
  expect(store.failed("flaky")).toBe(true);
  expect(read()).toBe("fallback");
  expect(calls).toBe(1); // not asked again at once
  fail = false;
  await pause(FAILED_READ_RETRY_MS + 150);
  expect(read()).toBe("fine");
  expect(store.failed("flaky")).toBe(false);
});

test("the session's scope narrows the taxpayers and the deadlines it may see", () => {
  const t = createDemoTaxpayer();
  bir.create_taxpayer({ tin_root: "000-111-229", name: "Sample Trading Corp", taxpayer_type: "Corporation", head_office: { rdo_code: "039" } });
  store.bump();
  expect(store.taxpayers().length).toBeGreaterThanOrEqual(2);
  store.setScope(t.tin_root);
  expect(store.scopeRoot()).toBe(t.tin_root);
  expect(store.taxpayers().map((x) => x.tin_root)).toEqual([t.tin_root]);
  expect(store.canSee(t.tin_root)).toBe(true);
  expect(store.canSee("000111229")).toBe(false);
  store.setScope(null);
  expect(store.taxpayers().length).toBeGreaterThanOrEqual(2);
});

test("the form deadlines are grouped by branch, form and year", () => {
  const t = createDemoTaxpayer();
  const tin = t.branches[0].tin;
  const year = new Date().getFullYear();
  if (!store.taxYears(tin).includes(year)) {
    store.write("add_tax_year", tin, year, false);
    store.write("set_yearly_forms", tin, year, ["1601C"]);
  }
  const all = store.deadlines(`${year}-01-01`, `${year}-12-31`);
  const mine = store.formDeadlines(tin, "1601C", year, `${year}-01-01`, `${year}-12-31`);
  expect(mine.length).toBeGreaterThan(0);
  expect(mine.length).toBe(all.filter((d) => d.tin === tin && d.form_code === "1601C" && d.taxable_year === year).length);
});
