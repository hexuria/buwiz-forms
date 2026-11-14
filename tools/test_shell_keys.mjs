// The chrome's shortcuts: ⌘T theme, ⌘B sidebar, ⌘⇧B tax profile, ⌘⇧N
// notifications (Ctrl elsewhere; both are bound). Each chord has exactly one
// owner across the native menu (crates/bir-shell/src/app_menu.rs, read here
// as text) and the script (app/main.js bindShortcuts), so one press is one
// action, on the main window, on Settings and while a text box has the
// keyboard. The old ⌘⇧T and ⌘⌥B do nothing. Tooltips and the ⌘K list show
// the keys from one table (SHORTCUTS in app/shell/ui.js).
import assert from "node:assert/strict";
import fs from "node:fs";
import { makeApp, cx, one, arg, dispatch, ui } from "./shell_mount.mjs";

const host = (v) => JSON.parse(JSON.stringify(v));

// ---------------------------------------------------------------- the keymap

// The native side: `bind(cx, "<key>", Action)` in bind_global_keys binds
// cmd-<key> and ctrl-<key>; register_actions forwards some actions to the
// script by id.
const rust = fs.readFileSync(new URL("../crates/bir-shell/src/app_menu.rs", import.meta.url), "utf8");
const body = rust.slice(rust.indexOf("fn bind_global_keys"), rust.indexOf("pub(crate) fn app_menus"));
assert.ok(body.length > 0, "bind_global_keys is in app_menu.rs");
const forwards = new Map([...rust.matchAll(/on_action\(\|_: &(\w+), cx\| dispatch_if_unlocked\("([a-z-]+)", cx\)\)/g)].map((m) => [m[1], m[2]]));
const native = [];
for (const m of body.matchAll(/bind\(cx, "([^"]+)", (\w+)\);/g)) {
  for (const mod of ["cmd", "ctrl"]) native.push({ keystroke: `${mod}-${m[1]}`, owner: "menu", action: m[2], id: forwards.get(m[2]) || null });
}
for (const m of body.matchAll(/KeyBinding::new\("([^"]+)", (\w+), None\)/g)) {
  native.push({ keystroke: m[1], owner: "menu", action: m[2], id: forwards.get(m[2]) || null });
}
assert.ok(native.length >= 16, "the native chords were read");

// The script side, as bindShortcuts installs it.
function scriptBindings(os) {
  const app = makeApp({ os });
  const bound = [];
  app.bindShortcuts({ bind_keys: (list) => bound.push(...list) });
  return host(bound).map((b) => ({ ...b, owner: "script", id: b.action }));
}

/** Who answers a chord: GPUI skips a binding whose context excludes the focused text box. */
function owners(os, keystroke, inText) {
  return [...native, ...scriptBindings(os)].filter((b) => b.keystroke === keystroke && !(inText && b.context === "!Input"));
}

/** One press: its owner's action reaches the app root once. Answers the script action id, or null. */
function press(app, keystroke, { inText = false } = {}) {
  const found = owners(app.isMac ? "macos" : "windows", keystroke, inText);
  assert.ok(found.length <= 1, `${keystroke} has one owner, not ${found.map((b) => b.owner).join(" and ")}`);
  if (!found.length) return null;
  const id = found[0].id;
  assert.ok(id, `${keystroke} reaches the script`);
  assert.ok(dispatch(app.paint(cx), id, cx), `the app root handles ${id}`);
  return id;
}

// Every chord either side binds has exactly one owner.
for (const os of ["macos", "windows", "linux"]) {
  const all = [...native, ...scriptBindings(os)];
  const seen = new Map();
  for (const b of all) {
    const key = `${b.keystroke}|${b.context || ""}`;
    assert.ok(!seen.has(key), `${os}: ${b.keystroke} is bound by ${seen.get(key)} and ${b.owner}`);
    seen.set(key, b.owner);
  }
}

