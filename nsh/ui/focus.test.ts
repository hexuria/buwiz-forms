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
