// The right panel is a read-only summary of the tax profile (owner decision
// 13), and the New return dialog names a fact the chosen form reads that is
// not set (decision 3), through the real app/main.js and app/shell/chrome.js
// on a mocked kit and host. The demo taxpayer is Juana Santos,
// TIN 000-123-453-00000.
import assert from "node:assert/strict";
import { makeApp, cx, one, byId, nodes, click, TIN } from "./shell_mount.mjs";

/** Every string drawn under `m`. */
const texts = (tree) => nodes(tree).flatMap((m) => m.kids.filter((k) => typeof k === "string"));

/** A year view in the host's shape (`year_facts`). */
function view({ notSet = [], usedBy = {} } = {}) {
  const facts = ["birth_date", "tax_classification", "income_tax_election", "eopt_tier", "business_start_date", "line_of_business", "is_government_withholding_entity", "atc_codes"];
  return {
    tin: TIN,
    year: 2026,
    facts: { tax_classification: "SelfEmployed", income_tax_election: null, eopt_tier: null, business_start_date: null, line_of_business: null, is_government_withholding_entity: false, atc_codes: [] },
    owner: {},
    used_by: Object.fromEntries(facts.map((f) => [f, usedBy[f] || []])),
    not_set: notSet,
    election_locked: false,
    election_source: null,
  };
}

// ---------------------------------------------------------------- the panel
{
  const app = makeApp({ width: 1000, view: "home" });
  app.store.taxTypes = () => [
    { type: "percentage_tax", label: "Percentage tax", forms: ["2551Q"] },
    { type: "withholding_compensation", label: "Withholds on compensation", forms: ["1601C"] },
  ];
  app.store.yearFacts = () => view({ notSet: ["eopt_tier", "income_tax_election"], usedBy: { eopt_tier: ["2551Q"], income_tax_election: ["2551Q"] } });
  app.store.electionReadiness = () => ({ gap: "not_business_income" });
  app.panel = "profile";
  let tree = app.paint(cx);
  one(tree, "panel-summary");
  const words = texts(tree);
  assert.ok(words.includes("Percentage tax") && words.includes("Withholds on compensation"), "the year's tax-type chips");
  assert.ok(words.includes("1 not set"), "facts not set are counted; a rate the taxpayer does not choose is not");
  for (const id of ["panel-edit-btn", "panel-edit", "pe-name", "pe-code", "pe-bname", "pe-address", "pe-save"]) {
    assert.equal(byId(tree, id).length, 0, `no ${id}: the panel edits nothing`);
  }
  for (const id of ["panel-open-profile", "panel-open-forms", "panel-open-templates", "panel-branch-00000", "panel-add-branch"]) one(tree, id);

  click(tree, "panel-edit-profile", cx);
  assert.equal(app.view, "profile", "Edit in profile opens the Profile tab");

  // Everything set: no count.
  app.store.yearFacts = () => view();
  tree = app.paint(cx);
  assert.equal(byId(tree, "panel-not-set").length, 0, "nothing not set, no count");
}

// ---------------------------------------------------------------- new return
{
  const app = makeApp({ width: 1000, view: "profile" });
  app.store.taxTypes = () => [];
  app.store.electionReadiness = () => ({ gap: "ready" });
  app.store.yearFacts = () => view({ notSet: ["eopt_tier"], usedBy: { eopt_tier: ["2551Q", "1601C"] } });
  app.startNewReturn(cx, "2551Q");
  assert.equal(app.overlay, "new", "the dialog opens: nothing blocks");
  let tree = app.paint(cx);
  one(tree, "new-return-not-set");
  assert.ok(texts(tree).includes("EOPT tier is not set. It sets the penalty rates on this return."), "one line naming the fact");
  one(tree, "new-return-open");

  // Two facts: one line names both.
  app.store.yearFacts = () =>
    view({ notSet: ["birth_date", "eopt_tier"], usedBy: { eopt_tier: ["2551Q"], birth_date: ["2551Q"] } });
  tree = app.paint(cx);
  assert.ok(texts(tree).includes("Birth date and EOPT tier are not set. This return reads them."));

  // A form that does not read the fact: no line.
  app.store.yearFacts = () => view({ notSet: ["eopt_tier"], usedBy: { eopt_tier: ["1601C"] } });
  tree = app.paint(cx);
  assert.equal(byId(tree, "new-return-not-set").length, 0, "the chosen form does not read it");

  // The rate is not named for a taxpayer who does not choose one.
  app.store.yearFacts = () => view({ notSet: ["income_tax_election"], usedBy: { income_tax_election: ["2551Q"] } });
  app.store.electionReadiness = () => ({ gap: "not_individual" });
  tree = app.paint(cx);
  assert.equal(byId(tree, "new-return-not-set").length, 0, "no rate to choose");

  // The link goes to the profile and closes the dialog.
  app.store.yearFacts = () => view({ notSet: ["eopt_tier"], usedBy: { eopt_tier: ["2551Q"] } });
  tree = app.paint(cx);
  app.view = "home";
  click(tree, "new-return-profile", cx);
  assert.equal(app.overlay, null, "the dialog closes");
  assert.equal(app.view, "profile", "and the profile opens");
}

console.log("test_profile_panel: ok");
