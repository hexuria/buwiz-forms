import { expect, test } from "nsh/test";
import { bir } from "../bir";
import { createDemoTaxpayer } from "../bir/testing";
import type { PollTask } from "../lib/poll";
import { app, boot } from "./app";
import { dispatchDevCommand, startDevCommandMailbox } from "./devCommand";
import { store } from "./store";

test("disabled dev mailbox starts no timer and never reads a command", () => {
  let reads = 0;
  let schedules = 0;
  const stop = startDevCommandMailbox({
    enabled: () => false,
    read: () => { reads++; return "lock"; },
    schedule: () => { schedules++; return () => {}; },
  });
  stop();
  expect([reads, schedules]).toEqual([0, 0]);
});

test("dev mailbox dispatches lock and logout through the real access handlers, ignoring unknown commands", async () => {
  createDemoTaxpayer();
  bir.setup_solo("000123453");
  bir.set_admin_pin("1234");
  store.forget("auth_state");
  await boot(undefined, { firstRun: false });
  app.startSession({ kind: "admin" });
  expect(app.chromeVisible()).toBe(true);

  let changes = 0;
  let task: PollTask | undefined;
  let stopped = false;
  const commands = ["unknown", "lock", "logout"];
  const unlisten = app.onSessionChange(() => changes++);
  const stop = startDevCommandMailbox({
    enabled: () => true,
    read: () => commands.shift() ?? null,
    schedule: (tasks) => { task = tasks[0]; return () => { stopped = true; }; },
  });
  try {
    expect(task?.every).toBe(500);
    await task!.run();
    expect([changes, app.appLocked()]).toEqual([0, false]);
    await task!.run();
    expect([changes, app.appLocked(), app.session().kind]).toEqual([1, true, "open"]);
    await task!.run();
    expect([changes, app.appLocked(), app.session().kind]).toEqual([2, true, "open"]);
    dispatchDevCommand(null);
    dispatchDevCommand("LOCK");
    expect(changes).toBe(2);
  } finally {
    stop();
    unlisten();
    store.cancelRereads();
  }
  expect(stopped).toBe(true);
});
