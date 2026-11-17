// Filing chrome stays hidden until setup is finished and sign-in is done.
import assert from "node:assert/strict";
import fs from "node:fs";
import vm from "node:vm";

const root = new URL("../", import.meta.url);
const context = vm.createContext({ console });
const source = new vm.SourceTextModule(fs.readFileSync(new URL("app/shell/chrome_gate.js", root), "utf8"), {
  context,
  identifier: "app/shell/chrome_gate.js",
});
await source.link(() => {
  throw new Error("chrome_gate.js imports nothing");
});
await source.evaluate();
const { shellChromeVisible } = source.namespace;

const ready = {
  booted: true,
  cloudLocked: false,
  appLocked: false,
  mode: "solo",
  setupPending: null,
  setupStep: "choose",
};

assert.equal(shellChromeVisible(ready), true);
assert.equal(shellChromeVisible({ ...ready, mode: "helpdesk" }), true);

assert.equal(shellChromeVisible({ ...ready, booted: false }), false, "cloud gate before boot");
assert.equal(shellChromeVisible({ ...ready, cloudLocked: true }), false, "account gate");
assert.equal(shellChromeVisible({ ...ready, appLocked: true }), false, "sign-in screen");
assert.equal(shellChromeVisible({ ...ready, mode: null }), false, "who will use this computer");
assert.equal(shellChromeVisible({ ...ready, mode: null, setupPending: "solo" }), false, "first taxpayer");
assert.equal(shellChromeVisible({ ...ready, mode: "helpdesk", setupStep: "helpdesk-key" }), false, "recovery key");
