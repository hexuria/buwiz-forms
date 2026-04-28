# E-BIRForms

A modern, native, and secure desktop application for managing and filing eBIRForms in the Philippines. Built in Rust using the [GPUI](https://gpui.rs/) framework, E-BIRForms delivers a highly responsive, offline-first experience that fully respects your data privacy. E-BIRForms completely reverse-engineers the traditional eBIRForms workflow into a seamless, native Mac, Windows, and Linux application.

---

## 🌟 Key Features

### 🔐 Security & Privacy
- **Touch ID / Biometrics & App Lock**: Lock the application securely with a 4-digit PIN. Leverage native OS-level authentication (Touch ID on macOS, Windows Hello) via Robius Authentication for seamless unlocking.
- **Offline-First & Local Storage**: All profiles, TINs, and drafts are securely encrypted and stored entirely on your local machine using SQLite (`bir_data.db`). 

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
- **Cron Jobs & Background Service**: A robust standalone daemon (`bir-daemon`) handles background tasks without needing the main UI open.
- **Auto Fetch & Auto Receipt Tracking**: Integrated IMAP fetching automatically scans your inbox for official BIR confirmation receipts and automatically updates the status of your submitted forms from "Submitted" to "Confirmed" and "Paid".

### 💼 Taxpayer Management
- **Multi-Profile Support**: Seamlessly manage multiple taxpayer profiles from a single unified workspace.
- **Global Dashboard**: A comprehensive overview of all actionable tax deadlines and historical filings across all profiles.
- **Form Generation**: Robust, schema-driven form generation (e.g., 2551Q) mapping directly to official BIR XML standards.

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

2. **Build and Run using Make**
   The project is completely standardized via a `Makefile`. We rarely use raw `cargo` commands unless doing specific deep-dives.

   To see all available commands, run:
   ```bash
   make help
   ```

   **To run the application (development mode):**
   ```bash
   cargo run -p bir-desktop
   ```
   *(Note: The main entry points are `bir-desktop` for the UI and `bir-daemon` for background tasks).*

---

## 🏗 Available Make Commands

- `make check`: Run `cargo check` across the entire workspace.
- `make clippy`: Run strict linting.
- `make test`: Run all test suites.
- `make build-mac`: Build a release for your current macOS architecture.
- `make build-mac-universal`: Build a Universal Binary (ARM64 + x86_64) for macOS.
- `make build-win`: Build release for Windows.
- `make build-linux`: Build release for Linux.
- `make package-mac`: Package the `.app` bundle and generate a `.dmg`.
- `make sign-mac`: Sign and notarize the macOS application for distribution.
- `make clean`: Clean release artifacts and cargo cache.

---

## 📂 Architecture

- `crates/bir-core/`: Contains all domain logic, SQLite database integrations, API communications, IMAP automated email tracking, cryptography, and XML generation logic.
- `crates/bir-desktop/`: The GPUI-based frontend application managing windows, forms, inputs, locking, and theming.
- `crates/bir-print/`: High-performance PDF generation and native OS printing integrations.
- `crates/gpui-component/`: A centralized design system and UI toolkit customized exclusively for GPUI.

---

## 📜 Development Notes

- **Database Location:** 
  - macOS: `~/Library/Application Support/Taxman/eBIRForms/bir_data.db`
  - Linux/Windows: `~/.taxman-ebir/bir_data.db`
- **Background Daemon:** Background cron tasks (auto-fetch) are decoupled from the active taxpayer profile and can be managed globally in the settings.
