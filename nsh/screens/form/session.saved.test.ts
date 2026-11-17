// F8-14: the session tells the frame of a return's first save even when nothing was typed (an amendment saved as it is), so the
// frame's history and Recent hear of it from the one place that knows (no watch on the save state). Through the real door
// (scratch database, the demo taxpayer Juana Santos).
import { expect, test } from "nsh/test";
import { bir } from "../../bir";
import { createDemoTaxpayer, DEMO, warmForms } from "../../bir/testing";
import { FormSession, type FormTarget, type SessionHooks } from "./session";

// The first form call of a process builds every form's schema (the host does it lazily, once per process); a test's JS call has 2 s on the
// headless runner, a file's load 10 s.
warmForms();

const FILED: FormTarget = { tin: DEMO.tin, formCode: "2550Q", year: 2025, period: 2, mode: "return" };

/** A filed 2550Q: saved through a session, then stored as Submitted (the draft with the new status, which is what amend checks). */
async function seedFiled() {
  createDemoTaxpayer();
  const s = new FormSession(FILED);
  expect(await s.open()).toBe(null);
  await s.idle();
  s.edit("txtDateCovered30", "04/01/2025");
  s.edit("txtDateCovered3To0", "06/30/2025");
  s.edit("txtNameWithHoldingAgent30", "Agent One");
  s.edit("txtIncomePayment30", "1000");
  await s.flushNow();
  s.dispose();
  const draft = bir.get_form_draft(DEMO.tin, "2550Q", 2025, 2);
  if (!draft) throw new Error("the 2550Q was not saved");
  bir.save_form_draft(DEMO.tin, "2550Q", 2025, 2, "Submitted", { ...draft, status: "Submitted" });
}

function hooks() {
  const saved: FormTarget[] = [];
  const h: SessionHooks = { toast: () => "t", onSaved: (target) => saved.push(target) };
  return { h, saved };
}

test("an amendment saved with nothing typed is reported to the frame once; saving it again as it is is not news", async () => {
  await seedFiled();
  const { h, saved } = hooks();
  const s = new FormSession({ ...FILED, mode: "amend" }, h);
  expect(await s.open()).toBe(null);
  await s.idle();
  expect(s.saveState().state).not.toBe("saved");
  s.saveNow();
  await s.idle();
  expect(s.saveState().state).toBe("saved");
  expect(saved.length).toBe(1);
  expect(saved[0].formCode).toBe("2550Q");
  expect(saved[0].period).toBe(2);
  // Nothing changed since: another save says nothing to the frame (Recent does not move).
  s.saveNow();
  await s.idle();
  expect(saved.length).toBe(1);
  s.dispose();
});

test("an edit of a stored return is reported as before", async () => {
  const { h, saved } = hooks();
  const s = new FormSession({ ...FILED, period: 2, mode: "return" }, h);
  expect(await s.open()).toBe(null);
  await s.idle();
  expect(saved.length).toBe(0);
  s.edit("txtNameWithHoldingAgent30", "Agent Two");
  await s.flushNow();
  expect(saved.length).toBe(1);
  s.dispose();
});
