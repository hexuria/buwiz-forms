// store.screenIndex(): the form library's index is kept for the run once it is built, what shows it re-runs when it fills after a failure,
// and a failed (or empty) build is not asked of the host again until the retry is due: 2 s, then 4 s ... (failedReadDelay), as every other
// failed read. Without that every call during a failure re-ran the ~1 s synchronous build (New return per row, Recent per row, the profile's
// forms per row) and what showed the empty list stayed empty until something unrelated re-rendered. Plain test (no app): the host's
// screen_forms_index is faked, so one build is cheap. One flow in one test: the index is module state that is kept for the life of the app.
import { computed } from "nsh";
import { expect, test } from "nsh/test";
import { bir } from "../bir";
import { FAILED_READ_RETRY_MS, failedReadDelay, store } from "./store";

const pause = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));
const fake = bir as unknown as { screen_forms_index: () => unknown[] };
const real = fake.screen_forms_index;
const ROWS = [{ c: "1601C", t: "Monthly Remittance Return", freq: "Monthly" }];

test("a failed build answers [] without asking the host until it is due again (2 s, 4 s), re-runs what shows it, and a success is kept", async () => {
  let calls = 0;
  let answer: "empty" | "throws" | "rows" = "empty";
  fake.screen_forms_index = () => {
    calls += 1;
    if (answer === "throws") throw new Error("the form library could not be built");
    return answer === "empty" ? [] : ROWS;
  };
  try {
    // What shows the index: re-runs only when the revision moves.
    const shown = computed(() => store.screenIndex().length);
    expect(shown()).toBe(0); // an empty answer is a failed build
    expect(calls).toBe(1);
    for (let i = 0; i < 6; i++) store.screenIndex(); // N rows, N asks: the host is not called again
    expect(calls).toBe(1);
    expect(shown()).toBe(0);
    expect(calls).toBe(1);

    answer = "throws";
    await pause(failedReadDelay(1) + 150); // 2 s: due again, and what shows it re-runs
    expect(shown()).toBe(0);
    expect(calls).toBe(2);
    for (let i = 0; i < 6; i++) store.screenIndex();
    expect(calls).toBe(2);

    answer = "rows";
    await pause(FAILED_READ_RETRY_MS + 150); // 2 s after the second failure: still waiting (it waits 4 s now)
    for (let i = 0; i < 6; i++) store.screenIndex();
    expect(shown()).toBe(0);
    expect(calls).toBe(2);

    await pause(failedReadDelay(2) - FAILED_READ_RETRY_MS); // the 4 s are over: what shows it re-runs and the build works
    expect(shown()).toBe(1);
    expect(calls).toBe(3);
    // Kept for the life of the app: not asked again, past a bump and past any wait.
    store.bump();
    expect(store.screenIndex()).toBe(store.screenIndex());
    expect(store.screenIndex().length).toBe(1);
    await pause(FAILED_READ_RETRY_MS + 150);
    expect(shown()).toBe(1);
    expect(calls).toBe(3);
  } finally {
    fake.screen_forms_index = real;
  }
}, 30_000);
