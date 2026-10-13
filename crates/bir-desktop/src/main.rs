#![allow(unexpected_cfgs)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::let_unit_value)]
#![allow(unused)]
#![allow(clippy::redundant_pattern_matching)]
// Suppress the console window on Windows release builds.
// Debug builds retain the console for tracing output.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
//! BIR Desktop Application — GPUI-powered tax filing interface.

#[cfg(target_os = "windows")]
fn main() {
    bir_desktop::prefer_hwnd_swap_chain();
    bir_desktop::run_gui();
}

#[cfg(not(target_os = "windows"))]
fn main() {
    bir_desktop::run_gui();
}
