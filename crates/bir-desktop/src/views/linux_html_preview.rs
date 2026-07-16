//! Linux-owned HTML form preview host.
//!
//! Wry's raw child WebView only supports X11.  On Wayland this module uses a
//! separate application-owned GTK/WebKit window instead of handing the form to
//! an external browser.  Both hosts load the same immutable render envelope,
//! local protocol, initialization guard, geometry preflight, and WebKit print
//! operation.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxDisplayServer {
    X11,
    Wayland,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinuxHtmlHostStrategy {
    /// A Wry child attached to the GPUI preview window.  Wry documents this
    /// path as X11-only.
    GpuiWryChild,
    /// A separate eBIRForms-owned GTK window containing WebKitGTK.  This is
    /// the supported Wayland path and is never an external browser.
    GtkTopLevel,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxDisplayEnvironment {
    pub xdg_session_type: Option<String>,
    pub wayland_display: Option<String>,
    pub display: Option<String>,
    pub host_override: Option<String>,
}

impl LinuxDisplayEnvironment {
    #[cfg(target_os = "linux")]
    fn from_process() -> Self {
        Self {
            xdg_session_type: non_empty_env("XDG_SESSION_TYPE"),
            wayland_display: non_empty_env("WAYLAND_DISPLAY"),
            display: non_empty_env("DISPLAY"),
            host_override: non_empty_env("EBIRFORMS_HTML_LINUX_HOST"),
        }
    }

    fn detected_server(&self) -> Option<LinuxDisplayServer> {
        match self
            .xdg_session_type
            .as_deref()
            .map(str::trim)
            .map(str::to_ascii_lowercase)
            .as_deref()
        {
            Some("wayland") => return Some(LinuxDisplayServer::Wayland),
            Some("x11" | "xorg") => return Some(LinuxDisplayServer::X11),
            _ => {}
        }

        // A Wayland session commonly exposes DISPLAY as an XWayland bridge.
        // Prefer native Wayland whenever WAYLAND_DISPLAY is present.
        if self.wayland_display.as_deref().is_some_and(is_non_empty) {
            Some(LinuxDisplayServer::Wayland)
        } else if self.display.as_deref().is_some_and(is_non_empty) {
            Some(LinuxDisplayServer::X11)
        } else {
            None
        }
    }
}

fn is_non_empty(value: &str) -> bool {
    !value.trim().is_empty()
}

#[cfg(target_os = "linux")]
fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| is_non_empty(value.as_str()))
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub enum LinuxHostSelectionError {
    #[error("Linux HTML preview requires an X11 or Wayland display")]
    MissingDisplay,
    #[error("unsupported EBIRFORMS_HTML_LINUX_HOST value `{0}`; expected `child` or `gtk`")]
    InvalidOverride(String),
    #[error("the Wry child host is X11-only; use the GTK host on Wayland")]
    ChildRequestedOnWayland,
}

