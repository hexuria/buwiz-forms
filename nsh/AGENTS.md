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
Specifier `ebirforms/native` (crate `nsh-app-ebirforms`, `src/lib.rs`). Today: `Bridge.ping(): string` (`"pong"`), used by `screens/base/ThemePage.tsx`, tested in
`ThemePage.test.tsx` and `src/lib.rs`. Types are generated into `.nsh/types/ebirforms/native.d.ts` by `nsh check` from the Rust signatures and doc comments.
- Rust signature decides the call: plain `fn` = synchronous, runs in the TSX call, `Result` Err becomes a thrown Error; `async fn` = Promise; `cx: &mut ModuleCx` = op.
  `#[module] struct` + `#[module] impl`, register in `#[native_module]` with `registry.module::<T>()`. Doc-comment every export (`///` on the struct, never on the impl block).
- Rust state is invisible to the reactive graph: mirror what a screen shows in a signal and update it from the return value.
- `nsh-base` is NOT a dependency now (unused-dependency warning); add it to Cargo.toml only when Rust code uses it. All `nsh-*` deps are PATH deps on the one checkout.

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
9. The `ThemePage` is a throwaway preview: B3's frame replaces `App.tsx`'s root; keep `shots/`, `theme/` and the Rust bridge.