// The table the screen shows is the keymap the menu binds.
const { SHORTCUTS: ALL_SHORTCUTS, shortcutLabel } = ui;
// The menu's chords; the script's own (the pill's ⌘→ / ⌘←) are checked below.
const SHORTCUTS = Object.fromEntries(Object.entries(ALL_SHORTCUTS).filter(([, s]) => !s.script));
assert.deepEqual(Object.keys(SHORTCUTS), ["sidebar", "theme", "profile", "notifs"]);
assert.deepEqual(Object.keys(ALL_SHORTCUTS).filter((id) => ALL_SHORTCUTS[id].script), ["nextTodo", "prevTodo"]);
// The pill's keys: the script binds `secondary-` (⌘ on macOS, Ctrl elsewhere)
// once, with no context, so they also answer inside a text box; the menu binds none.
for (const [id, keystroke, action, mac, other] of [
  ["nextTodo", "secondary-right", "next-to-do", "⌘→", "Ctrl →"],
  ["prevTodo", "secondary-left", "previous-to-do", "⌘←", "Ctrl ←"],
]) {
  for (const os of ["macos", "windows", "linux"]) {
    const found = owners(os, keystroke, true);
    assert.equal(found.length, 1, `${os}: ${keystroke} is bound once`);
    assert.equal(found[0].owner, "script");
    assert.equal(found[0].id, action);
    assert.equal(found[0].context, undefined, `${keystroke} has no context`);
  }
  assert.equal(ALL_SHORTCUTS[id].action, action);
  assert.deepEqual([shortcutLabel(id, true), shortcutLabel(id, false)], [mac, other]);
  assert.ok(!native.some((b) => /right|left/.test(b.keystroke)), "the menu binds no arrow chord");
}
for (const [id, s] of Object.entries(SHORTCUTS)) {
  for (const mod of ["cmd", "ctrl"]) {
    const found = owners("macos", `${mod}-${s.chord}`, false);
    assert.equal(found.length, 1, `${mod}-${s.chord} (${id}) is bound once`);
    assert.equal(found[0].owner, "menu", `${mod}-${s.chord} belongs to the menu, which shows it`);
    assert.equal(found[0].id, s.action, `${mod}-${s.chord} runs ${s.action}`);
  }
}
assert.deepEqual(
  host(Object.keys(SHORTCUTS).map((id) => [shortcutLabel(id, true), shortcutLabel(id, false)])),
  [["⌘B", "Ctrl B"], ["⌘T", "Ctrl T"], ["⌘⇧B", "Ctrl Shift B"], ["⌘⇧N", "Ctrl Shift N"]],
);
// Every other chord the app had stays.
for (const [keystroke, id] of [
  ["cmd-k", "open-palette"],
  ["cmd-n", "new-return"],
  ["cmd-,", "open-settings"],
  ["cmd-l", "lock-app"],
  ["cmd-[", "go-back"],
  ["cmd-]", "go-forward"],
  ["escape", "dismiss"],
]) {
  const found = owners("macos", keystroke, false);
  assert.equal(found.length, 1, `${keystroke} is still bound`);
  assert.equal(found[0].id, id, `${keystroke} still runs ${id}`);
}
assert.equal(owners("macos", "cmd-z", true).length, 0, "Undo of an app action never fires in a text box");
assert.equal(owners("macos", "cmd-z", false)[0].id, "undo-last");

// ---------------------------------------------------------------- presses

const state = (app) => JSON.stringify({ sidebar: app.sidebarOpen, panel: app.panel, theme: app.themePref });
const TARGET = {
  "toggle-theme": (app) => app.themePref,
  "toggle-sidebar": (app) => app.sidebarOpen,
  "toggle-profile": (app) => app.panel === "profile",
  "toggle-notifications": (app) => app.panel === "notifs",
};
const CHORDS = [
  ["t", "toggle-theme"],
  ["b", "toggle-sidebar"],
  ["shift-b", "toggle-profile"],
  ["shift-n", "toggle-notifications"],
];

for (const os of ["macos", "windows"]) {
  for (const mod of ["cmd", "ctrl"]) {
    for (const inText of [false, true]) {
      for (const [chord, id] of CHORDS) {
        const app = makeApp({ os, view: "form" });
        const read = TARGET[id];
        const start = read(app);
        const others = () => JSON.stringify(Object.entries(TARGET).filter(([k]) => k !== id).map(([, f]) => f(app)));
        const rest = others();
        assert.equal(press(app, `${mod}-${chord}`, { inText }), id);
        assert.notEqual(read(app), start, `${os} ${mod}-${chord}${inText ? " in a text box" : ""}: one press toggles`);
        assert.equal(others(), rest, `${mod}-${chord} touches nothing else`);
        press(app, `${mod}-${chord}`, { inText });
        if (id === "toggle-theme") {
          // The theme button's own cycle, one step a press: Light, Dark, System.
          assert.equal(app.themePref, "system", `${os} ${mod}-${chord}: two presses are two steps`);
        } else {
          assert.equal(read(app), start, `${os} ${mod}-${chord}: a second press toggles back`);
        }
      }
    }
  }
}

{
  // The two panels share the right side: one closes the other.
  const app = makeApp();
  press(app, "cmd-shift-b");
  assert.equal(app.panel, "profile");
  press(app, "cmd-shift-n");
  assert.equal(app.panel, "notifs");
  press(app, "cmd-shift-n");
  assert.equal(app.panel, null);
}

// The old chords do nothing.
for (const keystroke of ["cmd-shift-t", "ctrl-shift-t", "cmd-alt-b", "ctrl-alt-b"]) {
  for (const inText of [false, true]) {
    const app = makeApp({ os: "macos" });
    const before = state(app);
    assert.equal(press(app, keystroke, { inText }), null, `${keystroke} is not bound`);
    assert.equal(state(app), before, `${keystroke} changes nothing`);
  }
}
{
  // And nothing answers the old panel action any more.
  const app = makeApp();
  assert.equal(dispatch(app.paint(cx), "toggle-right-panel", cx), false);
}

