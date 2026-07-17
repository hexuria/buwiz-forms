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

const REQUIRED_FORM_WIDTH_POINTS: f64 = 612.0;
const REQUIRED_FORM_HEIGHT_POINTS: f64 = 936.0;

/// Immutable GTK/WebKit print contract for the 8.5 x 13 inch BIR paper used by
/// the HTML renderer. Keeping this part independent of GTK makes the required
/// geometry testable on every development host, including macOS CI.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct LinuxNativePrintContract {
    pub width_points: f64,
    pub height_points: f64,
    pub margin_top_points: f64,
    pub margin_right_points: f64,
    pub margin_bottom_points: f64,
    pub margin_left_points: f64,
    pub scale_percent: f64,
    pub print_backgrounds: bool,
    /// WebKitGTK's `WebKitPrintOperation` exposes page setup and GTK print
    /// settings, but no browser-generated header/footer setting. The backend
    /// must not invent an unsupported key; native output evidence must verify
    /// the resulting document rather than claiming suppression from an API that
    /// does not exist.
    pub webkitgtk_exposes_header_footer_control: bool,
}

#[derive(Debug, thiserror::Error, Clone, PartialEq)]
pub(super) enum LinuxNativePrintContractError {
    #[error(
        "Linux HTML output paper geometry must be finite and positive; got {width_points} x {height_points} points"
    )]
    InvalidGeometry {
        width_points: f64,
        height_points: f64,
    },
    #[error(
        "Linux HTML output requires exactly 612 x 936 point (8.5 x 13 inch) paper; got {width_points} x {height_points} points"
    )]
    UnsupportedPaper {
        width_points: f64,
        height_points: f64,
    },
}

pub(super) fn linux_native_print_contract(
    width_points: f64,
    height_points: f64,
) -> Result<LinuxNativePrintContract, LinuxNativePrintContractError> {
    if !width_points.is_finite()
        || !height_points.is_finite()
        || width_points <= 0.0
        || height_points <= 0.0
    {
        return Err(LinuxNativePrintContractError::InvalidGeometry {
            width_points,
            height_points,
        });
    }
    if width_points != REQUIRED_FORM_WIDTH_POINTS || height_points != REQUIRED_FORM_HEIGHT_POINTS {
        return Err(LinuxNativePrintContractError::UnsupportedPaper {
            width_points,
            height_points,
        });
    }

    Ok(LinuxNativePrintContract {
        width_points,
        height_points,
        margin_top_points: 0.0,
        margin_right_points: 0.0,
        margin_bottom_points: 0.0,
        margin_left_points: 0.0,
        scale_percent: 100.0,
        print_backgrounds: true,
        webkitgtk_exposes_header_footer_control: false,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LinuxRendererRetryAction {
    Hidden,
    Disabled,
    Enabled,
}

pub(super) fn linux_renderer_retry_action(
    renderer_failed: bool,
    webview_available: bool,
) -> LinuxRendererRetryAction {
    match (renderer_failed, webview_available) {
        (false, _) => LinuxRendererRetryAction::Hidden,
        (true, false) => LinuxRendererRetryAction::Disabled,
        (true, true) => LinuxRendererRetryAction::Enabled,
    }
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
    fn gtk_retry_is_enabled_only_after_a_renderer_failure() {
        assert_eq!(
            linux_renderer_retry_action(false, true),
            LinuxRendererRetryAction::Hidden
        );
        assert_eq!(
            linux_renderer_retry_action(true, true),
            LinuxRendererRetryAction::Enabled
        );
    }

    #[test]
    fn x11_retry_is_disabled_when_the_child_webview_is_unavailable() {
        assert_eq!(
            linux_renderer_retry_action(true, false),
            LinuxRendererRetryAction::Disabled
        );
        assert_eq!(
            linux_renderer_retry_action(true, true),
            LinuxRendererRetryAction::Enabled
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

    #[test]
    fn native_print_contract_is_exact_folio_with_zero_margins() {
        let contract = linux_native_print_contract(8.5 * 72.0, 13.0 * 72.0)
            .expect("the required BIR paper is accepted");

        assert_eq!(contract.width_points, 612.0);
        assert_eq!(contract.height_points, 936.0);
        assert_eq!(contract.margin_top_points, 0.0);
        assert_eq!(contract.margin_right_points, 0.0);
        assert_eq!(contract.margin_bottom_points, 0.0);
        assert_eq!(contract.margin_left_points, 0.0);
        assert_eq!(contract.scale_percent, 100.0);
        assert!(contract.print_backgrounds);
        assert!(!contract.webkitgtk_exposes_header_footer_control);
    }

    #[test]
    fn native_print_contract_rejects_other_or_non_finite_paper() {
        assert!(matches!(
            linux_native_print_contract(612.0, 792.0),
            Err(LinuxNativePrintContractError::UnsupportedPaper { .. })
        ));
        assert!(matches!(
            linux_native_print_contract(f64::NAN, 936.0),
            Err(LinuxNativePrintContractError::InvalidGeometry { .. })
        ));
        assert!(matches!(
            linux_native_print_contract(612.0, f64::INFINITY),
            Err(LinuxNativePrintContractError::InvalidGeometry { .. })
        ));
    }

    #[test]
    fn explicit_x11_session_wins_over_a_stale_wayland_environment() {
        let environment = environment(Some("x11"), Some("wayland-0"), Some(":0"), None);
        assert_eq!(
            select_linux_html_host(&environment),
            Ok(LinuxHtmlHostStrategy::GpuiWryChild)
        );
    }

    #[test]
    fn x11_can_explicitly_use_the_application_owned_gtk_host() {
        let environment = environment(Some("x11"), None, Some(":0"), Some(" GTK "));
        assert_eq!(
            select_linux_html_host(&environment),
            Ok(LinuxHtmlHostStrategy::GtkTopLevel)
        );
    }

    #[test]
    fn invalid_host_override_fails_closed() {
        let environment = environment(Some("x11"), None, Some(":0"), Some("browser"));
        assert_eq!(
            select_linux_html_host(&environment),
            Err(LinuxHostSelectionError::InvalidOverride(
                "browser".to_string()
            ))
        );
    }
}
