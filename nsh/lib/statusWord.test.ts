// The word a return's status shows: "Test send: not filed" for a Submitted return whose send was only simulated, from either fact a host answer
// carries (`submitted_dry_run`, or `send.stage` "test_sent"); every other return keeps its stored status.
import { expect, test } from "nsh/test";
import { countsAsFiled, isDryRun, isOpenDeadline, statusChipName, statusWord } from "./statusWord";

const send = (stage: string) => ({ stage });

test("a simulated send says Test send: not filed, from either fact", () => {
  expect(statusWord({ status: "Submitted", submitted_dry_run: true })).toBe("Test send: not filed");
  expect(statusWord({ status: "Submitted", send: send("test_sent") })).toBe("Test send: not filed");
  expect(statusChipName({ status: "Submitted", send: send("test_sent") })).toBe("dry_run");
  expect(isDryRun({ status: "Submitted", submitted_dry_run: true, send: null })).toBe(true));
});

test("the legacy stored word Filed reads as Submitted does: a dry run is a test send, a real one is filed", () => {
  // "Filed" is the legacy stored status; the Rust helper (bir-core forms/dry_run.rs) reads it as Submitted, and so do the screens.
  expect(isDryRun({ status: "Filed", submitted_dry_run: true })).toBe(true);
  expect(statusWord({ status: "Filed", submitted_dry_run: true })).toBe("Test send: not filed");
  expect(isDryRun({ status: "Filed" })).toBe(false);
  expect(countsAsFiled({ status: "Filed" })).toBe(true);
  expect(countsAsFiled({ status: "Filed", submitted_dry_run: true })).toBe(false);
});

test("a real send keeps Submitted, and every other status its own word", () => {
  expect(statusWord({ status: "Submitted" })).toBe("Submitted");
  expect(statusWord({ status: "Submitted", submitted_dry_run: false, send: null })).toBe("Submitted");
  expect(statusWord({ status: "Submitted", send: send("sent") })).toBe("Submitted");
  expect(statusChipName({ status: "Submitted", send: send("sent") })).toBe("submitted");
  // The fact belongs to a send: a Draft, a Queued, a Confirmed or a Paid return keeps its word whatever it carries.
  for (const status of ["Draft", "Queued", "Confirmed", "Paid", "Not started"]) {
    expect(statusWord({ status, submitted_dry_run: true, send: send("test_sent") })).toBe(status);
  }
});

test("a dry run does not count as filed and leaves its deadline open; a real send, Confirmed and Paid are filed", () => {
  expect(countsAsFiled({ status: "Submitted", submitted_dry_run: true })).toBe(false);
  expect(countsAsFiled({ status: "Submitted", send: send("test_sent") })).toBe(false);
  expect(isOpenDeadline({ status: "Submitted", submitted_dry_run: true })).toBe(true);
  expect(isOpenDeadline({ status: "Submitted", send: send("test_sent") })).toBe(true);
  // A real send, through the relay with a receipt or before the fact was kept.
  for (const r of [{ status: "Submitted" }, { status: "Submitted", submitted_dry_run: false }, { status: "Submitted", send: send("sent") }]) {
    expect(countsAsFiled(r)).toBe(true);
    expect(isOpenDeadline(r)).toBe(false);
  }
  for (const status of ["Confirmed", "Paid"]) {
    expect(countsAsFiled({ status })).toBe(true);
    expect(isOpenDeadline({ status })).toBe(false);
  }
  // Not filed: no return, a Draft (open) and a Queued one (on its way, not open).
  expect(countsAsFiled({ status: "Not started" })).toBe(false);
  expect(countsAsFiled({ status: "Draft" })).toBe(false);
  expect(countsAsFiled({ status: "Queued" })).toBe(false);
  expect(isOpenDeadline({ status: "Not started" })).toBe(true);
  expect(isOpenDeadline({ status: "Draft" })).toBe(true);
  expect(isOpenDeadline({ status: "Queued" })).toBe(false);
});
