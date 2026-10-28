// BIR as a Rox Mini App Protocol v1 plugin, under the real v1 broker.
//
// Imports Rox's broker modules (`server/v1/` of the grok-shell checkout in
// ROX_BROKER_DIR) and does what Rox does: a publisher key and trust file of
// this run, BIR's manifest signed with it (`print-v1-manifest`, then the
// broker's signManifest), inspect and install with a file keystore and an
// in-memory db, then session opens and signed bridge calls from a fake Rox.
// BIR runs on a scratch database (BIR_DATABASE_PATH under a temp dir, test
// key, auth bypass of a debug build, dry-run filing) with HOME pointed at
// the temp dir, so it never touches the owner's data, Keychain or ports.
//
//   ROX_BROKER_DIR=<grok-shell checkout> BIR_MINI_APP_BIN=<target>/debug/bir-mini-app \
//     node --test crates/bir-shell/v1-tests/broker.test.mjs
//
// The test taxpayer is made up (TIN 123-456-788, "Dela Cruz Jane").
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { DatabaseSync } from "node:sqlite";
import { after, test } from "node:test";
import { fileURLToPath, pathToFileURL } from "node:url";

const BROKER = process.env.ROX_BROKER_DIR;
const BIN = process.env.BIR_MINI_APP_BIN;
const skip = (!BROKER || !BIN) && "set ROX_BROKER_DIR and BIR_MINI_APP_BIN";
const APPS_DIR = join(dirname(fileURLToPath(import.meta.url)), "..", "mini-apps");
const BIR = "dev.goldcoders.bir";
const ROOT = "123456788";
const TIN = `${ROOT}00000`;
const TICKET = /^tkt_[0-9A-HJKMNP-TV-Z]{26}$/;

const load = (path) => import(pathToFileURL(join(BROKER, path)).href);

let dir;
const stops = [];
after(async () => {
  for (const stop of stops) await stop().catch(() => {});
  if (dir && !process.env.BIR_V1_KEEP) rmSync(dir, { recursive: true, force: true });
});

/** `value` with every number made an integer: what a signed call can carry (JCS here takes safe integers only). */
const integral = (value) => JSON.parse(JSON.stringify(value, (_, v) => (typeof v === "number" ? Math.round(v) : v)));

/** Nothing in `value` names the test TIN, in any form. */
function noTin(value, what) {
  const text = JSON.stringify(value);
  assert.ok(!text.includes(ROOT) && !text.includes("123-456-788"), `${what} names the TIN: ${text.slice(0, 300)}`);
}

