import { expect, test } from "nsh/test";
import { importSentence } from "./importState";

test("restored with kept and skipped parts", () => {
  expect(importSentence(12, 3, 1)).toBe(
    "Restored 12 items from the backup. 3 were left as they are, because this device has newer versions. 1 couldn't be matched or read and was skipped.",
  );
});

test("one restored item singular, zero parts left out", () => {
  expect(importSentence(1, 0, 0)).toBe("Restored 1 item from the backup.");
  expect(importSentence(2, 0, 4)).toBe(
    "Restored 2 items from the backup. 4 couldn't be matched or read and were skipped.",
  );
});

test("nothing restored, nothing skipped", () => {
  expect(importSentence(0, 5, 0)).toBe(
    "Nothing was restored: this device already has the same or newer versions of everything in the backup.",
  );
  expect(importSentence(0, 0, 0)).toBe(
    "Nothing was restored: this device already has the same or newer versions of everything in the backup.",
  );
});

test("nothing restored, some skipped", () => {
  expect(importSentence(0, 0, 2)).toBe(
    "Nothing was restored. 2 couldn't be matched or read and were skipped.",
  );
});

test("nothing restored, kept and skipped", () => {
  expect(importSentence(0, 3, 2)).toBe(
    "Nothing was restored. 3 were left as they are, because this device has newer versions. 2 couldn't be matched or read and were skipped.",
  );
  expect(importSentence(0, 1, 1)).toBe(
    "Nothing was restored. 1 was left as it is, because this device has a newer version. 1 couldn't be matched or read and was skipped.",
  );
});
