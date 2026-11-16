# eBIRForms NSH (TSX) app: Windows handover

For whoever (a person or a coding agent) picks this up cold on a Windows machine. This file stands alone: everything you need to build, run and test the NSH app on Windows is here or in the repository paths it names. Paths are relative to the buwiz-forms checkout unless they start with a drive letter or say "NSH checkout". Line numbers are at commit 11675f95; if a file has moved on, search for the symbol named next to the number.

## 1. What this is and the state today

The NSH app lives in `nsh/` of hexuria/buwiz-forms: TSX screens over one Rust crate (`nsh/src`), on Native Shell (NSH) 0.5.1, which is the hexuria/nsh fork at commit `c346e920ee4ba12f6dc5804842e60814d2c02480` (branch `buwiz`; the pin is `NSH_PINNED_REF` in `.github/workflows/nsh.yml`). It is the desktop replacement for the older app in `app/` and `crates/bir-shell` (which you never touch). The branch is `feat/nsh-tsx`, head commit `11675f95d982b6ebd480dfbe0bb1ef000cd9d02e` when this was written, tracked by PR #221 (open against `main`; its body says "Draft. Not for merge yet"; every job showed as passing in `gh pr checks 221` when this was written). On the owner's Mac (Apple silicon) the app is verified: type check, build, Rust tests, a full headless TSX run (1135 tests passed at commit 4e3a7085 per the PR body; the count at the head commit is not verified), real-window screenshots of every screen, all 43 forms open from the library. On Windows nothing has been built, linked or run: the Rust crate was only type-checked for `x86_64-pc-windows-msvc` from the Mac with stand-in C tools (`docs/platforms/nsh.md`), the NSH Win32 backend was never built with this app, and CI (`.github/workflows/nsh.yml`) builds the NSH app on macOS only. Windows CI covers only the older workspace crates (`bir-core`, `bir-app-api` and others on `windows-latest` in `.github/workflows/ci.yml`). Known unfinished on Windows, in the code today: the database guard refuses every database unless `BIR_DATABASE_PATH` names it, and Print and Save as PDF are stubs. Everything else is "written, compiles, never run".

## 2. Guard rails (rules, not advice)

1. **Dry run only.** Every process you start has these set (the session block in section 3.5 sets them):

   | Variable | Value | What it does (where it is read) |
   |---|---|---|
   | `BIR_SFTP_DRY_RUN` | `1` | The transport is a dry run: a queued return is "sent" to a folder on this computer and nothing leaves it. The door also sets it to 1 itself at start unless the local mock BIR server is in use (`nsh/src/guards.rs:77`). |
   | `BIR_SFTP_LIVE` | never set | The live acknowledgement. The door removes it at start (`guards.rs:77`), but do not depend on that: never set it. |
   | `BIR_SFTP_HOST`, `BIR_SFTP_PORT` | never set | A loopback host here would select the local mock BIR server, the only way out of a dry run (`guards.rs:123`). Out of scope for this handover. |
   | `BUWIZ_NO_KEYCHAIN` | `1` | Every credential-store read in bir-core answers "not found" and every write fails, without calling Windows (`crates/bir-core/src/platform/mod.rs`). Debug builds force it on (`nsh/src/door.rs:66`). |
   | `EBIR_TEST_ENV` | `1` | A dev or test run. A database nobody named becomes a scratch one (`door.rs:106`), the OS sign-in prompt answers "no" without asking Windows (`nsh/src/auth.rs:13`), the one-copy guard is off unless `BIR_SINGLE_INSTANCE=1` (`nsh/src/desktop.rs:516`), and Print.savePdf honours a given path (`nsh/src/print.rs:1033`). |
   | `BIR_AUTH_BYPASS` | `1` | With `EBIR_TEST_ENV=1`, in debug builds only, skips the Buwiz account sign-in. Both must be set; either alone does nothing (`crates/bir-app-api/src/account_gate.rs:20`). |
   | `BIR_DATABASE_PATH` | a file you name, or unset | The database file. On Windows it is required today (see section 5, "Database location and guard"). Never point it into `%USERPROFILE%\BuwizNsh\data` or `\BuwizNsh\migration` (the guard refuses those names) and never at any folder holding a real eBIRForms database. |

