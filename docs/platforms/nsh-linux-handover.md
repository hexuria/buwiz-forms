# eBIRForms NSH (TSX) app: Linux handover

For whoever (a person or a coding agent) picks this up cold on a Linux machine. This file stands alone: everything you need to build, run and test the NSH app on Linux is here or in the repository paths it names. Paths are relative to the buwiz-forms checkout unless they start with `~`, `$HOME` or say "NSH checkout". Line numbers are at commit 11675f95; if a file has moved on, search for the symbol named next to the number.

## 1. What this is and the state today

The NSH app lives in `nsh/` of hexuria/buwiz-forms: TSX screens over one Rust crate (`nsh/src`), on Native Shell (NSH) 0.5.1, which is the hexuria/nsh fork at commit `87e0a4ea44256e59c15736c2f95b4d0e9ba91ae8` (branch `buwiz`; the pin is `NSH_PINNED_REF` in `.github/workflows/nsh.yml`). It is the desktop replacement for the older app in `app/` and `crates/bir-shell` (which you never touch). It was written on `feat/nsh-tsx` (head commit `11675f95d982b6ebd480dfbe0bb1ef000cd9d02e` when this was written), which PR #221 squash-merged into `main` on 2026-11-17: `main` holds the app now. On the owner's Mac (Apple silicon) the app is verified: type check, build, Rust tests, a full headless TSX run (1135 tests passed at commit 4e3a7085 per the PR body; the count at the head commit is not verified), real-window screenshots of every screen, all 43 forms open from the library. On Linux the NSH app crate has never been built, linked or run: it was only type-checked for `x86_64-unknown-linux-gnu` from the Mac with stand-in C tools and a fake `dbus-1.pc` (`docs/platforms/nsh.md`), and NSH's GTK 4 backend was never built with this app. What CI does on Linux: the job "bir-core and bir-app-api, plain SQLite" (`.github/workflows/nsh.yml`, `core-plain-test` on `ubuntu-24.04`) and the older workspace's checks on `ubuntu-22.04` (`.github/workflows/ci.yml`) were green on the head commit; the NSH app jobs run on macOS (type check, build, Rust tests, headless TSX tests) and on Windows (type check, build, the development host); none runs on Linux. Known unfinished on Linux, in the code today: Print and Save as PDF are stubs, and the OS sign-in prompt is a stub that answers "no". Everything else is "written, compiles, never run".

## 2. Guard rails (rules, not advice)

1. **Dry run only.** Every process you start has these set (the session block in section 3.5 sets them):

   | Variable | Value | What it does (where it is read) |
   |---|---|---|
   | `BIR_SFTP_DRY_RUN` | `1` | The transport is a dry run: a queued return is "sent" to a folder on this computer and nothing leaves it. The door also sets it to 1 itself at start unless the local mock BIR server is in use (`nsh/src/guards.rs:77`). |
   | `BIR_SFTP_LIVE` | never set | The live acknowledgement. The door removes it at start (`guards.rs:77`), but do not depend on that: never set it. |
   | `BIR_SFTP_HOST`, `BIR_SFTP_PORT` | never set | A loopback host here would select the local mock BIR server, the only way out of a dry run (`guards.rs:123`). Out of scope for this handover. |
   | `BUWIZ_NO_KEYCHAIN` | `1` | Every credential-store read in bir-core answers "not found" and every write fails, without calling the Secret Service (`crates/bir-core/src/platform/mod.rs`). Debug builds force it on (`nsh/src/door.rs:66`). |
   | `EBIR_TEST_ENV` | `1` | A dev or test run. A database nobody named becomes a scratch one under the temp folder (`door.rs:106`), the OS sign-in prompt answers "no" (`nsh/src/auth.rs:13`), the one-copy guard is off unless `BIR_SINGLE_INSTANCE=1` (`nsh/src/desktop.rs:516`), and Print.savePdf honours a given path (`nsh/src/print.rs:1033`). |
   | `BIR_AUTH_BYPASS` | `1` | With `EBIR_TEST_ENV=1`, in debug builds only, skips the Buwiz account sign-in. Both must be set; either alone does nothing (`crates/bir-app-api/src/account_gate.rs:20`). |
   | `BIR_DATABASE_PATH` | unset, or a file you name | Unset in a test environment gives each engine its own scratch database, which is what the headless tests want. Name a file only for a real-window run (section 4.6), under `~/BuwizNsh/agents/`. Never inside `~/BuwizNsh/data` or `~/BuwizNsh/migration` (the guard refuses those names), and never in a folder that holds a real eBIRForms database: the older Linux app keeps its data in `~/.taxman-ebir` (`crates/bir-core/src/platform/linux.rs`) and the guard does not know that folder (`guards.rs:144-150`). |

2. **Never send live, never sign in.** A run without `EBIR_TEST_ENV` is the shipped behaviour: it stops at the Buwiz account sign-in (PR #221 body) and would use the real Secret Service. Never do that run, never type a Buwiz or BIR credential, never open a database that is not a scratch file or the owner's copy (rule 4).
3. **No signing, no store uploads, no packages.** Do not run `nsh package`, `nsh keygen`, `nsh certify`, `nsh publish`, `nsh update`, `nsh update-manifest`, or any `just` recipe not named in this file (the root `justfile` has release recipes). Updates stay off: `nsh/nsh.json` has no `"updater"` key and must not get one. Do not install the app on the machine.
4. **Demo taxpayer only.** Use only Juana Santos, TIN 000-123-453-00000 (`nsh/bir/testing.ts` makes her). Tests, screenshots and anything you commit name only her or generated data. If the owner brings real data, the owner puts a copy of a database file at `~/BuwizNsh/agents/<track>.db` and you point `BIR_DATABASE_PATH` there; you never open the original, never copy real names, TINs or amounts into the repository, a PR, a log you attach or a report, and pictures of real data stay under `~/BuwizNsh/evidence/` and are never attached to a PR.
5. **Draft PRs, never merge.** Work on a branch off `feat/nsh-tsx`, open a draft PR against `feat/nsh-tsx`, and never merge anything, yours or anyone's. Never push to `feat/nsh-tsx` or `main` directly, never force-push.
6. **Never weaken a validator, a threshold or an existing assertion** to make something pass (`rules/agent-boundaries/SKILL.md`). A test that is wrong for this OS is fixed by making it assert what is true on this OS and saying so in the PR; it is never loosened. The database guard in `nsh/src/guards.rs` is a safety guard: never relax it.
7. **Boundaries.** The NSH checkout is read-only: never edit it, never patch NSH to fit the app. Never edit `app/` or `crates/bir-shell`. If an installed eBIRForms (any version) or any process named eBIRForms or bir is on this machine and you did not start it, do not launch, focus, script or stop it. Stop only processes you started. No OS-wide GUI automation (no xdotool, ydotool, AT-SPI scripts, screen scrapers or desktop-wide screen capture): the only automation allowed is the app's own test API (`nsh test --target linux`). Do not run `just` with no arguments or `just check`: both start with `cargo fmt --all` (`justfile` lines 24 and 157-172). Never format a whole crate; format only the lines you write, in the style around them (`nsh/` is not kept rustfmt-clean: commit 4b192967 undid a formatter's rewrite of six files).

## 3. Machine setup

