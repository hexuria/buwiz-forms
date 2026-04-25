# Agent Instructions for `bir` (eBIR Forms Desktop App)

This document contains critical system context, architecture rules, and hard-earned lessons for any AI agent interacting with this repository. Read this completely before attempting any code modifications.

## 1. Project Architecture
- **Framework:** GPUI (`gpui` + `gpui-component`).
- **Structure:** Modular Cargo Workspace.
  - `bir-core`: Pure Rust data models, SQLite encryption, XML parsing, ZERO UI dependencies.
  - `bir-desktop`: The GPUI frontend application.

## 2. GPUI Specific Rules (CRITICAL)
GPUI is a fast-moving UI framework. You must adhere to the specific version API locked in this workspace to prevent fatal compiler errors.

### Stable Rust ONLY
GPUI historically required Rust Nightly. **This is no longer true.** This workspace uses **Stable Rust**. Do not attempt to add `rust-toolchain.toml` with nightly channels. Nightly will break `pathfinder_simd` and other graphics dependencies on Apple Silicon.

### The `Entity` vs `View` Paradigm
In this specific version of GPUI, the core reactive state wrapper is `Entity<T>`, **NOT** `View<T>`.
- **Incorrect:** `dashboard: View<DashboardView>`
- **Correct:** `dashboard: Entity<DashboardView>`

### Creating Views
Views are constructed inside context closures using `cx.new()` (not `cx.new_view` or `cx.build_view`).
- **Incorrect:** `let dashboard = cx.new_view(|cx| DashboardView::new());`
- **Correct:** `let dashboard = cx.new(|_| DashboardView::new());`

### Styling and `StyledExt` Trait
GPUI uses Tailwind-like styling primitives (e.g., `.w_full()`, `.bg()`, `.flex()`). These methods do NOT exist on the base components like `div()`. They are injected via traits.
- **Rule:** If the compiler says `no method named 'w_full' found for struct 'gpui::Div'`, you are missing the trait import.
- **Fix:** ALWAYS include `use gpui_component::StyledExt;` at the top of your view files.

### Match Arms and `AnyElement`
Because `div()` returns a `Div` type and not a generic element trait, `match` statements returning different view components will fail with mismatched types.
- **Fix:** Always append `.into_any_element()` to the end of your elements inside match branches.

## 3. Rust Best Practices for `bir`
- **Error Handling:** Use `thiserror` for library crates (`bir-core`) to define explicit, recoverable errors. Use `anyhow` for the application binary (`bir-desktop`) for bubbling up opaque errors.
- **Cloning Views:** GPUI `Entity<T>` is cheaply cloneable. Do not pass references to entities around; clone them.
- **Security:** Sensitive data (TINs, Profiles, Form Drafts) must use `zeroize` when in memory and AES-256-GCM when written to SQLite.

## 4. Design Guidelines
- **Aesthetic:** We use a modern, sleek, dark-themed UI (frosted glass, `rgba` hover states, premium rounded borders).
- **Colors:** Use explicit `gpui::rgb(0xHEX)` or `gpui::rgba(0xHEX, alpha)` for styling. Note that `rgba()` takes ONE argument (the hex + alpha combined like `0xffffff1a`) or use `rgba(0xffffff, 0.1)` if the specific `gpui_component` wrapper allows it, but verify compiler output.
