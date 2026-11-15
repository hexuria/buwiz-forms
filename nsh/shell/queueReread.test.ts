// After a queue or a cancel-queue the store reads the returns lists again at once and after 1 s, 3 s and 10 s (the submission worker
// answers within a second or two), and a refused cancel-queue is said in one plain sentence. Plain test (no app): list_returns is faked.
import { untracked } from "nsh";
import { expect, test } from "nsh/test";
import { bir } from "../bir";
import { ALREADY_SENT, cancelQueueMessage, isAlreadySent } from "./queue";
import { QUEUE_FUNCTIONS, QUEUE_REREAD_MS, store } from "./store";

/** A look at the filed list from outside any reactive scope. */
const peek = () => untracked(() => store.returns("filed"));
const pause = (ms: number) => new Promise<void>((done) => setTimeout(done, ms));

test("the re-reads come at once and after 1 s, 3 s and 10 s, for the host functions that queue and cancel", () => {
  expect([...QUEUE_REREAD_MS]).toEqual([1000, 3000, 10000]);
  expect([...QUEUE_FUNCTIONS].sort()).toEqual(["cancel_queued", "queue_form_draft", "screen_form_cancel_queue", "screen_form_queue"]);
});

// The frame's own reads (the notifications) hold the filed list, so a dropped key is read again by them at once: the counts below are
// the host's list_returns calls, compared before and after, not an exact number.
test("a plain bump starts no re-reads; naming a queue action reads at once, again at 1 s and again at 3 s", async () => {
  let reads = 0;
  const fake = bir as unknown as { list_returns: (which: string) => unknown[] };
  const real = fake.list_returns;
  fake.list_returns = () => {
    reads += 1;
    return [];
  };
  try {
    store.bump();
    peek();
    const memoised = reads;
    peek();
    expect(reads).toBe(memoised);

    store.bump();
    peek();
    const afterPlain = reads;
    expect(afterPlain).toBeGreaterThan(memoised); // dropped, so read again
    await pause(1150);
    peek();
    expect(reads).toBe(afterPlain); // nothing was scheduled by a plain bump

    store.bump("queue_form_draft"); // what a page's afterWrite hands over after queueing
    peek();
    const atOnce = reads;
    expect(atOnce).toBeGreaterThan(afterPlain);
    await pause(1150);
    peek();
    const after1s = reads;
    expect(after1s).toBeGreaterThan(atOnce); // the 1 s
    peek();
    expect(reads).toBe(after1s);
    await pause(2000);
    peek();
    expect(reads).toBeGreaterThan(after1s); // the 3 s
  } finally {
    fake.list_returns = real;
    store.cancelRereads(); // the 10 s re-read must not outlive the test
  }
});

test("cancelRereads ends the pending re-reads of a queue action (a lock, a sign-out, the frame gone), and a new action starts them again", async () => {
  let reads = 0;
  const fake = bir as unknown as { list_returns: (which: string) => unknown[] };
  const real = fake.list_returns;
  fake.list_returns = () => {
    reads += 1;
    return [];
  };
  try {
    store.bump();
    peek();
    store.afterQueueChange();
    peek();
    const atOnce = reads;
    store.cancelRereads();
    expect(store.lastQueueAction()).toBeGreaterThan(0);
    await pause(1250);
    peek();
    expect(reads).toBe(atOnce); // the 1 s re-read never came

    store.afterQueueChange(); // a new action: the series starts again
    peek();
    const again = reads;
    await pause(1250);
    peek();
    expect(reads).toBeGreaterThan(again);
  } finally {
    fake.list_returns = real;
    store.cancelRereads();
  }
});

test("a refused cancel-queue says the return was already sent, in one plain sentence", () => {
  const refused = new Error("Other error: Only an unclaimed queued 1601C snapshot can be canceled");
  expect(isAlreadySent(refused)).toBe(true);
  expect(isAlreadySent(new Error("Only an unclaimed queued 2551Q snapshot can be canceled"))).toBe(true);
  expect(cancelQueueMessage(refused)).toBe("This return was already sent.");
  expect(ALREADY_SENT).toBe("This return was already sent.");
  // Another refusal keeps the host's words, without the label of the error type.
  expect(isAlreadySent(new Error("Other error: The database is locked"))).toBe(false);
  expect(cancelQueueMessage(new Error("Other error: The database is locked"))).toBe("The database is locked");
});
