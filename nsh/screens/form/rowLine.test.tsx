// PR 6 (Q5/P5a): a row's message line is drawn only while the row has a message — a clean row mounts no
// fp-msg-<id> view at all (before, an empty one reserved the line), Check brings the line and its
// sentence, a box's own sentence stays under its box with no second row line, and fixing the box takes
// the line away again. 0605's Item 4 due date is the row message; Item 6's ATC is the box's own.
import { expect, open, test } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";
const PARAMS = { formCode: "0605", year: 2023, period: 1, mode: "return" };
const MSG = "id:fp-msg-0:3";
const SENTENCE = "Item 4 needs a due date.";
const DATE_BOX = "id:fp-frm0605:txtDueDateMonth#date";

for (const look of [
  { width: 390, platform: "ios" as const },
  { width: 1440, platform: "macos" as const },
]) {
  test(`PR 6: the row message line only while there is a message at ${look.width}`, async () => {
    const app = await open(ENTRY, { width: look.width, height: 900, platform: look.platform, host: { params: PARAMS } });
    await app.waitFor("id:fp-item-0:3");
    // Clean rows mount no message view: Item 4 has none yet, and Item 5 never has one (even after Check).
    expect(app.count(MSG)).toBe(0);
    expect(app.count("id:fp-msg-0:4")).toBe(0);
    await app.press("id:fp-check");
    await app.waitFor(`text:${SENTENCE}`, { timeout: 30_000 });
    // The line is there with its sentence; Item 5 stays clean; Item 6's sentence is its box's, not a row line.
    expect(app.count(MSG)).toBe(1);
    expect(app.count("id:fp-msg-0:4")).toBe(0);
    await app.waitFor("id:fp-boxmsg-txtATCCode", { timeout: 15_000 });
    expect(app.count("id:fp-msg-0:5")).toBe(0);
    // Fixing the box takes the line away again.
    if (look.width < 600) {
      await app.press(DATE_BOX);
      await app.type(`${DATE_BOX}-sheet-input`, "08172023");
      await app.press(`${DATE_BOX}-sheet-done`);
      await app.waitFor(`${DATE_BOX}-sheet-input`, { state: "gone" });
    } else {
      await app.type(DATE_BOX, "08172023");
    }
    await app.waitFor(MSG, { state: "gone", timeout: 20_000 });
    expect(app.count(MSG)).toBe(0);
  });
}
