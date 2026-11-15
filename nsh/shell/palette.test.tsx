// The command palette and the start-a-return dialog, at desktop size and at phone size.
import { expect, open, test } from "nsh/test";

const ENTRY = "../ShellDemo.tsx";
const WIDE = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });

test("desktop: Cmd+K searches taxpayers by name and by TIN, and Enter runs the row", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking({ branch: true }) });
  await app.waitFor("id:sidebar");
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  // Everything is listed before typing: taxpayers, the open taxpayer's branches, pages, actions.
  // The group headings are the one place the design keeps small capitals (the old palette): 11/600, and the rows 13/400 on 36 points.
  for (const word of ["TAXPAYERS", "BRANCHES", "GO TO", "ACTIONS", "Toggle sidebar", "Change theme"]) expect(app.texts()).toContain(word);
  expect(app.texts()).not.toContain("Go to");
  expect(app.texts()).not.toContain("Taxpayers");
  const heading = app.spanStyle("text:GO TO");
  expect(heading.fontSize).toBe(11);
  expect(heading.fontWeight).toBeGreaterThanOrEqual(600);
  expect(app.spanStyle("text:Toggle sidebar").fontWeight).toBe(400);

  await app.type("id:palette-input", "juana");
  await app.waitUntil(() => !app.texts().includes("GO TO")); // the list narrowed to the one taxpayer
  expect(app.texts()).toContain("Juana Santos");
  expect(app.shows("id:palette-item-1")).toBe(false);
  await app.submit("id:palette-input");
  await app.waitFor("id:tp-page-profile");
  expect(app.shows("id:palette-input")).toBe(false);

  // Digits match a TIN, with or without dashes: nine name the taxpayer, ten or more a branch.
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "000-123-45");
  await app.waitFor("text:000-123-453-00000");
  expect(app.texts()).toContain("Juana Santos");
  await app.clear("id:palette-input");
  await app.type("id:palette-input", "0001234530000");
  await app.waitFor("text:Juana Santos · Head office");
  await app.clear("id:palette-input");
  await app.type("id:palette-input", "999");
  await app.waitFor("text:No matches. Try a form code like 2550Q.");
  await app.keyboard.press("escape");
  await app.waitFor("id:palette-input", { state: "gone" });
});

test("desktop: the arrows move the highlight, a page runs, a form opens the start-a-return dialog, the dev kit is listed", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  // Rows: Juana Santos, Home, Form library, ...: two Downs and Enter is "Form library".
  await app.keyboard.press("down");
  await app.keyboard.press("down");
  await app.submit("id:palette-input");
  await app.waitFor("id:library");

  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "calen");
  await app.waitFor("id:palette-item-0");
  await app.press("id:palette-item-0");
  await app.waitFor("id:calendar");

  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "2551");
  await app.waitFor("text:2551Q · Quarterly Percentage Tax Return");
  await app.submit("id:palette-input");
  await app.waitFor("id:new-return");
  expect(app.shows("id:new-return-2551Q")).toBe(true);

  await app.press("id:new-return-cancel");
  await app.waitFor("id:new-return", { state: "gone" });
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "ui kit");
  await app.submit("id:palette-input");
  await app.waitFor("id:dev-pages");
  await app.press("id:dev-back");
  await app.waitFor("id:calendar");
});

test("desktop: the start-a-return dialog lists the year's forms, period, template and what is not set, and opens the form", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  await app.press("id:sb-new");
  await app.waitFor("id:new-return");
  expect(app.texts()).toContain("Start a return");
  // The old header: "Juana Santos ·", the TIN with its branch code, "Head office" (no dot between the last two), the code regular.
  await app.waitFor("id:new-return-who");
  expect(app.texts()).toContain("Juana Santos ·");
  expect(app.texts()).toContain("000-123-453-00000");
  expect(app.texts()).toContain("Head office");
  expect(app.spanStyle("id:new-return-who >> text:000-123-453-00000").fontWeight).toBe(400);
  expect(app.shows("id:new-return-1601C")).toBe(true);
  // The list's rows are the old ones: 13/400 labels (the form's title), not the sidebar's 13/500.
  expect(app.spanStyle("text:Quarterly Percentage Tax Return").fontWeight).toBe(400);
  expect(app.shows("id:new-return-2551Q")).toBe(true);
  expect(app.texts()).toContain("No template");
  expect(app.shows("id:new-return-period")).toBe(true);
  // 2551Q reads facts the demo taxpayer has not set: named, never blocking.
  await app.press("id:new-return-2551Q");
  await app.waitFor("id:new-return-not-set");
  expect(app.enabled("id:new-return-open")).toBe(true);
  await app.press("id:new-return-cancel");
  await app.waitFor("id:new-return", { state: "gone" });

  await app.keyboard.press("mod+n");
  await app.waitFor("id:new-return");
  await app.press("id:new-return-1601C");
  await app.press("id:new-return-open");
  await app.waitFor("id:form-page", { timeout: 20000 });
  expect(app.shows("id:new-return")).toBe(false);
}, 60_000);

