// The Toaster's layer lets presses through to what is under it, and its toasts still take their own presses: the
// form page's pill sits under it, at phone and at desktop size.
import { expect, test } from "nsh/test";
import { SIZES } from "./harness";
import { open } from "nsh/test";

const ENTRY = "./ToasterProbe.tsx";

for (const size of SIZES) {
  test(`${size.name}: with no toast showing, a press at the bottom reaches the bar under the Toaster`, async () => {
    const app = await open(ENTRY, size.options);
    await app.press("id:probe-pill");
    expect(app.texts()).toContain("Pill pressed: 1");
  });

  test(`${size.name}: with a toast showing, a press outside its card reaches the bar and its own buttons work`, async () => {
    const app = await open(ENTRY, size.options);
    await app.press("id:probe-undo");
    await app.waitFor("text:Return deleted");
    await app.press("id:probe-pill");
    expect(app.texts()).toContain("Pill pressed: 1");
    await app.press("id:toast-action");
    await app.waitFor("text:Undone: 1");
    await app.waitFor("text:Return deleted", { state: "gone" });
  });
}
