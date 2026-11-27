import { effect } from "nsh";
import { expect, test } from "nsh/test";
import { Door } from "ebirforms/native";
import { bir } from "../bir";
import { createDemoTaxpayer } from "../bir/testing";
import { boot, pollSteps } from "./app";
import { store } from "./store";

test("unchanged event polls keep reactive reads; changes, unknown tokens and errors refresh", async () => {
  await boot(undefined, { firstRun: false });
  const original = Door.dataChangeToken;
  let token: string | null = "idle:first";
  let failed = false;
  Door.dataChangeToken = () => {
    if (failed) throw new Error("database unavailable");
    return token;
  };
  let evaluations = 0;
  effect(() => { store.returns("filed"); evaluations++; });
  try {
    pollSteps.events();
    const before = evaluations;
    for (let i = 0; i < 20; i++) pollSteps.events();
    expect(evaluations - before).toBe(0);
    token = "idle:changed";
    pollSteps.events();
    expect(evaluations - before).toBe(1);
    token = null;
    pollSteps.events();
    pollSteps.events();
    expect(evaluations - before).toBe(3);
    failed = true;
    pollSteps.events();
    expect(evaluations - before).toBe(4);
  } finally {
    Door.dataChangeToken = original;
  }
});

test("the real database token keeps an empty returns read until a write", async () => {
  pollSteps.events();
  let evaluations = 0;
  effect(() => { store.returns("filed"); evaluations++; });
  const before = evaluations;
  const start = Date.now();
  for (let i = 0; i < 20; i++) pollSteps.events();
  console.log(`SPEED idle: 20 unchanged polls; reactive evaluations=${evaluations - before}; wall=${Date.now() - start}ms`);
  expect(evaluations - before).toBe(0);
  // The theme is a device setting since accounts-and-lock A1 (device.db): the database token does not follow it, and no
  // screen needs it to (the preference is read once at boot and set directly by its toggle).
  bir.set_setting("theme_preference", "dark");
  pollSteps.events();
  expect(evaluations - before).toBe(0);
  // A write to the database moves the token.
  createDemoTaxpayer();
  pollSteps.events();
  expect(evaluations - before).toBe(1);
});
