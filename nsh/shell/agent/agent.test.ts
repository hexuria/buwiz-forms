// The agent bridge without a frame: the commands the host queues are read as the old app read them, applied one at a time to
// what the frame offers, and nothing is polled unless the agent listens. The frame itself is in bridge.test.tsx.
import { expect, test } from "nsh/test";
import type { AgentStatus } from "../../bir";
import { startAgentBridge } from "./bridge";
import { applyCommand, type AgentState, type FrameDeps, parseCommand, REVIEW_MESSAGE, type ReturnRef } from "./commands";

const sleep = (ms: number) => new Promise<void>((resolve) => setTimeout(resolve, ms));
/** Waits for what a test proves to have happened, not for a count of ticks in a fixed sleep: a slow machine runs fewer ticks in the same time. */
async function until(condition: () => boolean, what: string, cap = 15_000): Promise<void> {
  const end = Date.now() + cap;
  while (!condition()) {
    if (Date.now() > end) throw new Error(`never happened in ${cap} ms: ${what}`);
    await sleep(5);
  }
}

/** A frame that writes down what it was asked, in order. */
function recorder(overrides: Partial<FrameDeps> = {}) {
  const said: string[] = [];
  const deps: FrameDeps = {
    go: (view) => void said.push(`go ${view}`),
    openReturn: (o) => void said.push(`open ${o.code} ${o.year}/${o.period} ${o.tin}`),
    refresh: (root) => void said.push(`refresh ${root ?? "-"}`),
    toast: (message) => void said.push(`toast ${message}`),
    review: (message) => void said.push(`review ${message}`),
    whenReady: async (ref: ReturnRef) => {
      said.push(`ready? ${ref.formCode} ${ref.year}/${ref.period}`);
      return true;
    },
    choose: (edits) => {
      said.push(`choose ${JSON.stringify(edits)}`);
      return true;
    },
    quit: async () => void said.push("quit"),
    ...overrides,
  };
  return { deps, said };
}

const ACTIVE: AgentStatus = { built: true, active: true, address: "127.0.0.1:1", note: null };
/** There is no frame here: it stands as signed in and unlocked (F8-3: the bridge drops commands while the real frame is locked; lock.test.ts). */
const OPEN = () => true;

test("a command is read from the JSON text the host hands over, as the old app read it", () => {
  expect(parseCommand('{"op":"refresh","tin_root":"000123453"}')).toEqual({ op: "refresh", tinRoot: "000123453" });
  expect(parseCommand('{"op":"refresh"}')).toEqual({ op: "refresh", tinRoot: null });
  expect(parseCommand({ op: "go", view: "filings" })).toEqual({ op: "go", view: "filings" });
  // The year and the period are numbers whatever they arrive as; no period is an annual return.
  expect(parseCommand('{"op":"open","tin":"00012345300000","code":"2551Q","year":"2025","period":1}')).toEqual({
    op: "open",
    tin: "00012345300000",
    code: "2551Q",
    year: 2025,
    period: 1,
  });
  expect(parseCommand('{"op":"open","tin":"00012345300000","code":"1701","year":2025}')).toEqual({ op: "open", tin: "00012345300000", code: "1701", year: 2025, period: 0 });
  expect(parseCommand('{"op":"review"}')).toEqual({ op: "review", message: REVIEW_MESSAGE });
  expect(parseCommand('{"op":"review","message":"Look it over"}')).toEqual({ op: "review", message: "Look it over" });
  // Every edit is text, as a box holds it.
  expect(parseCommand('{"op":"choose","edits":{"a":3,"b":"x"}}')).toEqual({ op: "choose", edits: { a: "3", b: "x" } });
  expect(parseCommand('{"op":"quit"}')).toEqual({ op: "quit" });
});

test("what is not a command is ignored", () => {
  for (const raw of [null, undefined, "", "not json", "[]", "3", '{"op":"dance"}', '{"op":"go"}', '{"op":"open","tin":"1"}', '{"op":"choose"}', '{"op":"choose","edits":3}']) {
    expect(parseCommand(raw)).toBeNull();
  }
});

test("go, refresh and open act on the frame at once", async () => {
  const { deps, said } = recorder();
  const state: AgentState = { form: null };
  await applyCommand({ op: "go", view: "home" }, deps, state);
  await applyCommand({ op: "refresh", tinRoot: "000123453" }, deps, state);
  await applyCommand({ op: "open", tin: "00012345300000", code: "2551Q", year: 2025, period: 1 }, deps, state);
  expect(said).toEqual(["go home", "refresh 000123453", "open 2551Q 2025/1 00012345300000"]);
  expect(state.form).toEqual({ tin: "00012345300000", formCode: "2551Q", year: 2025, period: 1 });
});

