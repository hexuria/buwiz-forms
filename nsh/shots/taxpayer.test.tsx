// Real-window screenshots of the taxpayer screens: light and dark, wide and narrow, with the demo taxpayer. The Profile tab
// (top, scrolled, a section in edit: inline when wide, the sheet when narrow, a fact row in edit), Forms (saved and in
// edit with the setups), Templates, New taxpayer (with its errors) and New branch. `just nsh-shots taxpayer` saves
// ~/BuwizNsh/evidence/taxpayer/*.png. Headless (plain nsh test) it takes no pictures and passes.
import { expect, open, test } from "nsh/test";
import { clearCells, LOOKS, shoot } from "./shoot";

const ENTRY = "../TaxpayerDemo.tsx";

for (const look of LOOKS) {
  const wide = look.width > 600;
  const size = { width: look.width, height: look.height, platform: look.platform, colorScheme: look.colorScheme };
  const at = (start: string, params: Record<string, unknown> = {}) => open(ENTRY, { ...size, host: { params: { start, branch: true, setup: true, ...params } } });

  test(`taxpayer profile, ${look.name}`, async () => {
    const app = await at("view=profile");
    await app.waitFor("id:tp-page-profile");
    await app.wait(500);
    await shoot(app, `taxpayer-${look.name}-profile`);
    await app.press("id:tp-sec-taxpayer-edit");
    await app.waitFor("id:tp-taxpayer-name");
    await clearCells(app, "id:tp-taxpayer-tin");
    await app.type("id:tp-taxpayer-tin", "0001");
    await app.press("id:tp-taxpayer-save");
    await app.wait(400);
    await shoot(app, `taxpayer-${look.name}-profile-edit`);
    await app.press("id:tp-taxpayer-cancel");
    await app.wait(300);
    await app.waitFor("id:tp-fact-eopt_tier-edit");
    await app.press("id:tp-fact-eopt_tier-edit");
    await app.wait(300);
    await shoot(app, `taxpayer-${look.name}-profile-fact-edit`);
    await app.scroll("id:main-scroll", 700);
    await app.wait(300);
    await shoot(app, `taxpayer-${look.name}-profile-facts`);
    await app.scroll("id:main-scroll", 900);
    await app.wait(300);
    await shoot(app, `taxpayer-${look.name}-profile-inbox`);
    expect(app.shows("id:tp-page-profile")).toBe(true);
  }, 120_000);

  test(`taxpayer branch, ${look.name}`, async () => {
    const app = await at("branch=00001;view=profile");
    await app.waitFor("id:tp-sec-address-edit");
    await app.press("id:tp-sec-address-edit");
    await app.waitFor("id:tp-inherit-phone");
    await app.press("id:tp-inherit-phone");
    await app.wait(400);
    await shoot(app, `taxpayer-${look.name}-branch-address-edit`);
    expect(app.shows("id:tp-page-profile")).toBe(true);
  }, 120_000);

  test(`taxpayer forms and templates, ${look.name}`, async () => {
    const app = await at("view=forms");
    await app.waitFor("id:tp-form-1601C");
    await app.press("id:tp-add-year-open");
    await app.wait(400);
    await shoot(app, `taxpayer-${look.name}-forms`);
    await app.press("id:tp-forms-edit");
    await app.waitFor("id:tp-setups");
    await app.wait(400);
    await shoot(app, `taxpayer-${look.name}-forms-edit`);
    await app.press("id:tp-forms-cancel");
    await app.press("label:Templates");
    await app.waitFor("id:tp-template-1601C");
    await app.wait(400);
    await shoot(app, `taxpayer-${look.name}-templates`);
    expect(app.shows("id:tp-page-templates")).toBe(true);
  }, 120_000);

  test(`taxpayer new taxpayer and branch, ${look.name}`, async () => {
    const app = await at("view=new_taxpayer", { peer: true });
    await app.waitFor("id:new-tp-card");
    await app.press("id:new-tp-save");
    await app.press("id:new-tp-start-trigger");
    await app.press("text:Same as a taxpayer I have…");
    await app.wait(400);
    await shoot(app, `taxpayer-${look.name}-new-taxpayer`);
    await app.press("id:new-tp-rdo_code-trigger");
    await app.type("id:new-tp-rdo_code-search", "makati");
    await app.wait(400);
    await shoot(app, `taxpayer-${look.name}-new-taxpayer-rdo`);
    const branch = await at(wide ? "view=new_branch" : "view=new_branch");
    await branch.waitFor("id:new-branch-card");
    await branch.press("id:new-branch-inherit-rdo_code");
    await branch.wait(400);
    await shoot(branch, `taxpayer-${look.name}-new-branch`);
    expect(branch.shows("id:new-branch-card")).toBe(true);
  }, 120_000);
}
