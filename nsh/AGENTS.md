# nsh/ - the NSH (TSX) app

eBIRForms on Native Shell (NSH 0.4.0, the hexuria/nsh fork at `f373bb3dc`), mixed mode: TSX screens over a Rust crate.
Id `dev.goldcoders.bir.nsh`, version 0.2.0, own cargo workspace (empty `[workspace]` in Cargo.toml). The old app (`app/`,
`crates/bir-shell`) is untouched. Contract and Hard rules: `docs/plans/nsh-tsx-migration.md`. Read this file fully first.

## Commands (repo root; every recipe puts the CLI on PATH and sets CARGO_TARGET_DIR)
| Recipe | Does |
|---|---|
| `just nsh-check` | cargo check of the crate, then tsc over every .ts/.tsx (the only type check). Success is silent: add `-v`. |
| `just nsh-build` | Bundle + bytecode into `nsh/.nsh/out`. Sees what tsc cannot (text in a View, unknown class, missing asset). Every warning is a finding. |
| `just nsh-test [file] [--filter x]` | `cargo test`, then all `*.test.ts(x)`, headless. Dev env is set, no BIR_DATABASE_PATH. |
| `just nsh-run [db]` | macOS window with hot TSX edits (Ctrl-C ends it). DB from the arg or BIR_DATABASE_PATH. |
| `just nsh-shots <name>` | `nsh/shots/<name>.test.tsx` in real windows; PNGs into `~/BuwizNsh/evidence/<name>/`. |
Gate before you report: `just nsh-check && just nsh-build && just nsh-test`, all green, no unexplained warning.
- CLI: `~/BuwizNsh/tools/bin/nsh` (`nsh -V` = 0.4.0 f373bb3). Missing? `cargo install --locked --path
  /Volumes/goldcoders/features/nsh-tsx/nsh/crates/nsh-cli --root ~/BuwizNsh/tools` (10 min cold). Never use `~/.nsh` (nsh 0.1.1).
- NSH source is read-only at `/Volumes/goldcoders/features/nsh-tsx/nsh`; never edit it, never patch NSH to fit the app.
- Dev env for run/shots/tests: `BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1`;
  real data only from a copy: `cp ~/BuwizNsh/data/bir_data.db ~/BuwizNsh/agents/<track>.db`. Recipes refuse the owner's data folders.
- Raw form: `export PATH="$HOME/BuwizNsh/tools/bin:$PATH" CARGO_TARGET_DIR="$HOME/BuwizNsh/target-nsh"; cd nsh; nsh test`.
- Cold times on this machine: CLI install 10m16s; first `nsh check` 90s; first headless runner 57s; first macOS host 63s.
  Warm: check 1-3s, test 18s, a shots run about 1 min (each real window ~3.5s).

## Folder map (what belongs where)
- `nsh.json` name/id/version/window (1440x900, min 420x480)/appearance system/theme/tailwind (type scale, breakpoints). `App.tsx` entry (`mount`).
- `src/lib.rs` Rust: `#[native_module("ebirforms/native")]`. `bir/` typed wrapper over the door (screens import only from here).
- `theme/` `theme.json` (shadcn cssVars, light+dark), `tokens.ts` (TYPE SPACE RADIUS SIZE BREAK), `fonts.ts` + `fonts/` (Inter, OFL), `theme.test.ts`.
- `ui/` the kit: each `nsh/base` control wrapped once, styled from tokens. `shell/` frame (title bar, sidebar, panel, toasts, tab bar).
- `screens/<name>/` one folder per screen with its `*.test.tsx`. `lib/` pure helpers. `assets/icons/` SVGs. `shots/` screenshot tests.
- `justfile` here only forwards `just check|build|test|run|shots` to the root `nsh-*` recipes. `README.md` points at this file.
- Generated, git-ignored, never edit: `.nsh/` (types, runner sources, `out/`), `target/`. `Cargo.lock` is committed.