{
  // On Settings ⌘T still works; what Settings covers does not change.
  const app = makeApp({ os: "macos", view: "settings" });
  const before = JSON.stringify({ sidebar: app.sidebarOpen, panel: app.panel });
  press(app, "cmd-t");
  assert.equal(app.themePref, "dark", "⌘T on Settings changes the theme");
  for (const keystroke of ["cmd-b", "cmd-shift-b", "cmd-shift-n"]) press(app, keystroke);
  assert.equal(JSON.stringify({ sidebar: app.sidebarOpen, panel: app.panel }), before, "⌘B, ⌘⇧B and ⌘⇧N leave Settings' hidden chrome alone");
}

// ---------------------------------------------------------------- on screen

const tip = (tree, id) => String(arg(one(tree, id), "tooltip"));
{
  const app = makeApp({ os: "macos" });
  let tree = app.paint(cx);
  assert.equal(tip(tree, "tb-sidebar"), "Hide sidebar (⌘B)");
  assert.equal(tip(tree, "tb-theme"), "Theme: light, click to change (⌘T)");
  assert.equal(tip(tree, "tb-notifications"), "Notifications (⌘⇧N)");
  assert.equal(tip(tree, "tb-panel"), "Show tax profile (⌘⇧B)");
  app.togglePanel("notifs", cx);
  tree = app.paint(cx);
  assert.equal(tip(tree, "tb-panel"), "Hide notifications (⌘⇧N)");
  app.togglePanel("notifs", cx);
  app.toggleSidebar(cx);
  tree = app.paint(cx);
  assert.equal(tip(tree, "tb-sidebar"), "Show sidebar (⌘B)");
  assert.equal(tip(tree, "tb-panel"), "Show notifications (⌘⇧N)", "the panel button reopens the last panel, with its key");
  assert.equal(tip(app.paint(cx), "tb-theme"), "Theme: light, click to change (⌘T)");
  assert.equal(tip(makeApp({ os: "macos", view: "settings" }).paint(cx), "tb-theme"), "Theme: light, click to change (⌘T)", "Settings' theme button shows its key");
}
{
  const app = makeApp({ os: "windows" });
  const tree = app.paint(cx);
  assert.equal(tip(tree, "tb-sidebar"), "Hide sidebar (Ctrl B)");
  assert.equal(tip(tree, "tb-theme"), "Theme: light, click to change (Ctrl T)");
  assert.equal(tip(tree, "tb-notifications"), "Notifications (Ctrl Shift N)");
  assert.equal(tip(tree, "tb-panel"), "Show tax profile (Ctrl Shift B)");
}

for (const [os, keys] of [
  ["macos", { "Toggle sidebar": "⌘B", "Show tax profile": "⌘⇧B", "Show notifications": "⌘⇧N", "Change theme": "⌘T" }],
  ["windows", { "Toggle sidebar": "Ctrl B", "Show tax profile": "Ctrl Shift B", "Show notifications": "Ctrl Shift N", "Change theme": "Ctrl T" }],
]) {
  const app = makeApp({ os });
  const items = app.paletteItems();
  const hints = Object.fromEntries(items.filter((x) => x.group === "Actions" && x.hint).map((x) => [String(x.label), String(x.hint)]));
  for (const [label, key] of Object.entries(keys)) assert.equal(hints[label], key, `${os}: ⌘K shows ${label} with ${key}`);
  assert.ok(!items.some((x) => /⌥|Alt/.test(String(x.hint || ""))), `${os}: no ⌘K row shows the old ⌘⌥B`);
  // Each row runs what its key runs.
  const run = (label) => items.find((x) => String(x.label) === label).run(cx);
  run("Show tax profile");
  assert.equal(app.panel, "profile");
  run("Show notifications");
  assert.equal(app.panel, "notifs");
  assert.equal(String(app.paletteItems().find((x) => x.hint === shortcutLabel("notifs", app.isMac)).label), "Hide notifications");
}

// ⌘→ / ⌘← off the form page: the handler passes the chord on (a text box's
// line end or word jump), and moves nothing.
for (const view of ["home", "calendar", "settings"]) {
  const app = makeApp({ os: "macos", view });
  for (const id of ["next-to-do", "previous-to-do"]) {
    let propagated = 0;
    assert.ok(dispatch(app.paint(cx), id, { ...cx, propagate() { propagated += 1; } }), `the app root handles ${id}`);
    assert.equal(propagated, 1, `${view}: ${id} goes on to what has the keyboard`);
  }
}
{
  // On a form page they step (the form's own tests drive the steps).
  const app = makeApp({ os: "macos", view: "form" });
  const steps = [];
  app.formUi = { pillView: () => ({}), step: (dir) => steps.push(dir) };
  app.isFresh = () => false;
  let propagated = 0;
  const pcx = { ...cx, propagate() { propagated += 1; } };
  app.stepTodo(1, pcx);
  app.stepTodo(-1, pcx);
  assert.deepEqual(steps, [1, -1], "› then ‹");
  assert.equal(propagated, 0, "the chord stops on the form page");
  // An overlay (the ⌘K list) keeps the chord for its text box.
  app.overlay = "palette";
  app.stepTodo(1, pcx);
  assert.equal(propagated, 1);
  assert.deepEqual(steps, [1, -1]);
}

console.log("test_shell_keys: ok");