### 3.1 What the machine needs

| Need | Why | Source of the fact |
|---|---|---|
| A distribution with GTK 4.14 or later: Ubuntu 24.04 is what NSH's CI and docs use (NSH's prebuilt binaries also want glibc 2.39; that does not matter for a source build) | NSH's Linux window is GTK 4; Pango draws the text | NSH checkout `website/docs/linux.md`, `installation.md`, `.github/workflows/linux.yml` |
| A display, or a virtual one (`xvfb-run`) and a session bus (`dbus-run-session`) | Real-window tests and `nsh run`. Headless tests need neither. | NSH checkout `.github/workflows/linux.yml` (`dbus-run-session -- xvfb-run -a nsh test ... --target linux`) |
| GTK 4 and WebKitGTK 6.0 development packages, GStreamer | The Linux host links them. NSH's host turns the web view on by default (`crates/nsh-linux-host/Cargo.toml`, feature `webview`), and the print preview is a web view; without WebKitGTK 6.0 NSH builds a host whose web views are placeholders. | NSH checkout `website/docs/linux.md` |
| clang and libclang | NSH's QuickJS bindings are generated with bindgen. | NSH checkout `.github/workflows/linux.yml` |
| `pkg-config`, `libdbus-1-dev` | The credential-store crate (`keyring` with `sync-secret-service` in `crates/bir-core/Cargo.toml`) links libdbus; `nsh/Cargo.lock` has `libdbus-sys`. | `docs/platforms/nsh.md` (the cross-check found this), `.github/workflows/nsh.yml` |
| A C compiler, `make`, `perl` | SQLite and the vendored OpenSSL (`openssl-src` in `nsh/Cargo.lock`) are built from source. | `scripts/setup-linux.sh` (its comment says openssl-src compiles from source and needs perl) |
| Rust 1.99.0 with `rustfmt` and `clippy` | `rust-toolchain.toml`. The NSH checkout's own `rust-toolchain.toml` says "stable" plus mobile targets, so set `RUSTUP_TOOLCHAIN=1.99.0` for everything, as `.github/workflows/nsh.yml` does. | `rust-toolchain.toml` |
| `just` 1.45.0 | The version CI installs. | `.github/workflows/nsh.yml` |
| huacnlee/cargo 0.102.0-9 (recommended, optional) | What the owner's Mac and NSH's CI use; building the app with stock cargo has not been tried. | `docs/platforms/nsh.md`, NSH checkout `.github/actions/cargo/action.yml` |
| Network to crates.io, github.com and the npm registry | The first `nsh check` downloads the TypeScript 7.0.2 compiler (about 9 MB) into `~/.cache/nsh` (or `NSH_CACHE`); offline, give `nsh check` a compiler with `--tsc` or `NSH_TSC`. One git dependency (`hexuria/gpui-agent`, public, at the rev in `nsh/Cargo.toml`) may be fetched even with the `agent` feature off: not verified. | NSH checkout `crates/nsh-cli/src/check.rs`, `website/docs/cli.md` |

No Node, bun or npm is needed for the NSH app: its type checker is a native binary the CLI fetches (`website/docs/installation.md`, "Editor setup"). The `nsh-check`, `nsh-build`, `nsh-test`, `core-plain-test` and `_nsh-db-guard` recipes carry `[unix]` (`justfile` lines 147, 938, 947, 962, 1005) and should work on Linux as they are (never run there). The `nsh-run`, `nsh-shots` and `nsh-agent-check` recipes carry `[macos]` (lines 1075, 1094, 1129): `just` does not define them on Linux; section 4 gives the raw commands.

### 3.2 Install (Ubuntu 24.04; for another distribution translate the names, using the NSH checkout's `website/docs/linux.md` for the GTK, WebKitGTK and GStreamer packages)

The names below are the union of what NSH's Linux CI installs (`.github/workflows/linux.yml`, job "check"), what this repository's CI installs for the plain-SQLite job (`nsh.yml`: the `libdbus-1-dev` and `libssl-dev` line is called "a guess" in that file) and `scripts/setup-linux.sh` for the build tools. `scripts/setup-linux.sh` itself is for the older app (GTK 3, WebKitGTK 4.1, GPUI libraries) and does not install GTK 4 or WebKitGTK 6.0, so it is not enough. Whether this exact list is complete on a fresh image is not verified; the first error of a failed build names what is missing.

```bash
sudo apt-get update
sudo apt-get install -y build-essential curl git pkg-config perl python3 \
  clang libclang-dev libgtk-4-dev libwebkitgtk-6.0-dev \
  libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev \
  gstreamer1.0-plugins-base gstreamer1.0-plugins-good \
  libdbus-1-dev libssl-dev \
  xvfb xauth at-spi2-core libxtst6
# Rust, pinned
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.99.0 --profile minimal
. "$HOME/.cargo/env"
rustup component add rustfmt clippy --toolchain 1.99.0
cargo install just --version 1.45.0 --locked
# Recommended: the cargo fork the owner's Mac and both repositories' CI use. This runs a script from the internet; read it first if that matters here.
curl -fsSL https://huacnlee.github.io/cargo/install.sh | sh -s -- --version 0.102.0-9
cargo -V     # must name huacnlee/cargo
```

`at-spi2-core` and `libxtst6` are in NSH's CI install line; whether `nsh test --target linux` needs them is not verified. `dbus-run-session` is used by NSH's CI and the package that provides it is not named in the repository: check `command -v dbus-run-session xvfb-run` and install your distribution's D-Bus package if the first is missing (not verified). The older app's setup script also installs the fonts `fonts-noto-core fonts-noto-color-emoji` (`scripts/setup-linux.sh`); the NSH app bundles Inter, so they matter only as glyph fallback (section 5).

With the cargo fork, `CARGO_TARGET_DIR` is ignored and builds land under `~/.cargo/targets/` and `nsh/.nsh/rust/target` (`nsh/AGENTS.md`, "Pitfalls hit so far", item 1). The `just` recipes set `CARGO_TARGET_DIR` anyway (they were written on a Mac with that cargo); with stock cargo it would be honoured, and nobody has tried that with nsh (not verified).

### 3.3 Checkout layout

`buwiz-forms` and `nsh` must be siblings, because `nsh/Cargo.toml` names the NSH crates as `../../nsh/crates/...`.

```bash
mkdir -p ~/w && cd ~/w
git clone https://github.com/hexuria/buwiz-forms.git
git -C buwiz-forms switch feat/nsh-tsx
git clone https://github.com/hexuria/nsh.git
git -C nsh checkout --detach 87e0a4ea44256e59c15736c2f95b4d0e9ba91ae8
```

Both repositories are public (read with `gh repo view` when this was written), so cloning needs no token. Both set `* text=auto eol=lf` in `.gitattributes`. Check the pins (the branch may have moved since this file was written: read the pin from `nsh.yml`, not from this file):

```bash
git -C ~/w/buwiz-forms rev-parse HEAD        # 11675f95d982b6ebd480dfbe0bb1ef000cd9d02e when written; if different: git log --oneline 11675f95..HEAD
git -C ~/w/nsh rev-parse HEAD                 # must equal NSH_PINNED_REF below
grep -n 'NSH_PINNED_REF:' ~/w/buwiz-forms/.github/workflows/nsh.yml
```

