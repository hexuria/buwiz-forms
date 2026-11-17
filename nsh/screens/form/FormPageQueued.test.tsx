// The form page follows the submission worker (FormPageQueued.tsx stands the worker in). A page left open on a Queued return said "Queued ·
// read-only" with Cancel queue long after the worker had sent it: nothing told the page. Now, while it says Queued, it looks at the frame's
// returns lists (read again by the Queued watch, the polls and a queue action), and once they no longer list its return as Queued it reads
// the return again. It never loops on a disagreement, never runs after the page is gone, and a Cancel queue on the page restarts the watch.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { warmForms } from "../../bir/testing";

const ENTRY = "./FormPageQueued.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };

// The first form call of a process builds every form's schema; a new app has 5 s to render on the headless runner.
warmForms();

const count = (app: App, label: string): number => Number(/(\d+)$/.exec(app.texts().find((t) => t.startsWith(label + ": ")) ?? "")?.[1] ?? Number.NaN);
const opens = (app: App) => count(app, "opens");
const reads = (app: App) => count(app, "reads");
/** A look of the frame, and wait until the lists were read again (not a count of ticks: a slow machine takes longer). */
async function look(app: App): Promise<void> {
  const before = reads(app);
  await app.press("id:fx-look");
  await app.waitUntil(() => reads(app) > before, { timeout: 10_000 });
}

test("desktop: a Queued page moves on once the lists stop listing it as Queued, reads once, and stays quiet after", async () => {
  const app = await open(ENTRY, DESKTOP);
  await app.waitFor("text:Queued · read-only", { timeout: 15_000 });
  await app.waitUntil(() => opens(app) === 1);
  expect(app.shows("id:fp-cancel-queue")).toBe(true);

  // The lists are read again and still hold the return as Queued: the page is left alone.
  await look(app);
  await look(app);
  expect(opens(app)).toBe(1);
  expect(app.texts()).toContain("Queued · read-only");

  // The worker sends it. The next look reads the lists, they stop listing it as Queued, and the page reads it again: Submitted, no Cancel queue.
  await app.press("id:fx-send");
  await look(app);
  await app.waitFor("text:Submitted · read-only", { timeout: 10_000 });
  await app.waitUntil(() => !app.shows("id:fp-cancel-queue"));
  expect(app.texts()).not.toContain("Queued · read-only");
  expect(opens(app)).toBe(2);

  // Further looks read nothing more.
  await look(app);
  await look(app);
  expect(opens(app)).toBe(2);
  expect(app.texts()).toContain("Submitted · read-only");
});

test("desktop: Cancel queue and the chip move together: both say Queued until the host's read answers, then both change at once", async () => {
  const app = await open(ENTRY, DESKTOP);
  await app.waitFor("text:Queued · read-only", { timeout: 15_000 });
  await app.waitUntil(() => opens(app) === 1);
  /** One look at the screen: Cancel queue and the chip, from the one list of texts. The two never disagree. */
  const view = () => {
    const texts = app.texts();
    const chip = texts.includes("Queued · read-only") ? "Queued" : texts.includes("Submitted · read-only") ? "Submitted" : "none";
    const cancel = texts.includes("Cancel queue") && app.shows("id:fp-cancel-queue");
    return { chip, cancel };
  };
  expect(view()).toEqual({ chip: "Queued", cancel: true });

  // The worker sent it and the frame looks at the lists: they say Submitted, but the page's own read of the return is still under
  // way (held, as a slow one is). The page has not moved: it says Queued and offers Cancel queue, the two from the one answer.
  await app.press("id:fx-send");
  await app.press("id:fx-hold");
  await look(app);
  await app.waitUntil(() => opens(app) === 2, { timeout: 10_000 });
  for (let i = 0; i < 5; i++) {
    expect(view()).toEqual({ chip: "Queued", cancel: true });
    await app.wait(40);
  }
  // The read answers: the chip says Submitted and Cancel queue is gone in the same step.
  await app.press("id:fx-release");
  await app.waitUntil(() => view().chip === "Submitted", { timeout: 10_000 });
  expect(view()).toEqual({ chip: "Submitted", cancel: false });
});

test("desktop: a page the lists keep disagreeing with reads once, not on every look", async () => {
  const app = await open(ENTRY, DESKTOP);
  await app.waitFor("text:Queued · read-only", { timeout: 15_000 });
  await app.waitUntil(() => opens(app) === 1);
  // The lists say Submitted, the host's open still says Queued (the stored return is not the lists' view): one read, then quiet.
  await app.press("id:fx-list");
  await look(app);
  await app.waitUntil(() => opens(app) === 2, { timeout: 10_000 });
  for (let i = 0; i < 3; i++) await look(app);
  expect(opens(app)).toBe(2);
  expect(app.texts()).toContain("Queued · read-only");
});

test("desktop: a page that is gone reads nothing when the lists change", async () => {
  const app = await open(ENTRY, DESKTOP);
  await app.waitFor("text:Queued · read-only", { timeout: 15_000 });
  await app.waitUntil(() => opens(app) === 1);
  await app.press("id:fx-hide");
  await app.waitUntil(() => !app.shows("id:form-page"));
  await app.press("id:fx-send");
  await look(app);
  await look(app);
  expect(opens(app)).toBe(1);
});

test("desktop: Cancel queue on the page restarts the frame's fast watch (the page tells afterWrite, which starts the queue clock)", async () => {
  const app = await open(ENTRY, DESKTOP);
  await app.waitFor("text:Queued · read-only", { timeout: 15_000 });
  expect(app.texts()).toContain("queue action: none");
  // The host refuses (the stored return is no queued one): the page says so, and still tells the frame, as a refused cancel does.
  await app.press("id:fp-cancel-queue");
  await app.waitFor("text:queue action: set", { timeout: 10_000 });
});