test("desktop: the dialog says why it cannot open: no taxpayer, no tax year", async () => {
  const none = await open(ENTRY, { ...WIDE, ...asking({ demo: false }) });
  await none.waitFor("id:sidebar");
  await none.press("id:sb-new");
  await none.waitFor("text:Add a taxpayer first, then open a form for them.");
  expect(none.shows("id:new-return")).toBe(false);

  const noYear = await open(ENTRY, { ...WIDE, ...asking({ years: false }) });
  await noYear.waitFor("id:sidebar");
  await noYear.press("id:sb-new");
  await noYear.waitFor("text:Head office has no tax year yet. Add one to choose the forms it files.");
});

test("desktop: Recent lists the returns opened (the last four), marks the one on screen, and is kept", async () => {
  const app = await open(ENTRY, { ...WIDE, ...asking() });
  await app.waitFor("id:sidebar");
  expect(app.texts()).not.toContain("Recent");
  await app.press("id:sb-new");
  await app.waitFor("id:new-return");
  await app.press("id:new-return-1601C");
  await app.press("id:new-return-open");
  await app.waitFor("id:form-page", { timeout: 20000 });
  await app.waitFor("text:Recent");
  const row = () => app.texts().find((t) => /^1601C .* · Juana Santos$/.test(t));
  await app.waitUntil(() => row() !== undefined);
  // The whole label shows: 12/500 with the letters a little closer, and a second line for a longer name (never an ellipsis cut).
  expect(app.spanStyle("text:" + row()).fontSize).toBe(12);
  // What the app keeps: under the logical key, suffixed by the database path.
  await app.waitUntil(() => Object.keys(app.storage()).some((k) => k.startsWith("redesign.recent@")));
  const kept = Object.entries(app.storage()).find(([k]) => k.startsWith("redesign.recent@"))!;
  expect(JSON.parse(kept[1])[0].code).toBe("1601C");

  // Leaving the form and pressing its Recent row opens it again.
  await app.press("id:nav-home");
  await app.waitFor("id:home");
  await app.press(`text:${row()}`);
  await app.waitFor("id:form-page", { timeout: 20000 });
}, 90_000);

test("desktop: the showRecent preference hides Recent", async () => {
  const app = await open(ENTRY, { ...WIDE, storage: { "redesign.prefs": JSON.stringify({ showRecent: false }) }, ...asking() });
  await app.waitFor("id:sidebar");
  await app.press("id:sb-new");
  await app.waitFor("id:new-return");
  await app.press("id:new-return-1601C");
  await app.press("id:new-return-open");
  await app.waitFor("id:form-page", { timeout: 20000 });
  await app.wait(300);
  expect(app.texts()).not.toContain("Recent");
}, 60_000);

test("phone: the palette opens from the header and a taxpayer opens its profile", async () => {
  const app = await open(ENTRY, asking());
  await app.waitFor("id:narrow-bar");
  await app.press("id:nb-search");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "juana");
  await app.press("id:palette-item-0");
  await app.waitFor("id:tp-page-profile");
  expect(app.shows("id:palette-input")).toBe(false);
});

test("desktop: Next to do and Previous to do are in the palette only while a form is open, and step its pill", async () => {
  const home = await open(ENTRY, { ...WIDE, ...asking() });
  await home.waitFor("id:sidebar");
  await home.keyboard.press("mod+k");
  await home.waitFor("id:palette-input");
  await home.type("id:palette-input", "to do");
  await home.waitFor("text:No matches. Try a form code like 2550Q.");

  const app = await open(ENTRY, { ...WIDE, ...asking({ start: "form=2551Q/2026/3" }) });
  await app.waitFor("id:fp-pill");
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "to do");
  await app.waitFor("text:Next to do");
  expect(app.texts()).toContain("Previous to do");
  // The pill counts what is to do before anything is touched; a step goes to the first of those rows (Cmd+Right does the same).
  expect(app.texts().some((t) => /^\d+ to do · /.test(t))).toBe(true);
  await app.submit("id:palette-input");
  await app.waitFor("id:palette-input", { state: "gone" });
  await app.waitUntil(() => app.texts().some((t) => /^1 of \d+ to do · /.test(t)));

  // Next once more: the second row. Previous: the first again.
  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "next to");
  await app.waitFor("text:Next to do");
  await app.submit("id:palette-input");
  await app.waitFor("id:palette-input", { state: "gone" });
  await app.waitUntil(() => app.texts().some((t) => /^2 of \d+ to do · /.test(t)));

  await app.keyboard.press("mod+k");
  await app.waitFor("id:palette-input");
  await app.type("id:palette-input", "previous");
  await app.waitFor("text:Previous to do");
  await app.submit("id:palette-input");
  await app.waitFor("id:palette-input", { state: "gone" });
  await app.waitUntil(() => app.texts().some((t) => /^1 of \d+ to do · /.test(t)));
});
