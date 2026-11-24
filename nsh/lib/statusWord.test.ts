// The word a return's status shows: "Dry run" for a Submitted return whose send was only simulated, from either fact a host answer
// carries (`submitted_dry_run`, or `send.stage` "test_sent"); every other return keeps its stored status.
import { expect, test } from "nsh/test";
import { isDryRun, statusChipName, statusWord } from "./statusWord";

const send = (stage: string) => ({ stage });

test("a simulated send says Dry run, from either fact", () => {
  expect(statusWord({ status: "Submitted", submitted_dry_run: true })).toBe("Dry run");
  expect(statusWord({ status: "Submitted", send: send("test_sent") })).toBe("Dry run");
  expect(statusChipName({ status: "Submitted", send: send("test_sent") })).toBe("dry_run");
  expect(isDryRun({ status: "Submitted", submitted_dry_run: true, send: null })).toBe(true);
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