pub fn select_linux_html_host(
    environment: &LinuxDisplayEnvironment,
) -> Result<LinuxHtmlHostStrategy, LinuxHostSelectionError> {
    let display_server = environment
        .detected_server()
        .ok_or(LinuxHostSelectionError::MissingDisplay)?;

    match environment
        .host_override
        .as_deref()
        .map(str::trim)
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("gtk" | "top-level" | "toplevel") => Ok(LinuxHtmlHostStrategy::GtkTopLevel),
        Some("child") if display_server == LinuxDisplayServer::X11 => {
            Ok(LinuxHtmlHostStrategy::GpuiWryChild)
        }
        Some("child") => Err(LinuxHostSelectionError::ChildRequestedOnWayland),
        Some(other) => Err(LinuxHostSelectionError::InvalidOverride(other.to_string())),
        None => match display_server {
            LinuxDisplayServer::X11 => Ok(LinuxHtmlHostStrategy::GpuiWryChild),
            LinuxDisplayServer::Wayland => Ok(LinuxHtmlHostStrategy::GtkTopLevel),
        },
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxHostLifecycle {
    Starting { strategy: LinuxHtmlHostStrategy },
    Ready { strategy: LinuxHtmlHostStrategy },
    Closing { strategy: LinuxHtmlHostStrategy },
    Closed,
    Failed(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinuxHostLifecycleEvent {
    Started,
    CloseRequested,
    Closed,
    Failed(String),
}

impl LinuxHostLifecycle {
    pub fn transition(self, event: LinuxHostLifecycleEvent) -> Result<Self, LinuxLifecycleError> {
        match (self, event) {
            (Self::Starting { strategy }, LinuxHostLifecycleEvent::Started) => {
                Ok(Self::Ready { strategy })
            }
            (Self::Starting { strategy }, LinuxHostLifecycleEvent::CloseRequested)
            | (Self::Ready { strategy }, LinuxHostLifecycleEvent::CloseRequested) => {
                Ok(Self::Closing { strategy })
            }
            (Self::Closing { .. }, LinuxHostLifecycleEvent::Closed) => Ok(Self::Closed),
            (
                Self::Starting { .. } | Self::Ready { .. } | Self::Closing { .. },
                LinuxHostLifecycleEvent::Failed(reason),
            ) => Ok(Self::Failed(reason)),
            (state, event) => Err(LinuxLifecycleError { state, event }),
        }
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
#[error("invalid Linux HTML host lifecycle transition from {state:?} using {event:?}")]
pub struct LinuxLifecycleError {
    state: LinuxHostLifecycle,
    event: LinuxHostLifecycleEvent,
}

/// Environment snapshot used by diagnostics and packaged X11/Wayland smoke
/// drivers.  Values are deliberately limited to display selection and never
/// include unrelated process environment data.
pub fn linux_display_diagnostics(
    environment: &LinuxDisplayEnvironment,
) -> BTreeMap<&'static str, String> {
    let mut diagnostics = BTreeMap::new();
    if let Some(value) = environment.xdg_session_type.as_ref() {
        diagnostics.insert("xdg_session_type", value.clone());
    }
    if let Some(value) = environment.wayland_display.as_ref() {
        diagnostics.insert("wayland_display", value.clone());
    }
    if let Some(value) = environment.display.as_ref() {
        diagnostics.insert("display", value.clone());
    }
    if let Some(value) = environment.host_override.as_ref() {
        diagnostics.insert("host_override", value.clone());
    }
    diagnostics
}

#[cfg(target_os = "linux")]
mod runtime;

#[cfg(target_os = "linux")]
pub(crate) use runtime::{
    LinuxEmbeddedHtmlPreviewView, LinuxHtmlPreviewError, LinuxHtmlPreviewLaunch,
    launch_linux_html_preview,
};

#[cfg(test)]
mod tests {
    use super::*;

    fn environment(
        session: Option<&str>,
        wayland: Option<&str>,
        x11: Option<&str>,
        override_value: Option<&str>,
    ) -> LinuxDisplayEnvironment {
        LinuxDisplayEnvironment {
            xdg_session_type: session.map(str::to_string),
            wayland_display: wayland.map(str::to_string),
            display: x11.map(str::to_string),
            host_override: override_value.map(str::to_string),
        }
    }

    #[test]
    fn wayland_uses_the_application_owned_gtk_window_even_with_xwayland() {
        let environment = environment(Some("wayland"), Some("wayland-0"), Some(":0"), None);
        assert_eq!(
            select_linux_html_host(&environment),
            Ok(LinuxHtmlHostStrategy::GtkTopLevel)
        );
    }

    #[test]
    fn x11_uses_the_gpui_wry_child() {
        let environment = environment(Some("x11"), None, Some(":0"), None);
        assert_eq!(
            select_linux_html_host(&environment),
            Ok(LinuxHtmlHostStrategy::GpuiWryChild)
        );
    }

    #[test]
    fn missing_display_fails_closed() {
        let environment = environment(None, None, None, None);
        assert_eq!(
            select_linux_html_host(&environment),
            Err(LinuxHostSelectionError::MissingDisplay)
        );
    }

    #[test]
    fn child_override_is_rejected_on_wayland() {
        let environment = environment(Some("wayland"), Some("wayland-0"), None, Some("child"));
        assert_eq!(
            select_linux_html_host(&environment),
            Err(LinuxHostSelectionError::ChildRequestedOnWayland)
        );
    }

    #[test]
    fn lifecycle_covers_startup_and_shutdown_without_skipping_states() {
        let lifecycle = LinuxHostLifecycle::Starting {
            strategy: LinuxHtmlHostStrategy::GtkTopLevel,
        }
        .transition(LinuxHostLifecycleEvent::Started)
        .expect("the host can become ready")
        .transition(LinuxHostLifecycleEvent::CloseRequested)
        .expect("the ready host can start closing")
        .transition(LinuxHostLifecycleEvent::Closed)
        .expect("the closing host can close");
        assert_eq!(lifecycle, LinuxHostLifecycle::Closed);
    }

    #[test]
    fn lifecycle_rejects_an_unordered_shutdown() {
        let error = LinuxHostLifecycle::Starting {
            strategy: LinuxHtmlHostStrategy::GpuiWryChild,
        }
        .transition(LinuxHostLifecycleEvent::Closed)
        .expect_err("a starting host cannot jump directly to closed");
        assert!(
            error
                .to_string()
                .contains("invalid Linux HTML host lifecycle")
        );
    }
}