test("BIR v1 under the Rox broker", { skip, timeout: 120_000 }, async (t) => {
  const { generate, random128, sign } = await load("server/v1/ed.mjs");
  const { signedBytes } = await load("server/v1/jcs.mjs");
  const { keystore } = await load("server/v1/keystore.mjs");
  const { permsDigest } = await load("server/v1/manifest.mjs");
  const { createPlugins, resultData } = await load("server/v1/plugins.mjs");
  const { createSessions } = await load("server/v1/sessions.mjs");
  const { signManifest, fingerprintOf } = await load("server/v1/sign-manifest.mjs");
  const { loadTrust } = await load("server/v1/trust.mjs");
  const { v1Tools, v1AgentCall } = await load("server/v1/agent.mjs");

  dir = mkdtempSync(join(process.env.BIR_V1_SCRATCH || tmpdir(), "bir-v1-"));
  mkdirSync(join(dir, "home"), { recursive: true });

  // A publisher key of this run under the manifest's key id, trusted by a
  // trust file of this run only.
  const publisher = generate();
  const trustFile = join(dir, "trust.json");
  writeFileSync(trustFile, JSON.stringify({
    v: 1,
    publishers: { "gc-2026-01": { publisher: "dev.goldcoders", alg: "ed25519", public_key: publisher.pub } },
    approved_plugins: [{ id: BIR, publisher_key_id: "gc-2026-01" }],
  }));
  const env = {
    HOME: join(dir, "home"),
    BIR_DATABASE_PATH: join(dir, "bir.db"),
    BIR_MINI_APPS_DIR: APPS_DIR,
    EBIR_TEST_ENV: "1",
    BIR_AUTH_BYPASS: "1",
    BIR_SFTP_DRY_RUN: "1",
  };
  const unsigned = JSON.parse(execFileSync(BIN, ["print-v1-manifest"], { env: { PATH: process.env.PATH, ...env } }).toString());
  const manifestFile = join(dir, "manifest.json");
  writeFileSync(manifestFile, JSON.stringify(signManifest(unsigned, publisher.seed), null, 2));
  const spec = { command: BIN, args: ["mcp"], env: { ...env, BIR_V1_MANIFEST: manifestFile } };

  const plugins = createPlugins({
    db: new DatabaseSync(":memory:"),
    keys: keystore(`file:${join(dir, "keys")}`),
    logDir: join(dir, "logs"),
    trust: () => loadTrust(trustFile),
  });
  const sessions = createSessions(plugins);
  stops.push(() => plugins.stopAll());

  // ── install: the review, provision, verification ─────────────────────
  const inspected = await plugins.inspect(spec);
  const review = inspected.review;
  await t.test("the install review lists every app's permissions and the routines' reads", () => {
    assert.ok(review.lines.includes("BIR 2.4.3 by Goldcoders"), review.lines.join("\n"));
    const lines = (id) => review.apps.find((a) => a.id === id).lines;
    assert.match(lines("forms")[0], /^Can call \d+ BIR tools: bir_ticket_subject, /);
    assert.ok(lines("forms").includes("Can open: BIR Tax Profile"));
    assert.ok(lines("forms").includes("Can attach: Taxpayer, Return"));
    assert.ok(lines("forms").includes("Can read files: .pdf, .png, .jpg, .jpeg, .gif, .webp, .heic up to 25 MB"));
    assert.ok(lines("tax-profile").includes("Copy to clipboard"));
    assert.ok(lines("forms").includes("Show print previews (static pages, no scripts)"));
    assert.ok(!lines("tax-profile").some((l) => l.startsWith("Show print previews")));
    assert.deepEqual(lines("deadlines"), ["Can call 1 BIR tool: bir_calendar_get", "Can open: BIR Forms, BIR Calendar"]);
    assert.deepEqual(review.apps.find((a) => a.id === "deadlines").inline, { width: 480, height: 320 });
    assert.deepEqual(review.routines, [
      "Routine 'BIR filing confirmations' reads via dev.goldcoders.google-workspace: gws_search_mail, gws_read_mail",
      "Routine 'BIR payments (MyEG and banks)' reads via dev.goldcoders.google-workspace: gws_search_mail, gws_read_mail",
    ]);
    for (const app of review.apps) assert.ok(!app.lines.join(" ").includes("bir_call"));
  });
  const installed = await plugins.install({ ...spec, manifest_sha256: inspected.manifest_sha256 });
  assert.equal(installed.ok, true);
  assert.equal(plugins.runtime(BIR).state, "running", "provisioned and verified");
  const manifest = plugins.manifestOf(plugins.rowOf(BIR));
  // the plugin key as the file keystore holds it, to look for in the logs
  const seed = readFileSync(join(dir, "keys", `${BIR}.key`), "utf8").trim();

  // ── a fake Rox: session keys and signed envelopes (4.4, 8.1) ─────────
  async function open(app) {
    const ks = generate();
    const req = {
      plugin: BIR, app, open_id: random128(), ks_pub: ks.pub,
      perms_sha256: permsDigest(manifest.apps.find((a) => a.id === app).permissions), not_after: Date.now() + 3600_000,
    };
    await sessions.open(req);
    return { ...req, ks };
  }
  const envelope = (o, tool, args = {}, signer = o.ks.seed, over = {}) => {
    const env = { v: 1, typ: "rox-call", plugin: BIR, app: o.app, open_id: o.open_id, tool, args, ts: Date.now(), nonce: random128(), ...over };
    return { envelope: env, sig: sign(signer, signedBytes(env)) };
  };
  const call = async (o, tool, args) => resultData((await sessions.call(envelope(o, tool, args))).result);
  const codeOf = (p) => p.then(() => "resolved", (e) => e.code || String(e));

  // the test taxpayer, created the way the sign-in app creates one
  const register = await open("register");
  const created = await call(register, "bir_app_create_taxpayer", {
    args: [{ tin_root: ROOT, name: "Dela Cruz Jane", taxpayer_type: "Individual", head_office: { rdo_code: "040" } }],
  });
  assert.match(created.result.ticket, TICKET);

  let ticket;
  await t.test("resolve: a typed TIN becomes a token with a ticket, and no TIN comes back", async () => {
    const token = await sessions.resolve({ plugin: BIR, kind: "scope", name: "taxprofile", value: ROOT, complete: false });
    assert.equal(token.result, "token");
    assert.match(token.ticket, TICKET);
    assert.equal(token.token, `@bir:taxprofile:${token.ticket}`);
    assert.equal(token.title, "Dela Cruz Jane");
    noTin(token, "the token");
    ticket = token.ticket;
    const opened = await sessions.resolve({ plugin: BIR, kind: "command", name: "taxform", value: `${ROOT}/2551Q-2026-Q3`, complete: false });
    assert.equal(opened.result, "open");
    assert.equal(opened.app, "forms");
    assert.match(opened.ticket, TICKET);
    noTin(opened, "the open");
    const choices = await sessions.resolve({ plugin: BIR, kind: "scope", name: "taxprofile", value: "Dela", complete: true });
    assert.equal(choices.result, "choices");
    assert.equal(choices.choices[0].value, ticket);
  });

  await t.test("the ticket's title is the name, and nothing else", async () => {
    assert.deepEqual(await sessions.ticketTitle({ plugin: BIR, ticket }), { ok: true, valid: true, title: "Dela Cruz Jane" });
    assert.deepEqual(await sessions.ticketTitle({ plugin: BIR, ticket: `tkt_${"0".repeat(26)}` }), { ok: true, valid: false });
  });

  await t.test("every app opens and a signed call from its permissions succeeds", async () => {
    const forms = await open("forms");
    const subject = await call(forms, "bir_ticket_subject", { ticket });
    assert.equal(subject.result.tin, TIN, "the app learns its ticket's TIN from BIR");
    const years = await call(forms, "bir_app_list_tax_years", { args: [TIN], ticket });
    assert.ok(Array.isArray(years.result));
    const profile = await open("tax-profile");
    assert.equal((await call(profile, "bir_app_get_taxpayer", { args: [ROOT], ticket })).result.name, "Dela Cruz Jane");
    const calendar = await open("calendar");
    assert.ok((await call(calendar, "bir_app_deadline_overrides", { args: [{ year: 2026 }] })).result);
    const card = await open("deadlines");
    const dues = await call(card, "bir_calendar_get", { tin: ticket });
    noTin(dues, "the deadline card's data");
    assert.ok(Array.isArray(dues.result.events));
    assert.ok((await call(register, "bir_app_list_rdos", { args: [] })).result.length > 0);
    // the scope is the ticket: another taxpayer is out of reach
    assert.equal(await codeOf(sessions.call(envelope(forms, "bir_app_list_tax_years", { args: ["98765432100000"], ticket }))), "tool_error");
    // and an app never names a TIN to BIR's own tools
    assert.equal(await codeOf(sessions.call(envelope(card, "bir_calendar_get", { tin: ROOT }))), "tool_error");
  });

  await t.test("rox.preview: Forms' print preview comes back as an HTML document, past 1 MiB only as a preview", async () => {
    const forms = await open("forms");
    const preview = (o, args) => sessions.call(envelope(o, "bir_app_print_preview", args, o.ks.seed, { purpose: "preview" }));
    const sample = (code) => call(forms, "bir_app_screen_form_sample", { args: [TIN, code], ticket });
    const doc = (r) => r.result.content.find((c) => c.type === "resource").resource;
    // a saved return, by id: BIR reads it (its amounts are not integers, so it is never signed)
    const opened = (await call(forms, "bir_app_screen_form_open", { args: [TIN, "2551Q", 2026, 3], ticket })).result;
    assert.ok(opened.draft, "2551Q Q3 2026 opens");
    const saved = (await call(forms, "bir_app_screen_form_save", { args: ["2551Q", integral(opened.draft)], ticket })).result;
    const r = await preview(forms, { ticket, form: "2551Q", draft_id: saved.id });
    assert.equal(doc(r).mimeType, "text/html");
    assert.match(doc(r).text, /<html/i);
    assert.ok(!/<script/i.test(doc(r).text), "the page is static");
    assert.equal(r.result.structuredContent.title, "BIR Form 2551Q \u2014 Dela Cruz Jane");
    // as Forms asks for a 2551Q: by its quarter (kept by period, its draft has no id)
    const byQuarter = await preview(forms, { ticket, form: "2551Q", year: 2026, quarter: 3 });
    assert.equal(doc(byQuarter).text, doc(r).text);
    // the largest sheet is over the bridge's 1 MiB: a preview only
    const annualOpened = (await call(forms, "bir_app_screen_form_open", { args: [TIN, "1701", 2025, 0], ticket })).result;
    const annual = (await call(forms, "bir_app_screen_form_save", { args: ["1701", integral(annualOpened.draft)], ticket })).result;
    const big = { ticket, form: "1701", draft_id: annual.id };
    assert.ok(Buffer.byteLength(doc(await preview(forms, big)).text) > 1024 * 1024, "1701's sheet is over 1 MiB");
    assert.equal(await codeOf(sessions.call(envelope(forms, "bir_app_print_preview", big))), "result_too_large");
    // held to the ticket: no ticket, a forged one, or nothing to print
    assert.equal(await codeOf(preview(forms, { form: "2551Q", draft_id: saved.id })), "tool_error");
    assert.equal(await codeOf(preview(forms, { ticket: `tkt_${"0".repeat(26)}`, form: "2551Q", draft_id: saved.id })), "tool_error");
    assert.equal(await codeOf(preview(forms, { ticket, form: "2551Q" })), "tool_error");
    // a Form library sample previews from the page's draft, sent as JSON text (its amounts are not integers)
    const sampled = (await sample("2551Q")).result;
    assert.throws(() => envelope(forms, "bir_app_print_preview", { draft: sampled.draft }), /integers/, "a draft's amounts cannot be signed as numbers");
    const fromPage = await preview(forms, { ticket, form: "2551Q", draft_json: JSON.stringify(sampled.draft), edits: {} });
    assert.match(doc(fromPage).text, /<html/i);
    // another app has neither the tool nor the permission
    const profile = await open("tax-profile");
    assert.equal(await codeOf(preview(profile, { ticket, form: "2551Q", draft_id: saved.id })), "permission_denied");
  });

  await t.test("the broker's gates: undeclared, switched off, forged, replayed", async () => {
    const card = await open("deadlines");
    assert.equal(await codeOf(sessions.call(envelope(card, "bir_profiles_list", {}))), "permission_denied");
    assert.equal(await codeOf(sessions.call(envelope(card, "bir_app_list_taxpayers", {}))), "permission_denied");
    plugins.setTool(BIR, "bir_calendar_get", true);
    assert.equal(await codeOf(sessions.call(envelope(card, "bir_calendar_get", {}))), "disabled");
    plugins.setTool(BIR, "bir_calendar_get", false);
    assert.equal(await codeOf(sessions.call(envelope(card, "bir_calendar_get", {}, generate().seed))), "bad_signature");
    const once = envelope(card, "bir_calendar_get", {});
    assert.equal((await sessions.call(once)).ok, true);
    assert.equal(await codeOf(sessions.call(once)), "replayed");
  });

  await t.test("app sources load and match their signed fingerprints", async () => {
    for (const app of manifest.apps) {
      const { source, fingerprint } = await sessions.appSource({ plugin: BIR, app: app.id });
      assert.equal(fingerprint, app.fingerprint);
      assert.equal(fingerprintOf(Buffer.from(source, "utf8")), app.fingerprint);
    }
  });

  await t.test("the agent sees bir_ tools, no rox.* and no bir_call, and reads tickets not TINs", async () => {
    const names = v1Tools(plugins).map((x) => x.name);
    assert.ok(names.includes("bir_profiles_list") && names.includes("bir_app_open"));
    assert.ok(!names.some((n) => n.startsWith("rox.")) && !names.includes("bir_call"));
    // 10.1: the apps' bir_app_* tools (rox/audience ["app"]) stay off the model's surface
    const appOnly = names.filter((n) => (n.startsWith("bir_app_") && n !== "bir_app_open") || n.startsWith("bir_ticket_"));
    assert.deepEqual(appOnly, [], "app-only tools reach the model");
    const served = plugins.runtime(BIR).tools.filter((x) => !x.name.startsWith("rox."));
    const marked = served.filter((x) => x._meta?.["rox/audience"]?.[0] === "app");
    assert.equal(names.length, served.length - marked.length);
    assert.equal(names.length, 45, `the model sees ${names.length} BIR tools`);
    // a taxpayer's forms and returns, by ticket, for the model
    for (const tool of ["bir_profile_forms", "bir_returns_list"]) {
      assert.ok(names.includes(tool), `${tool} is on the model's surface`);
      const forged = await v1AgentCall(plugins, null, tool, { ticket: `tkt_${"0".repeat(26)}` });
      assert.equal(forged.is_error, true, `${tool} refuses a ticket BIR did not issue`);
      const got = await v1AgentCall(plugins, null, tool, { ticket, year: 2026 });
      assert.equal(got.is_error, false, got.content);
      assert.match(got.content, /Dela Cruz Jane/);
      noTin(got.content, tool);
    }
    console.log(`  agent surface: ${names.length} BIR tools (${marked.length} app-only kept off)`);
    // 9.8: an answer carries its inline card, which the broker accepts
    const job = {};
    const forms = await v1AgentCall(plugins, job, "bir_profile_forms", { ticket, year: 2026 });
    assert.equal(forms.is_error, false, forms.content);
    const card = job.linkTurn?.links?.[0]?.card;
    assert.ok(card, "the answer's card reached the job");
    assert.equal(card.app, "card-forms");
    assert.equal(card.placement, "inline");
    assert.equal(card.props.card, "forms");
    noTin(card, "the card");
    assert.ok(Buffer.byteLength(JSON.stringify(card.props)) <= 8192);
    const appCall = await v1AgentCall(plugins, null, "bir_app_list_taxpayers", {});
    assert.equal(appCall.is_error, true, "a model call to an app-only tool is refused");
    const listed = await v1AgentCall(plugins, null, "bir_profiles_list", {});
    assert.equal(listed.is_error, false, listed.content);
    noTin(listed.content, "bir_profiles_list for the agent");
    assert.match(listed.content, /tkt_/);
    const refused = await v1AgentCall(plugins, null, "bir_ticket_subject", { ticket });
    assert.equal(refused.is_error, true, "app-only tools refuse the agent");
  });

  await t.test("the plugin key and the TIN never reach a log, and the signed manifest is this build's", () => {
    const files = (d) => readdirSync(d, { withFileTypes: true }).flatMap((e) => (e.isDirectory() ? files(join(d, e.name)) : [join(d, e.name)]));
    const logs = files(dir).filter((f) => f.endsWith(".log"));
    assert.ok(logs.some((f) => f.endsWith(`${BIR}.log`)), "the broker logged BIR's stderr");
    for (const f of logs) {
      const text = readFileSync(f, "utf8");
      assert.ok(!text.includes(seed), `${f} holds the plugin key`);
      assert.ok(!text.includes(ROOT), `${f} names the TIN`);
      assert.ok(!text.includes("not this build's manifest"), `${f}: the signed manifest is stale`);
    }
  });

  await t.test("uninstall removes BIR's tickets", async () => {
    assert.deepEqual(await plugins.uninstall(BIR), { ok: true });
    // a fresh spawn, provisioned by hand: the old ticket no longer resolves
    const { Client } = await load("node_modules/@modelcontextprotocol/sdk/dist/esm/client/index.js");
    const { StdioClientTransport } = await load("node_modules/@modelcontextprotocol/sdk/dist/esm/client/stdio.js");
    const client = new Client({ name: "test", version: "1" });
    await client.connect(new StdioClientTransport({ ...spec, env: { PATH: process.env.PATH, ...spec.env }, stderr: "pipe" }));
    stops.push(() => client.close());
    const k = generate();
    await client.callTool({ name: "rox.provision", arguments: { protocol: "1.0.0", plugin: BIR, key: { alg: "ed25519", seed: k.seed }, pub: k.pub } });
    const r = await client.callTool({ name: "rox.ticket.resolve", arguments: { ticket } });
    assert.deepEqual(r.structuredContent, { valid: false });
    await client.close();
  });
});