### 3.4 The nsh command line (built from the pinned NSH checkout)

Do not install `nsh` with the install script, Homebrew or mise: they give another version (on the owner's Mac an nsh 0.1.1 in `~/.nsh` must not be used: `nsh/AGENTS.md`). Build it from the pin (`.github/workflows/nsh.yml` does the same; the owner's Mac took 10 minutes cold, Linux time is not known):

```bash
cd ~/w/nsh
RUSTUP_TOOLCHAIN=1.99.0 cargo install --locked --path crates/nsh-cli --root "$HOME/BuwizNsh/tools"
```

The binary is `~/BuwizNsh/tools/bin/nsh`, the path every `just` recipe expects.

### 3.5 The session block (paste at the start of every shell session)

Nothing in it is permanent. It is what every `nsh-*` recipe sets, plus a place for logs.

```bash
export REPO="$HOME/w/buwiz-forms"
export RUSTUP_TOOLCHAIN=1.99.0
export PATH="$HOME/BuwizNsh/tools/bin:$HOME/.cargo/bin:$PATH"
# The four safety variables. Never remove them.
export BUWIZ_NO_KEYCHAIN=1 EBIR_TEST_ENV=1 BIR_AUTH_BYPASS=1 BIR_SFTP_DRY_RUN=1
unset BIR_SFTP_LIVE BIR_DATABASE_PATH
HANDBACK="$HOME/BuwizNsh/handback/$(date +%F)"; mkdir -p "$HANDBACK"
# Runs a command, shows its output, keeps a full copy in the handback folder, prints its exit code.
step() { local name="$1"; shift; "$@" 2>&1 | tee "$HANDBACK/$name.log"; echo "$name exit code: ${PIPESTATUS[0]}" | tee -a "$HANDBACK/$name.log"; }
# Real-window tests and shots: PNGs are written into the current folder, so run from the evidence folder, never from the repository.
# On a machine without a display this makes a virtual one and a session bus (as NSH's CI does); the 1920x1080 screen is this file's choice,
# because the shots ask for 1440x900 windows (not verified that the default screen is too small).
shots() {
  local name="$1"; shift
  local out="$HOME/BuwizNsh/evidence/$name"; mkdir -p "$out"
  ( cd "$out" && dbus-run-session -- xvfb-run -a -s '-screen 0 1920x1080x24' nsh test "$REPO/nsh/shots/$name.test.tsx" --target linux "$@" ) 2>&1 | tee "$HANDBACK/shots-$name.log"
  echo "$name exit code: ${PIPESTATUS[0]}"; echo "pictures in $out"
}
```

If the machine has a real desktop session and you want to watch the windows, drop the `dbus-run-session -- xvfb-run -a -s ...` prefix from `shots`.

### 3.6 Check the machine before building

```bash
step 00-versions bash -c 'rustc -V; cargo -V; just --version; git --version; cc --version | head -1; perl -v | head -2; clang --version | head -1
pkg-config --modversion gtk4 webkitgtk-6.0 dbus-1; command -v dbus-run-session xvfb-run
. /etc/os-release; echo "$PRETTY_NAME"; uname -a; echo "DISPLAY=$DISPLAY WAYLAND_DISPLAY=$WAYLAND_DISPLAY XDG_SESSION_TYPE=$XDG_SESSION_TYPE"'
```

Pass: `rustc` and `cargo` say 1.99.0 (cargo names huacnlee/cargo if you installed it), `just` says 1.45.0, `gtk4` is 4.14 or later, `webkitgtk-6.0` is 2.40 or later (NSH's floors: `website/docs/linux.md`), both `command -v` lines print a path. The three module names are the ones the `gtk4-sys`, `webkit6-sys` and `libdbus-sys` crates probe for.

## 4. First commands, in order

Run them from a shell with the block in 3.5 pasted. One `nsh` command at a time per checkout: two at once print each other's results (`nsh/AGENTS.md`, "Integration pass": the item that says two agents running `nsh test` at once can print each other's results); on the Mac a small lock script does this, and here you simply do not overlap runs. Cold builds are long: on the owner's Mac the first `nsh check` took 90 s, the first headless runner 57 s, the first host 63 s; on GitHub's `ubuntu-22.04` the older workspace's check job took 11 minutes and the plain-SQLite job (`ubuntu-24.04`) 13 minutes. A build in progress is not "stuck".

**4.1 Type check.** `just nsh-check` is `cd nsh && nsh check` with the CLI on PATH (`justfile` 938). It runs `cargo check --all-targets` on the crate, then the TypeScript check (`website/docs/cli.md`).

```bash
cd "$REPO" && step 01-check just nsh-check -v
```

Pass: exit code 0 and no error lines. The success line is shown only with `-v` (`nsh/AGENTS.md`, "Pitfalls hit so far", item 5); its exact text is not verified.

**4.2 Build.**

```bash
cd "$REPO" && step 02-build just nsh-build
```

Pass: exit code 0, no warning lines (every warning is a finding), and `nsh/.nsh/out/bundle.nshc`, `bundle.js` and `assets/` exist. A warning means text outside a Text view, an unknown class or a missing asset (`nsh/AGENTS.md`).

**4.3 Rust tests of the crate.**

```bash
cd "$REPO" && step 03-rust-tests cargo test --manifest-path nsh/Cargo.toml --no-fail-fast
```

Pass: every `test result` line says `ok` and 0 failed. Nobody has run these on Linux; write down each failure with its first assertion line. Known to look at: `docs/platforms/nsh.md` says one test row that assumes a case-insensitive file system is now macOS-only because on Linux it would have failed (not run there). The test `bir_core_is_built_as_plain_sqlite_with_the_nsh_data_folder` (`nsh/src/lib.rs:154`) fails if bir-core was built with sqlcipher; the gate is `cargo tree -e features -i libsqlite3-sys --manifest-path nsh/Cargo.toml`, which must show no `bundled-sqlcipher` (comment in `nsh/Cargo.toml`).

**4.4 Plain-SQLite core tests** (`core-plain-test` in `justfile` 147; CI runs it on `ubuntu-24.04` and it was green on the head commit):

```bash
cd "$REPO" && step 04-core-plain just core-plain-test
```

Pass: both `cargo test` runs it makes (`bir-core` and `bir-app-api`, both with `--no-default-features`) report 0 failed. (For orientation: `cargo test -p bir-app-api` with default features passed 666 tests on the Mac per the PR body; the plain count is not known.)

**4.5 Headless TSX tests.** Headless runs need no display and play an iOS-like platform unless a test says otherwise (the TSX side does not know which OS it runs on; the Rust side does), and each file runs on an engine of its own, each with its own scratch database under `ebirforms-door/` in the temp folder, `/tmp` unless `TMPDIR` says otherwise (`nsh/src/door.rs:106`). `just nsh-test` runs `cargo test` for the crate and then one `nsh test <file>` per test file (131 tracked files; `nsh/verify/` is git-ignored and left out; `justfile` 962-1000):

```bash
cd "$REPO" && step 05-headless just nsh-test
```

