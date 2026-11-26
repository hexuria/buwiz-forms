// The store: memoised reads, forget and bump, failed reads and their retry, the session's scope. Plain test (no app):
// it reads a scratch database through the door.
import { expect, test } from "nsh/test";
import { bir } from "../bir";
import { createDemoTaxpayer } from "../bir/testing";
import { FAILED_READ_RETRY_MAX_MS, FAILED_READ_RETRY_MS, failedReadDelay, store } from "./store";

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

test("the wait between two asks of a failing read doubles from two seconds and stops at a minute", () => {
  expect(failedReadDelay(1)).toBe(FAILED_READ_RETRY_MS);
  expect([1, 2, 3, 4, 5, 6, 7, 8].map(failedReadDelay)).toEqual([2000, 4000, 8000, 16000, 32000, 60000, 60000, 60000]);
  expect(failedReadDelay(40)).toBe(FAILED_READ_RETRY_MAX_MS);
  expect(failedReadDelay(0)).toBe(FAILED_READ_RETRY_MS);
});

// Two keys fail from the same moment (about 6.5 s in all). "backoff" is never touched: its second ask comes after two
// seconds, its third only after four more (before the backoff it came every two seconds). "reset" is forgotten after its
// second failure: its next failure counts as the first again, so it is asked again after two seconds, not four.
test("a read that keeps failing is asked again less and less often; a forget starts its wait over", async () => {
  const calls = { backoff: 0, reset: 0 };
  const failing = (key: "backoff" | "reset") =>
    store.read(
      key,
      () => {
        calls[key]++;
        throw new Error("locked");
      },
      "fallback",
    );
  expect([failing("backoff"), failing("reset")]).toEqual(["fallback", "fallback"]);
  expect(calls).toEqual({ backoff: 1, reset: 1 });

  await pause(FAILED_READ_RETRY_MS + 150); // t = 2.15 s: both are asked again (the second ask)
  failing("backoff");
  failing("reset");
  expect(calls).toEqual({ backoff: 2, reset: 2 });
  store.forget("reset");
  failing("reset");
  expect(calls.reset).toBe(3);

  await pause(FAILED_READ_RETRY_MS + 150); // t = 4.3 s: "reset" waited two seconds again; "backoff" is waiting out four
  failing("backoff");
  failing("reset");
  expect(calls).toEqual({ backoff: 2, reset: 4 });
  expect(store.failed("backoff")).toBe(true);

  await pause(FAILED_READ_RETRY_MS + 150); // t = 6.45 s: four seconds after its second failure, "backoff" is asked a third time
  failing("backoff");
  expect(calls.backoff).toBe(3);
}, 30_000);

test("a read that works after failing stays cached; a bump starts a failing key's wait over", async () => {
  let calls = 0;
  let fail = true;
  const read = () =>
    store.read(
      "recovers",
      () => {
        calls++;
        if (fail) throw new Error("locked");
        return "fine";
      },
      "fallback",
    );
  expect(read()).toBe("fallback");
  fail = false;
  await pause(FAILED_READ_RETRY_MS + 150);
  expect(read()).toBe("fine");
  expect(calls).toBe(2);
  // Nothing is scheduled any more: a good read stays cached, however long it is left.
  await pause(FAILED_READ_RETRY_MS + 150);
  expect([read(), calls]).toEqual(["fine", 2]);
  // A bump forgets every key, a failing one included: it fails once more as a first failure and waits two seconds.
  fail = true;
  store.bump();
  expect(read()).toBe("fallback");
  expect(calls).toBe(3);
  fail = false;
  await pause(FAILED_READ_RETRY_MS + 150);
  expect([read(), calls]).toEqual(["fine", 4]);
});

test("the session's scope narrows the taxpayers and the deadlines it may see", () => {
  const t = createDemoTaxpayer();
  bir.create_taxpayer({ tin_root: "000-111-225", name: "Sample Trading Corp", taxpayer_type: "Corporation", head_office: { rdo_code: "039" } });
  store.bump();
  expect(store.taxpayers().length).toBeGreaterThanOrEqual(2);
  store.setScope(t.tin_root);
  expect(store.scopeRoot()).toBe(t.tin_root);
  expect(store.taxpayers().map((x) => x.tin_root)).toEqual([t.tin_root]);
  expect(store.canSee(t.tin_root)).toBe(true);
  expect(store.canSee("000111225")).toBe(false);
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
