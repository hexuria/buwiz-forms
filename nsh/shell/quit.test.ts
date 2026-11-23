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
  const api = realApi(async () => void r.order.push("stage"), () => void r.order.push("install and restart"));
  await api.install();
  r.restore();
  expect(r.order).toEqual(["stage", "hook done", "door shutdown", "install and restart"]);
});

test("an update that cannot be staged leaves the door and app running", async () => {
  const r = record();
  const api = realApi(async () => { throw new Error("not writable"); }, () => void r.order.push("install and restart"));
  try {
    await api.install();
    throw new Error("installation should have failed");
  } catch (error) {
    expect(String(error)).toContain("not writable");
  } finally {
    r.restore();
  }
  expect(r.order).toEqual([]);
});
