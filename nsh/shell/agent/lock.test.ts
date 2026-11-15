// F8-3, the frame's half: while the app is locked (the sign-in lock or the Buwiz gate in front, or not booted) or a taxpayer is signed
// in, the bridge drops the agent's screen commands (go, open, refresh: refresh selects and persists a taxpayer) instead of applying them
// or holding them for whoever signs in next; a quit still ends the run. The host's half (the drain refuses) is src/agent.rs. Pure: the
// bridge with a fake listener and fake frame.
import { expect, test } from "nsh/test";
import { app } from "../app";
import { frameOpen, startAgentBridge, type FrameDeps } from "./index";

function fakeFrame() {
  const did: string[] = [];
  const deps: FrameDeps = {
    go: (view) => void did.push("go " + view),
    openReturn: (o) => void did.push("open " + o.code),
    refresh: (root) => void did.push("refresh " + root),
    toast: () => {},
    review: () => void did.push("review"),
    whenReady: async () => true,
    choose: () => true,
    quit: async () => void did.push("quit"),
  };
  return { did, deps };
}

const until = async (ok: () => boolean) => {
  for (let i = 0; i < 200 && !ok(); i++) await new Promise<void>((done) => setTimeout(done, 5));
};

test("a locked frame drops go, open and refresh, and takes them again once it is open; quit always passes", async () => {
  const { did, deps } = fakeFrame();
  const queue: string[] = [];
  let open = false;
  const stop = startAgentBridge({
    start: () => ({ built: true, active: true, address: "127.0.0.1:1", note: null }),
    drain: () => false,
    take: () => queue.shift() ?? null,
    deps,
    every: 5,
    drainEvery: 1000,
    open: () => open,
  });
  queue.push(JSON.stringify({ op: "refresh", tin_root: "000123453" }), JSON.stringify({ op: "go", view: "filings" }));
  queue.push(JSON.stringify({ op: "open", tin: "00012345300000", code: "2551Q", year: 2026, period: 1 }));
  await until(() => queue.length === 0);
  await new Promise<void>((done) => setTimeout(done, 30));
  expect(did).toEqual([]);
  // Signed in: the next command applies; the dropped ones never come back.
  open = true;
  queue.push(JSON.stringify({ op: "go", view: "library" }));
  await until(() => did.length > 0);
  expect(did).toEqual(["go library"]);
  // Locked again; a quit still goes through.
  open = false;
  queue.push(JSON.stringify({ op: "go", view: "home" }), JSON.stringify({ op: "quit" }));
  await until(() => did.length > 1);
  stop();
  expect(did).toEqual(["go library", "quit"]);
});

test("the frame is not open to the agent before it has booted", () => {
  expect(app.booted()).toBe(false);
  expect(app.agentLocked()).toBe(true);
  expect(frameOpen()).toBe(false);
});
