// F8-6: every way out runs the screens' beforeLeave hooks, then the door's shutdown (the submission worker stops, the database is
// checkpointed), then ends the run: app.quit() with no exit of its own (the agent's quit, the tray's Quit), and the updater's install
// and restart. The two last steps are quitSteps, which a test stands in for.
import { expect, test } from "nsh/test";
import { app, quitSteps } from "./app";
import { realApi } from "./desktop/updates";

function record() {
  const order: string[] = [];
  const real = { ...quitSteps };
  quitSteps.shutdown = () => void order.push("door shutdown");
  quitSteps.end = () => void order.push("run ends");
  const stop = app.beforeLeave(async () => {
    await new Promise<void>((done) => setTimeout(done, 10));
    order.push("hook done");
  });
  const restore = () => {
    stop();
    quitSteps.shutdown = real.shutdown;
    quitSteps.end = real.end;
  };
  return { order, restore };
}

test("app.quit() with no exit runs the hooks, then the door's shutdown, then ends the run", async () => {
  const r = record();
  await app.quit();
  r.restore();
  expect(r.order).toEqual(["hook done", "door shutdown", "run ends"]);
});

test("a shutdown that throws still ends the run", async () => {
  const order: string[] = [];
  const real = { ...quitSteps };
  quitSteps.shutdown = () => {
    throw new Error("the worker did not stop");
  };
  quitSteps.end = () => void order.push("run ends");
  await app.quit();
  quitSteps.shutdown = real.shutdown;
  quitSteps.end = real.end;
  expect(order).toEqual(["run ends"]);
});

test("Install now: the hooks, the door's shutdown, then the swap and restart (never the plain end)", async () => {
  const r = record();
  const api = realApi(async () => void r.order.push("install and restart"));
  // install settles only on failure: the restart ends the run. Wait for it to have run.
  void api.install();
  for (let i = 0; i < 100 && r.order.length < 3; i++) await new Promise<void>((done) => setTimeout(done, 10));
  r.restore();
  expect(r.order).toEqual(["hook done", "door shutdown", "install and restart"]);
});
