// Quitting for real (Quit, Cmd/Ctrl+Q, the tray's Quit, Install now): the frame's quit path runs every screen's beforeLeave hook first
// (an open form flushes its unsaved edits), and only then does the run end. A plain test: the end of the run is a stand-in.
import { expect, test } from "nsh/test";
import { app } from "../app";
import { quitApp } from "./actions";

test("Quit waits for the screens' beforeLeave hooks, then ends the run", async () => {
  const order: string[] = [];
  const stop = app.beforeLeave(async () => {
    order.push("hook starts");
    await new Promise<void>((done) => setTimeout(done, 20));
    order.push("hook done");
  });
  await quitApp(() => order.push("run ends"));
  stop();
  expect(order).toEqual(["hook starts", "hook done", "run ends"]);
});

test("a hook that throws does not keep the app from quitting", async () => {
  const order: string[] = [];
  const stop = app.beforeLeave(() => {
    throw new Error("the form could not save");
  });
  await quitApp(() => order.push("run ends"));
  stop();
  expect(order).toEqual(["run ends"]);
});

test("with nothing to flush, Quit ends the run at once", async () => {
  const order: string[] = [];
  await quitApp(() => order.push("run ends"));
  expect(order).toEqual(["run ends"]);
});