## The reactive rule
A component function runs ONCE. Only JSX expressions and closures that read a signal run again.
- Read props and signals inside JSX or a closure: `<Text>{props.title}</Text>`, `const label = () => props.a + count()`.
- Never destructure props (`function Row({title})`, `const {a} = props`) and never copy a read into a body `const`: it goes stale and nothing reports it.
- Write signals with `.set(v)` or `.update(fn)` and always a NEW array/object. `items().push(x)` does nothing.
- `<Each of={list()} key={(x) => x.id}>{(x) => ...x().field}</Each>`: row gets accessors (`x()`). `If` and `Each` are globals. No `.map`, no `cond && <A/>` for stateful parts.
- Long or growing lists: `List` (virtualized, recycled): no signals, no `const t = row().title` in a row; per-item state lives in data keyed by id.
- Import the JSX type: `import type { JSX } from "nsh"` (tsc fails with "Cannot find namespace 'JSX'" otherwise).

## Non-negotiables (NSH skill)
Never invent an API: look it up in `.nsh/types/nsh/*.d.ts` (props, events) or the docs below, and cite it. One owner per state (a screen's draft is
its signal; shared state is a module store with private signals). Text only inside `<Text>` (a bare string in a View is a build error; a string
child of a Button or a number in a View shows NOTHING and no tool reports it). Use Button/Dialog/Sheet/Menu/Select/Switch/Input from `nsh/base`,
never a View with onPress. `nsh/base` parts draw nothing until styled (a Switch with no trackClass/thumbClass is invisible and its test passes):
look at it. A decision the system needs at once is a prop (`disabled`, `popOnBack`), never a callback. Handlers get ONE object (`onChange={({value}) =>`).
No DOM, no Node, no npm UI: imports are `nsh`, `nsh/base`, `nsh/desktop`, `nsh/mobile`, `nsh/test`, relative files, `ebirforms/native`, and
buffer console crypto fs/promises net os path process url websocket zlib. Screens use `nsh/ui`/`nsh/shell` and `nsh/bir`, never raw base or the raw module.

## Tokens and the font (no raw colors in screens)
- Colors: classes naming theme tokens (`bg-card text-muted-foreground border-border`) or `token("name")` (typed, reactive) where no class fits (canvas).
  Never `#hex`, `rgb()`, `bg-[#..]`, palette classes (`bg-gray-100`). A missing role goes into `theme/theme.json` first, light AND dark.