Pass: the last line reads "nsh-test: every test file passed (nsh/verify left out)". Otherwise it lists the failing files. Expected failure on Linux: `nsh/bir/door.test.ts:19` says `expect(info.os).toBe("macos")`, and `info.os` is Rust's `std::env::consts::OS` (`crates/bir-app-api/src/bir_module.rs:427`), so it reads `"linux"` here; change the test to assert the OS it runs on (still an exact comparison) and say so in the PR (work item L5).

Slow machines: headless, one JS call has 2 s before it is interrupted, a file's load 10 s, and a new app 5 s to render (`nsh/AGENTS.md`, "Slow machines"). If a file fails on a timing budget, rerun that file alone once (`just nsh-test shell/frame.test.tsx`, a path relative to `nsh/`); if it still fails, record it. Never raise a budget or add a sleep: tests wait for the condition.

**4.6 A real window, the app itself.** The NSH target name for Linux is `linux` (the default on Linux). This is the Linux form of `just nsh-run` (macOS only, `justfile` 1075). It needs a display; on a headless sandbox use the `shots` function (4.7) instead, or a virtual display you can look at (how to look at it is not covered here). Name a database file in the agents folder first, and run the same guard check the macOS recipe runs (`_nsh-db-guard`, `justfile` 1005; it exits 1 and prints a refusal if the path is inside a protected folder):

```bash
mkdir -p ~/BuwizNsh/agents
export BIR_DATABASE_PATH="$HOME/BuwizNsh/agents/linux-first.db"
cd "$REPO" && just _nsh-db-guard "$BIR_DATABASE_PATH" && (cd nsh && nsh run --target linux 2>&1 | tee "$HANDBACK/06-run.log")
unset BIR_DATABASE_PATH
```

A fresh file should show the first-run setup, where you can create the demo taxpayer (Juana Santos, TIN 000-123-453, an individual; the other fields as in `createDemoTaxpayer` in `nsh/bir/testing.ts`). Pass: a GTK window opens with the app's own 40-point title bar (`nsh/nsh.json` `window.titleBar`; the window buttons are NSH's, placed by the desktop's button layout), then the first-run setup or Home, with text in Inter, no crash and no red error overlay, and the terminal shows log lines without panics. On Linux the close button quits through the frame (`CLOSE_POLICY.linux.closeHides` is false, `nsh/shell/desktop/menu_model.ts:67`), which also ends `nsh run`; Ctrl-C in the terminal ends it too.

**4.7 Real-window tests and the shots recipes.** `just nsh-shots <name>` is `[macos]` (`justfile` 1129): it runs `nsh test nsh/shots/<name>.test.tsx --target macos` from `~/BuwizNsh/evidence/<name>/` because the PNGs are written relative to the current directory, and never into the worktree. On Linux use the `shots` function from 3.5:

```bash
shots base          # the first real-window test: four windows, asserts "Native module says: pong"
```

Pass for `base`: the four tests pass (wide and narrow, light and dark), eight PNGs appear in `~/BuwizNsh/evidence/base`, and the pictures look right: open them. Check in the `base-*-end.png` pictures that the "Tabular figures" sample (1,111.11 / 0,000.00 / 8,888.88) aligns, and that Inter, not a fallback font, draws the text. Then run the names in the work list (section 6). Do not use `just nsh-test <file> --target linux` for pictures: the recipe runs from `nsh/`, so the PNGs would land in the worktree. If the first real-window run says "host did not connect in 60 s", run it again: a freshly linked host can start slowly the first time (`justfile` comment in `nsh-agent-check`). If GTK cannot open a window without a GPU, `GSK_RENDERER=cairo` is what NSH's own CI sets for one of its Linux steps (not verified to help here), and `WEBKIT_DISABLE_DMABUF_RENDERER=1` is what its CI sets for web views in a container (not verified for this app).

### Where the recipes assume macOS, and what to use instead

| The recipes assume | Where | On Linux use |
|---|---|---|
| The CLI at `~/BuwizNsh/tools/bin/nsh`, put first on PATH | every `nsh-*` recipe (`justfile` 938-1144) | the same; 3.4 installs it there |
| `CARGO_TARGET_DIR=$HOME/BuwizNsh/target-nsh` | every `nsh-*` recipe | leave it; ignored by huacnlee/cargo, untried with stock cargo (3.2) |
| `[unix]` recipes: `nsh-check`, `nsh-build`, `nsh-test`, `core-plain-test`, `_nsh-db-guard` | `justfile` 147, 938, 947, 962, 1005 | defined on Linux; use as they are (sections 4.1 to 4.5). `_nsh-db-guard` needs `python3` |
| `[macos]` recipes: `nsh-run`, `nsh-shots`, `nsh-agent-check` | `justfile` 1075, 1094, 1129 | not defined on Linux: 4.6 and 4.7 |
| `--target macos` inside `nsh-run` and `nsh-shots` | `justfile` 1084 and 1143 | `--target linux` |
| `nsh-serial`, a lock wrapper that runs one command at a time and reaps stray `-macos-host` windows | `nsh/AGENTS.md`; the script is in `~/BuwizNsh/tools/bin` on the Mac, not in the repository | run one command at a time; before and after a real-window run list `pgrep -af nsh-linux-host` and stop only processes that appeared after you started (`flock` would also do for serial runs: not verified) |
| Retina pictures (1440 points are 2880 pixels) | `nsh/shots/pixels.ts`, `nsh/shots/f10.test.tsx` | the pixel tests derive the scale from the picture width (`r.width / 1440`) but need a 1440-wide window; the virtual screen in `shots` is big enough, and Linux screenshots are not verified to decode |
| `nsh/agent/cargo-with-agent.sh` is `sh` | only for the agent feature | works as is |
| macOS display flags | `--display`, `--foreground` in `nsh test` are macOS only (NSH checkout `website/docs/cli.md`) | omit |

## 5. Known platform differences

macOS is what the code was written and tested on. "Today" is what the code does on Linux; every row is read from the code and the NSH checkout's docs, none was run.

