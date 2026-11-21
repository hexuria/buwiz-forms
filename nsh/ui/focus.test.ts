import { expect, test } from "nsh/test";
import { revealAndFocus } from "./focus";

test("a linked editor is revealed in its enclosing scroll views before it receives focus", () => {
  const calls: unknown[] = [];
  revealAndFocus({
    reveal: (options) => calls.push(["reveal", options]),
    focus: () => calls.push(["focus"]),
  });
  expect(calls).toEqual([["reveal", { vertical: "nearest", animated: false }], ["focus"]]);
});

test("a reveal failure is propagated without focusing an unrevealed editor", () => {
  let focused = false;
  expect(() => revealAndFocus({
    reveal: () => { throw new Error("reveal failed"); },
    focus: () => { focused = true; },
  })).toThrow("reveal failed");
  expect(focused).toBe(false);
});

test("a linked choice reveals the whole editor containing its Save action, not just the choice", () => {
  const calls: unknown[] = [];
  revealAndFocus({
    reveal: () => calls.push(["choice reveal"]),
    focus: () => calls.push(["choice focus"]),
  }, {
    reveal: (options) => calls.push(["editor including Save", options]),
  });
  expect(calls).toEqual([["editor including Save", { vertical: "nearest", animated: false }], ["choice focus"]]);
});

test("a container reveal failure does not focus its choice below the fold", () => {
  let focused = false;
  expect(() => revealAndFocus({
    reveal: () => {},
    focus: () => { focused = true; },
  }, {
    reveal: () => { throw new Error("container reveal failed"); },
  })).toThrow("container reveal failed");
  expect(focused).toBe(false);
});
