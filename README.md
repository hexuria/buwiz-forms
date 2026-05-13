# E-BIRForms

[![CI](https://github.com/codeitlikemiley/ebirforms/actions/workflows/ci.yml/badge.svg)](https://github.com/codeitlikemiley/ebirforms/actions/workflows/ci.yml)
[![Release](https://github.com/codeitlikemiley/ebirforms/actions/workflows/release.yml/badge.svg)](https://github.com/codeitlikemiley/ebirforms/actions/workflows/release.yml)

A modern, native, and secure desktop application for managing and filing eBIRForms in the Philippines. Built in Rust using the [GPUI](https://gpui.rs/) framework, E-BIRForms delivers a highly responsive, offline-first experience that fully respects your data privacy. E-BIRForms completely reverse-engineers the traditional eBIRForms workflow into a seamless, native Mac, Windows, and Linux application.

---

## 🌟 Key Features

### 🔐 Security & Privacy
- **Touch ID / Biometrics & App Lock**: Lock the application securely with a 4-digit PIN. Leverage native OS-level authentication (Touch ID on macOS, Windows Hello) via Robius Authentication for seamless unlocking.
- **Offline-First & Local Storage**: All profiles, TINs, and drafts are securely encrypted and stored entirely on your local machine using SQLite (`bir_data.db`). 

### 🗄️ Lean & Ephemeral Architecture
- **On-Demand PDF Generation**: Documents (e.g., BIR forms, email confirmation receipts) are generated instantly on-the-fly only when you need to view or print them, completely eliminating the need to store large PDF files in the database.
- **Zero-Bloat Ephemeral Storage**: When previewing generated documents, artifacts are written to a unique temporary directory that is automatically garbage-collected the moment the window closes. The app leaves absolutely zero leftover bloat on your hard drive.
- **Explicit Exporting**: Because files are ephemeral, users maintain total control over persistence by explicitly **Exporting** PDFs to their desired persistent storage locations.

### ⚡️ Power User Capabilities
- **Keyboard Shortcuts**: Built for speed with global shortcuts.
  - `Cmd + Enter`: Submit Current Form
  - `Cmd + N`: Create New Profile
  - `Cmd + B`: Toggle Sidebar
  - `Cmd + F`: Focus Search / Command Palette
  - `Cmd + Shift + X`: Open Cron Tasks
  - `Cmd + K`: Open Command Palette
- **Advanced Easy Filters**: Instantly sort and filter your dashboard by predefined timeframes: **Q1-Q4, Monthly, Yearly**, and by status (Pending, Confirmed, Paid).

### 🤖 Automation & Background Tasks
- **In-Process Background Engine**: A robust cron engine runs background tasks on a dedicated thread within the app — no separate daemon process needed.
- **Auto Fetch & Auto Receipt Tracking**: Integrated IMAP fetching automatically scans your inbox for official BIR confirmation receipts and automatically updates the status of your submitted forms from "Submitted" to "Confirmed" and "Paid".

### 💼 Taxpayer Management
- **Multi-Profile Support**: Seamlessly manage multiple taxpayer profiles from a single unified workspace.
- **Global Dashboard**: A comprehensive overview of all actionable tax deadlines and historical filings across all profiles.
- **Form Generation**: Robust, schema-driven form generation (e.g., 2551Q) mapping directly to official BIR XML standards.

### 🛠 Form Digitization & Developer Tools
- **Visual PDF Layout Editor**: Developers can graphically digitize new BIR forms by dragging, dropping, and resizing field boundaries directly on an SVG overlay. This replaces the slow process of manually nudging coordinates in JSON files, drastically speeding up the integration of new forms.
- **Dev-Only Inspect Button**: A feature-gated `Inspect` button (enabled via `--features dev-tools`) lets developers preview the PDF output without going through the full submission pipeline. It automatically saves the form before rendering to ensure data integrity.
- **Structured Tracing**: Debug builds automatically log form save, sync, and PDF rendering events to the terminal via `tracing`. Override log levels at runtime with `RUST_LOG=bir_desktop=trace just run`.

### 🛡️ Data Integrity
- **Profile Snapshot at Save Time**: When you save a form, it captures your tax profile information at that exact moment. If you later update your profile, the saved form retains the original data — guaranteeing consistency between what was filed and what is stored.
- **Post-Submission Lock**: Once a form is submitted, all fields become read-only. The only way to update data is to revert to Draft status and re-submit.
- **PDF Reflects Saved State**: The PDF Viewer always renders from the last-saved database state — never from unsaved, in-progress edits. This ensures the PDF you see is exactly what was (or will be) submitted.

---

## 🛠 Prerequisites

- **Rust Toolchain**: Ensure you have the latest stable Rust installed.
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

### 🍏 macOS Dependencies
- **Typst** (Required for PDF generation & tests):
  ```bash
  brew install typst
  ```

### 🪟 Windows Dependencies
- **OpenSSL** & **Typst** (Required for SQLCipher, networking, and PDF generation):
  ```powershell
  choco install openssl typst -y
  ```
  *Note: Ensure the `OPENSSL_DIR` environment variable is set to your OpenSSL installation path (e.g., `C:\Program Files\OpenSSL`).*

### 🐧 Linux Dependencies (Ubuntu/Debian)
Building the GPUI frontend and running tests requires various graphic, windowing, and system libraries:
```bash
sudo apt-get update
sudo apt-get install -y \
  pkg-config libx11-dev libxcb1-dev libxcb-render0-dev libxcb-shape0-dev \
  libxcb-xfixes0-dev libxkbcommon-dev libxkbcommon-x11-dev libwayland-dev \
  libwayland-client0 libasound2-dev libudev-dev libvulkan-dev \
  libfontconfig1-dev libfreetype-dev libssl-dev libpolkit-gobject-1-dev \
  mesa-vulkan-drivers libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf
```
- **Typst**: Download the latest Linux binary from the [Typst GitHub Releases](https://github.com/typst/typst/releases) and place it in your `PATH` (e.g., `/usr/local/bin/`).

---

## 🚀 Getting Started

1. **Clone the Repository**
   ```bash
   git clone <repository_url>
   cd bir
   ```

2. **Install `just` (Command Runner)**
   The project is completely standardized via a `justfile` for cross-platform simplicity. You will need `just` to build, run, and package the app.
   - **macOS:** `brew install just`
   - **Windows:** `choco install just`
   - **Linux / Cargo:** `cargo install just`

3. **Install Developer Tools (Optional but Recommended)**
   We use a few standard Cargo extensions for maintaining code quality, dependencies, and security:
   ```bash
   cargo install cargo-audit cargo-outdated cargo-machete
   ```

4. **Build and Run**
   To see all available commands, simply run:
   ```bash
   just help
   ```

   **To run the application locally (development mode):**
   ```bash
   just run
   ```

---

## 🏗 Available Commands

We follow a "less is better" philosophy. You only need to remember a few core commands:

- `just run` — Run the app locally for development (automatically enables `dev-tools` and `layout-editor` features).
- `just install` — Automatically figure out your OS and build the installer package (macOS DMG, Windows Zip, Linux DEB/Tarball).
- `DEV_MODE=true just install --layout-editor --inspector` — Compiles a production-ready package with internal developer tools unlocked.
- `just publish` — Auto-increment the patch version, tag, and push (triggers the release workflow in GitHub Actions).

**Quality & Testing:**
- `just check` — Run code formatting (`cargo fmt`), linting (`cargo clippy`), and type checking (`cargo check`) across the workspace.
- `just test` — Run all unit and integration test suites.

**Dependency Management:**
- `just audit` — Check for security vulnerabilities in dependencies using the RustSec database.
- `just outdated` — Display a list of dependencies that have newer versions available.
- `just unused` — Scan `Cargo.toml` for dependencies that are no longer being used.

---

## 🔢 Versioning & Build Numbers

The project uses **two independent version identifiers** to satisfy both Apple App Store and Microsoft Store requirements:

| Identifier | Source of Truth | Changed By | When |
|---|---|---|---|
| **App Version** (semver) | `Cargo.toml` → `version = "0.1.0"` | You, manually | Feature release or hotfix |
| **Build Number** (counter) | `justfile` → `BUILD_NUMBER := "21"` | `just bump-build` (auto) | Every store submission |

### Why Two Numbers?

Apple and Microsoft handle versioning differently:

| | macOS App Store | Microsoft Store |
|---|---|---|
| Marketing version | `CFBundleShortVersionString = "0.1.0"` | Store listing description |
| Build identifier | `CFBundleVersion = "21"` (separate field) | `Version = "0.1.21.0"` in AppxManifest |
| Multiple builds per version? | ✅ Yes | ❌ No — each submission needs a higher `Version` |

**macOS** has two separate fields — one for the user-facing version, one for the internal build counter. **Microsoft** has only a single `Version` field, and the 4th part (Revision) must always be `0`.

### How the Version Maps to Each Store

Given `Cargo.toml version = "0.1.0"` and `BUILD_NUMBER = 22`:

```
macOS App Store:
  CFBundleShortVersionString = "0.1.0"    ← what users see
  CFBundleVersion            = "22"       ← internal build number

Microsoft Store:
  AppxManifest Version       = "0.1.22.0" ← Major.Minor.BuildNumber.0
  Store listing              = "0.1.0"    ← what users see (set in Partner Center)
```

### Developer Workflow

**Submitting a new build** (most common):
```bash
just bump-build    # Queries App Store Connect API, increments BUILD_NUMBER in justfile
git add justfile && git commit -m "build: bump to $(just --evaluate BUILD_NUMBER)"
# On macOS:
just app           # Builds .app with version 0.1.0 (build 23)
# On Windows:
just msix          # Builds .msix with version 0.1.23.0
```

**Releasing a new version** (feature release or hotfix):
```bash
# 1. Manually edit Cargo.toml: version = "0.2.0"
# 2. Then bump and build as usual:
just bump-build
git add Cargo.toml justfile && git commit -m "release: v0.2.0"
```

> **Note:** `just bump-build` only works on macOS — it requires the Apple App Store Connect API credentials (`.p8` key, `APP_STORE_ISSUER_ID`, `APP_STORE_KEY_ID` in `.env`). On Windows, simply `git pull` to get the latest `BUILD_NUMBER` from the justfile, then run `just msix`.

### Version Progression Example

| Action | Cargo.toml | BUILD_NUMBER | macOS | Windows MSIX |
|---|---|---|---|---|
| Initial | `0.1.0` | 21 | `0.1.0` (build 21) | `0.1.21.0` |
| Bump build | `0.1.0` | 22 | `0.1.0` (build 22) | `0.1.22.0` |
| Bump build | `0.1.0` | 23 | `0.1.0` (build 23) | `0.1.23.0` |
| Hotfix | `0.1.1` | 24 | `0.1.1` (build 24) | `0.1.24.0` |
| Feature release | `0.2.0` | 25 | `0.2.0` (build 25) | `0.2.25.0` |
| Stable release | `1.0.0` | 30 | `1.0.0` (build 30) | `1.0.30.0` |

## 📂 Architecture

- `crates/bir-core/`: Contains all domain logic, SQLite database integrations, API communications, IMAP automated email tracking, cryptography, and XML generation logic.
  - `forms/` — Form data models, `FormValidator` trait, ATC tax code tables, and the form registry.
  - `db/` — Decomposed database layer with domain-specific modules (`profiles.rs`, `drafts.rs`, `submissions.rs`, `receipts.rs`, `jobs.rs`, `notices.rs`, `migrations.rs`).
- `crates/bir-desktop/`: The GPUI-based frontend application managing windows, forms, inputs, locking, and theming.
  - `components/form_engine.rs` — `FormViewTrait` providing shared status pipeline, header, and action infrastructure for all tax forms.
  - `components/form_parts.rs` — Reusable UI primitives: `form_accordion`, `taxpayer_info_section`, `atc_schedule_table`, `computation_row_*`, `penalty_summary_section`, and more.
  - `views/` — Per-form view implementations (e.g., `form_2551q_view.rs`, `form_1701q_view.rs`) that compose the shared components.
- `crates/bir-print/`: High-performance PDF generation and native OS printing integrations.
- `crates/gpui-component/`: A centralized design system and UI toolkit customized exclusively for GPUI.

### 🧩 Form Engine

Adding a new BIR tax form follows a standardized 4-phase lifecycle:

1. **Core Model** (`bir-core/forms/`) — Draft struct + `FormValidator` + optional `recompute()`
2. **Database** (`bir-core/db/`) — Migration + CRUD methods for draft persistence
3. **UI View** (`bir-desktop/views/`) — Implement `FormViewTrait` + compose UI from `form_parts`
4. **Submission** — XML field mapping + PDF rendering

See [docs/adding-a-new-form.md](docs/adding-a-new-form.md) for the comprehensive developer guide with code examples and a 16-step checklist.

---

## 🧠 Agent Skills

This repository uses **Project-Local Skills** to automatically equip AI agents (like Antigravity or Gemini) with project-specific capabilities without requiring manual installation.

- **Location:** All custom tools, scripts, and instructions are stored in the `.agent/skills/` directory within the repository (e.g., `.agent/skills/form-generator`).
- **How It Works:** When you clone this repository on a new machine, the agent system will automatically discover the `.agent/skills/` folder at the root. Project-local skills take precedence over any global skills installed on your machine.
- **Portability:** Because the skills are committed to version control, you don't need to run `npx skills add` or install anything globally. The agent will automatically recognize the `form-generator` skill (and any others) as soon as it interacts with this repository.

---

## [Download Forms](https://www.bir.gov.ph/bir-forms)

## 📜 Development Notes

- **Database Location:** 
  - macOS: `~/Library/Application Support/Taxman/eBIRForms/bir_data.db`
  - Linux/Windows: `~/.taxman-ebir/bir_data.db`
- **Background Engine:** Background cron tasks (auto-fetch) run in-process on a dedicated thread and are decoupled from the active taxpayer profile.
- **Schema Migrations:** Managed via a `schema_version` table with forward-only numbered migrations in `bir-core/src/db/migrations.rs`.
- **Security:** Sensitive credential fields (`imap_app_password`, `oauth_access_token`, `oauth_refresh_token`, `profile_pin_hash`) are zeroed on `Drop` via the `zeroize` crate.
- **Feature Flags:**
  - `dev-tools` — Enables the Inspect button and additional developer diagnostics. Automatically included in `just run`.
  - `layout-editor` — Enables the PDF Layout Editor view for form digitization. Automatically included in `just run`.
- **Tracing:** Debug builds initialize `tracing-subscriber` automatically. Control verbosity with `RUST_LOG` (default: `bir_desktop=debug,bir_print=debug,bir_core=info`).