- Old name -> class: bg=`background`, surface=`card`, raised=`raised` (=popover), hover=`hover` (=muted/secondary), selected=`selected` (=accent),
  border=`border`, borderStrong=`border-strong` (=input), fg=`foreground`, fg2=`muted-foreground`, fg3=`subtle-foreground`, primary/primaryFg=`primary`/
  `primary-foreground`, accent (blue)=`accent-blue` (=ring) NOT `accent` (shadcn's `accent` is the selected-row fill), success/warning/danger and
  `*-soft` tints, `destructive`=danger, scrim, knob, caption-close(-foreground). Opacity works: `bg-accent-blue/20`.
- fg3 fixed for 4.5:1 on background, card, raised, hover, selected: light `#64646d` (was #71717a, 4.40 on the page), dark `#91919b` (was #8b8b95, 4.29 on selected).
  `theme/theme.test.ts` guards it; change colors only through it.
- Type: `text-page text-title text-emph text-body text-meta text-cap` (20/16/14/13/12/11 with line heights) + `font-normal|medium|semibold|bold`
  (Inter 400/500/600/700). `TYPE.<role>.cls` in `theme/tokens.ts` gives the combined class for page/title/emph/body/control/meta/metaStrong/cap/num.
- Space: Tailwind's 4 pt scale equals SPACE (`p-2`=8 ... `p-8`=32). Radius: `rounded-(--radius-ctl|row|card|dialog)` (6/8/12/14), `rounded-full`.
  Controls are `h-8` (32). `SIZE` (sidebar 248, panel 320, contentMax 880 ...) has no classes: use it in `style={{ width: SIZE.sidebar }}`.
- Breakpoints (min-width variants): `wide:` 600, `content:` 640, `partnav:` 760, `docked:` 860, same numbers as `BREAK`. Narrow = under 600; default styles are the
  narrow layout, `wide:` adds the desktop one. Branch in code with `windowSize().width >= BREAK.phone`.
- Font: Inter is registered by `import "./theme/fonts"` in App.tsx (side effect; do not import the .woff2 files elsewhere) and is the theme's `font-sans`, so
  every Text uses it. Do not set fontFamily. Tabular figures: `class="tabular-nums"` (or `style={{ fontVariant: ["tabular-nums"] }}`); TINs, amounts, dates, counters.
  Proved in the real-window shot (`base-*-end.png`, "Tabular figures": 1,111.11 / 0,000.00 / 8,888.88 align only with the class). Right-align amounts: `text-right`.
- A static unknown class warns in `nsh build`; a class built in a template literal is only logged once at run time (grep the `-v` test output for "unknown").

## The Rust bridge
Specifier `ebirforms/native` (crate `nsh-app-ebirforms`, `src/lib.rs`). Today: `Door` (next section) and `Bridge.ping(): string` (`"pong"`), used by `screens/base/ThemePage.tsx`, tested in
`ThemePage.test.tsx` and `src/lib.rs`. Types are generated into `.nsh/types/ebirforms/native.d.ts` by `nsh check` from the Rust signatures and doc comments.
- Rust signature decides the call: plain `fn` = synchronous, runs in the TSX call, `Result` Err becomes a thrown Error; `async fn` = Promise; `cx: &mut ModuleCx` = op.
  `#[module] struct` + `#[module] impl`, register in `#[native_module]` with `registry.module::<T>()`. Doc-comment every export (`///` on the struct, never on the impl block).
- Rust state is invisible to the reactive graph: mirror what a screen shows in a signal and update it from the return value.
- `nsh-base` is NOT a dependency now (unused-dependency warning); add it to Cargo.toml only when Rust code uses it. All `nsh-*` deps are PATH deps on the one checkout.

## The door (`bir/`, `src/door.rs`): how a screen reaches the Rust brain
Import ONLY from `bir` (relative, e.g. `import { bir, type Taxpayer } from "../../bir"`; a directory import works). Never import `Door` from `ebirforms/native` in a screen.
- `bir.<host_function>(...)`: 116 typed wrappers, named exactly as the Rust host functions and the old app's `bir.<name>(` (snake_case). Sync ones return the value, async ones a Promise
  (`screen_form_open`, `screen_form_validate`, `import_archive`, `export_profile`, `cloud_account_*`, inbox ones). A host error is a thrown Error whose `message` is the host's sentence, unchanged
  (the old `errorText` regex is not needed). Every type (`Taxpayer`, `ScreenFormOpened`, ...) is exported from `bir` too. `doorStatus()` = `{ sending: "dry_run" | "mock" | "refused", databasePath, testEnvironment }`.
- 18 host functions the old app never calls are NOT wrapped (`NOT_WRAPPED` in `bir/functions.ts`). Need one? Add its name to `bir/extra-functions.txt` and run the script below. Generated files (never edit): `bir/types.d.ts` (from the Rust declarations; a Rust test fails while it is stale) and `bir/functions.ts`.
  Regenerate with `bash nsh/bir/regenerate.sh` after bir-app-api's declarations change.
- Tests: `import { createDemoTaxpayer, DEMO } from "bir/testing"` (relative) gives Juana Santos (`DEMO.tin` = "00012345300000" as host functions take it). In a test environment (`EBIR_TEST_ENV=1`, which every recipe sets) the
  door makes a fresh scratch database in the temp folder per engine thread: each `open()` and each test FILE starts EMPTY, tests inside one file share it. `useDatabase(path)` names a file instead (before the first call; guarded).
  Real data: `BIR_DATABASE_PATH=~/BuwizNsh/agents/<track>.db` (raw `nsh test` / `nsh run`; `just nsh-test` unsets it). `bir/DoorProbe.tsx` + `.test.tsx` is the pattern for a mounted component calling the door.
- The door boots itself on the first call of an engine thread (old host's order: hooks, send guard, database guard, `prepare_launch`, database + submission worker, updater hook). Send guard: `BIR_SFTP_DRY_RUN=1` forced,
  `BIR_SFTP_LIVE` removed, worker refused otherwise; the only exception is the mock BIR server on a listening loopback `BIR_SFTP_HOST`. Database guard: no path inside the owner's Group Container, `~/BuwizNsh/data`,
  `~/BuwizNsh/migration` (symlinks and `..` resolved; also by device+inode, so `/System/Volumes/Data/Users/...` firmlinks, other cases and hard links are caught; the home folder is the account's own from getpwuid, not `$HOME`), checked before every call. Debug builds always set `BUWIZ_NO_KEYCHAIN=1`; release leaves it to the caller. `EBIR_TEST_ENV` is read as bir-core reads it (`1|true|yes|on`, `bir_core::db::test_environment`). `shutdown()` (from `bir`) is the quit path (stops the worker, checkpoints): call it once on quit and use the door no more (a retention thread keeps the database and its owner lock until exit, as in the old host).
- Hooks (`src/hooks.rs`): file panels are NSH's own dialogs (real). Not available yet, each answers honestly (wave 4): app menu and chrome (D4), updater (`update_status` says so; D4), print preview window (D3: the file is
  written, no window, `print_preview_has_live` false), OS sign-in prompt (`os_authenticate` false; D2), agent send (`Err`; D6).

## Tests
Files: `*.test.tsx` / `*.test.ts` anywhere (hidden dirs skipped). Only that suffix counts: `nsh test x.tsx` on any other name runs it as an app entry from stdin.
```tsx
import { expect, open, test } from "nsh/test";
test("phone", async () => {                       // default viewport 390x844, platform "ios", light
  const app = await open("../../App.tsx");        // path relative to THIS file
  await app.press("role:button[Save]");           // also text:Add, label:Close, id:testID, "text:/^Buy/i", "id:x >> role:button"
  expect(app.texts()).toContain("Saved");
});
test("desktop", async () => {
  const app = await open("../../App.tsx", { width: 1440, height: 900, platform: "macos", colorScheme: "dark" });
  await app.setViewport(500, 700);                // resize live; app.setColorScheme("light") too
});
```
- Every screen needs one phone test (default) and one desktop test (width>=1440, platform "macos"). `app.waitFor("text", {state:"gone"})`, `app.type(sel, s)`,
  `app.value(sel)`, `app.enabled(sel)`, `app.back()`, `app.onDialog(d => "Remove")`, `open(.., { storage, network: {"GET url": {body}}, clock: {} })`.
- A press on a text finds the button around it. Selectors must match exactly one view for actions. Test with the demo taxpayer (Juana Santos, TIN 000-123-453-00000) only.
- Headless proves behavior, not looks. Pure logic tests (no `open`) are fine (`theme/theme.test.ts` imports JSON). In test files `process.env` shows only COLORTERM/LANG/LC_ALL/TERM and `process.cwd` does not exist.
- `nsh test` hardens the runtime (no eval); `--filter "name"`, `--trace` (mount batches per step), `--junit file.xml`. Default test timeout 30 s.

## Real-window screenshots (light and dark, wide and narrow)
Write `shots/<name>.test.tsx` like `shots/base.test.tsx`: loop `LOOKS` from `shots/shoot.ts` (wide-light/dark 1440x900, narrow-light/dark 390x844, platform macos), `open` each,
drive the screen, `await shoot(app, "<name>-<look>-<step>")`, scroll with `app.scroll("id:page-scroll", 5000)` (give the Scroll a testID). Then `just nsh-shots <name>`.
- It runs `nsh test <file> --target macos` from `~/BuwizNsh/evidence/<name>/` because PNGs are written relative to the current directory. Never save PNGs in the worktree.
- Windows open in the background (no focus steal), one per test, on a secondary display if there is one. Pictures are Retina: 2x pixels (1440x900 -> 2880x1800).
- `app.screenshot` rejects headless ("a screenshot needs a real platform"); `shoot()` swallows exactly that, so shots tests also pass in `just nsh-test`.
- Look at the PNG (Read tool shows images). A tool that passes does not prove it looks right. `nsh run` window size is not settable for this app (the flag had no effect; 900 pt wide seen).
- nsh.json `window` (defaultSize/minSize) is for packaged apps; the dev host and tests size windows from `open()`.

## Docs on disk (NSH checkout: /Volumes/goldcoders/features/nsh-tsx/nsh)
Skill `skills/nsh/SKILL.md`, `skills/nsh/references/{tsx,mixed,tooling,coding-guides,tsx-recipe}.md`, design `skills/nsh-design-guides/references/design-guides.md`.
Site docs `website/docs/`: project-structure, testing-and-trace (every test step), fonts, tailwind (theme tokens), layout, lists, forms-and-keyboard, navigation, text, styling,
macos, cli. API: `website/modules/{nsh,nsh-base,nsh-desktop,style-reference,tailwind-reference}.md`. Examples `examples/{tasks,rust-exports,gallery}`. Exact types: `nsh/.nsh/types/nsh/`.

## Pitfalls hit so far
1. This machine's `cargo` is huacnlee/cargo: it IGNORES CARGO_TARGET_DIR and prints "ignoring the configured target directory" on every command (benign). Real builds live in
   `~/.cargo/targets/nsh-app-ebirforms-*`, host binaries in `nsh/.nsh/rust/target` (about 380 MB, ignored); `~/BuwizNsh/target-nsh` stays unused. Other benign noise: "patch nsh-bundler was not used" (generated .nsh/rust) and an `ld: __eh_frame section too large` linker warning on the macOS host.
2. `nsh init` derived the specifier from the folder name (`nsh/native`); it was renamed by hand to `ebirforms/native` (lib.rs attribute + package name). Never rename again: it breaks every import.
3. Cargo.toml points at the NSH checkout by ABSOLUTE path; a clone elsewhere (CI) needs that path adapted. Parked for the owner.
4. A `.tsx` file passed to `nsh test` without the `.test.` suffix is an app entry: it creates a stray `.nsh/` beside it and looks for `nsh-macos-host` in tools/bin. Delete the stray folder.
5. The type-check success line is hidden unless `-v`; `nsh check` still exits 1 on errors.
6. Widths, heights and platform in `open()` only: iOS/Android targets reject them (`ERR_NSH_TEST_UNSUPPORTED`); headless is iOS-like unless you pass `platform`.
7. Unused imports are dropped by TS; a font or asset imported for its side effect must be a bare `import "./x.woff2"`.
8. Do not run a long `nsh run` inside a script you await: it never ends. Start it detached and stop it with SIGINT to the `nsh run` pid (it closes the window).
9. The `ThemePage` is a throwaway preview: `App.tsx` is the frame now (next section); the old switch lives on as `BasePreview.tsx` and as the dev pages `kit`/`theme`.
10. Date code takes the clock as a parameter (`daysUntil(iso, now)`, `periodOptions(..., now)`, `loadDashboard(store, at)`). Tests build local dates with `new Date(y, m, d)` and SQLite timestamps at midday UTC, so they pass in every time zone (the old code reads those timestamps as UTC and shows the local day).
11. A sync host call blocks the engine: the first `screen_forms_index()` builds the form library (about 1 s in a debug build). `app.press` can return before a slow press handler ends: `await app.waitFor("text:...")`, never `expect(app.texts())` right after.
12. Host functions take JSON: `undefined` arrives as `null` (so `get_setting(undefined)` says "got null"), amounts and ids are numbers, the draft is an opaque `object` you pass back unchanged from `screen_form_open`/`screen_form_fill`.
13. The database slot is per engine thread, and one database file has one owner lock per process: two engines on the SAME `BIR_DATABASE_PATH` in one test run refuse each other. Leave it unset and let each test get a scratch file.
14. UI kit (`ui/`): screens import from `ui/index.ts`, never nsh/base. Gallery and states: `screens/base/KitPage.tsx` (the dev page `kit`: palette "UI kit", or the "Dev" door on an empty database; `ui/harness.ts` presses it). `TOUCH` (`!platform.desktop`) picks 44-pt targets.
15. nsh/base's Select/Combobox list filters by ONE run of text in the title (crates/nsh-base/src/listbox.rs), even with a controlled `query`: "east mak" matches, "050 makati" does not.
16. A Select's/Menu's/Combobox's `testID` names the base view, not what a press opens: give the trigger element its own testID (`kit-dropdown-trigger`).
17. State variants: `hover:` and `focus:` beat `selected:`; keep a selected fill with `selected:hover:bg-x selected:focus:bg-x`. A Calendar/DatePicker without `renderItem` shows weekdays as 0-6.
18. Headless: `keyboard.press("left")` does not move an input's caret, Escape does not close a Dialog (use `app.back()`): test mid-text edits on the pure mask (`ui/mask.ts`).
19. `app.scroll` throws past the end ("at X of Y"); real-window tests over 30 s need `test(name, fn, 120_000)`. `just nsh-test x` paths are relative to nsh/; a `--filter` with spaces is split by just.
20. Never spread props into a kit component (`{...props}`): pass each prop (`label={props.label}`) or the props object itself, so reads stay live.
14. A real-window run (`--target macos`, `just nsh-shots`) serves assets (fonts, icons) only from the folder of the entry `open()` names, down: an entry in a subfolder gets HTTP 404 for `theme/fonts` (registerFont throws: "the app threw: asset://...woff2") and blank icons. Headless runs do not show it. Shots fixtures live at the nsh root (`FormShot.tsx`).
15. FIXED (U1): the kit's `<Toaster />` layer used to take presses across the bottom of the window, so a floating bar under it (the form pill) got none. An `Overlay` with no anchor is a fixed box that includes its padding and takes presses; a full-width layer needs `style={{ pointerEvents: "box-none" }}` (style-reference.md), which the Toaster now has: a bar under it is pressable with or without a toast, the toast's card and Undo still work (`ui/toaster.test.tsx`, phone and desktop, headless and `--target macos`). Lift toasts above a bottom bar with `<Toaster bottom={96} />` (default 20, plus the safe area). The shots of the form page can mount it again.
16. `position: sticky` is the class `sticky top-0` on a child of a `<Scroll>` (tailwind-reference.md): it sticks while its parent shows. Do not put `overflow-hidden` on that parent. `pointerEvents` is a style key (`style={{ pointerEvents: "box-none" }}`), not a prop; `onLayout` gives `e.bounds.size.width`.

## Form controls (B2: `screens/form/controls/`)
Entry `FieldControl`, `RadioGroupControl`, `ScheduleControl` (alias `ScheduleTable`), `FieldRow`, `SearchPick` from `./controls`; they report money as the plain number ("1250000.50"), a TIN as digits, a date as MM/DD/YYYY, a pick as its code, a tick as "true"/"false". Gallery `ControlsGallery.tsx` (entry `nsh/ControlsShot.tsx`), tests `controls.test.tsx`, shots `just nsh-shots controls`.
- The kit's `DateField` prints DATE_ERROR under its own box; `DateBox` clips the box to its height so the page's message line says it once. `MaskedFieldProps` has no `inputRef`: money, pct, int and TIN boxes cannot take a pill step's focus (`focusRef` works for text, email, phone). The kit's select root is taller than its trigger: wrap it in a box of the trigger's height or neighbours misalign.
- A popover that must be a sheet when narrow: control the Popover's `open` (false while narrow) and route its `onOpenChange(true)` to a `Sheet` (`screens/form/controls/Pick.tsx`). `CheckboxLabel` gives per-box testIDs and a press on a ticked one, which a real `RadioGroup` radio does not report (no untick, no testID).
- `app.scroll` stays quiet at the very end of some pages (no "at X of Y" error): a shots loop ends on a last-line check (`app.shows("text:/^last line/")`). Several agents running `nsh test` at once can print another file's results under your file name: run it again.

## The frame (B3: `shell/`, `App.tsx`)
`App.tsx` mounts `<Shell start={process.env.BIR_SHELL_START}/>` (`shell/Shell.tsx`): title bar, sidebar, main surface, right panel, palette, New return, toaster, tab bar + sheets under 600. Screens import ONLY `../../shell` (`shell/index.ts`):
- `go(view, params)`, `replace`, `back()`, `forward()`, `openReturn({ tin, code, year, period, returnKey?, mode? })`, `openProfile(section?)`, `openTaxpayer(root)`, `toast(message, icon?)`.
- `app`: read-only accessors, read inside JSX or a closure: `app.taxpayer() branch() year() yearForms() templates() view() params() recent() prefs() notifications() themePref() layout.narrow() layout.mainWidth()`; actions `app.selectBranch(code)`, `app.setPref(k, v)`, `app.bumpRecent(...)`. State has one owner (`shell/app.ts`, private signals).
- `store`: the cached reads (memo by key, one signal per key): `store.taxpayers() taxYears(tin) yearlyForms(tin, y) templates(tin) yearFacts deadlines(from, to) returns("filed") savedReturns() setting(k) ...`. Use it, not `bir.<read>`, in a screen; writes go through `store.write("fn", ...args)` (drops every key) or `store.writeFor(/^templates:/, "fn", ...args)` so what shows re-reads. `store.read(key, fn, fallback)` memoises your own; a read that throws answers the fallback and is asked again after 2 s.
- **Add a screen:** (1) `shell/routes.ts`: one line in `TABLE` (label, icon, needsTaxpayer, layout `page` 880 column in the frame's scroll | `wide` 1120 | `full` no cap | `self`: you fill the pane and scroll yourself); (2) `shell/views.tsx`: replace the Placeholder case with your component. A view is built once per move (`route.seq`), params in `app.route().params`. The sidebar, palette ("Go to") and tab bar read `routes.ts`.
- Dev: `BIR_SHELL_START="tp=<root>;branch=<code>;theme=dark;view=library;panel=profile;palette=<q>;form=2551Q/2026/3"`. Tests: `open("../ShellDemo.tsx", { host: { params: { demo, years, branch, second, theme, start, poll, show } } })` (header of `ShellDemo.tsx`; `host.params` is how a test hands arguments to a fixture; polls are off unless `poll: true`; `demo: false` = empty database).
- Kept in Storage: `redesign.recent|selection|read_notifications@<db path>`, `redesign.prefs`; theme in bir setting `theme_preference`. `app.storage()` shows them in a test.
- Not there yet (wave 4): sign-in and locks (`app.session/appLocked/cloudLocked/startSession/setSetup` are the seam; `FIRST_RUN_GATE` in app.ts), the update banner (`UpdateSlot.tsx`; `app.updateStatus()` is polled), the form page's "Next to do" palette actions.
- Pitfalls: a module-level `computed` runs when created (before a fixture's own writes), so `boot()` bumps the store and a fixture must seed before the Shell mounts. A kit `Sheet`'s `testID` names the base view, not the popup: wait on an id inside it. Two `Toaster`s show every toast twice (the kit gallery has one: the Shell leaves its own off the dev pages). `Each` reuses a row for the same key, and a testID computed from the index stays at its first value: put the index in the key. A press on a text that has two matches ("Forms" page and group) needs an id: give every frame control a testID. The app draws its own 40-pt title bar by `currentWindow().set({ titleBar: { style: "hidden", height: 40 } })` at start (not nsh.json) and pads by `titleBar().left/right` (nsh/desktop) for the traffic lights or caption buttons. The kit gallery and theme page are the dev pages `kit`/`theme` (palette, or the "Dev" door on an empty database); `BasePreview.tsx` is the old preview kept for `ThemePage.test.tsx` and `shots/base`.
21. The door changes the process environment in ONE place, `door::process()` (keychain switch + send guard), run when the native module registers (`lib.rs`), before NSH starts a thread. Never `set_var` anywhere else in `nsh/src`. A scratch or `useDatabase` file is named with `bir_core::db::set_database_path_override`, not `BIR_DATABASE_PATH` (the env var stays the caller's). Tests that need the protected folders use `guards::with_home_for_test(temp_home, ..)`; a test never reads the owner's real folders.
22. Rust gates for the plain build: `just core-plain-test` (bir-core and bir-app-api with `--no-default-features`, the four safety vars). A `cargo test` of `nsh/` fails if bir-core was built with sqlcipher (`bir_core_is_built_as_plain_sqlite_with_the_nsh_data_folder`). A database import is staged next to the live file, checked (integrity, core tables, schema not newer) and renamed over it: `bir_core::import::stage_database_zip` then `install`.
23. A controlled Input answered with the text its signal already holds (a refused key, a pasted minus) keeps the typed text in the native box: core keeps the raw text and no value op is sent (nsh-core core/mod.rs `Prop::Value`, nsh-platform-macos text_input.rs `apply_value`; headless showed "12x"). Set the signal to the raw text and then to the answer in the same handler (`text.set(raw); text.set(answer)`): both ops go out in order and the box reverts. `ui/fields.tsx` MaskedField does it; do the same in any controlled filter.
24. Input events on macOS (text_input.rs): `onKeyPress` fires BEFORE the text changes (deleting either way says "Backspace"; a paste's key is the whole text), then `onChange`, then `onSelectionChange` (after the edit). Keep the last selection and read it at the key press: it is the selection the edit started from (`reconcile(prev, next, mask, caret?, before?, typed?)` in `ui/mask.ts` uses it for Delete vs Backspace and for a digit typed beside an equal one). `app.type`/`app.paste` always act at the END of the text and `app.type` sends every key before the app's answer to the first lands, so test mid-text edits on the pure `reconcile`, and never rely on a per-key refusal inside one `app.type`.
25. Amounts are strings: never `Number()` a money text (past 15 digits it changes, "1e21" and "Infinity" break toFixed). `ui/mask.ts`: `formatMoney(text, whole?, round?)` rounds half up on the digits, `sameAmount`, `isPlainAmount`, `shownMoney`. `MoneyInput` reports plain text ("-250.5", no separators), always shows a negative as negative, and reports on leaving only when rounding changed the number. `lib/money.ts` keeps the old toFixed quirks on purpose.