| Feature | What macOS does | What the code does on Linux today | Expected to need work |
|---|---|---|---|
| Print preview window | A second window with a web view on a `file://` URL of the filled sheet (`nsh/screens/print/host.tsx`); NSH's macOS web view gives the page read access to its own folder (NSH checkout `crates/nsh-platform-macos/src/native.rs:1828-1883`). | The TSX host and the Rust side that writes the sheet run on every desktop (`nsh/src/print.rs:558`). The sheet's URL comes from `url_for` (`print.rs:252`): a Unix path gives a plain `file:///...` URL (its unit test, `print.rs:1183`). The page is a WebKitGTK 6.0 page drawn inside NSH's view, so NSH dialogs and sheets cover it (NSH `web-view-and-video.md`). The sheet folder is `$TMPDIR/ebirforms-print-<pid>` with modes 0700 and 0600 (`print.rs:284-304`), and stale ones are swept at start (`print.rs:1011-1012`). Never opened. | Open it for real; check that WebKitGTK loads the sheet and the live-update script runs without a reload (the TSX test expects "page loads: 1"). Whether NSH's capability check lets a `file://` page load on Linux is not verified. |
| Save as PDF | A hidden WKWebView makes the PDF from the same HTML and bir-print merges the pages (`print.rs:717` on, `mod mac`). | Stub: `platform_pdf` answers "Print and Save as PDF are available on macOS only for now." (`print.rs:700-701`), which the preview window shows. NSH 0.5.1 has no print or PDF call in its Linux backend or docs (searched `nsh-platform-linux/src`, `nsh-platform-windows/src`, `nsh-core/src` and `website/modules` for pdf, PrintToPdf, print_to_pdf: no match). The older Linux app opened the filled HTML in the default browser for printing (`docs/platforms/linux.md:23,176`) and printed receipt PDFs with `lp`. | A decision first (section 8, question 1), then code of our own. Parked in `docs/platforms/nsh.md`. |
| Print | PDFKit's print panel for that PDF (`print.rs:706`). | The same stub (`print.rs:711-712`). | The same. |
| Tray and hide-to-tray | `Tray.set` puts the icon in the menu bar (`nsh/shell/desktop/Desktop.tsx:117`). The close button, Cmd+W and Cmd+H hide the window; Cmd+Q hides too; Quit is in the tray (`shell/desktop/README.md`, `menu_model.ts:64-68`). | The tray is a StatusNotifierItem: Plasma, Waybar and most panels show it, GNOME needs the AppIndicator extension (NSH `nsh-desktop.md`, "Tray"). `CLOSE_POLICY.linux`: the close button quits through the frame (`actions.ts:52-55`), Ctrl+Q is the quit shortcut, and the File menu's Hide (`mod+h`) still hides: with no tray host on the desktop a hidden window has no way back except the hotkey or a second launch. | Needs a real desktop session to look at; a headless virtual display has no tray host. Decide whether Hide should be offered without one. |
| Application menu | The menu bar. | `Menu.setApplicationMenu` gets File, Edit, View, Window, Help, with Settings in Edit on Linux (`Desktop.tsx:104`, `menu_model.ts:199-204`). NSH shows it as the window's menu bar only while the title bar is not hidden; this app hides it (`nsh.json`) and nothing in `shell/`, `ui/` or `screens/` calls `Menu.popup` (searched), so the menu is probably not reachable by mouse; its shortcuts still work (`website/modules/nsh-desktop.md`, "Menu"). | Confirm in a window; decide (section 8, question 2), likely a menu button in the title bar that calls `Menu.popup`. |
| Global hotkey | Default modifiers Control+Alt. | Default modifiers are Super+Shift (`crates/bir-app-api/src/hotkey_combo.rs:4`, `nsh/src/desktop.rs:944`). On Linux shortcuts go through the desktop portal (`org.freedesktop.portal.GlobalShortcuts`): the desktop may ask the user to confirm the chord or pick another, and the portal knows the app by its id, which needs a `.desktop` file (NSH `nsh-desktop.md`, "GlobalShortcut"). A headless sandbox has no portal. | Read the status in Settings on a real desktop; expect "refused" on a sandbox. Whether `nsh run` writes the `.desktop` file this app needs is not verified (NSH says it does only for apps with URL schemes). |
| Cmd/Ctrl key bindings | `mod` is Command. | `mod` is Ctrl. The app binds `mod+h`, `mod+,`, `mod+n`, `mod+k`, `mod+b`, `mod+l`, `mod+t`, `mod+shift+n`, `mod+shift+b`, `mod+[`, `mod+]` (`nsh/shell/*.tsx`, `menu_model.ts`) and `mod+left` and `mod+right` on the form page, bound "not while editing" so they do not take caret keys (`nsh/AGENTS.md`). NSH itself uses Ctrl+W (close window), F11, Ctrl+Q (quit) and Ctrl+R (reload under `nsh run`) and says Ctrl+Q and F11 "cannot be bound" (NSH `linux.md`); the app still lists `mod+q` for Quit and `mod+w` for Close Window on Linux (`menu_model.ts:67, 152`). Whether NSH's own Ctrl+Q runs the frame's quit path (flush drafts, close the database) is not verified. | Walk the list once in a real window; test Ctrl+Q with a draft open. |
| One-copy guard | A lock file `<database>.instance` and a loopback port file; a second launch asks the first to show and exits 0 (`nsh/src/desktop.rs:411`, `570`). | The same Rust: a std file lock and a loopback port; the database guard accepts the path (Unix code), so it applies. Off under `EBIR_TEST_ENV` unless `BIR_SINGLE_INSTANCE=1` (`desktop.rs:516`). Rust tests exist (`desktop.rs:1187`, `1208`); never run on Linux. | Run it for real with `BIR_SINGLE_INSTANCE=1`: the second `nsh run` on the same database prints "already running" and exits 0, the first window comes forward (on Wayland a window may not be allowed to raise itself: not verified). |
| Remembered window bounds | `nsh.json` `window.restoreBounds: true`; NSH keeps the placement under the window's id in the app's data and fits it to the displays. | The same setting, NSH's Linux backend; never run. NSH says the window manager places every window, so the position may not come back, especially on Wayland (`linux.md`, "Several windows sharing data"). | Resize, quit, relaunch: size should come back; record whether position does. |
| Updater | Off: no `"updater"` in `nsh.json`; status "idle, not configured"; nothing touches the network (`desktop.rs` `NOT_CONFIGURED`). | The same. | None. Do not configure it (section 2, rule 3). |
| Biometric or OS sign-in recovery | Touch ID or the account password through `robius-authentication` (`nsh/src/auth.rs:26-55`). | Stub: `prompt` logs "this platform has no OS sign-in prompt" and answers false (`auth.rs:56-60`); the crate is not a dependency on Linux (`nsh/Cargo.toml`). `mac_login_available` is false on Linux (`crates/bir-app-api/src/bir_module.rs:1273`), so the recovery button is hidden, but the sentence "Use your recovery key, or your Mac login." still shows (`nsh/screens/auth/logic.ts:19`). Admin recovery here is the PIN, the authenticator and the recovery key only. | Wording (question 5). Whether Linux ever gets an OS prompt is the owner's call (question 4). |
| Database location and guard | Data folder `~/Library/Application Support/dev.goldcoders.bir.nsh`. The guard finds the account's home from the password database and knows folders by device and inode (`nsh/src/guards.rs:157-181`, `343-366`). | Data folder `$XDG_DATA_HOME/dev.goldcoders.bir.nsh`, else `~/.local/share/dev.goldcoders.bir.nsh` (`crates/bir-core/src/platform/linux.rs`). The guard is the full Unix one: the account's real home from `getpwuid_r`, folders and hard links by device and inode (`guards.rs:157-181`, `343-366`). Names compare case-sensitively here (`guards.rs:369-377`). The protected names are three macOS-style folders under home (`guards.rs:144-150`); the older Linux app's `~/.taxman-ebir` is not in the list. Never run on Linux. | Run the guard's Rust tests (4.3) and `just _nsh-db-guard` on a protected path and read the refusal. Protecting `~/.taxman-ebir` changes the contract in `docs/plans/nsh-tsx-migration.md`: question 3. |
| Fonts and icons | Inter (four `.woff2` faces registered by `nsh/theme/fonts.ts`), SVG icons in `nsh/assets/icons`, drawn by Core Text. | The same files through NSH's Pango text engine and GTK. Glyphs Inter lacks come from whatever fonts fontconfig finds. The older app needed Noto Sans installed or it drew a blank frame (`scripts/setup-linux.sh`), but that was its own text engine; whether NSH needs it is not verified. | In the `base` pictures: Inter draws, tabular figures align, the peso sign and the dashes come out, icons draw. If a glyph is missing, try `fonts-noto-core` and report it. |
| File dialogs | NSH's file panels (`nsh/src/hooks.rs` `prompt_for_open_path`, `prompt_for_save_path`). | NSH's GTK file dialog, the desktop portal's where there is one (NSH checkout `crates/nsh-platform-linux/src/file_requests.rs`). The save panel starts in `~/Documents` only when that folder exists (`print.rs:975`). Never run. | A person presses Import and Export in Settings in a real window and operates the dialogs; no test API drives them. |
| Credential store | Keychain. | The `keyring` crate's `sync-secret-service` store, over D-Bus (`crates/bir-core/Cargo.toml`); a desktop needs a Secret Service provider. With `BUWIZ_NO_KEYCHAIN=1` nothing reads or writes it. | None while the switch stays on. Never turn it off here. |
| Agent control (cargo feature `agent`) | Built and tested: the data plane (loopback listener with a token, `GPUI_AGENT=1` to start) and NSH's window control plane (`nsh/Cargo.toml` line 50, `nsh/src/agent.rs:42`). | Off by default. The data plane is plain Rust and type-checks; the window control plane is macOS-only. `nsh/tests/agent.rs` and `agent_send.rs` need only `--features agent`; `window_plane.rs` is macOS only. | Run the two data-plane tests (L10). Do not turn the feature on in a run. |
| Title bar and window buttons | The app draws a 40-point bar; padding for the traffic lights comes from `titleBar().left`. | The window buttons are the desktop's, at the start or the end by its button layout; the shell pads by `titleBar().left` and `.right` (`nsh/AGENTS.md`, "The frame"; NSH `nsh-desktop.md`). | Look for anything under the buttons in the `shell` shots. |
| Real windows without a screen | Windows open in the background on a secondary display. | `xvfb-run` gives a virtual display; GTK needs a session bus (`dbus-run-session`). Screenshots are implemented in NSH's Linux backend (`crates/nsh-platform-linux/src/agent.rs`). | Section 4.7. |

