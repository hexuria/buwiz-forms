// The Form library on the frame, at phone size and at desktop size, against the demo taxpayer (ShellDemo.tsx: Juana Santos,
// with 1601C and 2551Q filed this year). Each test asks for its own database through open's host params.
import { expect, open, test } from "nsh/test";
import { bir } from "../../bir";
import { DEMO } from "../../bir/testing";
import { searchText } from "./rules";

const ENTRY = "../../ShellDemo.tsx";
const DESKTOP = { width: 1440, height: 900, platform: "macos" as const };
const asking = (params: Record<string, unknown> = {}) => ({ host: { params } });
const YEAR = new Date().getFullYear();
/** The forms of the library: all 43 (the index is the same on every run). */
const TOTAL = 43;
const forms = () => bir.screen_forms_index();
const count = (test: (f: ReturnType<typeof forms>[number]) => boolean) => forms().filter(test).length;

test("phone: the Forms tab opens the library on the forms filed this year, and a card opens its form as a sample", async () => {
  const app = await open(ENTRY, asking()); // 390 x 844, iOS: Home first
  await app.waitFor("id:tab-bar");
  await app.press("id:tab-library");
  await app.waitFor("id:library-grid");
  expect(app.shows("id:sidebar")).toBe(false);
  expect(forms()).toHaveLength(TOTAL);
  expect(app.texts()).toContain("Form library");
  expect(app.texts()).toContain(`Filed in ${YEAR} (2)`);
  expect(app.texts()).toContain(`All forms (${TOTAL})`);
  expect(app.texts()).toContain(`2 of ${TOTAL}`);
  expect(app.shows("id:library-1601C")).toBe(true);
  expect(app.shows("id:library-2551Q")).toBe(true);

  await app.press("id:library-2551Q");
  await app.waitFor("id:form-page");
  expect(app.texts().some((t) => t.startsWith("A sample from the Form library"))).toBe(true);
});

test("phone: a taxpayer with no forms filed this year sees the empty Filed in list, and All forms lists every form", async () => {
  const app = await open(ENTRY, asking({ start: "view=library", years: false }));
  await app.waitFor("id:library-empty");
  expect(app.texts()).toContain(`${DEMO.name} has no forms for this year yet. Choose them under Forms, or see All forms.`);
  expect(app.texts()).toContain("Filed in this year (0)");
  expect(app.texts()).toContain(`0 of ${TOTAL}`);

  await app.press("id:library-scope >> text:/^All forms/");
  await app.waitFor("id:library-grid");
  expect(app.shows("id:library-empty")).toBe(false);
  expect(app.texts()).toContain(`${TOTAL} of ${TOTAL}`);
});

test("phone: a search that finds nothing says so, and the count reads 0", async () => {
  const app = await open(ENTRY, asking({ start: "view=library" }));
  await app.waitFor("id:library-grid");
  await app.type("id:library-search", "zzz");
  await app.waitFor("id:library-empty");
  expect(app.texts()).toContain("No form matches. Try a code like 1601C or a word like VAT.");
  expect(app.texts()).toContain(`0 of ${TOTAL}`);
});

test("desktop: the sidebar opens the library, and the scope, the frequency and the search narrow it, the count says how many", async () => {
  const app = await open(ENTRY, { ...DESKTOP, ...asking() });
  await app.waitFor("id:sidebar");
  await app.press("id:nav-library");
  await app.waitFor("id:library-grid");
  expect(app.shows("id:tab-bar")).toBe(false);
  expect(app.texts()).toContain(`2 of ${TOTAL}`);

  await app.press("id:library-scope >> text:/^All forms/");
  await app.waitFor(`text:${TOTAL} of ${TOTAL}`);

  const quarterly = count((f) => f.freq === "Quarterly");
  await app.press("id:library-freq >> text:Quarterly");
  await app.waitFor(`text:${quarterly} of ${TOTAL}`);

  await app.type("id:library-search", "vat");
  const quarterlyVat = count((f) => f.freq === "Quarterly" && searchText(f).includes("vat"));
  await app.waitFor(`text:${quarterlyVat} of ${TOTAL}`);

  await app.press("id:library-freq >> text:Monthly");
  const monthlyVat = count((f) => f.freq === "Monthly" && searchText(f).includes("vat"));
  await app.waitFor(`text:${monthlyVat} of ${TOTAL}`);
  expect(app.value("id:library-search")).toBe("vat");

  // The search, the scope and the frequency are kept when the person goes to another page and back, as in the old app.
  await app.press("id:nav-home");
  await app.waitFor("id:library-grid", { state: "gone" });
  await app.press("id:nav-library");
  await app.waitFor("id:library-grid");
  expect(app.value("id:library-search")).toBe("vat");
  expect(app.texts()).toContain(`${monthlyVat} of ${TOTAL}`);
});

test("desktop: every card shows its code, its title and its chips, and Draft layout only where the layout is a draft", async () => {
  const app = await open(ENTRY, { ...DESKTOP, ...asking({ start: "view=library" }) });
  await app.waitFor("id:library-grid");
  await app.press("id:library-scope >> text:/^All forms/");
  await app.waitFor(`text:${TOTAL} of ${TOTAL}`);

  expect(app.texts().filter((t) => t === "Draft layout")).toHaveLength(count((f) => f.q < 0.8));
  const sample = forms().find((f) => f.c === "1601C");
  expect(sample).toBeDefined();
  if (sample) {
    expect(app.shows("id:library-1601C")).toBe(true);
    expect(app.texts()).toContain(sample.freq);
    expect(app.texts()).toContain(`${sample.pages}p`);
    expect(app.texts()).toContain(`${sample.n} fields`);
  }
});

test("desktop: a card opens its form as a sample", async () => {
  const app = await open(ENTRY, { ...DESKTOP, ...asking({ start: "view=library" }) });
  await app.waitFor("id:library-grid");
  await app.press("id:library-1601C");
  await app.waitFor("id:form-page");
  expect(app.texts().some((t) => t.startsWith("A sample from the Form library"))).toBe(true);
  expect(app.shows("id:library-grid")).toBe(false);
});

test("desktop: a taxpayer with two branches names the selected branch in the lede", async () => {
  const app = await open(ENTRY, { ...DESKTOP, ...asking({ start: "view=library", branch: true }) });
  await app.waitFor("id:library-grid");
  expect(app.texts().some((t) => /^Forms Juana Santos \(.+\) files in \d{4}\. /.test(t))).toBe(true);
  expect(app.texts()).toContain("Filed in " + YEAR + " (2)");
});

test("desktop: with no taxpayer the library lists every form, and a card asks for one first", async () => {
  const app = await open(ENTRY, { ...DESKTOP, ...asking({ start: "view=library", demo: false }) });
  await app.waitFor("id:library-grid");
  expect(app.texts()).toContain("Every return eBIRForms can prepare. Add a taxpayer to fill one in.");
  expect(app.texts().some((t) => t.startsWith("Filed in"))).toBe(false);
  expect(app.texts()).toContain(`${TOTAL} of ${TOTAL}`);

  await app.press("id:library-1601C");
  await app.waitFor("text:Add a taxpayer first, then open a form for them.");
  expect(app.shows("id:form-page")).toBe(false);
});
