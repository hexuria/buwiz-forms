// Previous-payment gates on the page: shut until Amended Yes; closing the gate clears the amount.
import { expect, open, test } from "nsh/test";
import type { App } from "nsh/test";
import { warmForms } from "../../bir/testing";

warmForms();

const ENTRY = "./FormPageAny.tsx";

async function reach(app: App, sel: string): Promise<void> {
  for (let i = 0; i < 60 && !app.shows(sel); i++) await app.scroll("id:form-scroll", 420).catch(() => {});
  await app.waitFor(sel, { timeout: 15_000 });
}

interface Case {
  code: string;
  title: string;
  boxKey: string;
  yesKey: string;
  noKey: string;
  sentence: string;
  cleared?: string;
  removal?: string;
}

async function openForm(c: Case, setup?: ({ pick: string } | { edits: Record<string, string> } | { confirmRemoval: true })[]): Promise<App> {
  const app = await open(ENTRY, {
    width: 1440,
    height: 900,
    platform: "macos",
    host: { params: { formCode: c.code, year: 2025, period: 1, openParts: "all", setup } },
  });
  await app.waitFor(`text:${c.title}`, { timeout: 20_000 });
  return app;
}

async function exercise(c: Case): Promise<void> {
  const box = `id:fp-${c.boxKey}`;

  // A fresh return draws the box shut at zero with its amended-return sentence;
  // typing into it takes nothing.
  const fresh = await openForm(c);
  await reach(fresh, box);
  expect(fresh.texts()).toContain(c.sentence);
  await fresh.type(box, "42").catch(() => {});
  await fresh.submit(box).catch(() => {});
  expect(String(fresh.value(box))).not.toContain("42");

  // At Amended Yes the same box opens and keeps a typed amount.
  const amended = await openForm(c, [{ pick: c.yesKey }]);
  await reach(amended, box);
  await amended.waitUntil(() => !amended.texts().includes(c.sentence), { timeout: 10_000 });
  await amended.clear(box);
  await amended.type(box, "42");
  await amended.submit(box);
  await amended.waitFor("text:✓ Saved", { timeout: 15_000 });
  await amended.waitUntil(() => String(amended.value(box)).includes("42"), { timeout: 10_000 });

  // Yes, an amount, then No is the closing transition: zero is shown shut and
  // nothing dormant comes back. The session fixture applies the ordered picks
  // through the page session itself, then this test reads what the page draws.
  const closed = await openForm(c, [
    { pick: c.yesKey },
    { edits: { [c.boxKey]: "42" } },
    { pick: c.noKey },
    ...(c.removal ? [{ confirmRemoval: true } as const] : []),
  ]);
  await reach(closed, box);
  if (c.cleared) {
    await closed.waitUntil(() => closed.texts().includes(c.cleared!), { timeout: 10_000 });
  }
  if (c.removal) {
    // The removal toast is short-lived; the host test owns its exact wording.
    // Here the page must not also carry the inline clearing sentence.
    expect(closed.texts()).not.toContain(c.sentence.replace("applies only", "was cleared: it applies only"));
  }
  await closed.waitUntil(() => closed.texts().includes("0.00"), { timeout: 10_000 });
  expect(closed.texts()).not.toContain("42.00");
}

test("1600WP item 13 and 1600PT item 19 are shut until Amended Yes and clear when it closes", async () => {
  await exercise({
    code: "1600WP",
    title: "BIR Form 1600WP",
    boxKey: "frm1600WP:txtTax13",
    yesKey: "frm1600WP:AmendedReturn_1",
    noKey: "frm1600WP:AmendedReturn_2",
    sentence: "Item 13 applies only to an amended return.",
    cleared: "Item 13 was cleared: it applies only to an amended return.",
  });
  await exercise({
    code: "1600PT",
    title: "BIR Form 1600PT",
    boxKey: "frm1600PT:txtPg1P2I19TaxRemittedInRtrn",
    yesKey: "frm1600PT:rdoPg1I2AmendedYes",
    noKey: "frm1600PT:rdoPg1I2AmendedNo",
    sentence: "Item 19 applies only to an amended return.",
    cleared: "Item 19 was cleared: it applies only to an amended return.",
  });
}, 120_000);


test("0619E item 15, 0619F item 16 and 2550M item 23D are gated on the page", async () => {
  await exercise({
    code: "0619E",
    title: "BIR Form 0619E",
    boxKey: "frm0619E:txtTax15",
    yesKey: "frm0619E:optAmend:Y",
    noKey: "frm0619E:optAmend:N",
    sentence: "Item 15 applies only to an amended return.",
    cleared: "Item 15 was cleared: it applies only to an amended return.",
  });
  await exercise({
    code: "0619F",
    title: "BIR Form 0619F",
    boxKey: "frm0619F:txtTax16",
    yesKey: "frm0619F:optAmend:Y",
    noKey: "frm0619F:optAmend:N",
    sentence: "Item 16 applies only to an amended return.",
    cleared: "Item 16 was cleared: it applies only to an amended return.",
  });
  await exercise({
    code: "2550M",
    title: "BIR Form 2550M",
    boxKey: "frm2550m:txtTax23D",
    yesKey: "frm2550m:OptAmendedYN1",
    noKey: "frm2550m:OptAmendedYN2",
    sentence: "Item 23D applies only to an amended return.",
    removal: "Cleared item 23D",
  });
}, 180_000);
