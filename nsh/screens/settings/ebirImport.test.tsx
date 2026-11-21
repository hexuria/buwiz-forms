// F18: Settings > Import / Export > Import from BIR eBIRForms, at phone size and at desktop size. The host's folder dialog cannot be
// pressed from a test: SettingsDemo's `ebir` param stands in for the host's answers (the real scan and import are tested in Rust,
// crates/bir-app-api/src/legacy_import_module.rs). Choose, review, import, cancel, an empty or wrong folder, host errors, busy.
import { type App, expect, test } from "nsh/test";
import { calls, expectTexts, openSettings, SIZES } from "./testing";

const HINT =
  "Brings in taxpayer details and filed returns from the old eBIRForms program's folder. It only reads: the folder is left as it is, and nothing is sent to BIR.";

const NOT_EBIRFORMS =
  "This is not an eBIRForms folder. Choose the folder the old program is installed in (C:\\eBIRForms), or the profile or IAF_RDO_Copy folder inside it.";
const NOTHING_FOUND = "Nothing to import here: this eBIRForms folder has no taxpayer profiles and no filed returns.";

const taxpayers = (app: App) => app.texts().find((t) => t.startsWith("taxpayers=")) ?? "";

async function review(app: App): Promise<void> {
  await app.waitFor("id:settings-ebir-choose");
  await app.press("id:settings-ebir-choose");
  await app.waitFor("id:settings-ebir-review", { timeout: 10000 });
}

