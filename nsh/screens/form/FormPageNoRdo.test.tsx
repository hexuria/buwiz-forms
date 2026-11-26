// 1601C for a taxpayer whose profile has no RDO: Item 7's RDO box is open and lists the real RDOs (the catalog), never the
// dummy "000" that 1601C's leftover schema option script used to leave as the only choice.
import { expect, open, test } from "nsh/test";

const RDO = "id:fp-frm1601c:txtRDOCode";

test("desktop: 1601C with no profile RDO lists the real RDOs and not 000, and takes a pick", async () => {
  const app = await open("./FormPageNoRdo.tsx", { width: 1440, height: 900, platform: "macos" });
  await app.waitFor("text:BIR Form 1601C", { timeout: 20000 });
  // Item 7 is in Part I, a folded card (only part 0 opens with the form): open it, then bring the box into view.
  await app.waitFor("id:fp-part-1-toggle");
  await app.press("id:fp-part-1-toggle");
  await app.waitUntil(() => app.count("id:fp-part-1 >> text:/\\S/") > 2, { timeout: 10000 });
  for (let i = 0; i < 12 && !app.shows(`${RDO}-trigger`); i++) await app.scroll("id:form-scroll", 300).catch(() => {});
  await app.waitFor(`${RDO}-trigger`, { timeout: 15000 });
  // The box draws just above the floating pill: scroll it clear before pressing.
  await app.scroll("id:form-scroll", 300).catch(() => {});
  await app.waitFor(`${RDO}-trigger`, { timeout: 5000 });
  expect(app.enabled(`${RDO}-trigger`)).toBe(true);

  // The catalog is over the search list's threshold, so it is the search pick, starting at RDO 001.
  await app.press(`${RDO}-trigger`);
  await app.waitFor(`${RDO}-opt-001`, { timeout: 10000 });
  expect(app.shows(`${RDO}-opt-000`)).toBe(false);
  expect(app.texts().some((t) => t.includes("rdo.description"))).toBe(false);
  // Searching "000" finds nothing: it is in no RDO's code or name.
  await app.type(`${RDO}-search`, "000");
  await app.waitFor("text:Nothing matches. Try a code or other words.", { timeout: 10000 });
  await app.clear(`${RDO}-search`);

  // A real code further down the list is found by its words and picked.
  await app.type(`${RDO}-search`, "040");
  await app.waitFor(`${RDO}-opt-040`, { timeout: 10000 });
  await app.press(`${RDO}-opt-040`);
  await app.waitFor(`${RDO}-opt-040`, { state: "gone", timeout: 10000 });
  await app.waitUntil(() => app.texts().some((t) => /^040\b/.test(t)), { timeout: 10000 });
}, 90_000);
