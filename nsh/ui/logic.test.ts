// The kit's pure helpers.
import { expect, test } from "nsh/test";
import { avatarColors, hueFor, initials, matchOptions, multiComboLabel, normalizeSelection, statusChip } from "./logic";

test("initials and hues", () => {
  expect(initials("Juana Santos")).toBe("JS");
  expect(initials("Juana dela Cruz Santos")).toBe("JS");
  expect(initials("Juana")).toBe("J");
  expect(initials("")).toBe("?");
  expect(hueFor("000123453")).toBe(hueFor("000123453"));
  expect(hueFor("a")).toBeLessThan(360);
  const light = avatarColors("Juana Santos", "light");
  expect(light.bg).toMatch(/^#[0-9a-f]{6}$/);
  expect(avatarColors("Juana Santos", "dark").bg).not.toBe(light.bg);
});

test("status chips", () => {
  expect(statusChip("due_soon", 0).label).toBe("Due today");
  expect(statusChip("due_soon", 1).label).toBe("In 1 day");
  expect(statusChip("due_soon", 3)).toEqual({ label: "In 3 days", tone: "warning", check: false });
  expect(statusChip("overdue", -2)).toEqual({ label: "2 days overdue", tone: "danger", check: false });
  expect(statusChip("filed").check).toBe(true);
  expect(statusChip("other").label).toBe("Not started");
});

const RDO = [
  { value: "050", label: "050 - South Makati" },
  { value: "047", label: "047 - East Makati" },
  { value: "039", label: "039 - South Quezon City" },
];

test("pick list filter: every word, any order, ignoring case", () => {
  expect(matchOptions(RDO, "").length).toBe(3);
  expect(matchOptions(RDO, "makati").map((o) => o.value)).toEqual(["050", "047"]);
  expect(matchOptions(RDO, "050 makati").map((o) => o.value)).toEqual(["050"]);
  expect(matchOptions(RDO, "south").map((o) => o.value)).toEqual(["050", "039"]);
  expect(matchOptions(RDO, "nowhere")).toEqual([]);
  expect(matchOptions(RDO, "", 2).length).toBe(2);
});

test("multi-select summary and selection", () => {
  const words = { all: "All forms", nouns: "forms" };
  expect(multiComboLabel(words, RDO, null)).toBe("All forms");
  expect(multiComboLabel(words, RDO, new Set())).toBe("No forms");
  expect(multiComboLabel(words, RDO, new Set(["047"]))).toBe("047 - East Makati");
  expect(multiComboLabel(words, RDO, new Set(["047", "050"]))).toBe("2 forms");
  expect(normalizeSelection(new Set(["047", "050", "039"]), RDO)).toBeNull();
  expect([...(normalizeSelection(new Set(["047"]), RDO) ?? [])]).toEqual(["047"]);
});
