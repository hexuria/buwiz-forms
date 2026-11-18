// A mounted app reaching the door through nsh/bir: phone size and desktop size, against a scratch database.
import { expect, open, test } from "nsh/test";
import { platform } from "os";

const ENTRY = "./DoorProbe.tsx";

test("phone: a component calls sync and async host functions and sees a host error", async () => {
  const app = await open(ENTRY); // 390 x 844
  // The system the test runs on, named as Rust names it (std::env::consts::OS), as the standard `os` module does.
  expect(app.texts()).toContain(`Host OS: ${platform()}`);
  expect(app.texts()).toContain("Sending: dry_run");
  await app.press("role:button[Count forms]");
  await app.waitFor("text:Forms: 43"); // the first call builds the form library: about a second in a debug build
  await app.press("role:button[Open return]");
  await app.waitFor("text:Return: 2551Q Draft");
  await app.press("role:button[Break it]");
  await app.waitFor("text:Error: argument 1 must be a string, got a number");
});

test("desktop: the same app at desktop size and platform", async () => {
  const app = await open(ENTRY, { width: 1440, height: 900, platform: "macos", colorScheme: "dark" });
  expect(app.texts()).toContain("Sending: dry_run");
  await app.press("role:button[Count forms]");
  await app.waitFor("text:Forms: 43");
});