## 6. The work list

One item at a time, in this order. Each item ends with its check; write the result in your report (section 7) before starting the next. Commit each item on your branch with a message that says what and why.

- **L1. The machine is ready.** Do section 3 and run 3.6. Check: `00-versions.log` shows the versions in 3.6 and both pins match.
- **L2. The nsh command line builds.** 3.4. Check: `nsh -V` reports 0.5.1.
- **L3. `just nsh-check -v` passes** (4.1). Check: exit code 0. If cargo fails, the first error and its package name go in the report.
- **L4. `just nsh-build` passes with no warnings** (4.2). Check: exit code 0, no warning line, `nsh/.nsh/out/bundle.nshc` exists.
- **L5. The Rust tests of the crate** (4.3). Check: the full list of failures, each with a one-line cause (a macOS path, a case-sensitivity assumption, a timing, other). Fix code, never an expectation. The one expectation that is about the OS is `nsh/bir/door.test.ts:19` (4.5): make it assert `"linux"` on Linux without loosening it, and say so in the PR.
- **L6. Plain-SQLite core tests** (4.4). Check: both runs report 0 failed.
- **L7. The headless TSX tests, all files** (4.5). Check: "every test file passed", or the failing files, each with a cause. Apply the timing rule in 4.5.
- **L8. The first real-window tests.** 4.7: `shots base`. Check: the four tests pass and the pictures look right (Inter, tabular figures, icons, title bar and window buttons).
- **L9. The window opens** (4.6), only where a display exists. Check: the pass description in 4.6 and the log with no panic. If the machine has no display, write "no display" and rely on L8 and the later shots.
- **L10. The agent data-plane tests.** `cargo test --manifest-path nsh/Cargo.toml --features agent --test agent --test agent_send` from `$REPO`, with the session block. It fetches `hexuria/gpui-agent` from GitHub. Check: both test binaries pass, or the failure is recorded.
- **L11. The database guard on Linux.** Run `cargo test --manifest-path nsh/Cargo.toml guards` and `just _nsh-db-guard "$HOME/BuwizNsh/data/x.db"` (it must refuse and exit 1; it does not create the file). Check: both outputs in the report. Do not try to open a protected path with the app.
- **L12. The window's chrome, by hand** (only on a real desktop session; no test API reads the tray or the menu bar). Tray icon appears (GNOME needs the AppIndicator extension), a click shows the window, the close button quits and `nsh run` ends, Ctrl+Q with a draft open leaves the draft saved after relaunch, File > Hide hides. Check: each of those written as seen or not seen. On a headless sandbox write "not testable".
- **L13. The application menu on Linux.** Answered (question 2): KDE Plasma with a Global Menu gets the menu in its panel; GNOME and every other desktop get an in-window two-row bar. Build and check it as section 9 says. Check: every line of 9.4 written as seen or not seen, with pictures, for each desktop tried.
- **L14. The hotkey, the one-copy guard and the remembered bounds** (a real desktop session for the hotkey; the other two also work under a virtual display with `nsh run`). Hotkey: Settings shows the status for each preset. One copy: run `nsh run` twice on the same named database with `BIR_SINGLE_INSTANCE=1` (the second must print that eBIRForms is already running and exit 0; stop only the processes you started). Bounds: resize, quit, relaunch. Check: three short notes.
- **L15. Print preview opens on Linux.** Run `shots print`. Check: the preview window opens, shows the sheet (not a placeholder), and an edit in the form reaches it without a reload; the Save as PDF and Print buttons answer with the stub sentence. The test's own PDF step is expected to stop at the stub; note where.
- **L16. The OS sign-in wording on Linux.** Check: the sentences that say "Mac login" and still show on Linux listed with file and line (`screens/auth/logic.ts:19`, `state.ts`, `screens/settings/Security.tsx`); confirm in a window that the recovery button is hidden.
- **L17. The shots sweep.** Run `shots <name>` for each name in `nsh/shots` except `agent`, `base`, `forms43` and `print` (`shell`, `form`, `home`, `library`, `calendar`, `filings`, `taxpayer`, `settings`, `auth`, `controls`, `controls-form`, `kit`, `desktop`, `f10`). Check: for each, pass or fail with the first failing line, and the pictures reviewed; layout defects (clipped text, overlap with the window buttons, wrong font, missing glyphs) listed with picture names. `desktop` cannot see a tray or a menu bar under a virtual display: say which of its tests that ends.
- **L18. All 43 forms open.** First headless: `just nsh-test screens/form/all43.test.ts` (one test per form; each prints a row that starts with `D8`, the test's own tag, then the form code, parts, fields and milliseconds; 44 tests in all). Then in real windows, in eight batches (`b1-` to `b8-`, six forms each, the last has one): `shots forms43 --filter b1-`, then the same with `b2-` and so on. Check: 43 rows that start with `D8`; 86 PNGs (`<code>-top.png` and `<code>-all.png` for each form) in `~/BuwizNsh/evidence/forms43`, none blank, none still saying "Opening".
- **L19. The real-window shots, the whole set again** after every fix above, with the `f10` pixel tests. Check: the report table in section 7 lists every shots name with its final result, and the evidence folders are tarred for the owner.

## 7. How to report back

**Capture, for every item:** the command exactly as typed, its exit code, and the full output. The `step` and `shots` functions in 3.5 keep each log in `~/BuwizNsh/handback/<date>/`. Add the machine facts once (`00-versions.log`: tool versions, distribution, kernel, whether a display and which session type, GTK and WebKitGTK versions). Pictures: real-window shots come from the `shots` function into `~/BuwizNsh/evidence/<name>/` (never into the repository). Look at each picture; a test that passes does not prove it looks right. Keep a `REPORT.md` in the handback folder (not in the repository) with one row per work item: id, command, exit code, result (pass, fail, parked), one sentence, log or picture names.

**Push:**

```bash
cd "$REPO"
git fetch origin
git switch -c feat/nsh-tsx-linux-<topic> origin/feat/nsh-tsx
# ...work, commit...
git push -u origin feat/nsh-tsx-linux-<topic>
gh pr create --draft --base feat/nsh-tsx --head feat/nsh-tsx-linux-<topic> --title "NSH on Linux: <topic>" --body-file "$HANDBACK/REPORT.md"
```

A branch per topic (the test expectation, the guard, the wording), each with its own draft PR against `feat/nsh-tsx` (CI runs on those: `ci.yml` accepts pull requests into `feat/**`). Never merge, never mark a PR ready, never push to `feat/nsh-tsx` or `main`. If a push is refused with a message about the `workflow` scope (it happens when a change touches `.github/workflows` and the HTTPS token lacks that scope), push over SSH instead of HTTPS. Attach pictures of the demo taxpayer to the draft PR as comments, or tar `~/BuwizNsh/evidence` and hand it to the owner; never commit PNGs (`.gitignore` ignores `/nsh/*.png`).

**When stuck:** after about 20 minutes on one obstacle that is not just a long build (`docs/plans/nsh-tsx-migration.md`, "Hard rules", item 8), park it: in `REPORT.md` write the item id, the command, the first 30 lines of the error, what you tried, and what you think is missing; then go on to the next item that does not depend on it. L2 blocks everything after it; L3 to L6 are independent of each other; L8 onward need a working host build. Do not fix a failure by weakening a test, a validator or the guard.

## 8. Open questions for the owner

1. **Print and Save as PDF on Linux.** NSH 0.5.1 has no PDF call, so this needs code of our own. Options visible in the repository: the older app's route (open the filled HTML in the default browser and print from there, `docs/platforms/linux.md:176`), our own call into WebKitGTK's print operation, or keep the stub for now. Which?
2. **The application menu on Linux.** Answered: KDE's Global Menu where there is one, else an in-window menu row above the toolbar (section 9).
3. **The database guard.** Protect the older Linux app's data folder `~/.taxman-ebir` as well? That adds to the three folders listed in the contract (`docs/plans/nsh-tsx-migration.md`, "The door").
4. **The sandbox.** Which distribution and version is it, does it have a desktop session (tray, hotkey portal, file dialogs, a Secret Service) or only a virtual display, and may packages be installed with `sudo`? Is Linux a real target for users, and on which display server (X11 or Wayland)?
5. **Wording and OS sign-in.** Leave Linux with no OS sign-in prompt (recovery by PIN, authenticator and recovery key)? The recovery sentence still says "Mac login" there (`screens/auth/logic.ts:19`); say it per platform?
6. **Ctrl+Q.** NSH treats Ctrl+Q as its own quit and says it cannot be bound. Keep the app's `mod+q` Quit item on Linux (`menu_model.ts:67`), or drop the shortcut label and rely on NSH's?
7. **CI.** Add a Linux job for `nsh check`, `nsh build`, the Rust tests and the headless TSX tests to `.github/workflows/nsh.yml` (`ubuntu-24.04` already runs the plain-SQLite job there), with `xvfb` for a first real-window run?
8. **PR #221** shows as not a draft on GitHub although its body says "Draft. Not for merge yet" (read with `gh pr view 221`). Mark it as a draft?

## 9. The application menu (branch `feat/app-menu`)

The owner approved one design for the menu on every desktop; macOS was built and run on the Mac, Linux was not run (it cannot be run from the Mac; the NSH Linux crate was compile-checked in a Debian trixie container, see 9.2). The app's part is `hexuria/buwiz-forms` PR #230 (branch `feat/app-menu`, on `main`). The Global Menu is NSH's: hexuria/nsh#14, merged into NSH's `buwiz` as `8e59996b6`. `Menu.inGlobalMenu()`, `Tray.available()` and `nsh/test`'s `open({ tray, globalMenu })` are hexuria/nsh#16 (branch `feat/menu-tray-availability`), on top of it. The app's NSH pin (`NSH_PINNED_REF` in `.github/workflows/nsh.yml`) is #16's pushed head until #16 merges, then moves to that merge commit; the sibling `nsh` checkout follows the pin as in 3.3.

### 9.1 What Linux shows

- **KDE Plasma with a Global Menu** (`XDG_CURRENT_DESKTOP` contains `KDE` and `com.canonical.AppMenu.Registrar` is on the session bus; NSH watches the name without blocking, so a widget added or removed while the app runs counts): NSH exports the application menu over D-Bus (`com.canonical.dbusmenu` at `/com/canonical/menu/nsh`) and ties the window to it: on Wayland with KWin's `org_kde_kwin_appmenu` protocol, on X11 with the registrar's `RegisterWindow`. The menu shows in the panel's Global Menu: `eBIRForms File Edit View Window Help Profiles`. The window is ONE row: our toolbar, with the window buttons (GTK's, client-side, KWin's title bar hidden) at the end the desktop's button layout gives. Phone width keeps the normal narrow header (no ☰). Code: `crates/nsh-platform-linux/src/global_menu.rs` (NSH, feature `global-menu`, on by default). The app asks NSH, never the bus: `Menu.inGlobalMenu()` (nsh/desktop, reactive) is true only once the menu is exported and the window tied to it (KWin took the Wayland appmenu, or the registrar answered `RegisterWindow`), and the one-row layout follows it. When the widget is removed, NSH un-exports the menu and the answer turns false: the window goes back to the two rows below. `NSH_GLOBAL_MENU=0` keeps the menu in the window.
- **KDE without a Global Menu, GNOME, every other desktop:** TWO rows. Row 1, 32 points: the titles (with small chevrons when wide), empty drag space, and GTK's window buttons where `gtk-decoration-layout` puts them (at the right by default; the row keeps room at the left too when the layout puts buttons there). Row 2: the toolbar, no room kept for window buttons. Tablet width (600 to 860): no chevrons, the pill without the TIN. Phone width: no menu row; the narrow header is ☰, the title, the account badge, the window buttons; ☰ lists every menu with submenus. GTK's `PopoverMenuBar` is not used.
- Keys in the two-row layout: F10 puts the keyboard on the first title, Left/Right move, Down/Enter/Space open, Esc leaves; Alt with a title's letter opens it (B eBIRForms, F, E, V, W, H, P). A lone Alt does nothing (NSH's keymap cannot bind a lone modifier). Under 600, F10 opens the ☰ list.
- Items as on Windows: File starts with "New Tax Profile…" (no shortcut) above "New Return…" (Ctrl+N); Profiles is the taxpayer switcher plus "New Tax Profile…"; the eBIRForms menu is About, Settings…, Hide, Sign Out, Quit (locked: About, Quit), with no shortcuts of its own. Close and Ctrl+Q behave as before.