for (const { name, size } of SIZES) {
  test(`${name}: the card says what it brings in; Choose folder, the review's counts, Import, the result and Done`, async () => {
    const app = await openSettings(size, { ebir: "found" }, "transfer");
    await app.waitFor("id:settings-ebir-choose");
    expectTexts(app, ["Import data", "Import from BIR eBIRForms", HINT, "Choose folder…"]);
    expect(app.enabled("id:settings-ebir-choose")).toBe(true);
    expect(app.shows("id:settings-ebir-standard-0")).toBe(false);
    expect(app.shows("id:settings-ebir-review")).toBe(false);
    await app.waitFor("text:taxpayers=1");

    await review(app);
    expect(calls(app)).toEqual(["legacy_scan_folder"]);
    expectTexts(app, ["Ready to import", "/Volumes/TRANSFER/eBIRForms", "Taxpayers", "Filed returns", "Saved drafts (not imported)", "Files not read", "Not read", "garbage.xml", "invalid return filename", "Cancel", "Import"]);
    expectTexts(app, ["1", "3"]);
    // While the review is open, another scan waits.
    expect(app.enabled("id:settings-ebir-choose")).toBe(false);

    await app.press("id:settings-ebir-import");
    await app.waitFor("id:settings-ebir-result", { timeout: 10000 });
    expect(calls(app)).toEqual(["legacy_scan_folder", "legacy_import 7"]);
    await app.waitFor("text:Imported 1 taxpayer and 3 filed returns from eBIRForms.");
    expectTexts(app, ["Import finished", "Taxpayers added", "Taxpayers with details filled in", "Taxpayers already here", "Filed returns added", "Filed returns already here"]);
    // F20: the result says where the filed returns are (Filings), not that no page lists them.
    expectTexts(app, ["New taxpayers are in the taxpayer list. Filed returns are listed in Filings, under Filed in BIR eBIRForms, for the taxpayer you have open."]);
    expect(app.texts().some((t) => t.includes("no page lists them"))).toBe(false);
    // Every cached read was dropped: the frame lists the new taxpayer at once.
    await app.waitFor("text:taxpayers=2", { timeout: 5000 });
    expect(app.shows("id:settings-ebir-review")).toBe(false);

    await app.press("id:settings-ebir-done");
    await app.waitFor("id:settings-ebir-result", { state: "gone" });
    expect(app.enabled("id:settings-ebir-choose")).toBe(true);
  }, 120_000);

  test(`${name}: Cancel lets the scan go; a dismissed folder dialog changes nothing`, async () => {
    const app = await openSettings(size, { ebir: "found" }, "transfer");
    await review(app);
    await app.press("id:settings-ebir-cancel");
    await app.waitFor("id:settings-ebir-review", { state: "gone" });
    expect(calls(app)).toEqual(["legacy_scan_folder", "legacy_scan_cancel"]);
    expect(app.enabled("id:settings-ebir-choose")).toBe(true);
    expect(taxpayers(app)).toBe("taxpayers=1");

    const dismissed = await openSettings(size, { ebir: "cancelled" }, "transfer");
    await dismissed.waitFor("id:settings-ebir-choose");
    await dismissed.press("id:settings-ebir-choose");
    await dismissed.waitFor("text:calls: legacy_scan_folder");
    await dismissed.waitFor("text:Choose folder…");
    expect(dismissed.enabled("id:settings-ebir-choose")).toBe(true);
    for (const id of ["review", "result", "problem"]) expect(dismissed.shows(`id:settings-ebir-${id}`)).toBe(false);
  }, 120_000);

  test(`${name}: a dismissed folder dialog leaves the result on the card as it was`, async () => {
    const app = await openSettings(size, { ebir: "foundonce" }, "transfer");
    await review(app);
    await app.press("id:settings-ebir-import");
    await app.waitFor("id:settings-ebir-result", { timeout: 10000 });
    // The person opens the folder dialog again and dismisses it: the result is still there to read.
    await app.press("id:settings-ebir-choose");
    await app.waitFor("text:calls: legacy_scan_folder | legacy_import 7 | legacy_scan_folder");
    await app.waitFor("id:settings-ebir-result");
    expectTexts(app, ["Import finished"]);
    expect(app.enabled("id:settings-ebir-choose")).toBe(true);
  }, 120_000);

  test(`${name}: restoring a database backup during the review ends the review`, async () => {
    // The host lets a scan go when the open database is replaced; the page must not keep offering to import it.
    const app = await openSettings(size, { ebir: "found", import: "database" }, "transfer");
    await review(app);
    await app.press("id:settings-import-btn");
    await app.waitFor("text:Database imported.");
    await app.waitFor("id:settings-ebir-review", { state: "gone" });
    expect(app.enabled("id:settings-ebir-choose")).toBe(true);
  }, 120_000);

  test(`${name}: on Windows the standard folder is offered by its path with Scan`, async () => {
    const app = await openSettings(size, { ebir: "standard" }, "transfer");
    await app.waitFor("id:settings-ebir-scan-0");
    expectTexts(app, ["Found on this computer", "C:\\eBIRForms", "Scan"]);
    await app.press("id:settings-ebir-scan-0");
    await app.waitFor("id:settings-ebir-review", { timeout: 10000 });
    expect(calls(app)).toEqual(["legacy_scan_standard 0"]);
    expect(app.enabled("id:settings-ebir-scan-0")).toBe(false);
  }, 60_000);

  test(`${name}: an empty folder and a folder that is not eBIRForms say so in plain words`, async () => {
    const empty = await openSettings(size, { ebir: "empty" }, "transfer");
    await empty.waitFor("id:settings-ebir-choose");
    await empty.press("id:settings-ebir-choose");
    await empty.waitFor("id:settings-ebir-problem", { timeout: 10000 });
    expectTexts(empty, [NOTHING_FOUND, "It holds 2 saved drafts; drafts are not imported."]);
    expect(empty.shows("id:settings-ebir-import")).toBe(false);
    await empty.press("id:settings-ebir-ok");
    await empty.waitFor("id:settings-ebir-problem", { state: "gone" });

    const wrong = await openSettings(size, { ebir: "wrong" }, "transfer");
    await wrong.waitFor("id:settings-ebir-choose");
    await wrong.press("id:settings-ebir-choose");
    await wrong.waitFor("id:settings-ebir-problem", { timeout: 10000 });
    expectTexts(wrong, [NOT_EBIRFORMS]);
    expect(wrong.texts().some((t) => t.includes("C:\\eBIRForms"))).toBe(true);
    expect(wrong.shows("id:settings-ebir-import")).toBe(false);
  }, 120_000);

  test(`${name}: a host error says what failed; a refused import imports nothing and says why`, async () => {
    const fail = await openSettings(size, { ebir: "fail" }, "transfer");
    await fail.waitFor("id:settings-ebir-choose");
    await fail.press("id:settings-ebir-choose");
    await fail.waitFor("text:The folder could not be read: The file dialog could not open: no window", { timeout: 10000 });
    expect(fail.enabled("id:settings-ebir-choose")).toBe(true);

    const refused = await openSettings(size, { ebir: "importfail" }, "transfer");
    await review(refused);
    await refused.press("id:settings-ebir-import");
    await refused.waitFor("text:Nothing was imported: The folder changed since it was read. Scan it again before importing.", { timeout: 10000 });
    await refused.waitFor("text:Import failed: The folder changed since it was read. Scan it again before importing.");
    expect(refused.shows("id:settings-ebir-result")).toBe(false);
    expect(taxpayers(refused)).toBe("taxpayers=1");
    expect(refused.enabled("id:settings-ebir-choose")).toBe(true);
  }, 120_000);

  test(`${name}: while the folder is read the button says so and a second press asks nothing`, async () => {
    const app = await openSettings(size, { ebir: "slow" }, "transfer");
    await app.waitFor("id:settings-ebir-choose");
    await app.press("id:settings-ebir-choose");
    await app.waitFor("text:Reading…");
    expect(app.enabled("id:settings-ebir-choose")).toBe(false);
    expect(String(await app.press("id:settings-ebir-choose").then(() => "pressed", (e) => e))).toContain("is disabled");
    await app.waitFor("id:settings-ebir-review", { timeout: 10000 });
    expect(calls(app)).toEqual(["legacy_scan_folder"]);
  }, 60_000);

  test(`${name}: a taxpayer who signed in sees no eBIRForms import`, async () => {
    const app = await openSettings(size, { ebir: "found", solo: true, pin: "1234", signin: "taxpayer" }, "transfer");
    await app.waitFor("id:tp-export", { timeout: 15000 });
    expect(app.shows("id:settings-ebir-choose")).toBe(false);
  }, 60_000);
}
