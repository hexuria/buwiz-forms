// Real-window run of the print preview (a 2551Q of the demo taxpayer): Print preview opens a second window with the filled sheet,
// an edit in the form changes it, Save as PDF writes a PDF (a test path stands in for the save panel; the system print panel
// is never opened here) whose pages and drawing are read back. Light and dark. `just nsh-shots print` saves
// ~/BuwizNsh/evidence/print/*.png. A phone has no second window, so there are no narrow looks (print.test.tsx says why).
// Headless (plain `nsh test`, `just nsh-test`, CI) there is no second window to read: the test ends once the form is up, as the other shots do.
import { open, expect, test } from "nsh/test";
import { LOOKS, shoot } from "./shoot";

for (const look of LOOKS.filter((l) => l.width >= 1000)) {
  test(`print preview, ${look.name}`, async () => {
    const pdf = `/tmp/ebirforms-print-${look.name}-${Date.now()}.pdf`;
    const app = await open("../PrintDemo.tsx", {
      width: look.width,
      height: look.height,
      platform: look.platform,
      colorScheme: look.colorScheme,
      host: { params: { start: "form=2551Q/2026/3", savePath: pdf } },
    });
    await app.waitFor("text:BIR Form 2551Q");
    await app.waitUntil(() => app.enabled("id:fp-print") && !app.shows("text:Opening…"));
    await app.wait(600);
    // `shoot` answers false where a screenshot needs a real platform (headless); everything below needs a real window.
    if (!(await shoot(app, `print-${look.name}-form`))) return;

    // Print preview: one second window, with the filled sheet.
    await app.press("id:fp-print");
    await app.waitFor("text:preview 2551Q: revision 1", { timeout: 30_000 });
    await app.waitFor("id:print-window");
    await app.wait(2500);
    const preview = app.window("print:2551Q:2551Q-00012345300000-2026-3-return");
    expect(preview.state()).not.toBeNull();
    const first = `print-${look.name}-preview.png`;
    await preview.screenshot(first).catch((error) => console.log("preview screenshot:", String(error)));

    // The main window next to it keeps its own layout (the frame reads its own window's width, not the preview's).
    await shoot(app, `print-${look.name}-form-with-preview`);

    // An edit in the form (Item 4, Amended Return? Yes) follows into the open preview, without a reload.
    await app.press("text:Yes");
    await app.waitFor("text:/^preview 2551Q: revision [2-9]/", { timeout: 8000 });
    await app.wait(800);
    await preview.screenshot(`print-${look.name}-edited.png`).catch((error) => console.log("preview screenshot:", String(error)));

    // The edit reached the open page by script: it was loaded once and never again (so its scroll position stayed).
    expect(app.texts()).toContain("page loads: 1");

    // The zoom choice: the sheet at its size.
    await app.press("text:100%");
    await app.wait(800);
    await preview.screenshot(`print-${look.name}-100.png`).catch((error) => console.log("preview screenshot:", String(error)));
    await app.press("text:Fit width");
    await app.wait(500);

    // Save as PDF (the toolbar's own button; a test path stands in for the save panel) and read the file back.
    await app.press("id:print-save-pdf");
    await app.waitFor("text:/^Saved ebirforms-print-/", { timeout: 60_000 });
    await preview.screenshot(`print-${look.name}-saved.png`).catch((error) => console.log("preview screenshot:", String(error)));
    await app.press("id:px-read-pdf");
    await app.waitFor("text:/^pdf: /");
    const read = app.texts().find((t) => t.startsWith("pdf: "))!;
    console.log(read);
    // Two sheets in the window, two pages in the file, A4 (594.75 x 841.5 pt: WebKit's whole-pixel capture), drawn, with her name.
    expect(read).toMatch(/^pdf: 2 pages, /);
    expect(Number(/pages, (\d+) bytes/.exec(read)![1])).toBeGreaterThan(20_000);
    expect(read).toMatch(/size 595x842/);
    expect(Number(/(\d+) drawing/.exec(read)![1])).toBeGreaterThan(2000);
    expect(read).toMatch(/name found/);
    expect(app.texts().some((t) => /^A4 · 2 pages/.test(t))).toBe(true);
  }, 240_000);
}