2. **Never send live, never sign in.** A run without `EBIR_TEST_ENV` is the shipped behaviour: it stops at the Buwiz account sign-in (PR #221 body) and would use the real credential store. Never do that run, never type a Buwiz or BIR credential, never open a database that is not a scratch file or the owner's copy (rule 4).
3. **No signing, no store uploads, no installers.** Do not run `nsh package`, `nsh keygen`, `nsh certify`, `nsh publish`, `nsh update`, `nsh update-manifest`, or any `just` recipe not named in this file (the root `justfile` has release recipes). Updates stay off: `nsh/nsh.json` has no `"updater"` key and must not get one. Do not install the app on the machine.
4. **Demo taxpayer only.** Use only Juana Santos, TIN 000-123-453-00000 (`nsh/bir/testing.ts` makes her). Tests, screenshots and anything you commit name only her or generated data. If the owner brings real data, the owner puts a copy of a database file at `%USERPROFILE%\BuwizNsh\agents\<track>.db` and you point `BIR_DATABASE_PATH` there; you never open the original, never copy real names, TINs or amounts into the repository, a PR, a log you attach or a report, and pictures of real data stay under `%USERPROFILE%\BuwizNsh\evidence\` and are never attached to a PR.
5. **Draft PRs, never merge.** Work on a branch off `feat/nsh-tsx`, open a draft PR against `feat/nsh-tsx`, and never merge anything, yours or anyone's. Never push to `feat/nsh-tsx` or `main` directly, never force-push.
6. **Never weaken a validator, a threshold or an existing assertion** to make something pass (`rules/agent-boundaries/SKILL.md`). A test that is wrong for this OS is fixed by making it assert what is true on this OS and saying so in the PR; it is never loosened. The database guard in `nsh/src/guards.rs` is a safety guard: on Windows the job is to make it as strict as on Unix, never to relax it.
7. **Boundaries.** The NSH checkout is read-only: never edit it, never patch NSH to fit the app. Never edit `app/` or `crates/bir-shell`. If an installed eBIRForms (any version) or any process named eBIRForms or bir is on this machine and you did not start it, do not launch, focus, script or stop it. Stop only processes you started. No OS-wide GUI automation (no AutoHotkey, UI Automation or SendInput scripts, no desktop-wide screen capture): the only automation allowed is the app's own test API (`nsh test --target windows`). Do not run `just` with no arguments or `just check`: both start with `cargo fmt --all` (`justfile` lines 24 and 157-172). Never format a whole crate; format only the lines you write, in the style around them (`nsh/` is not kept rustfmt-clean: commit 4b192967 undid a formatter's rewrite of six files).

## 3. Machine setup

### 3.1 What the machine needs

| Need | Why | Source of the fact |
|---|---|---|
| Windows 10 version 1809 or later, or Windows 11 | NSH's floor. Windows Server editions are not named there: not verified. | NSH checkout `website/docs/windows.md` |
| WebView2 Runtime | The print preview window is a web view. Without the runtime a web view draws a neutral placeholder. It ships with Windows 11 and with Edge on Windows 10; whether your EC2 image has it is not verified (check in 3.6). | NSH checkout `website/docs/windows.md`, `website/docs/web-view-and-video.md` |
| Visual Studio 2022 Build Tools: the C++ workload (`Microsoft.VisualStudio.Workload.VCTools`), `Microsoft.VisualStudio.Component.VC.Tools.x86.x64` and `Microsoft.VisualStudio.Component.Windows11SDK.22621` | Rust MSVC target and C dependencies | `scripts/setup-windows.ps1` |
| LLVM (libclang), in `C:\Program Files\LLVM` or named by `LIBCLANG_PATH` | NSH's QuickJS bindings are generated with bindgen. The installer or package name is not in the repository: not verified. | NSH checkout `website/docs/installation.md`, `.github/workflows/windows.yml` |
| Strawberry Perl and NASM, ahead of Git's own perl on PATH | bir-core builds OpenSSL from source (`openssl` with `vendored` in `crates/bir-core/Cargo.toml`, locked as `openssl-src` in `nsh/Cargo.lock`). The script says Perl and NASM are required for it; no NSH build has run on Windows to confirm. | `scripts/setup-windows.ps1` |
| Git for Windows | Cloning; cargo fetches one git dependency (`hexuria/gpui-agent` at the rev in `nsh/Cargo.toml`, public). Whether cargo needs it with the `agent` feature off is not verified. | `nsh/Cargo.toml`, `nsh/Cargo.lock` |
| PowerShell 7 (`pwsh`) | The root `justfile` sets `windows-shell` to `pwsh`; all commands below are PowerShell. | `justfile` line 5 |
| Rust 1.99.0 (MSVC host) with `rustfmt` and `clippy` | `rust-toolchain.toml`. The NSH checkout's own `rust-toolchain.toml` says "stable" plus mobile targets, so set `RUSTUP_TOOLCHAIN=1.99.0` for everything, as `.github/workflows/nsh.yml` does. | `rust-toolchain.toml` |
| `just` 1.45.0 | The version CI installs. | `.github/workflows/nsh.yml` |
| huacnlee/cargo 0.102.0-9 (recommended, optional) | What the owner's Mac and NSH's CI use; building the app with stock cargo has not been tried. | `docs/platforms/nsh.md`, NSH checkout `.github/actions/cargo/action.yml` |
| Network to crates.io, github.com and the npm registry | The first `nsh check` downloads the TypeScript 7.0.2 compiler (about 9 MB) into `%LOCALAPPDATA%\nsh` (or `NSH_CACHE`); offline, give `nsh check` a compiler with `--tsc` or `NSH_TSC`. | NSH checkout `crates/nsh-cli/src/check.rs`, `website/docs/cli.md` |

No Node, bun or npm is needed for the NSH app: its type checker is a native binary the CLI fetches (`website/docs/installation.md`, "Editor setup"). Bash is not needed either: the `nsh-*` and `core-plain-test` recipes in the `justfile` carry the `[unix]` or `[macos]` attribute (lines 147, 938, 947, 962, 1005, 1075, 1094, 1129), so `just` does not define them on Windows (not verified on Windows: `just --list` shows what exists). Section 4 gives the PowerShell equivalent of each. Git Bash is useful only for `nsh/agent/cargo-with-agent.sh` (`sh`).

### 3.2 Install (elevated PowerShell 7)

Option A, the repository's own idempotent script (run it after the clone in 3.3; it needs administrator rights; installs more than the NSH app needs: OpenSSL, Inno Setup, Python, the gpui-agent CLI; and puts Strawberry Perl and NASM at the front of the machine PATH):

```powershell
powershell -ExecutionPolicy Bypass -File C:\w\buwiz-forms\scripts\setup-windows.ps1
```

Option B, only what the NSH app needs (the same sources the script uses). Chocolatey is the script's installer; if it is missing, run step 1 of the script or install the same tools another way:

```powershell
choco install strawberryperl nasm powershell-core -y --no-progress
curl.exe -sSfL -o $env:TEMP\vs_buildtools.exe https://aka.ms/vs/17/release/vs_buildtools.exe
Start-Process -Wait -FilePath $env:TEMP\vs_buildtools.exe -ArgumentList '--quiet','--wait','--norestart','--nocache','--add','Microsoft.VisualStudio.Workload.VCTools','--add','Microsoft.VisualStudio.Component.VC.Tools.x86.x64','--add','Microsoft.VisualStudio.Component.Windows11SDK.22621'
curl.exe -sSfL -o $env:TEMP\rustup-init.exe https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe
& $env:TEMP\rustup-init.exe -y --default-toolchain 1.99.0 --profile minimal
```

Either way, then install LLVM (section 3.1), Git for Windows, and WebView2 if 3.6 says it is missing. Open a new PowerShell 7 window so PATH is fresh, then:

```powershell
rustup component add rustfmt clippy --toolchain 1.99.0
cargo install just --version 1.45.0 --locked
# Recommended: the cargo fork the owner's Mac and both repositories' CI use. This runs a script from the internet; read it first if that matters here.
$install = [scriptblock]::Create((irm https://huacnlee.github.io/cargo/install.ps1))
& $install -Version 0.102.0-9
cargo -V     # must name huacnlee/cargo
```

With that cargo, `CARGO_TARGET_DIR` is ignored and builds land under `%USERPROFILE%\.cargo\targets\` and `nsh\.nsh\rust\target` (`nsh/AGENTS.md`, "Pitfalls hit so far", item 1). Do not set `CARGO_TARGET_DIR` yourself: the Mac recipes set it, but with stock cargo it would move builds to a place nobody has tried with nsh (not verified).

### 3.3 Checkout layout

`buwiz-forms` and `nsh` must be siblings, because `nsh/Cargo.toml` names the NSH crates as `../../nsh/crates/...`. Keep the parent folder short (here `C:\w`); this is a precaution against Windows path limits, nothing in the repository says it is needed (not verified).

```powershell
New-Item -ItemType Directory -Force C:\w | Out-Null
Set-Location C:\w
git clone https://github.com/hexuria/buwiz-forms.git
git -C buwiz-forms switch feat/nsh-tsx
git clone https://github.com/hexuria/nsh.git
git -C nsh checkout --detach c346e920ee4ba12f6dc5804842e60814d2c02480
```

Both repositories are public (read with `gh repo view` when this was written), so cloning needs no token. Both set `* text=auto eol=lf` in `.gitattributes`: files are LF in every checkout; keep your editor on LF. Check the pins (the branch may have moved since this file was written: read the pin from `nsh.yml`, not from this file):

```powershell
git -C C:\w\buwiz-forms rev-parse HEAD        # 11675f95d982b6ebd480dfbe0bb1ef000cd9d02e when written; if different: git log --oneline 11675f95..HEAD
git -C C:\w\nsh rev-parse HEAD                 # must equal NSH_PINNED_REF below
Select-String -Path C:\w\buwiz-forms\.github\workflows\nsh.yml -Pattern 'NSH_PINNED_REF:'
```

### 3.4 The nsh command line (built from the pinned NSH checkout)

Do not install `nsh` with the install script, Homebrew, Scoop or mise: they give another version. Build it from the pin (`.github/workflows/nsh.yml` does the same; the owner's Mac took 10 minutes cold, Windows time is not known):

```powershell
Set-Location C:\w\nsh
$env:RUSTUP_TOOLCHAIN = "1.99.0"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:RUST_MIN_STACK = "268435456"      # NSH's Windows CI sets it: rustc can overflow its stack on the windows crate's bindings
cargo install --locked --path crates/nsh-cli --root "$HOME\BuwizNsh\tools"
```

The binary is `%USERPROFILE%\BuwizNsh\tools\bin\nsh.exe`. If the build fails, the first error line usually names the missing tool (libclang, perl, the SDK): fix that and rerun.

### 3.5 The session block (paste at the start of every PowerShell 7 session)

Nothing in it is permanent. It is the Windows form of what every `nsh-*` recipe sets (`justfile` lines 938-1144) plus the build settings above.

```powershell
$Repo = "C:\w\buwiz-forms"
$env:RUSTUP_TOOLCHAIN = "1.99.0"
$env:LIBCLANG_PATH = "C:\Program Files\LLVM\bin"
$env:RUST_MIN_STACK = "268435456"
$env:Path = "$HOME\BuwizNsh\tools\bin;C:\Strawberry\perl\bin;C:\Strawberry\perl\site\bin;C:\Strawberry\c\bin;C:\Program Files\NASM;$env:Path"
# The four safety variables. Never remove them.
$env:BUWIZ_NO_KEYCHAIN = "1"; $env:EBIR_TEST_ENV = "1"; $env:BIR_AUTH_BYPASS = "1"; $env:BIR_SFTP_DRY_RUN = "1"
Remove-Item Env:BIR_SFTP_LIVE -ErrorAction SilentlyContinue
Remove-Item Env:BIR_DATABASE_PATH -ErrorAction SilentlyContinue
$Handback = "$HOME\BuwizNsh\handback\$(Get-Date -Format yyyy-MM-dd)"
New-Item -ItemType Directory -Force $Handback | Out-Null
# Runs a block, shows its output, keeps a full copy in the handback folder, prints its exit code.
function Step($name, [scriptblock]$body) {
  & $body 2>&1 | Tee-Object -FilePath "$Handback\$name.log"
  "$name exit code: $LASTEXITCODE" | Tee-Object -FilePath "$Handback\$name.log" -Append
}
# Real-window tests and shots: PNGs are written into the current folder, so this runs from the evidence folder, never from the repository.
function Shots($name, [string[]]$more = @()) {
  $out = "$HOME\BuwizNsh\evidence\$name"
  New-Item -ItemType Directory -Force $out | Out-Null
  Push-Location $out
  try { nsh test "$Repo\nsh\shots\$name.test.tsx" --target windows @more 2>&1 | Tee-Object "$Handback\shots-$name.log" } finally { Pop-Location }
  "pictures in $out"
}
```

### 3.6 Check the machine before building

```powershell
Step "00-versions" {
  rustc -V; cargo -V; just --version; git --version; $PSVersionTable.PSVersion.ToString()
  (Get-Command perl).Source        # must be under C:\Strawberry, not Git's usr\bin
  nasm -v
  Test-Path $env:LIBCLANG_PATH
  & "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer\vswhere.exe" -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
  systeminfo | Select-String "OS Name","OS Version"
}
```

Pass: `rustc` and `cargo` say 1.99.0 (cargo names huacnlee/cargo if you installed it), `just` says 1.45.0, `perl` is Strawberry's, `vswhere` prints a path, `LIBCLANG_PATH` exists. WebView2 is not checked here: look for the WebView2 Runtime among the installed apps (its listed name is general Windows knowledge, not from this repository: not verified), and treat the print preview (W15) as the real test, because a placeholder where the sheet should be means the runtime is missing (NSH checkout `website/docs/windows.md`). If it is missing, install Microsoft's WebView2 Runtime (installer not named in the repository).

## 4. First commands, in order

Run them from a PowerShell 7 session with the block in 3.5 pasted. One `nsh` command at a time per checkout: two at once print each other's results (`nsh/AGENTS.md`, "Integration pass": the item that says two agents running `nsh test` at once can print each other's results). Cold builds are long: on the owner's Mac the first `nsh check` took 90 s, the first headless runner 57 s, the first host 63 s; on GitHub's `windows-latest` the older workspace's check job took 1 h 20 min. A build in progress is not "stuck".

**4.1 Type check.** `nsh check` runs `cargo check --all-targets` on the crate, then the TypeScript check (`website/docs/cli.md`).

```powershell
Set-Location $Repo\nsh
Step "01-check" { nsh check -v }
```

Pass: exit code 0 and no error lines. The success line is shown only with `-v`; without it a pass is silent (`nsh/AGENTS.md`, "Pitfalls hit so far", item 5). Its exact text is not verified.

**4.2 Build.**

```powershell
Step "02-build" { nsh build }
```

Pass: exit code 0, no warning lines (every warning is a finding), and `nsh\.nsh\out\bundle.nshc`, `bundle.js` and `assets\` exist. A warning means text outside a Text view, an unknown class or a missing asset (`nsh/AGENTS.md`).

**4.3 Rust tests of the crate.**

```powershell
Step "03-rust-tests" { cargo test --manifest-path Cargo.toml --no-fail-fast }
```

Pass: every `test result` line says `ok` and 0 failed. Nobody has run these on Windows; write down each failure with its first assertion line. The test `bir_core_is_built_as_plain_sqlite_with_the_nsh_data_folder` (`nsh/src/lib.rs:154`) fails if bir-core was built with sqlcipher; the gate is `cargo tree -e features -i libsqlite3-sys --manifest-path Cargo.toml`, which must show no `bundled-sqlcipher` (comment in `nsh/Cargo.toml`).

**4.4 Plain-SQLite core tests** (`just core-plain-test` is `[unix]`; this is its PowerShell form, from the repository root; CI runs it on Ubuntu only):

```powershell
Set-Location $Repo
Step "04-core-plain" { cargo test -p bir-core --no-default-features; cargo test -p bir-app-api --no-default-features }
```

Pass: both finish with 0 failed. (For orientation: `cargo test -p bir-app-api` with default features passed 666 tests on the Mac per the PR body; the plain count is not known.)

**4.5 Headless TSX tests.** Headless runs need no display and play an iOS-like platform unless a test says otherwise (the TSX side does not know which OS it runs on; the Rust side does), and each file runs on an engine of its own. This is the PowerShell form of the `nsh-test` recipe (`justfile` lines 962-1000). The 131 test files are the tracked `*.test.ts` and `*.test.tsx` files under `nsh/` (the same set the recipe's `find` gives; `nsh/verify/` is git-ignored and never included):

```powershell
Set-Location $Repo\nsh
Step "05-headless" {
  $failed = @()
  foreach ($file in (git ls-files -- '*.test.ts' '*.test.tsx' | Sort-Object)) {
    nsh test "./$file"
    if ($LASTEXITCODE -ne 0) { $failed += $file }
  }
  if ($failed.Count) { "failed in:"; $failed; $global:LASTEXITCODE = 1 } else { "every test file passed"; $global:LASTEXITCODE = 0 }
}
```

Pass: "every test file passed". **Until work item W7 is done, this step cannot pass on Windows**: the door's database guard refuses the scratch database a test environment makes, so every file that reaches the database should fail with "Refusing to open ... Set BIR_DATABASE_PATH to the file" (read from `nsh/src/door.rs` and `guards.rs`, not run). The stopgap, to get a first signal only, is to name a fresh file per test file (put it outside any `BuwizNsh\data` or `migration` folder); expect extra failures in files that open the app more than once, and in `bir/door.test.ts`, which looks for the scratch folder; those are artifacts of the stopgap, never a reason to edit a test:

```powershell
New-Item -ItemType Directory -Force "$HOME\BuwizNsh\agents\scratch" | Out-Null
foreach ($file in (git ls-files -- '*.test.ts' '*.test.tsx' | Sort-Object)) {
  $env:BIR_DATABASE_PATH = "$HOME\BuwizNsh\agents\scratch\$([guid]::NewGuid().ToString('N')).db"
  nsh test "./$file"
}
Remove-Item Env:BIR_DATABASE_PATH
```

Slow machines: headless, one JS call has 2 s before it is interrupted, a file's load 10 s, and a new app 5 s to render (`nsh/AGENTS.md`, "Slow machines"). If a file fails on a timing budget, rerun that file alone once; if it still fails, record it. Never raise a budget or add a sleep: tests wait for the condition.

**4.6 A real window, the app itself.** The NSH target name for Windows is `windows` (the default on Windows). This is the Windows form of `just nsh-run` (macOS only, `justfile` line 1075). Name a database file in the agents folder; a fresh file should show the first-run setup, where you can create the demo taxpayer (Juana Santos, TIN 000-123-453, an individual; the other fields as in `createDemoTaxpayer` in `nsh/bir/testing.ts`):

```powershell
New-Item -ItemType Directory -Force "$HOME\BuwizNsh\agents" | Out-Null
$env:BIR_DATABASE_PATH = "$HOME\BuwizNsh\agents\windows-first.db"
Set-Location $Repo\nsh
nsh run --target windows 2>&1 | Tee-Object "$Handback\06-run.log"
```

Pass: a window opens with the app's own 40-point title bar and the three caption buttons NSH draws at its right (`nsh/nsh.json` `window.titleBar`; NSH checkout `website/modules/nsh-desktop.md` says 138 points for them), then the first-run setup or Home, with text in Inter, no crash and no red error overlay, and the terminal shows log lines without panics. **The close button hides the window to the tray on Windows** (`nsh/shell/desktop/menu_model.ts:66`), so `nsh run` does not end when you press it: end it with Ctrl-C in the terminal, or Quit in the tray menu. The tray icon may sit in the notification area's overflow.

**4.7 Real-window tests and the shots recipes.** `just nsh-shots <name>` is `[macos]` (`justfile` line 1129): it runs `nsh test nsh/shots/<name>.test.tsx --target macos` from `~/BuwizNsh/evidence/<name>/` because the PNGs are written relative to the current directory, and never into the worktree. On Windows use the `Shots` function from 3.5 (it runs `nsh test` on the shots file from the evidence folder):

```powershell
Shots "base"          # the first real-window test: four windows, asserts "Native module says: pong"
```

Pass for `base`: the four tests pass (wide and narrow, light and dark), eight PNGs appear in `$HOME\BuwizNsh\evidence\base`, and the pictures look right: open them. Check in the `base-*-end.png` pictures that the "Tabular figures" sample (1,111.11 / 0,000.00 / 8,888.88) aligns, and that Inter, not a fallback font, draws the text. Then run the names in the work list (section 6). The shots ask for 1440x900 windows (`nsh/shots/shoot.ts`): use a display of at least 1920x1080 (what Windows does with a window larger than the screen is not verified), and keep the RDP session connected while real-window tests run (a disconnected session is general Windows behaviour, not from this repository: not verified). If the first real-window run says "host did not connect in 60 s", run it again: a freshly linked host can start slowly the first time (`justfile` comment in `nsh-agent-check`).

### Where the recipes assume macOS, and what to use instead

| The recipes assume | Where | On Windows use |
|---|---|---|
| The CLI at `~/BuwizNsh/tools/bin/nsh`, put first on PATH | every `nsh-*` recipe (`justfile` 938-1144) | `$HOME\BuwizNsh\tools\bin` first on PATH (3.5) |
| `CARGO_TARGET_DIR=$HOME/BuwizNsh/target-nsh` | every `nsh-*` recipe | do not set it (3.2) |
| `[unix]` recipes: `nsh-check`, `nsh-build`, `nsh-test`, `core-plain-test`, `_nsh-db-guard` | `justfile` 147, 938, 947, 962, 1005 | not defined on Windows: sections 4.1 to 4.5 |
| `[macos]` recipes: `nsh-run`, `nsh-shots`, `nsh-agent-check` | `justfile` 1075, 1094, 1129 | 4.6 and 4.7 |
| `--target macos` inside `nsh-run` and `nsh-shots` | `justfile` 1084 and 1143 | `--target windows` |
| `nsh-serial`, a lock wrapper that runs one command at a time and reaps stray `-macos-host` windows | `nsh/AGENTS.md`; the script is in `~/BuwizNsh/tools/bin` on the Mac, not in the repository | run one command at a time; list `nsh-windows-host` processes before and after, stop only ones that appeared after you started |
| `_nsh-db-guard`: a Python script that refuses a database inside the owner's folders | `justfile` 1005-1068 | the door's own guard (`nsh/src/guards.rs`) is the check on Windows, and it is what you fix in W7; keep the database under `$HOME\BuwizNsh\agents` |
| Retina pictures (1440 points are 2880 pixels) | `nsh/shots/pixels.ts`, `nsh/shots/f10.test.tsx` | the pixel tests derive the scale from the picture width (`r.width / 1440`) but need a 1440-wide window; the PNG decoder on Windows output is not verified |
| `/tmp/...` for the print test's PDF | `nsh/shots/print.test.tsx:11` | not valid on Windows; Save as PDF is a stub there anyway (section 5) |
| `sh` scripts | `nsh/agent/cargo-with-agent.sh` | Git Bash; only for the agent feature |
| macOS display flags | `--display`, `--foreground` in `nsh test` are macOS only (NSH checkout `website/docs/cli.md`) | omit |

## 5. Known platform differences

macOS is what the code was written and tested on. "Today" is what the code does on Windows; every row is read from the code and the NSH checkout's docs, none was run.

| Feature | What macOS does | What the code does on Windows today | Expected to need work |
|---|---|---|---|
| Print preview window | A second window with a web view on a `file://` URL of the filled sheet (`nsh/screens/print/host.tsx`); NSH's macOS web view gives the page read access to its own folder (NSH checkout `crates/nsh-platform-macos/src/native.rs:1828-1883`). | The TSX host and the Rust side that writes the sheet run on every desktop (`nsh/src/print.rs:558`). The sheet's URL comes from `url_for` (`print.rs:252`), which percent-encodes every byte except letters, digits and `- _ . ~ /`, so `C:\Users\x\sheet.html` becomes `file://C%3A%5CUsers%5Cx%5Csheet.html`, which is not a valid Windows file URL; its unit test (`print.rs:1183`) covers a Unix path only. The page is a WebView2 child window; a dialog or sheet NSH draws stays under it (`web-view-and-video.md`). | Fix the URL for Windows paths, add a unit test, open it for real; check that WebView2 loads the sheet and the live-update script runs without a reload (the TSX test expects "page loads: 1"). Whether NSH's capability check lets a `file://` page load on Windows is not verified. |
| Save as PDF | A hidden WKWebView makes the PDF from the same HTML and bir-print merges the pages (`print.rs:717` on, `mod mac`). | Stub: `platform_pdf` answers "Print and Save as PDF are available on macOS only for now." (`print.rs:700-701`), which the preview window shows. NSH 0.5.1 has no print or PDF call in its Windows backend or docs (searched `nsh-platform-windows/src`, `nsh-platform-linux/src`, `nsh-core/src` and `website/modules` for pdf, PrintToPdf, print_to_pdf: no match). The old app's Windows route was the WebView2 print UI behind a Print button, listed as untested (`docs/platforms/windows.md:29,107`). | A decision first (section 8, question 1), then code of our own. Parked in `docs/platforms/nsh.md`. |
| Print | PDFKit's print panel for that PDF (`print.rs:706`). | The same stub (`print.rs:711-712`). | The same. |
| Tray and hide-to-tray | `Tray.set` puts the icon in the menu bar (`nsh/shell/desktop/Desktop.tsx:117`). The close button, Cmd+W and Cmd+H hide the window; Cmd+Q hides too; Quit is in the tray (`shell/desktop/README.md`, `menu_model.ts:64-68`). | The same TSX; the tray is the notification area. `CLOSE_POLICY.windows`: the close button hides, there is no quit shortcut (`menu_model.ts:66`), so the only quit is the tray menu's Quit or File > Quit. | Look at it: icon appears (maybe in overflow), a click shows the window, Show, Hide and Quit work, Quit ends `nsh run` and flushes drafts. |
| Application menu | The menu bar. | `Menu.setApplicationMenu` gets File, Edit, View, Window, Help (`Desktop.tsx:104`, `menu_model.ts:199`). NSH shows it as the window's menu bar only while the title bar is not hidden; this app hides it (`nsh.json`) and nothing in `shell/`, `ui/` or `screens/` calls `Menu.popup` (searched), so the menu is probably not reachable by mouse; its shortcuts still work (`website/modules/nsh-desktop.md`, "Menu"). | Confirm in a window; decide (section 8, question 2), likely a menu button in the title bar that calls `Menu.popup`. |
| Global hotkey | Default modifiers Control+Alt. | Default modifiers are Super+Shift, that is the Windows key and Shift (`crates/bir-app-api/src/hotkey_combo.rs:4`, `nsh/src/desktop.rs:944`). It goes through NSH's `GlobalShortcut.register` (`Desktop.tsx:123`), which resolves false when another app or the system holds the chord (NSH `nsh-desktop.md`). | Try each preset the Settings page offers (E, B, H, F12) and read the status; which Win+Shift chords the system keeps is not verified. |
| Cmd/Ctrl key bindings | `mod` is Command. | `mod` is Ctrl. The app binds `mod+h`, `mod+,`, `mod+n`, `mod+k`, `mod+b`, `mod+l`, `mod+t`, `mod+shift+n`, `mod+shift+b`, `mod+[`, `mod+]` (`nsh/shell/*.tsx`, `menu_model.ts`) and `mod+left` and `mod+right` on the form page, bound "not while editing" so they do not take caret keys (`nsh/AGENTS.md`). NSH reserves Alt+F4, F11 and Ctrl+Q ("cannot be bound", NSH `windows.md`); the app has no Quit shortcut on Windows, and its only use of a reserved key is F11 on the View menu's full-screen role item (`menu_model.ts`), which NSH handles itself. Alt+Left is NSH's own Back. | Walk the list once in a real window. |
| One-copy guard | A lock file `<database>.instance` and a loopback port file; a second launch asks the first to show and exits 0 (`nsh/src/desktop.rs:411`, `570`). | The same Rust: a std file lock, and a separate port file because Windows will not let another process read a locked file (`desktop.rs:355-358`). `prepare_process` skips the guard when the database guard refuses the path (`desktop.rs:578`), so it only works with `BIR_DATABASE_PATH` set. Off under `EBIR_TEST_ENV` unless `BIR_SINGLE_INSTANCE=1` (`desktop.rs:516`). Rust tests exist (`desktop.rs:1187`, `1208`); never run on Windows. | Run it for real with `BIR_SINGLE_INSTANCE=1`: the second `nsh run` on the same database prints "already running" and exits 0, the first window comes forward. |
| Remembered window bounds | `nsh.json` `window.restoreBounds: true`; NSH keeps the placement under the window's id in the app's data and fits it to the displays. | The same setting, NSH's Windows backend; never run. When it is written (the close button only hides) is not verified. | Move and resize, Quit from the tray, relaunch: size and place come back. |
| Updater | Off: no `"updater"` in `nsh.json`; status "idle, not configured"; nothing touches the network (`desktop.rs` `NOT_CONFIGURED`). | The same. | None. Do not configure it (section 2, rule 3). |
| Biometric or OS sign-in recovery | Touch ID or the account password through `robius-authentication` (`nsh/src/auth.rs:26-55`). | The same crate, with `WindowsText`: Windows Hello (`auth.rs:26-55`). Never run. `mac_login_available` is true on Windows (`crates/bir-app-api/src/bir_module.rs:1273`), so the recovery button shows, and its label says "Use my Mac login instead" (`nsh/screens/auth/SignIn.tsx:114`; also `screens/auth/logic.ts:19`). A test environment never asks the OS (`auth.rs:13-24`) and every dev run is one, so the real prompt cannot be tried in this handover. | Wording for Windows (question 5); whether Windows Hello is set up on the EC2 machine is not verified. |
| Database location and guard | Data folder `~/Library/Application Support/dev.goldcoders.bir.nsh`. The guard finds the account's home from the password database and knows folders by device and inode (`nsh/src/guards.rs:157-181`, `343-366`). | Data folder `%LOCALAPPDATA%\dev.goldcoders.bir.nsh` (`crates/bir-core/src/platform/windows.rs`). The guard opens a database only when `BIR_DATABASE_PATH` names that very file (`guards.rs:285-327`: `unverified_refusal`, with `cfg!(unix)` as "can verify"). Its home folder is `std::env::home_dir()` (`guards.rs:185`), which starts from `USERPROFILE`, and there is no file identity (`guards.rs:352`). In a test environment the door names a scratch database by override, not by the variable, so the Windows guard refuses it (`door.rs:106-130`, `266-275`). The protected names are three macOS-style folders under home (`guards.rs:144-150`); the older Windows app's folder `%LOCALAPPDATA%\Taxman\eBIRForms` (`platform/windows.rs`) is not in the list. | The main Windows code task, W7: a real profile-folder lookup and a same-file check. `crates/bir-print/Cargo.toml` already depends on `windows-sys` with `Win32_Storage_FileSystem` (whether it exposes what is needed is not verified). Protecting the older app's folder changes the contract in `docs/plans/nsh-tsx-migration.md`: question 3. |
| Fonts and icons | Inter (four `.woff2` faces registered by `nsh/theme/fonts.ts`), SVG icons in `nsh/assets/icons`, drawn by Core Text. | The same files through NSH's DirectWrite and Direct2D text engine. Never seen. | In the `base` pictures: Inter draws, tabular figures align, the peso sign and the dashes come out, icons draw. |
| File dialogs | NSH's file panels (`nsh/src/hooks.rs` `prompt_for_open_path`, `prompt_for_save_path`). | NSH's shell dialogs, `IFileOpenDialog` and `IFileSaveDialog` (NSH checkout `crates/nsh-platform-windows/src/file_requests.rs`). The save panel starts in `Documents` under std's home folder (`print.rs:975`). Never run. | A person presses Import and Export in Settings in a real window and operates the dialogs; no test API drives them. |
| Credential store | Keychain. | The `keyring` crate's `windows-native` store (`crates/bir-core/Cargo.toml`). With `BUWIZ_NO_KEYCHAIN=1` nothing reads or writes it. | None while the switch stays on. Never turn it off here. |
| Agent control (cargo feature `agent`) | Built and tested: the data plane (loopback listener with a token, `GPUI_AGENT=1` to start) and NSH's window control plane (`nsh/Cargo.toml` line 50, `nsh/src/agent.rs:42`). | Off by default. The data plane is plain Rust and type-checks; the window control plane is macOS-only. `nsh/tests/agent.rs` and `agent_send.rs` need only `--features agent`; `window_plane.rs` is macOS only. | Run the two data-plane tests (W10). Do not turn the feature on in a run. |
| Title bar and caption buttons | The app draws a 40-point bar; padding for the traffic lights comes from `titleBar().left`. | NSH draws the three caption buttons (Windows 11 style) 138 points wide at the right; the shell pads by `titleBar().right` (`nsh/AGENTS.md`, "The frame"). | Look for anything under the buttons in the `shell` shots. |
| Private print folder | Folder 0700 and files 0600, stale ones swept at start (`print.rs:284-304`, `1011-1012`). | A per-user temp folder with no ACLs, and no sweep, because `process_alive` answers true for every process (`print.rs:231-236`). A stale `ebirforms-print-<pid>` folder (it holds a filled return) stays in `%TEMP%` until the system cleans it. | Question 6. |

## 6. The work list

One item at a time, in this order. Each item ends with its check; write the result in your report (section 7) before starting the next. Commit each item on your branch with a message that says what and why.

- **W1. The machine is ready.** Do section 3 and run 3.6. Check: `00-versions.log` shows the versions in 3.6 and both pins match.
- **W2. The nsh command line builds.** 3.4. Check: `nsh -V` reports 0.5.1.
- **W3. `nsh check -v` passes** (4.1). Check: exit code 0. If cargo fails, the first error and its tool name go in the report.
- **W4. `nsh build` passes with no warnings** (4.2). Check: exit code 0, no warning line, `.nsh\out\bundle.nshc` exists.
- **W5. The Rust tests of the crate** (4.3). Check: the full list of failures, each with a one-line cause (a Unix path, the guard, a timing, other). Fix code, never an expectation. The one expectation that is about the OS: `nsh/bir/door.test.ts:19` says `expect(info.os).toBe("macos")`, and `info.os` is Rust's `std::env::consts::OS` (`crates/bir-app-api/src/bir_module.rs:427`, `system_info`), so it reads `"windows"` here; change it to assert the OS the test runs on (still an exact comparison), and say so in the PR.
- **W6. The window opens** (4.6). Check: the pass description in 4.6, a note of what is on screen, and the log with no panic. Press nothing else yet.
- **W7. Make the Windows database guard real.** In `nsh/src/guards.rs`: find the account's profile folder from Windows itself (not from `USERPROFILE`, which a caller can change), and add a same-file check (identity) so a protected folder is recognised under another name (junction, symlink, short name, case, `\\?\` prefix, hard link). Then pass `true` for "can verify" on Windows (the `cfg!(unix)` argument at `guards.rs:291-296`) so that a scratch database a test environment makes is accepted while the protected folders stay refused. Never loosen the Unix paths. Check: new Windows tests in `guards.rs` that mirror the Unix ones (by name, by identity, hard link, case) pass with `cargo test --manifest-path Cargo.toml guards`; the existing guard tests still pass; `nsh test ./bir/door.test.ts` passes without `BIR_DATABASE_PATH`; and a unit test shows a database named inside `$HOME\BuwizNsh\data` still refused (test it with a test; never try it with the app). Split it into two sittings if needed: profile lookup, then identity.
- **W8. Plain-SQLite core tests** (4.4). Check: both commands report 0 failed.
- **W9. The headless TSX tests, all files** (4.5, without the stopgap). Check: "every test file passed", or the failing files, each with a cause. Apply the timing rule in 4.5.
- **W10. The agent data-plane tests.** `cargo test --manifest-path Cargo.toml --features agent --test agent --test agent_send` from `nsh/`, with the session block. It fetches `hexuria/gpui-agent` from GitHub. Check: both test binaries pass, or the failure is recorded.
- **W11. The first real-window tests.** 4.7: `Shots "base"`. Check: the four tests pass and the pictures look right (Inter, tabular figures, icons, title bar).
- **W12. The window's chrome, by hand** (a person at the screen; no test API reads the tray or the menu bar). With `nsh run` from 4.6: the tray icon appears; a click shows the window; the close button hides it; Show, Hide and Quit in the tray menu work; Quit ends `nsh run`; Ctrl-C also ends it. Check: each of those written as seen or not seen.
- **W13. The application menu on Windows.** Look for the menu bar. If it is not visible, record that and apply the owner's answer to question 2; until then do not build a menu button. Check: a written yes or no with a picture of the title bar.
- **W14. The hotkey, the one-copy guard and the remembered bounds.** Hotkey: Settings shows the status for each preset. One copy: run `nsh run` twice on the same named database with `$env:BIR_SINGLE_INSTANCE = "1"` (the second must print that eBIRForms is already running and exit 0; stop only the processes you started). Bounds: move and resize, Quit from the tray, relaunch. Check: three short notes.
- **W15. Print preview opens on Windows.** Fix `url_for` for Windows paths with a unit test, then run `Shots "print"`. Check: the preview window opens, shows the sheet (not a placeholder), and an edit in the form reaches it without a reload; the Save as PDF and Print buttons answer with the stub sentence. The test's own PDF step uses `/tmp` and is expected to stop there; note where.
- **W16. The OS sign-in wording and Windows Hello.** Check: the label and sentences that say "Mac login" listed with file and line (`screens/auth/SignIn.tsx`, `logic.ts`, `state.ts`, `screens/settings/Security.tsx`); the real prompt is not reachable in a test environment, so write that down instead of trying a non-test run.
- **W17. The shots sweep.** Run `Shots` for each name in `nsh/shots` except `agent` and `forms43` (`shell`, `form`, `home`, `library`, `calendar`, `filings`, `taxpayer`, `settings`, `auth`, `controls`, `controls-form`, `kit`, `desktop`, `f10`; `print` was done in W15), each with `Shots`. Check: for each, pass or fail with the first failing line, and the pictures reviewed; layout defects (clipped text, overlap with the caption buttons, wrong font) listed with picture names.
- **W18. All 43 forms open.** First headless: `nsh test ./screens/form/all43.test.ts` from `nsh/` (one test per form; each prints a row that starts with `D8`, the test's own tag, then the form code, parts, fields and milliseconds; 44 tests in all). Then in real windows, in eight batches (`b1-` to `b8-`, six forms each, the last has one): `Shots "forms43" @("--filter","b1-")`, then the same with `"b2-"` and so on. Check: 43 rows that start with `D8`; 86 PNGs (`<code>-top.png` and `<code>-all.png` for each form) in `$HOME\BuwizNsh\evidence\forms43`, none blank, none still saying "Opening".
- **W19. The real-window shots, the whole set again** after every fix above, with the `f10` pixel tests. Check: the report table in section 7 lists every shots name with its final result, and the evidence folders are zipped for the owner.

## 7. How to report back

**Capture, for every item:** the command exactly as typed, its exit code, and the full output. The `Step` function in 3.5 keeps each log in `$HOME\BuwizNsh\handback\<date>\`. Add the machine facts once (`00-versions.log`: tool versions, `systeminfo` OS lines, whether WebView2 is installed, the display size). Pictures: real-window shots come from the `Shots` function into `$HOME\BuwizNsh\evidence\<name>\` (never into the repository). Look at each picture; a test that passes does not prove it looks right. Keep a `REPORT.md` in the handback folder (not in the repository) with one row per work item: id, command, exit code, result (pass, fail, parked), one sentence, log or picture names.

**Push:**

```powershell
Set-Location $Repo
git fetch origin
git switch -c feat/nsh-tsx-windows-<topic> origin/feat/nsh-tsx
# ...work, commit...
git push -u origin feat/nsh-tsx-windows-<topic>
gh pr create --draft --base feat/nsh-tsx --head feat/nsh-tsx-windows-<topic> --title "NSH on Windows: <topic>" --body-file $Handback\REPORT.md
```

A branch per topic (the guard, the print URL, the wording), each with its own draft PR against `feat/nsh-tsx` (CI runs on those: `ci.yml` accepts pull requests into `feat/**`). Never merge, never mark a PR ready, never push to `feat/nsh-tsx` or `main`. If a push is refused with a message about the `workflow` scope (it happens when a change touches `.github/workflows` and the HTTPS token lacks that scope), push over SSH instead of HTTPS. Attach pictures of the demo taxpayer to the draft PR as comments, or zip `$HOME\BuwizNsh\evidence` and hand it to the owner; never commit PNGs (`.gitignore` ignores `/nsh/*.png`).

**When stuck:** after about 20 minutes on one obstacle that is not just a long build (`docs/plans/nsh-tsx-migration.md`, "Hard rules", item 8), park it: in `REPORT.md` write the item id, the command, the first 30 lines of the error, what you tried, and what you think is missing; then go on to the next item that does not depend on it. W2 blocks everything after it; W3 to W5 are independent of each other; W7 blocks the meaning of W9, W17 and W18. Do not fix a failure by weakening a test, a validator or the guard.

## 8. Open questions for the owner

1. **Print and Save as PDF on Windows.** NSH 0.5.1 has no PDF call, so this needs code of our own. Options visible in the repository: the older app's route (the WebView2 print UI behind a Print button, `docs/platforms/windows.md:107`, untested there), or our own native call into WebView2, or keep the stub for now. Which?
2. **The application menu on Windows.** With the title bar hidden NSH does not show the menu bar. Add a menu button to the app's title bar that opens the same menus with `Menu.popup`?
3. **The database guard.** OK to implement the profile-folder lookup and same-file check (W7)? Should the older Windows app's data folder `%LOCALAPPDATA%\Taxman\eBIRForms` be protected too? That adds to the three folders listed in the contract (`docs/plans/nsh-tsx-migration.md`, "The door").
4. **The EC2 image.** Which Windows edition is it? NSH names Windows 10 1809 and later and Windows 11; Server editions are not named and WebView2 may be absent. Is a display of at least 1920x1080 available over RDP?
5. **Wording.** The sign-in recovery says "Mac login" on every platform (`screens/auth/SignIn.tsx:114`). Say "Windows Hello" or "your Windows sign-in" on Windows?
6. **Print folder on Windows.** It holds a filled return and has no ACLs or sweep (`print.rs:231-236`, `302-305`). Accept per-user temp, or add both?
7. **CI.** Add a Windows job for `nsh check`, `nsh build` and the Rust tests to `.github/workflows/nsh.yml` once it all runs by hand? (The older workspace's Windows check took 1 h 20 min.)
8. **PR #221** shows as not a draft on GitHub although its body says "Draft. Not for merge yet" (read with `gh pr view 221`). Mark it as a draft?
