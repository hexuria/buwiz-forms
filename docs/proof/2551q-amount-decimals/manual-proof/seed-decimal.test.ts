// Seeds the scratch database used by the real-window decimal proof. Not part of the suite.
import { expect, test } from "nsh/test";
import { bir } from "../bir";
import { createDemoTaxpayer, DEMO, useDatabase } from "../bir/testing";

useDatabase("/home/ubuntu/urgent2551/decimal/manual-proof/decimal-proof.db");
createDemoTaxpayer();

test("seed a demo taxpayer and a saved 2551Q draft", async () => {
  const opened = await bir.screen_form_open(DEMO.tin, "2551Q", 2025, 1);
  const saved = bir.screen_form_save("2551Q", opened.draft);
  expect(saved.id).toBeGreaterThan(0);
  expect(bir.list_returns("saved").length).toBe(1);
});
