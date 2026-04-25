# BIR Vault (eBIRForms Helper)

A modern, fast, and secure desktop application for managing and filing eBIRForms in the Philippines. Built in Rust using the [GPUI](https://gpui.rs/) framework for a native, responsive experience.

## Features

- **Taxpayer Profile Management**: Securely store and manage multiple taxpayer profiles locally.
- **Global Dashboard**: Unified view of all actionable tax forms across your profiles.
- **Compliance Calendar**: Track your upcoming tax deadlines and historical filings.
- **Form Generation**: Easy-to-use form filling (e.g. 2551Q, 1701Q) with automatic field computations.
- **Local Database**: All data, including encrypted form drafts and profiles, is saved securely on your local machine (`bir_data.db`).
- **Automated Filing Feedback**: Track the status of your submitted forms, up to "Paid" and "Confirmed" through receipt email parsing.
- **BIR News & Advisories**: Built-in news fetcher to keep you up to date on RDO advisories and tax deadlines.

## Prerequisites

- **Rust Toolchain**: You need the latest stable Rust installed. Install via [rustup](https://rustup.rs/):
  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```
- **macOS (Recommended)**: GPUI is currently optimized for macOS, though Linux and Windows support is available/experimental.

## Getting Started

1. **Clone the Repository**
   ```bash
   git clone <repository_url>
   cd bir
   ```

2. **Build and Run**
   The project is organized as a Cargo workspace with `bir-core` (business logic) and `bir-desktop` (UI).
   
   To run the application:
   ```bash
   cargo run --release -p bir-desktop
   ```
   
   To run in development/debug mode (which includes mock data generation tools in the sidebar):
   ```bash
   cargo run -p bir-desktop
   ```

## Architecture

- `crates/bir-core/`: Contains all the domain logic, SQLite database integration (`rusqlite`), API communication, email IMAP tracking, and XML generation for eBIRForms.
- `crates/bir-desktop/`: Contains the GPUI-based frontend, managing windows, states, rendering components, and handling user input.
- `crates/bir-print/`: Module for generating PDF prints for forms.
- `crates/gpui-component/`: A custom library of pre-built UI components customized for the GPUI framework.

## Data Privacy & Security

**Offline Secure**: By default, BIR Vault works entirely offline.
- Your profiles, TINs, and form data never leave your computer unless you explicitly submit a form.
- The local SQLite database is stored at: `~/Library/Application Support/Taxman/eBIRForms/bir_data.db` (on macOS) or `~/.taxman-ebir/bir_data.db` (on Linux/Windows).
- Sensitive operations like uploading XML files use secure encryption mechanisms before transmitting to the eBIRForms platform.

## Development Commands

**Format Code**
```bash
cargo fmt
```

**Check Compilation**
```bash
cargo check -p bir-desktop -p bir-core
```

**Run Unit Tests**
```bash
cargo test -p bir-core
```
