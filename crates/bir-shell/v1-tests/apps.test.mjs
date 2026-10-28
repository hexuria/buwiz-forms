// The BIR mini apps as Rox v1 compiles them (spec 6.3): imports from
// `nsh`, `nsh/base` and `rox` only, a default export, no network, no v0
// globals. With BIR_MINI_APP_BIN set, also the manifest the binary prints:
// every tool an app calls is in its permissions, and every bridge method it
// uses is granted. Rox's real compile runs in the end-to-end test; this is
// the cheap guard on every change.
//
//   node --test crates/bir-shell/v1-tests/apps.test.mjs
//   BIR_MINI_APP_BIN=<target>/debug/bir-mini-app node --test crates/bir-shell/v1-tests/apps.test.mjs
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdtempSync, readFileSync, readdirSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const APPS_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "mini-apps");
const ALLOWED = new Set(["nsh", "nsh/base", "rox"]);
const apps = readdirSync(APPS_DIR)
  .filter((f) => f.endsWith(".tsx"))
  .map((f) => ({ id: f.slice(0, -4), source: readFileSync(join(APPS_DIR, f), "utf8") }));

/** Every module specifier a source names: static and type imports,
 *  re-exports, side-effect imports, dynamic import() and require(). */
function specifiers(source) {
  const out = [];
  const patterns = [
    /^\s*import\s+(?:type\s+)?[^"';]*?\sfrom\s*["']([^"']+)["']/gm,
    /^\s*import\s*["']([^"']+)["']/gm,
    /^\s*export\s+[^"';]*?\sfrom\s*["']([^"']+)["']/gm,
    /\bimport\s*\(\s*["'`]([^"'`]+)["'`]\s*\)/g,
    /\brequire\s*\(\s*["'`]([^"'`]+)["'`]\s*\)/g,
  ];
  for (const re of patterns) for (const m of source.matchAll(re)) out.push(m[1]);
  return out;
}

const CARD_APPS = ["card-dates", "card-forms", "card-help", "card-money", "card-people"];
const KIT = readFileSync(join(APPS_DIR, "card-kit.tsx.in"), "utf8").trimEnd();
/** An app's own code: a card app's kit block left out (it is checked once, below). */
const own = (source) => source.replace(/\/\/ card-kit:begin[\s\S]*?\/\/ card-kit:end/, "");

test("there are the v1 apps", () => {
  assert.deepEqual(apps.map((a) => a.id).sort(), [...CARD_APPS, "calendar", "deadlines", "forms", "register", "tax-profile"].sort());
});

for (const id of CARD_APPS)
  test(`${id}: carries the card kit verbatim and answers rox.props.card`, () => {
    const { source } = apps.find((a) => a.id === id);
    assert.ok(source.includes(KIT), `${id}: run scripts/sync-card-kit.mjs`);
    assert.ok(!source.includes("@@CARD-KIT@@"));
    assert.match(own(source), /P\.card/);
  });

for (const app of apps) {
  test(`${app.id}: imports only nsh, nsh/base and rox, and has a default export`, () => {
    const named = specifiers(app.source);
    assert.ok(named.includes("rox"), "uses the rox bridge");
    for (const s of named) assert.ok(ALLOWED.has(s), `${app.id} imports ${s}`);
    assert.match(app.source, /^export\s+default\s+(?:async\s+)?function\b/m);
  });

  test(`${app.id}: no network, no v0 globals, no TIN links`, () => {
    for (const bad of ["XMLHttpRequest", "WebSocket", "localStorage", "sessionStorage", "127.0.0.1", "8787", "globalThis", "nsh://", "rox://app/bir/", "bir_call", "Clipboard"])
      assert.ok(!app.source.includes(bad), `${app.id} still has ${bad}`);
    assert.doesNotMatch(app.source, /(?<![\w.])fetch\s*\(/, `${app.id} still calls fetch`);
  });
}

/** Literal tool names an app calls: rox.call("x") and its bir("fn") helper (bir_app_fn). */
function toolsCalled(source) {
  const out = new Set();
  for (const m of source.matchAll(/\brox\s*\.call\(\s*"([a-z0-9_]+)"/g)) out.add(m[1]);
  for (const m of source.matchAll(/\btool(?:<.*?>)?\(\s*"([a-z0-9_]+)"/g)) out.add(m[1]);
  for (const m of source.matchAll(/\bbir(?:<.*?>)?\(\s*"([a-z0-9_]+)"/g)) out.add(`bir_app_${m[1]}`);
  return out;
}

const bin = process.env.BIR_MINI_APP_BIN;

test("the manifest grants each app what its source uses, and no more tools than it calls", { skip: !bin && "set BIR_MINI_APP_BIN" }, () => {
  // HOME is a temp dir: the binary must not reach BIR's real data directory
  const home = mkdtempSync(join(tmpdir(), "bir-v1-apps-"));
  let manifest;
  try {
    manifest = JSON.parse(execFileSync(bin, ["print-v1-manifest"], { env: { PATH: process.env.PATH, HOME: home, BIR_MINI_APPS_DIR: APPS_DIR } }).toString());
    assert.deepEqual(readdirSync(home), [], "print-v1-manifest writes nothing");
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
  assert.deepEqual(manifest.apps.map((a) => a.id).sort(), apps.map((a) => a.id).sort());
  for (const entry of manifest.apps) {
    const source = own(apps.find((a) => a.id === entry.id).source);
    const p = entry.permissions || {};
    const granted = new Set(p.tools || []);
    const called = toolsCalled(source);
    for (const t of called) assert.ok(granted.has(t), `${entry.id} calls ${t}, which its permissions leave out`);
    // A card app's Refresh calls the tool that answered (props.tool), by name
    // at run time: its grants are those answers' tools.
    if (!CARD_APPS.includes(entry.id))
      for (const t of granted) assert.ok(called.has(t), `${entry.id} is granted ${t} but never calls it`);
    const uses = (m) => new RegExp(`\\brox\\s*\\.${m}\\(`).test(source);
    if (uses("open")) assert.ok(p.open?.apps?.length, `${entry.id} uses rox.open`);
    if (uses("attach")) assert.ok(p.attach?.length, `${entry.id} uses rox.attach`);
    if (uses("pickFile")) assert.ok(p.pick_file, `${entry.id} uses rox.pickFile`);
    if (uses("copy")) assert.equal(p.copy, true, `${entry.id} uses rox.copy`);
    for (const target of source.matchAll(/rox\s*\.open\(\{\s*app:\s*"([a-z-]+)"/g)) assert.ok(p.open.apps.includes(target[1]), `${entry.id} opens ${target[1]}`);
  }
});

/** register.tsx's step-decision block, run as plain JavaScript. */
function registerDecisions() {
  const { source } = apps.find((a) => a.id === "register");
  const block = /\/\/ step-decision:begin([\s\S]*?)\/\/ step-decision:end/.exec(source);
  assert.ok(block, "register.tsx keeps its step-decision block");
  return new Function(`${block[1]}; return { stepAfterSignIn, accountState };`)();
}

test("register: a signed-in, set-up computer lands on Signed in, never on the new-taxpayer form", () => {
  const { stepAfterSignIn } = registerDecisions();
  for (const mode of ["solo", "helpdesk"]) {
    assert.equal(stepAfterSignIn(mode, true, 3), "signed-in");
    assert.equal(stepAfterSignIn(mode, true, 1), "signed-in");
    assert.equal(stepAfterSignIn(mode, true, 0), "taxpayer", "no taxpayer yet: the form");
    assert.equal(stepAfterSignIn(mode, false, 3), "account");
  }
  assert.equal(stepAfterSignIn(null, true, 0), "setup", "first launch");
  assert.equal(stepAfterSignIn(null, true, 2), "setup");
  assert.equal(stepAfterSignIn(null, false, 0), "account");
});

test("register: the account page names the sign-in state plainly", () => {
  const { accountState } = registerDecisions();
  const status = { server_url: "http://127.0.0.1:3008", offline: false, detail: "Not signed in." };
  assert.deepEqual(accountState(status, null), { kind: "never", host: "127.0.0.1:3008" });
  assert.equal(accountState(status, "could not reach buwiz-server at http://127.0.0.1:3008 to refresh sign-in: error sending request").kind, "offline");
  assert.equal(accountState({ ...status, offline: true }, null).kind, "offline");
  assert.equal(accountState(status, "Sign in again: Buwiz ended this computer's sign-in").kind, "expired");
  assert.equal(accountState(status, "The code expired before it was approved.").kind, "expired");
  assert.equal(accountState(status, "something else").kind, "error");
});
