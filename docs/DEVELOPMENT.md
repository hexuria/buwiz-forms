# Development guide

This page is for people who build, test and release the app. If you only want
to use it, read the [README](../README.md).

Platform differences (what works where) are in [FEATURES.md](FEATURES.md) and
the per-platform pages: [macOS](platforms/macos.md),
[Windows](platforms/windows.md), [Linux](platforms/linux.md). Form coverage and
parity are in [FORMS.md](FORMS.md).

## Set up a checkout

1. Install the platform prerequisites from the README
   ([macOS](../README.md#macos), [Windows](../README.md#windows),
   [Linux](../README.md#linux)). The setup scripts in `scripts/` install the
   toolchain, system libraries and the `gpui-agent` CLI.
2. Clone the repository.
   ```bash
   git clone <repository_url>
   cd bir
   ```
3. Install `just`, the command runner. The `justfile` drives build, run and
   package steps on every platform.
   - macOS: `brew install just`
   - Windows: `choco install just`
   - Linux or any platform with Cargo: `cargo install just`
4. Optional, for code quality, dependency and security checks:
   ```bash
   cargo install cargo-audit cargo-outdated cargo-machete
   ```
5. Run `just help` to list the commands, and `just run` to start the app in
   development mode.

## Available Commands

We follow a "less is better" philosophy. You only need to remember a few core commands:

- `just run` — Run the app locally in a development window (use a scratch database).
- `just install [format]` — Install a package you already built (macOS `.app`/`.pkg`/`.dmg`, Windows `msix`/`exe`, Linux `deb`/`tar`). It looks in `target/release-artifacts` and does not build anything.
- Build the packages with `scripts/package_nsh_windows.ps1` after `nsh package --target windows` (Windows: Setup program, MSI, MSIX), `just _package-mac` (macOS universal `.app` and DMG — it runs `scripts/package_macos_release.sh`, ad-hoc signed unless `CODESIGN_IDENTITY` is set) and `packaging/linux/build-packages.sh` (Linux `.deb`, `.rpm`, tarball, Arch package and install script). The Mac App Store `.pkg` is `just app` (queries App Store Connect for the next build number, then `just _package-mac-appstore`, which adds the `mas_build` feature via `nsh/mas/cargo-with-mas.sh`). The release workflows build the same packages.
- `just testflight [--agent]` — Build the App Store package and upload it to TestFlight (`just testflight-agent` is the same with the gpui-agent control plane; internal testing only). `just testflight-upload` re-uploads the last build without rebuilding.
- `just nsh-run-agent macos <db>` — Run the development host with the gpui-agent control planes compiled in (see [agent.md](agent.md)). Still silent until `GPUI_AGENT=1` (release binaries also need `GPUI_AGENT_ALLOW_RELEASE=1`).
- `just publish` — Auto-increment the patch version, tag, and push (triggers the release workflow in GitHub Actions).

**Quality & Testing:**
- `just check` — Run code formatting (`cargo fmt`), linting (`cargo clippy`), and type checking (`cargo check`) across the workspace.
- `just test` — Run all unit and integration test suites.
- `just mock-bir` / `just mock-bir-app <database copy>` — A local mock of the BIR SFTP endpoint, and the installed app run against it on a copy of the database. See [mock-bir.md](mock-bir.md).

**Frozen HTML:**
- Product CI checks out `hexuria/buwiz-forms-html` and runs `freeze_html.py --verify` plus `stamp_frozen_names.py --check-all`.
- `just build-frozen-html-identity` — Hash `html-frozen/` into the non-promotional packaging identity.

**Dependency Management:**
- `just audit` — Check for security vulnerabilities in dependencies using the RustSec database.
- `just outdated` — Display a list of dependencies that have newer versions available.
- `just unused` — Scan `Cargo.toml` for dependencies that are no longer being used.

---

## Driving the app with gpui-agent

The agent is a build feature, off by default. Nothing in a product build can
reach it unless it is compiled in (`just nsh-run-agent` wraps the nsh build
with `nsh/agent/cargo-with-agent.sh`; the headless `bir` builds its `agent`
and `headless` cargo features). Even a feature-enabled binary stays silent
until `GPUI_AGENT=1`. There is no `scripts/bir-agent` wrapper: launch the
host with env vars, then talk to it with `gpui-agent`.

The `gpui-agent` CLI is the [`gpui-agent-cli`](https://crates.io/crates/gpui-agent-cli) crate
and is installed automatically by every setup script — `setup-windows.ps1`,
`setup-linux.sh`, and `setup-macos.sh` all resolve the exact version the app
pins (`Cargo.toml` / `Cargo.lock`) so driver and host can never
drift on protocol version. Manually:
`cargo install gpui-agent-cli --version <pinned-version>`.

**Windows note:** the HMAC token handshake works at the current pinned
version. Set `GPUI_AGENT_TOKEN` for secure driving. For local-only driving,
`GPUI_AGENT_INSECURE_NO_TOKEN=1` remains available (never on shared machines).

```bash
# Start the NSH window with the control planes (scratch database only):
GPUI_AGENT=1 GPUI_AGENT_TOKEN=<local-development-token> \
  just nsh-run-agent macos <scratch.db>

# Or the headless host (no window):
cargo build --locked -p bir-cli --features headless
GPUI_AGENT=1 GPUI_AGENT_TOKEN=<local-development-token> \
  target/debug/bir serve

export GPUI_AGENT_ADDR=127.0.0.1:17421      # default; use 127.0.0.1:17423 when NativeChat owns 17421
# Release binaries also need:
# export GPUI_AGENT_ALLOW_RELEASE=1
export GPUI_AGENT_SCREENSHOT_DIR=/tmp/bir-shots
```

Then, from another shell with the same `GPUI_AGENT_TOKEN` / `GPUI_AGENT_ADDR`:

```bash
gpui-agent hello                 # protocol, app, platform, os, ready
gpui-agent keybindings           # {id, chord, scope, dangerous}
gpui-agent keybinding --id app.toggle_sidebar --scope focused --activate
gpui-agent invoke profile.set --arg tin=00000000000000
gpui-agent invoke filing.start --arg code=1601C --arg year=2026 --arg period=9
gpui-agent screenshot --out form.png --mode scrolled --target form-1601c-scroll
```

A bare `gpui-agent` command (no `--addr`, no `--connect`) talks to
`GPUI_AGENT_ADDR`, else `127.0.0.1:17421`; it never reuses an earlier
`--connect` choice. Pass `--connect last` to reuse the saved choice, or
`--connect <app|connect-id|pid>` to pick a host from the discovery list
(the NSH app and the headless host both publish app `bir-shell`).

Before driving a host, check who owns the port — `lsof -nP -iTCP:17421
-sTCP:LISTEN` (or `17423` when that is `GPUI_AGENT_ADDR`) must show the NSH app or
the headless `bir`. Another process there (an editor forwarding a remote machine,
say) answers the protocol convincingly; `hello.os` and the chord style
(`cmd-b` on macOS, `ctrl-b` on Linux) tell the two apart. A `focused`-scope
keybinding needs `--activate` when the app is not the frontmost application,
and macOS screenshots need Screen Recording granted to that exact binary path.

The full contract — authorization, the two-host port handoff, screenshot and
scroll semantics, and the AI-agent playbook — is in
[`docs/agent.md`](agent.md).

## Publishing the `bir` CLI

- The headless agent binary is `bir`, from `crates/bir-cli` in the root
  workspace. Build locally with `cargo build --locked -p bir-cli`.
  Install from a checkout with `cargo install --path crates/bir-cli --locked`.
- Desktop releases use tags `vMAJOR.MINOR.PATCH` (`just publish`); headless
  archives use `bir-vMAJOR.MINOR.PATCH`. `.github/workflows/bir.yml` builds
  `bir` from `crates/bir-cli` against the workspace version in the root
  `Cargo.toml` and archives it. Cargo publishing is not configured.
- The toolchain is pinned to Rust 1.99.0.
- `scripts/test_publish_bir.py` tests the publish script.

## Versioning & Build Numbers

The project uses **two independent version identifiers** to satisfy both Apple App Store and Microsoft Store requirements:

| Identifier | Source of Truth | Changed By | When |
|---|---|---|---|
| **App Version** (semver) | `nsh/nsh.json` → `version` (`"0.2.1"`) | You, manually | Feature release or hotfix |
| **Build Number** (counter) | `BUILD_NUMBER` environment override, with the `justfile` value as the committed default | `just app` (ephemeral) or `just bump-build` (committed) | Every Apple submission; the Windows package does not use it |

### Why Two Numbers?

Apple and Microsoft handle versioning differently:

| | macOS App Store | Microsoft Store |
|---|---|---|
| Marketing version | `CFBundleShortVersionString = "0.2.1"` | Store listing description |
| Build identifier | `CFBundleVersion = "27"` (separate field) | `Version = "0.2.1.0"` in AppxManifest — `Major.Minor.Patch.0`, no build counter |
| Multiple builds per version? | ✅ Yes | ❌ No — each submission needs a higher `Version` |

**macOS** has two separate fields — one for the user-facing version, one for the internal build counter. **Microsoft** has only a single `Version` field, and the 4th part (Revision) must always be `0`.

### How the Version Maps to Each Store

Given `nsh.json` `"version": "0.2.1"` and `BUILD_NUMBER = 27`:

```
macOS App Store:
  CFBundleShortVersionString = "0.2.1"    ← what users see
  CFBundleVersion            = "27"       ← internal build number

Microsoft Store:
  AppxManifest Version       = "0.2.1.0"   ← Major.Minor.Patch.0 (no build counter)
  Store listing              = "0.2.1"    ← what users see (set in Partner Center)
```

The MSIX `Version` is written by `scripts/package_msix.ps1` as
`Major.Minor.<patch>.0`; `scripts/package_nsh_windows.ps1` passes the semver
patch number (`BuildNumber = [int]$patch`), so Windows has no separate build
number.

### Developer Workflow

**Submitting a macOS App Store build** (most common):
```bash
just app  # Queries App Store Connect and passes the next counter without editing tracked source
```

Keeping the checkout clean lets packaged renderer identity verification bind
the build to its exact curated source revision before the app is assembled.

**Persisting one counter for both stores:**
```bash
just bump-build
git add justfile && git commit -m "build: bump to $(just --evaluate BUILD_NUMBER)"
just app   # macOS package; the live API counter is passed through the environment
# Windows: scripts/package_nsh_windows.ps1 builds the MSIX; its Version is Major.Minor.Patch.0
```

**Releasing a new version** (feature release or hotfix):
```bash
# 1. Manually edit nsh/nsh.json: "version": "0.3.0"
# 2. Then bump and build as usual:
just bump-build
git add Cargo.toml justfile && git commit -m "release: v0.2.0"
```

> **Note:** `just bump-build` only works on macOS — it requires the Apple App Store Connect API credentials (`.p8` key, `APP_STORE_ISSUER_ID`, `APP_STORE_KEY_ID` in `.env`). On Windows, simply `git pull` to get the latest `BUILD_NUMBER` from the justfile, then build the packages with `scripts/package_nsh_windows.ps1`.

### Version Progression Example

| Action | Cargo.toml | BUILD_NUMBER | macOS | Windows MSIX |
|---|---|---|---|---|
| Current | `0.2.1` | 27 | `0.2.1` (build 27) | `0.2.1.0` |
| Bump build | `0.2.1` | 28 | `0.2.1` (build 28) | `0.2.1.0` (unchanged) |
| Bump build | `0.2.1` | 29 | `0.2.1` (build 29) | `0.2.1.0` (unchanged) |
| Hotfix | `0.2.2` | 30 | `0.2.2` (build 30) | `0.2.2.0` |
| Feature release | `0.3.0` | 31 | `0.3.0` (build 31) | `0.3.0.0` |
| Stable release | `1.0.0` | 31 | `1.0.0` (build 31) | `1.0.0.0` |

## CI/CD Secrets & Codesigning

The GitHub Actions workflows (`ci.yml` and `release.yml`) are fully automated. Linux builds require **no secrets**. The Windows release job needs signing inputs and the macOS job needs the Apple secrets below. 

The **macOS** release job codesigns and notarizes the DMG. Configure these Repository Secrets in your GitHub settings:

| Secret Name | Required For | Description |
|---|---|---|
| `APPLE_CERTIFICATE_P12` | macOS Codesign | Base64-encoded `Developer ID Application` .p12 certificate |
| `APPLE_CERTIFICATE_PASSWORD` | macOS Codesign | Password to unlock the .p12 certificate |
| `APPLE_TEAM_ID` | macOS Codesign & Notarization | Your Apple Developer Team ID (e.g., `A1B2C3D4E5`) |
| `APPLE_ID` | macOS Notarization | Your Apple ID email address |
| `APPLE_APP_PASSWORD` | macOS Notarization | App-specific password generated at appleid.apple.com |

> **Note:** The release workflow's first macOS step checks that all five secrets are set and fails the job if one is missing (`Require macOS signing and notarization inputs`). Windows has a similar check for its signing inputs. Local `just _package-mac` builds are ad-hoc signed unless `CODESIGN_IDENTITY` is set.

## Architecture

- `crates/bir-core/`: Contains all domain logic, SQLite database integrations, API communications, IMAP automated email tracking, cryptography, and XML generation logic.
  - `forms/` — Form data models, `FormValidator` trait, ATC tax code tables, and the form registry.
  - `db/` — Decomposed database layer with domain-specific modules (`profiles.rs`, `drafts.rs`, `submissions.rs`, `receipts.rs`, `jobs.rs`, `notices.rs`, `migrations.rs`).
- `crates/bir-app-api/`: The UI-free host functions both hosts share: windows, forms, inputs, locking, and theming. Opt-in gpui-agent / headless `bir` runbook: [`docs/agent.md`](agent.md).
- `nsh/`: The eBIRForms app on Native Shell — TypeScript screens in
  `nsh/screens/`, the frame in `nsh/shell/`, the kit in `nsh/ui/`, and the
  Rust host and door in `nsh/src/` (windows, printing and the tray live
  here). See `nsh/AGENTS.md`.
- `crates/bir-print/`: Frozen HTML fill/print, native output coordination, PDF validation, and PDF merging.
- `packages/form-specs/`: Paper/pagination specifications plus migration and release-evidence manifests.
- `html-frozen/`: Committed freeze sheets used for fill/print preview. TIN `name=` stamps come from the identity catalog. 1601C/2551Q leftover identity keys, 2551Q header period fields, and demo tax money rows fill through `writer-cells.json`. Letter combs ASCII-uppercase on the fill path only.

### Form Engine

Adding a new BIR tax form now crosses two separately gated tracks:

1. **Filing support** — authoritative source evidence, typed domain model,
   formulas, validation, XML, persistence, queue submission, and desktop UI.
2. **Print presentation** — frozen HTML in `html-frozen/`, catalog TIN `name=`
   stamps, `writer-cells.json` identity and money fills, and `fill_by_name` from
   `to_bir_field_map()`.
3. **Promotion evidence** — filing support changes only after the in-app gates
   pass; HTML release changes only after freeze verify/stamp checks and
   packaged identity evidence pass.

Each of these three concerns is machine-checked. Freeze `--verify` / `--check-all`
states what a preview sheet must satisfy, `cargo run -p
bir-rules-codegen -- status` states the validation-rules boundary conditions,
and `npm run audit:no-legacy` states what must stay absent. Read those three
rather than any prose description of them.

---

## Reference and Boundary Tooling

- `scripts/reference/` — deterministic form provenance tooling.
  `inventory_form.py` pins an exact revision, `prepare_official_reference.py`
  renders a pinned official PDF into calibration-only rasters, and
  `verify_form_conversion.py` audits a conversion at the `preview` and
  `release` stages. Reference hashes are pinned in Rust.
- `rules/agent-boundaries/` — the fail-closed boundary record for the 2550Q
  candidate rule set. `cargo run -p bir-rules-codegen -- status` asserts on its
  contents, so it is a safety contract rather than documentation.

Narrative design notes, migration research, and agent workflow definitions are
maintained in a separate private repository and are deliberately not published
here. Paths of the form `docs/...` in source comments refer to that repository.
README and runbook examples use standard `npm`, `python3`, and `cargo` commands
so developers can run the workflow without an agent-specific command wrapper.

---

## Development notes

- **Database Location:** `bir_data.db` in the data directory. Override with `BIR_DATABASE_PATH`.
  - macOS: `~/Library/Application Support/dev.goldcoders.bir/bir_data.db`
    (the sandboxed Mac App Store build uses the group container)
  - Windows: `%LOCALAPPDATA%\dev.goldcoders.bir\bir_data.db`
  - Linux: `~/.local/share/dev.goldcoders.bir/bir_data.db`
  - gpui-agent / headless `bir` live path, token, and `--wait` handoff: [`docs/agent.md`](agent.md)
- **Background Engine:** Background cron tasks (auto-fetch) run in-process on a dedicated thread and are decoupled from the active taxpayer profile.
- **Schema Migrations:** Managed via a `schema_version` table with forward-only numbered migrations in `bir-core/src/db/migrations.rs`.
- **Security:** Sensitive credential fields (`imap_app_password`, `oauth_access_token`, `oauth_refresh_token`, `profile_pin_hash`) are zeroed on `Drop` via the `zeroize` crate. Google OAuth for a shared inbox is stored once in `inbox_oauth_tokens` and copied onto every profile that uses that mailbox. Never log raw tokens.
- **Feature Flags:**
  - `dev-tools` — Developer diagnostics: the `--dev-export-live-database` command, and (in a **release** build) permission to open a form whose HTML release certification is still in progress. A debug build already satisfies that gate through `debug_assertions`, so `dev-tools` changes nothing there. Automatically included in `just run`.
  - `agent` — Opt-in gpui-agent control plane for the NSH app and the headless `bir` command (`crates/bir-cli`). It does **not** need `dev-tools`: a debug build with `--features agent` alone binds the agent and drives every form the app can open. Release builds omit the feature unless explicitly enabled; the Mac App Store package (`just _package-mac-appstore`) builds with `mas_build`. Runtime bind requires `GPUI_AGENT=1` **and** `GPUI_AGENT_TOKEN` (protocol v2 HMAC). Release binaries also need `GPUI_AGENT_ALLOW_RELEASE=1`. `GPUI_AGENT_INSECURE_NO_TOKEN=1` is demo-only. There is no `scripts/bir-agent` wrapper; set those env vars and use `gpui-agent`. Authorization, `--wait` handoff, and the AI-agent playbook live in [`docs/agent.md`](agent.md).
- **Tracing:** Debug builds initialize `tracing-subscriber` automatically. Control verbosity with `RUST_LOG` (default: `nsh_app_ebirforms=debug,bir_print=debug,bir_core=info`).
