// startPolls: timing, one turn at a time, errors, stop. Plain timers, no app.
import { expect, test } from "nsh/test";
import { POLL, startPolls } from "./poll";

const pause = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));
/** Waits for what a test proves to have happened, not for a count of ticks in a fixed sleep: a slow machine runs fewer ticks in the same time. */
async function until(condition: () => boolean, what: string, cap = 15_000): Promise<void> {
  const end = Date.now() + cap;
  while (!condition()) {
    if (Date.now() > end) throw new Error(`never happened in ${cap} ms: ${what}`);
    await pause(5);
  }
}

test("the schedule of the app's polls", () => {
  expect(POLL).toEqual({ events: 30000, deviceFilings: 5000, update: 1000 });
});

test("a task runs on its interval, and stop ends it", async () => {
  let runs = 0;
  const stop = startPolls([{ name: "a", every: 15, run: () => void runs++ }]);
  await until(() => runs >= 2, "two runs");
  stop();
  const seen = runs;
  expect(seen).toBeGreaterThanOrEqual(2);
  await pause(60);
  expect(runs).toBe(seen);
});

test("the first run can come sooner than the interval", async () => {
  let runs = 0;
  const stop = startPolls([{ name: "a", every: 60_000, first: 5, run: () => void runs++ }]);
  await until(() => runs >= 1, "the first run, a minute before the interval");
  await pause(60);
  stop();
  expect(runs).toBe(1);
});

test("a slow task never overlaps itself", async () => {
  let active = 0;
  let worst = 0;
  let started = 0;
  const stop = startPolls([
    {
      name: "slow",
      every: 5,
      run: async () => {
        started++;
        active++;
        worst = Math.max(worst, active);
        await pause(25);
        active--;
      },
    },
  ]);
  await until(() => started >= 3, "three turns of the slow task");
  stop();
  expect(worst).toBe(1);
});

test("a throw goes to onError and the task goes on", async () => {
  const errors: string[] = [];
  let runs = 0;
  const stop = startPolls(
    [
      {
        name: "bad",
        every: 10,
        run: () => {
          runs++;
          throw new Error("boom");
        },
      },
    ],
    (name, error) => errors.push(`${name}: ${(error as Error).message}`),
  );
  await until(() => runs >= 2, "two runs of the throwing task");
  stop();
  expect(runs).toBeGreaterThanOrEqual(2);
  expect(errors[0]).toBe("bad: boom");
});
