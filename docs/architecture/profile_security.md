# Taxpayer Profile Architecture & Security

This document outlines the architectural patterns, security gates, and validation strategies implemented for managing Taxpayer Profiles within the eBIRForms desktop application.

## 1. Public Mode & Session Management

To support secure deployments in public environments (e.g., public accountants, internet cafes, shared office spaces), the application employs a strict **Active Session Architecture**.

### Hide Tax Profiles (Public Mode)
When `Enable Hide Tax Profiles from Sidebar` is active globally:
- All non-active profiles are hidden from the sidebar.
- The Command Palette search requires an **exact 12- or 13-digit TIN match** to locate and unlock an existing profile.
- Switching to a new profile via exact TIN match explicitly ends the previous session and establishes the new profile as the `active_session_tin`.

### Explicit Session Termination
- An **"Exit Session"** button is provided on the dashboard (represented by a logout icon) to immediately drop the `active_session_tin` from state, hiding the profile.
- Clicking the **"+" (Create Profile)** button in the sidebar explicitly clears the active session to prevent data leakage between the previous session and the new unsaved form context.

---

## 2. Profile Creation & Security Gates

Profile persistence is governed by strict, synchronous "Validation Gates" applied before any database interaction occurs.

### A. The Profile PIN Gate
If `Enable Profile PINs` is globally activated in the application settings:
- The "Secure this Profile with a PIN" checkbox is **automatically enabled** for all new profile creations.
- **Validation**: Attempting to save a profile without providing a valid 4-digit PIN will block the save, emit a toaster error, and automatically navigate the user to the "Security" tab.

### B. The Email Authentication Gate
If the application is globally configured to run automated background services (cron jobs):
- **Default Method**: All new profiles default to **Google OAuth2** as the preferred email authentication method.
- **In-Memory Pairing**: Users can authorize Google OAuth2 *before* saving the profile. Access and Refresh tokens are held in-memory and synced to the database exclusively upon a successful save.
- **Validation**: If background tracking is enabled, saving the profile requires a paired email. Failure to do so blocks the save, emits an error toaster, and redirects the user to the "Email Settings" tab.

---

## 3. Defense-in-Depth: TIN Uniqueness

Taxpayer Identification Numbers (TINs) serve as the primary key for all taxpayer records. Creating duplicate TINs is structurally impossible due to a 3-layer validation pipeline:

### Layer 1: Real-Time UI Feedback
The `ProfileManagerView` listens to events emitted by the `TinInput` component.
- The moment a valid 12 or 13-digit format is entered, the UI checks the database asynchronously.
- If a collision is detected during the creation of a *new* profile, a red warning box appears dynamically underneath the input:  
  `⚠ A profile with TIN 123-456-789-000 already exists. Each TIN must be unique.`

### Layer 2: Save-Time Validation
Even if the UI layer is bypassed, the `save_profile` execution blocks persistence.
- It re-queries the database for the TIN.
- If a match is found on a profile that lacks an `editing_id`, it appends a structural `ValidationError` to the form state, halting execution and notifying the user.

### Layer 3: Database Engine Constraint
The foundational `db.rs` implementation strictly forbids hijacking.
- The SQLite table is constructed with `tin TEXT UNIQUE NOT NULL`.
- If an `insert` or `update` attempts to write an existing TIN without explicitly targeting the existing row's `id`, the database layer throws a `rusqlite::ErrorCode::ConstraintViolation` rather than silently overwriting the target profile.

---

## 4. Background Services (Daemon)

The execution of automated tasks (such as email receipt parsing and IMAP polling) is governed globally but runs contextually per profile.

- **Unified Control**: The per-profile UI checkbox for "Enable Automated Background Services" was removed. Background daemon rules are now implicitly derived from the global App settings to enforce strict compliance.
- **Daemon Lifecycle**: During `save_profile()`, the application evaluates if *any* profile within the database requires background polling. If true, the system invokes `bir_core::daemon_installer::install()` to register the OS-level agent. If false, it invokes `bir_core::daemon_installer::uninstall()`.
