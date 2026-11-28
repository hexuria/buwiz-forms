// The form page follows the submission worker (FormPageQueued.tsx stands the worker in). A page left open on a Queued return said "Queued ·
// read-only" with Cancel queue long after the worker had sent it: nothing told the page. Now, while it says Queued, it looks at the frame's
// returns lists (read again by the Queued watch, the polls and a queue action), and once they no longer list its return as Queued, or hold
// another send state, it reads the return again. It never loops on a disagreement, never runs after the page is gone, and a Cancel queue
// on the page restarts the watch.
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

test("desktop: a still-Queued page reads again when the worker moves its next try", async () => {
  const past = new Date(Date.now() - 60_000).toISOString();
  const app = await open(ENTRY, DESK({ send: "waiting", nextTry: past }));
  await app.waitFor("text:Not sent yet: BIR is not accepting returns right now. It tries again by itself. Trying again now.", { timeout: 15_000 });
  await app.waitUntil(() => opens(app) === 1);

  await app.press("id:fx-retry");
  await look(app);
  await app.waitUntil(() => opens(app) === 2, { timeout: 10_000 });
  expect(app.texts().some((t) => t.includes("Next try"))).toBe(true);
  expect(app.texts().some((t) => t.includes("Trying again now"))).toBe(false);

  await look(app);
  expect(opens(app)).toBe(2);
});

test("desktop: a Queued page follows a send that jumps past Sending", async () => {
  const future = new Date(Date.now() + 60_000).toISOString();
  const app = await open(ENTRY, DESK({ send: "waiting", nextTry: future }));
  await app.waitFor("text:Queued · read-only", { timeout: 15_000 });
  await app.waitUntil(() => opens(app) === 1);

  await app.press("id:fx-sending");
  await app.waitUntil(() => opens(app) === 2, { timeout: 10_000 });
  await app.waitFor("text:Sending now. Keep eBIRForms open.", { timeout: 10_000 });

  await app.press("id:fx-send");
  await look(app);
  await app.waitUntil(() => opens(app) === 3, { timeout: 10_000 });
  await app.waitFor("text:Submitted · read-only", { timeout: 10_000 });
});

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

test("desktop: a real send says Sent to BIR, a dry-run send says it was not sent", async () => {
  const app = await open(ENTRY, DESKTOP);
  await app.waitFor("text:Queued · read-only", { timeout: 15_000 });
  await app.press("id:fx-send");
  await look(app);
  await app.waitFor("text:Sent to BIR Apr 10, 2025. Nothing here can be changed.", { timeout: 10_000 });
  expect(app.texts().some((t) => t.includes("Test send: not filed"))).toBe(false);
  expect(app.texts()).toContain("Submitted · read-only");

  const dry = await open(ENTRY, DESKTOP);
  await dry.waitFor("text:Queued · read-only", { timeout: 15_000 });
  await dry.press("id:fx-dry");
  await dry.press("id:fx-send");
  await look(dry);
  await dry.waitFor("text:Test send: not filed. Nothing reached the BIR. Nothing here can be changed.", { timeout: 10_000 });
  expect(dry.texts().some((t) => t.includes("Sent to BIR"))).toBe(false);
  // The status chip says the banner's word: a simulated send was no submission.
  expect(dry.texts()).toContain("Test send: not filed");
  expect(dry.texts()).not.toContain("Submitted · read-only");
});

test("phone: a dry-run send says it was not sent to BIR", async () => {
  const app = await open(ENTRY);
  await app.waitFor("text:Queued · read-only", { timeout: 15_000 });
  await app.press("id:fx-dry");
  await app.press("id:fx-send");
  await app.press("id:fx-look");
  // On a phone the line sits under the header, out of the first screen: read the texts, not what is in view.
  await app.waitUntil(() => app.texts().includes("Test send: not filed. Nothing reached the BIR. Nothing here can be changed."), { timeout: 10_000 });
  expect(app.texts().some((t) => t.includes("Sent to BIR"))).toBe(false);
  expect(app.texts()).toContain("Test send: not filed");
  expect(app.texts()).not.toContain("Submitted · read-only");
});