### 9.2 Build

```bash
cd ~/w
git -C buwiz-forms fetch origin && git -C buwiz-forms switch feat/app-menu
git -C nsh fetch origin && git -C nsh checkout --detach "$(grep -m1 'NSH_PINNED_REF:' buwiz-forms/.github/workflows/nsh.yml | awk '{print $2}')"   # the pin: hexuria/nsh#16's head until it merges, then the buwiz merge commit
sudo apt-get install -y libwayland-dev     # wayland-client links libwayland-client (gdk4-wayland's wayland_crate feature)
(cd ~/w/nsh && cargo test -p nsh-platform-linux global_menu tray)    # the Global Menu rule and tie, the dbusmenu numbering and EventGroup
cd "$REPO" && step am-01-check just nsh-check -v && step am-02-build just nsh-build
for f in shell/desktop/menu_model.test.ts shell/chrome_layout.test.ts shell/menuRow.test.tsx shell/narrow.test.tsx; do step am-03-$(basename $f) just nsh-test $f; done
```

Rebuild the nsh command line from that NSH checkout (3.4) so its Linux host links the new crate and `nsh check` knows `Menu.inGlobalMenu()` and `Tray.available()`. The container check that was done: `cargo check -p nsh-platform-linux --features global-menu --tests` on `rust:1-trixie` with `libgtk-4-dev libgstreamer1.0-dev libgstreamer-plugins-base1.0-dev libwayland-dev`; it compiled, nothing was run.

