import { expect, test } from "nsh/test";
import { eventToast, takeFresh } from "./events";

test("one new event is announced by its title, several by a count, none by nothing", () => {
  expect(eventToast([])).toBeNull();
  expect(eventToast([{ title: "BIR confirmed 1601C for March 2025", icon: "circle-check" }])).toEqual({ message: "BIR confirmed 1601C for March 2025", icon: "circle-check" });
  expect(eventToast([{ title: "a", icon: "send" }, { title: "b", icon: "circle-check" }])).toEqual({ message: "2 filing updates. Open notifications to see them.", icon: "send" });
});

test("an event is fresh once: the known set learns it", () => {
  const known = new Set(["return:1:submitted"]);
  const all = [{ id: "return:1:submitted" }, { id: "return:2:submitted" }];
  expect(takeFresh(known, all)).toEqual([{ id: "return:2:submitted" }]);
  expect(takeFresh(known, all)).toEqual([]);
  expect([...known]).toEqual(["return:1:submitted", "return:2:submitted"]);
});