test("phone: a real send's chip says Submitted", async () => {
  const app = await open(ENTRY);
  await app.waitFor("text:Queued · read-only", { timeout: 15_000 });
  await app.press("id:fx-send");
  await app.press("id:fx-look");
  await app.waitUntil(() => app.texts().includes("Sent to BIR Apr 10, 2025. Nothing here can be changed."), { timeout: 10_000 });
  expect(app.texts()).toContain("Submitted · read-only");
  expect(app.texts()).not.toContain("Test send: not filed");
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

// The send's words (the host's `send`, send_state.rs): where the page says a return's status, it says what the send is. The host's open
// is stood in for (FormPageQueued.tsx host.params { send, nextTry, refuse }), so each stage shows without a worker.
const DESK = (params: Record<string, unknown>) => ({ ...DESKTOP, host: { params } });
/** Tomorrow at 9:05 AM, local time, as the host writes a next try (RFC 3339, UTC). */
const tomorrowAt = () => {
  const d = new Date();
  d.setDate(d.getDate() + 1);
  d.setHours(9, 5, 0, 0);
  return d;
};

test("desktop: a Queued return says what its send is: queued, sending, held with its time, waiting with its next try", async () => {
  const queued = await open(ENTRY, DESK({ send: "queued" }));
  await queued.waitFor("text:Queued to send.", { timeout: 15_000 });
  expect(queued.texts()).toContain("Queued · read-only");
  expect(queued.shows("id:fp-email-came")).toBe(false);

  const sending = await open(ENTRY, DESK({ send: "sending" }));
  await sending.waitFor("text:Sending now. Keep eBIRForms open.", { timeout: 15_000 });
  expect(sending.shows("id:fp-email-came")).toBe(false);

  const clock = new Date(new Date().getFullYear(), 10, 5, 9, 0);
  const heldAt = new Date(clock);
  heldAt.setHours(15, 40, 0, 0);
  const held = await open(ENTRY, { ...DESK({ send: "held", nextTry: heldAt.toISOString() }), clock: { time: clock } });
  await held.waitFor("text:Buwiz is holding sends for now. It goes again at 3:40 PM.", { timeout: 15_000 });

  const at = tomorrowAt();
  const MONTHS = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
  const day = `${MONTHS[at.getMonth()]} ${at.getDate()}${at.getFullYear() === new Date().getFullYear() ? "" : `, ${at.getFullYear()}`}`;
  const waiting = await open(ENTRY, DESK({ send: "waiting", nextTry: at.toISOString() }));
  await waiting.waitFor(`text:Not sent yet: BIR is not accepting returns right now. It tries again by itself. Next try on ${day} at 9:05 AM.`, { timeout: 15_000 });
  expect(waiting.shows("id:fp-email-came")).toBe(false);
});

test("desktop: a sent return keeps its own dated words, whatever the host's send line says", async () => {
  for (const stage of ["sent", "test_sent"] as const) {
    const app = await open(ENTRY, DESK({ send: stage }));
    // A test send (the host's "test_sent") is a dry run: its chip says the banner's word.
    await app.waitFor(stage === "test_sent" ? "text:Test send: not filed" : "text:Submitted · read-only", { timeout: 15_000 });
    // The page's words for a sent return carry its date (F21); the host's line is for a return still in the queue.
    await app.waitUntil(() => app.texts().some((text) => text.endsWith("Nothing here can be changed.")));
    expect(app.texts().some((text) => text.includes("through Buwiz"))).toBe(false);
  }
});

test("desktop: an unknown outcome asks the person; The email came, confirmed, marks it sent and the page reads it again", async () => {
  const app = await open(ENTRY, DESK({ send: "needs_attention" }));
  await app.waitFor("text:The BIR may have received this return. Check for the BIR's confirmation email before sending it again.", { timeout: 15_000 });
  expect(app.shows("id:fp-email-came")).toBe(true);
  // Cancel queue is not offered beside the two answers: the host would refuse it for this return.
  expect(app.shows("id:fp-cancel-queue")).toBe(false);
  expect(app.shows("id:fp-email-missing")).toBe(true);
  // The confirm step says what the answer does; Cancel asks nothing of the host.
  await app.press("id:fp-email-came");
  await app.waitFor("text:Mark this return as sent to the BIR?");
  await app.press("id:resolve-cancel");
  await app.waitFor("text:Mark this return as sent to the BIR?", { state: "gone" });
  expect(app.texts()).toContain("resolved: none");
  await app.press("id:fp-email-came");
  await app.waitFor("text:Mark this return as sent to the BIR?");
  await app.press("id:resolve-continue");
  await app.waitFor("text:resolved: 00012345300000 1601C 2025 3 received", { timeout: 10_000 });
  await app.waitFor("text:Submitted · read-only", { timeout: 10_000 });
  await app.waitUntil(() => !app.shows("id:fp-email-came"));
  expect(app.texts().some((t) => t.includes("Marked as sent to the BIR."))).toBe(true);
});

test("desktop: a send the worker is checking with Buwiz says so and offers no answer and no Cancel queue", async () => {
  const app = await open(ENTRY, DESK({ send: "checking" }));
  await app.waitFor("text:Checking with Buwiz whether this return was sent. Keep eBIRForms open.", { timeout: 15_000 });
  expect(app.texts()).toContain("Queued · read-only");
  expect(app.shows("id:fp-email-came")).toBe(false);
  expect(app.shows("id:fp-email-missing")).toBe(false);
  expect(app.shows("id:fp-cancel-queue")).toBe(false);
});

test("desktop: It did not arrive, confirmed, puts the return back to Draft", async () => {
  const app = await open(ENTRY, DESK({ send: "needs_attention" }));
  await app.waitFor("id:fp-email-missing", { timeout: 15_000 });
  await app.press("id:fp-email-missing");
  await app.waitFor("text:Put this return back to Draft so you can send it again?");
  await app.press("id:resolve-continue");
  await app.waitFor("text:resolved: 00012345300000 1601C 2025 3 not_received", { timeout: 10_000 });
  await app.waitFor("id:fp-queue", { timeout: 10_000 });
  expect(app.shows("id:fp-email-missing")).toBe(false);
  expect(app.texts().some((t) => t.includes("Back to Draft. Queue it again when you are ready."))).toBe(true);
});

test("desktop: a refused answer is said as the host says it, and the question stays", async () => {
  const app = await open(ENTRY, DESK({ send: "needs_attention", refuse: true }));
  await app.waitFor("id:fp-email-came", { timeout: 15_000 });
  await app.press("id:fp-email-came");
  await app.waitFor("id:resolve-continue");
  await app.press("id:resolve-continue");
  await app.waitUntil(() => app.texts().some((t) => t.includes("This return is not waiting for your answer about the BIR's email.")), { timeout: 10_000 });
  expect(app.shows("id:fp-email-came")).toBe(true);
  expect(app.texts()).toContain("Queued · read-only");
});

test("desktop: a refused return opens as a Draft with the reason, and the Buwiz account sentence for a TIN refusal", async () => {
  const app = await open(ENTRY, DESK({ send: "refused" }));
  await app.waitFor("id:fp-send-notice", { timeout: 15_000 });
  expect(app.texts()).toContain("This TIN is not on your Buwiz account.");
  expect(app.texts()).toContain("Open your Buwiz account to fix this: https://account.buwiz.com");
  expect(app.shows("id:fp-queue")).toBe(true);
});

test("phone: a refused return's notice, and an unknown outcome's two answers", async () => {
  const refused = await open(ENTRY, { host: { params: { send: "refused" } } });
  await refused.waitUntil(() => refused.texts().includes("This TIN is not on your Buwiz account."), { timeout: 15_000 });
  expect(refused.texts()).toContain("Open your Buwiz account to fix this: https://account.buwiz.com");
  const unknown = await open(ENTRY, { host: { params: { send: "needs_attention" } } });
  await unknown.waitUntil(() => unknown.texts().includes("The email came") && unknown.texts().includes("It did not arrive"), { timeout: 15_000 });
});