### 9.3 Is the Global Menu there? (KDE only)

```bash
echo "$XDG_CURRENT_DESKTOP" "$XDG_SESSION_TYPE"
gdbus call --session --dest org.freedesktop.DBus --object-path /org/freedesktop/DBus --method org.freedesktop.DBus.NameHasOwner com.canonical.AppMenu.Registrar   # (true,) with a Global Menu
# While the app runs: its exported menu (replace :1.NN by the app's unique name from `busctl --user list | grep -i nsh`)
gdbus call --session --dest :1.NN --object-path /com/canonical/menu/nsh --method com.canonical.dbusmenu.GetLayout 0 -1 '[]'
```

Add the "Global Menu" widget to a panel (right-click the panel, Add Widgets) if `NameHasOwner` says `(false,)`.

### 9.4 What to check by eye (each as seen or not seen; a picture of each, per desktop)

1. KDE with a Global Menu, Wayland session: the panel shows eBIRForms, File, Edit, View, Window, Help, Profiles while the window is focused; the window has one row, window buttons at its end, no menu row. Choosing File > New Return…, View > Toggle Theme and Profiles > another taxpayer works from the panel. Repeat in an X11 session if there is one.
2. KDE with a Global Menu, narrow the window under 600: no ☰; the menu stays in the panel.
3. KDE without a Global Menu (remove the widget and log out and in, or run with `NSH_GLOBAL_MENU=0`): the two-row layout below.
4. GNOME (and KDE fallback): two rows; row 1 in the app's light and dark colours; the window buttons where the desktop's layout puts them (`gsettings get org.gnome.desktop.wm.preferences button-layout`), not over a title.
5. Drag row 1's empty space: the window moves; double-click: maximizes and restores; right-click: the window menu.
6. Each title opens its menu under it; F10, Right, Down opens File; Esc closes it and Esc again leaves the row; Alt+F opens File; Ctrl+N still starts a new return; Ctrl+Q quits as before.
7. About 700 points wide: same rows, no chevrons, the pill without the TIN. Under 600: one header row with ☰, title, badge, window buttons; ☰ lists every menu with submenus.
8. Lock the app (Ctrl+L): row 1 with eBIRForms, File, Edit, Help only.

### 9.5 Close, quit and hide (branch `feat/close-quit-hide`, stacked on `feat/app-menu`)

The owner's approved rules: Ctrl+Q quits, Ctrl+H hides to the tray with no question, ✕ and Alt+F4 ask "Hide to tray or Quit?" with "Don't ask again" (Settings › Keyboard changes the choice: Ask / Hide to tray / Quit), Super+Q is off by default but the key recorder can bind it, any quit warns while returns are still sending, the tray menu is Show / Hide / Quit with Show or Hide disabled to match the window. It needs NSH's `Tray.available()` (hexuria/nsh#14: true while a StatusNotifierWatcher is on the session bus): build `feat/close-quit-hide` of buwiz-forms against the NSH pin as in 9.2, then run `shell/desktop/menu_model.test.ts`, `shell/desktop/desktop.test.tsx`, `shell/desktop/keysRecorder.test.tsx`, `shell/desktop/app_keys.test.ts` and `shell/desktop/sending.test.ts`. Eye-check per desktop (GNOME with the AppIndicator extension, KDE; seen or not seen; a picture of each dialog):

1. ✕ (the window's own button, or the GTK header's) and Alt+F4: the box "Close eBIRForms?" with "Hide to tray keeps sending your queued returns and reminders. Quit stops everything.", the "Don't ask again. You can change this in Settings." checkbox, and Quit (red) and Hide to tray (primary). Esc closes the box and the window stays; Enter hides. File › Close Window (Ctrl+W) asks the same.
2. Hide to tray: the window goes, the tray icon (StatusNotifierItem) stays; its Show brings the window back. On GNOME without the AppIndicator extension there is no tray icon, and the app knows it: ✕ shows "This desktop shows no tray icons, so a hidden window could not be brought back. …" with Cancel and Quit (primary; Enter quits), no Hide to tray; Ctrl+H and the eBIRForms menu's Hide minimize the window instead (the dash brings it back); the sending warning has no Hide to tray. Turn the AppIndicator extension on while the app runs: the next ✕ offers Hide to tray again.
3. Tick "Don't ask again", choose Hide to tray: the next ✕ hides with no box; Settings › Keyboard › Close button (✕) says Hide to tray; Ask brings the box back. With Quit remembered, ✕ quits.
4. Ctrl+H (signed in, and on the sign-in screen): hides at once, no box, also from inside a text field (say if the field does anything else with it).
5. Ctrl+Q quits (the tray icon goes, `pgrep -f eBIRForms` finds nothing). Super+Q does nothing out of the box (the desktop may use it). Settings › Keyboard › Quit eBIRForms: record Super+Q: the eBIRForms menu shows it and Super+Q quits, unless the desktop keeps Super+Q for itself (then say so); record Ctrl+Q again.
6. Quit while a return is Queued: "N return(s) is/are still sending" with Quit anyway / Wait / Hide to tray, from Ctrl+Q, the eBIRForms menu and the tray's Quit (the window comes back with the box when it was hidden).
7. Tray menu: Show eBIRForms, Hide eBIRForms, Quit eBIRForms; Show greyed while the window is up, Hide greyed while it is hidden (on KDE and with the AppIndicator extension).
8. Remap: Quit to Ctrl+Shift+Q, then Hide to Ctrl+Q; the menu (row 1, or KDE's Global Menu) shows Hide eBIRForms Ctrl+Q and Quit eBIRForms Ctrl+Shift+Q, and the keys do that. Ctrl+K is refused under the box ("That shortcut is already Command Palette…."). Put both back.