test("a fill and a review wait for the page of the return the agent opened", async () => {
  const { deps, said } = recorder();
  const state: AgentState = { form: null };
  // No return opened by the agent: its fill is already in the database, and a page loads it. Nothing to apply, nothing lost.
  await applyCommand({ op: "choose", edits: { a: "1" } }, deps, state);
  expect(said).toEqual([]);

  await applyCommand({ op: "open", tin: "00012345300000", code: "2551Q", year: 2025, period: 1 }, deps, state);
  await applyCommand({ op: "choose", edits: { "frm2551Qv2018:txtSheets": "3" } }, deps, state);
  await applyCommand({ op: "review", message: "Check it" }, deps, state);
  expect(said).toEqual([
    "open 2551Q 2025/1 00012345300000",
    "ready? 2551Q 2025/1",
    'choose {"frm2551Qv2018:txtSheets":"3"}',
    "ready? 2551Q 2025/1",
    "review Check it",
    "toast Check it",
  ]);
});

test("a page that never comes takes nothing: the fill is dropped, not put on another return", async () => {
  const { deps, said } = recorder({ whenReady: async () => false });
  const state: AgentState = { form: { tin: "1", formCode: "2551Q", year: 2025, period: 1 } };
  await applyCommand({ op: "choose", edits: { a: "1" } }, deps, state);
  await applyCommand({ op: "review", message: "x" }, deps, state);
  expect(said).toEqual([]);
});

test("quit goes by the app's quit path", async () => {
  const { deps, said } = recorder();
  await applyCommand({ op: "quit" }, deps, { form: null });
  expect(said).toEqual(["quit"]);
});

test("a build without the agent, or an app not opted in, polls nothing", async () => {
  for (const status of [
    { built: false, active: false, address: null, note: "This build has no agent control." },
    { built: true, active: false, address: null, note: "Agent control is off." },
  ] satisfies AgentStatus[]) {
    let takes = 0;
    let drains = 0;
    const stop = startAgentBridge({ start: () => status, drain: () => (drains++, false), take: () => (takes++, null), drainEvery: 5, every: 5 });
    await sleep(60);
    stop();
    expect(takes).toBe(0);
    expect(drains).toBe(0);
  }
});

test("a listening agent is drained on the host's thread as well as polled", async () => {
  let drains = 0;
  let takes = 0;
  const stop = startAgentBridge({ start: () => ACTIVE, drain: () => (drains++, false), take: () => (takes++, null), drainEvery: 5, every: 5 });
  await until(() => drains > 2 && takes > 2, "the host drained and polled more than twice");
  stop();
  expect(drains).toBeGreaterThan(2);
  expect(takes).toBeGreaterThan(2);
  const after = drains;
  await sleep(30);
  expect(drains).toBe(after);
});

test("a listening agent is polled and its commands are applied one at a time, in order", async () => {
  const { deps, said } = recorder();
  const queue = ['{"op":"go","view":"filings"}', "garbage", '{"op":"open","tin":"00012345300000","code":"2551Q","year":2025,"period":1}', '{"op":"choose","edits":{"k":"v"}}'];
  const stop = startAgentBridge({ start: () => ACTIVE, drain: () => false, take: () => queue.shift() ?? null, deps, every: 5, open: OPEN });
  try {
    await until(() => said.length >= 4, "the four commands applied");
    await sleep(60); // and nothing more
  } finally {
    stop();
  }
  expect(said).toEqual(["go filings", "open 2551Q 2025/1 00012345300000", "ready? 2551Q 2025/1", 'choose {"k":"v"}']);
  expect(queue).toEqual([]);
  // Stopped means stopped.
  const after = said.length;
  queue.push('{"op":"go","view":"home"}');
  await sleep(40);
  expect(said.length).toBe(after);
});

test("a command that throws is reported and the poll goes on", async () => {
  const { deps, said } = recorder({
    go: (view) => {
      if (view === "boom") throw new Error("no such view");
      said.push(`go ${view}`);
    },
  });
  const queue = ['{"op":"go","view":"boom"}', '{"op":"go","view":"home"}'];
  const errors: string[] = [];
  const stop = startAgentBridge({ start: () => ACTIVE, drain: () => false, take: () => queue.shift() ?? null, deps, every: 5, open: OPEN, onError: (e) => void errors.push(String((e as Error).message)) });
  try {
    await until(() => errors.length >= 1 && said.length >= 1, "the failing command reported and the next one applied");
    await sleep(60); // and nothing more
  } finally {
    stop();
  }
  expect(errors).toEqual(["no such view"]);
  expect(said).toEqual(["go home"]);
});

test("the host refusing to start is a quiet no-op", async () => {
  let takes = 0;
  const stop = startAgentBridge({
    start: () => {
      throw new Error("the door is shut");
    },
    take: () => (takes++, null),
    every: 5,
  });
  await sleep(40);
  stop();
  expect(takes).toBe(0);
});
