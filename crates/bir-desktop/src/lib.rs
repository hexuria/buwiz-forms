#![allow(unexpected_cfgs)]
#![allow(clippy::collapsible_if)]
#![allow(clippy::let_unit_value)]
#![allow(unused)]
#![allow(clippy::redundant_pattern_matching)]
//! BIR Desktop library — GPUI app modules plus the headless agent host.
//!
//! The painted binary is `bir`. `bir-headless` (`--features agent`) serves the
//! same `BirAgentHost` over `gpui_agent::from_env` + mailbox/`spawn_host` with
//! no GPU window. Protocol v2: `GPUI_AGENT_TOKEN` is required to bind unless
//! `GPUI_AGENT_INSECURE_NO_TOKEN=1`.

mod actions;
pub mod agent;
mod app;
mod auth_overlays;
#[cfg(target_os = "macos")]
mod certification_evidence;
mod components;
mod cor_evidence;
mod cor_ocr;
pub mod events;
mod gui;
mod ipc;
pub(crate) mod layout_probe;
mod platform;
mod quit_guard;
mod sidebar;
mod theme;
mod views;

pub mod global_actions {
    gpui_kit::actions!(
        bir_desktop,
        [
            SubmitCurrentForm,
            ToggleSidebar,
            ToggleSidebarMini,
            ToggleAppVisibility,
            FocusSearch,
            CreateProfile,
            ToggleTheme,
            OpenCronTasks,
            OpenSettings,
            OpenCommandPalette,
            OpenGlobalDashboard,
            AboutApplication,
            QuitApplication,
            HideApplication,
            HideOthers,
            ShowAllApplications,
            CloseWindow,
            MinimizeWindow,
            ZoomWindow,
            ToggleFullScreen,
            BringAllToFront,
            OpenSupportEmail,
            OpenCompanyWebsite,
            ZoomIn,
            ZoomOut,
            ResetZoom,
            ToggleEditMode,
            SaveLayout,
            EditorNewBox,
            EditorDuplicateBox,
            EditorDeleteBox,
            EditorRenameField,
            EditorSetCharCount,
            EditorFocusSearch,
            EditorEscape,
            EditorNextField,
            EditorPrevField,
            EditorSelectBox1,
            EditorSelectBox2,
            EditorSelectBox3,
            EditorSelectBox4,
            EditorSelectBox5,
            EditorSelectBox6,
            EditorSelectBox7,
            EditorSelectBox8,
            EditorSelectBox9,
            EditorSelectLastBox,
            EditorCycleType,
            EditorToggleDirection,
            EditorNudgeUp,
            EditorNudgeDown,
            EditorNudgeLeft,
            EditorNudgeRight,
            NextPage,
            PrevPage,
            OpacityIncrease,
            OpacityDecrease
        ]
    );
}

pub(crate) use global_actions::*;

pub use gui::run_gui;
