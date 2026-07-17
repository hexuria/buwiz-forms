//! Native preview, system-print, and direct-PDF host for owned HTML forms.

use bir_print::html::RenderEnvelopeV1;
use bir_print::html_forms::RenderLayoutPlan;
#[cfg(any(target_os = "macos", all(feature = "dev-tools", target_os = "windows")))]
use bir_print::html_output::PdfValidationReport;
#[cfg(target_os = "macos")]
use bir_print::html_output::merge_single_page_pdfs;
use bir_print::html_output::{
    HtmlOutputKind, HtmlOutputState, HtmlOutputTimeoutStage, PdfExpectation,
    create_pdf_export_temp, discard_pdf_export_temp, finalize_pdf_export,
    html_output_timeout_stage,
};
use bir_print::html_support::{
    HtmlRendererSupport, RendererGeometryReport, RendererPageRect, RendererReadinessDecision,
    bundled_html_renderer_support, renderer_host_plan, renderer_readiness_decision,
    validate_renderer_geometry,
};
use std::path::PathBuf;

#[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
use bir_print::html_output_evidence::{
    ClippingCountersV1, DEVELOPMENT_NATIVE_OUTPUT_OBSERVATION_SCHEMA_VERSION,
    DevelopmentDestinationOutcomeV1, DevelopmentDestinationSnapshotV1,
    DevelopmentEvidenceAvailability, DevelopmentEvidenceScope, DevelopmentNativeOutputBackendV1,
    DevelopmentNativeOutputObservationV1, DevelopmentNativeOutputPlatformV1,
    EvidenceArtifactSource, GeometryReportEvidenceV1, NativeBackendCompletionObservationV1,
    NativeNonceObservationV1, NativePdfPagePayloadObservationV1,
    OFFLINE_RENDERER_BUILD_IDENTITY_FILE_NAME, PdfValidationEvidenceV1,
    decode_offline_renderer_build_identity, encode_development_native_output_observation,
    geometry_page_rect_sha256, hash_evidence_artifact,
};

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
use {
    bir_print::html::serialize_envelope,
    serde::Deserialize,
    sha2::{Digest, Sha256},
    std::borrow::Cow,
    std::path::Path,
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
use {
    gpui::prelude::FluentBuilder,
    gpui::{
        AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Task, Window, div,
        px,
    },
    gpui_component::ActiveTheme,
    gpui_component::Disableable,
    gpui_component::button::{Button, ButtonVariants},
    gpui_wry::WebView,
    std::collections::{HashMap, HashSet},
    std::sync::{Arc, Mutex},
    std::time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use wry::WebViewBuilderExtWindows;

#[cfg(target_os = "macos")]
use {
    block2::RcBlock,
    objc2_app_kit_modern::{
        NSPaperOrientation, NSPrintInfo, NSPrintOperation, NSPrintingPaginationMode,
    },
    objc2_core_foundation::{CGPoint, CGRect, CGSize},
    objc2_foundation_modern::{NSData, NSError, NSObjectProtocol},
    objc2_modern::{
        DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
        runtime::NSObject,
    },
    objc2_web_kit_modern::WKPDFConfiguration,
    std::ffi::c_void,
    wry::WebViewExtMacOS,
};

#[cfg(target_os = "windows")]
use {
    std::os::windows::ffi::OsStrExt,
    webview2_com::{
        Microsoft::Web::WebView2::Win32::{
            COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT, COREWEBVIEW2_PRINT_STATUS,
            COREWEBVIEW2_PRINT_STATUS_OTHER_ERROR, COREWEBVIEW2_PRINT_STATUS_PRINTER_UNAVAILABLE,
            COREWEBVIEW2_PRINT_STATUS_SUCCEEDED, ICoreWebView2_7, ICoreWebView2_16,
            ICoreWebView2Environment6, ICoreWebView2PrintSettings,
        },
        PrintCompletedHandler, PrintToPdfCompletedHandler,
    },
    windows_core::{Interface, PCWSTR},
    wry::WebViewExtWindows,
};

#[cfg(any(target_os = "macos", target_os = "windows"))]
const READINESS_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(any(target_os = "macos", target_os = "windows"))]
const PDF_EXPORT_TIMEOUT: Duration = Duration::from_secs(30);

#[cfg(any(target_os = "macos", target_os = "windows"))]
const RENDERER_WEBVIEW_IS_INCOGNITO: bool = true;

#[cfg(any(test, target_os = "windows"))]
const POINTS_PER_INCH: f64 = 72.0;

#[cfg(any(test, target_os = "windows"))]
#[derive(Debug, Clone, Copy, PartialEq)]
struct WindowsNativePrintSettingsSpec {
    page_width_inches: f64,
    page_height_inches: f64,
    scale_factor: f64,
    margin_top_inches: f64,
    margin_bottom_inches: f64,
    margin_left_inches: f64,
    margin_right_inches: f64,
    should_print_backgrounds: bool,
    should_print_selection_only: bool,
    should_print_header_and_footer: bool,
}

#[cfg(any(test, target_os = "windows"))]
fn windows_native_print_settings_spec(
    expectation: &PdfExpectation,
) -> Result<WindowsNativePrintSettingsSpec, String> {
    expectation
        .validate()
        .map_err(|error| format!("invalid native print expectation: {error}"))?;
    Ok(WindowsNativePrintSettingsSpec {
        page_width_inches: expectation.width_points / POINTS_PER_INCH,
        page_height_inches: expectation.height_points / POINTS_PER_INCH,
        scale_factor: 1.0,
        margin_top_inches: 0.0,
        margin_bottom_inches: 0.0,
        margin_left_inches: 0.0,
        margin_right_inches: 0.0,
        should_print_backgrounds: true,
        should_print_selection_only: false,
        should_print_header_and_footer: false,
    })
}

#[cfg(any(test, target_os = "windows"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum WindowsNativePrintStatus {
    Succeeded,
    PrinterUnavailable,
    OtherError,
    Unknown(i32),
}

#[cfg(any(test, target_os = "windows"))]
fn webview2_print_completion_decision(
    hresult_succeeded: bool,
    status: WindowsNativePrintStatus,
) -> Result<(), String> {
    if !hresult_succeeded {
        return Err("WebView2 Print completion returned a failing HRESULT".to_string());
    }
    match status {
        WindowsNativePrintStatus::Succeeded => Ok(()),
        WindowsNativePrintStatus::PrinterUnavailable => {
            Err("WebView2 Print reported that the printer is unavailable".to_string())
        }
        WindowsNativePrintStatus::OtherError => {
            Err("WebView2 Print reported a native printing error".to_string())
        }
        WindowsNativePrintStatus::Unknown(status) => Err(format!(
            "WebView2 Print returned unknown completion status {status}"
        )),
    }
}

#[cfg(any(test, target_os = "windows"))]
fn webview2_pdf_completion_decision(
    hresult_succeeded: bool,
    result_succeeded: bool,
) -> Result<(), String> {
    if !hresult_succeeded {
        return Err("WebView2 PrintToPdf completion returned a failing HRESULT".to_string());
    }
    if !result_succeeded {
        return Err("WebView2 PrintToPdf reported an unsuccessful result".to_string());
    }
    Ok(())
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
const RENDERER_CONTENT_SECURITY_POLICY: &str = concat!(
    "default-src 'self'; ",
    "connect-src 'none'; ",
    "img-src 'self' data:; ",
    "font-src 'self'; ",
    "style-src 'self' 'unsafe-inline'; ",
    "script-src 'self' ebirforms: http://ebirforms.localhost; ",
    "object-src 'none'; ",
    "base-uri 'none'; ",
    "form-action 'none'; ",
    "frame-src 'none'; ",
    "child-src 'none'; ",
    "worker-src 'none'; ",
    "frame-ancestors 'none'",
);

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
const RENDERER_PERMISSIONS_POLICY: &str = concat!(
    "accelerometer=(), ambient-light-sensor=(), autoplay=(), bluetooth=(), ",
    "browsing-topics=(), camera=(), clipboard-read=(), clipboard-write=(), ",
    "display-capture=(), encrypted-media=(), fullscreen=(), geolocation=(), ",
    "gyroscope=(), hid=(), identity-credentials-get=(), idle-detection=(), ",
    "local-fonts=(), magnetometer=(), microphone=(), midi=(), otp-credentials=(), ",
    "payment=(), picture-in-picture=(), publickey-credentials-create=(), ",
    "publickey-credentials-get=(), screen-wake-lock=(), serial=(), ",
    "speaker-selection=(), storage-access=(), usb=(), web-share=(), ",
    "window-management=(), xr-spatial-tracking=()",
);

#[derive(Debug, Clone, PartialEq, Eq)]
enum NativePrintDecision {
    WaitForRenderer,
    StartPrint,
    Fallback(String),
}

/// Keep the native print command behind the same fail-closed signals as the
/// preview itself. This decision is independent of GPUI/wry so every supported
/// host can exercise the edge cases in a focused unit test.
fn native_print_decision(
    ready: bool,
    page_count: Option<usize>,
    error: Option<&str>,
    webview_available: bool,
) -> NativePrintDecision {
    match renderer_readiness_decision(ready, page_count, error, false) {
        RendererReadinessDecision::Pending => NativePrintDecision::WaitForRenderer,
        RendererReadinessDecision::Ready { .. } if webview_available => {
            NativePrintDecision::StartPrint
        }
        RendererReadinessDecision::Ready { .. } => NativePrintDecision::Fallback(
            "HTML renderer became ready without an available native WebView".to_string(),
        ),
        RendererReadinessDecision::Fallback(reason) => NativePrintDecision::Fallback(reason),
    }
}

fn renderer_readiness_timed_out(
    has_reported_geometry: bool,
    epoch_deadline_reached: bool,
    initial_readiness_completed: bool,
    overall_initial_deadline_reached: bool,
) -> bool {
    !has_reported_geometry
        && (epoch_deadline_reached
            || (!initial_readiness_completed && overall_initial_deadline_reached))
}

#[derive(Debug, thiserror::Error)]
pub enum HtmlPreviewError {
    #[error("HTML preview is disabled for {code}:{revision}")]
    Disabled { code: String, revision: String },
    #[error(
        "HTML preview for {code}:{revision} is not release-certified; use a developer build for experimental calibration"
    )]
    NotReleaseCertified { code: String, revision: String },
    #[error("HTML renderer bundle was not found at {0}")]
    AssetsNotFound(PathBuf),
    #[error("HTML renderer entry point was not found at {0}")]
    MissingEntryPoint(PathBuf),
    #[error("failed to serialize the renderer envelope: {0}")]
    Serialization(#[from] bir_print::html::HtmlRendererError),
    #[error("failed to encode the renderer envelope for WebView injection: {0}")]
    EnvelopeEncoding(#[source] serde_json::Error),
    #[error("HTML renderer layout could not be resolved: {0}")]
    Layout(#[from] bir_print::html_forms::RenderLayoutError),
    #[error("HTML preview is not enabled on this platform")]
    UnsupportedPlatform,
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
#[derive(Clone)]
pub(crate) struct PreparedHtmlPreview {
    pub(crate) entry: PathBuf,
    pub(crate) url: String,
    pub(crate) initialization_script: String,
    #[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
    pub(crate) envelope_json: String,
    pub(crate) layout_plan: RenderLayoutPlan,
    pub(crate) pdf_expectation: PdfExpectation,
    pub(crate) default_pdf_name: String,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, PartialEq, Eq)]
struct RendererDocumentIdentity {
    document_run_id: String,
    envelope_hash: String,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl RendererDocumentIdentity {
    fn host_generated(envelope_hash: &str) -> Result<Self, String> {
        if envelope_hash.len() != 64
            || !envelope_hash
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
        {
            return Err("renderer document envelope hash is not canonical SHA-256 hex".to_string());
        }
        Ok(Self {
            document_run_id: uuid::Uuid::new_v4().to_string(),
            envelope_hash: envelope_hash.to_string(),
        })
    }

    #[cfg(test)]
    fn test_identity() -> Self {
        Self {
            document_run_id: "00000000-0000-4000-8000-000000000001".to_string(),
            envelope_hash: "a".repeat(64),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub(crate) fn prepare_html_form_preview(
    envelope: &RenderEnvelopeV1,
) -> Result<PreparedHtmlPreview, HtmlPreviewError> {
    let support = bundled_html_renderer_support(&envelope.form.code, &envelope.form.version);
    if !support.permits_preview() {
        return Err(HtmlPreviewError::Disabled {
            code: envelope.form.code.clone(),
            revision: envelope.form.version.clone(),
        });
    }
    if !html_preview_route_permitted(support, cfg!(feature = "dev-tools")) {
        return Err(HtmlPreviewError::NotReleaseCertified {
            code: envelope.form.code.clone(),
            revision: envelope.form.version.clone(),
        });
    }
    let layout_plan = renderer_host_plan(envelope)?;

    let renderer_dir = crate::platform::find_resource_dir("assets").join("form-renderer");
    if !renderer_dir.is_dir() {
        return Err(HtmlPreviewError::AssetsNotFound(renderer_dir));
    }
    let entry = renderer_dir.join("index.html");
    if !entry.is_file() {
        return Err(HtmlPreviewError::MissingEntryPoint(entry));
    }

    let envelope_json = serialize_envelope(envelope)?;
    let envelope_hash = format!("{:x}", Sha256::digest(envelope_json.as_bytes()));
    let encoded_json =
        serde_json::to_string(&envelope_json).map_err(HtmlPreviewError::EnvelopeEncoding)?;
    let initialization_script = renderer_initialization_script(&encoded_json);
    let expected_page_count = layout_plan.expected_page_count;
    let pdf_expectation = PdfExpectation {
        form_code: envelope.form.code.clone(),
        revision: envelope.form.version.clone(),
        envelope_hash,
        expected_page_count,
        width_points: layout_plan.page_geometry.width_points,
        height_points: layout_plan.page_geometry.height_points,
    };
    let period_suffix = envelope
        .period
        .quarter
        .map(|quarter| format!("-Q{quarter}"))
        .or_else(|| envelope.period.month.map(|month| format!("-M{month:02}")))
        .unwrap_or_default();
    let default_pdf_name = format!(
        "BIR-{}-{}{}.pdf",
        envelope.form.code, envelope.period.taxable_year, period_suffix
    );

    Ok(PreparedHtmlPreview {
        entry,
        url: "ebirforms://localhost/index.html".to_string(),
        initialization_script,
        #[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
        envelope_json,
        layout_plan,
        pdf_expectation,
        default_pdf_name,
    })
}

fn html_preview_route_permitted(support: HtmlRendererSupport, allow_experimental: bool) -> bool {
    if allow_experimental {
        support.permits_preview()
    } else {
        support.permits_release_routing()
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn renderer_initialization_script(encoded_json: &str) -> String {
    format!(
        r#"
        window.__EBIR_RENDER_ENVELOPE__ = JSON.parse({encoded_json});
        const postRendererHostMessage = (message) => {{
            try {{
                window.ipc.postMessage(JSON.stringify({{
                    ...message,
                    document_run_id: window.__EBIR_RENDER_DOCUMENT_RUN_ID__,
                    envelope_hash: window.__EBIR_RENDER_ENVELOPE_HASH__
                }}));
            }} catch (_error) {{
                // Native readiness handling will safely fall back after timeout.
            }}
        }};
        let printGuardInstallationFailed = false;
        (() => {{
            try {{
                const guardedRendererPrint = () => {{
                    const message = "Script-initiated printing is disabled; use the native validated Print button";
                    postRendererHostMessage({{ type: "renderer_error", message }});
                    throw new Error(message);
                }};
                Object.defineProperty(window, "print", {{
                    value: guardedRendererPrint,
                    writable: false,
                    configurable: false
                }});
            }} catch (_error) {{
                printGuardInstallationFailed = true;
            }}
        }})();
        if (printGuardInstallationFailed) {{
            const message = "Native print guard installation failed";
            // Report immediately when the bridge is already available and
            // again at DOM readiness so an early bridge race cannot silently
            // leave this preview eligible for native printing.
            postRendererHostMessage({{ type: "renderer_error", message }});
            window.addEventListener("DOMContentLoaded", () => {{
                postRendererHostMessage({{ type: "renderer_error", message }});
            }}, {{ once: true }});
        }}
        document.addEventListener("keydown", (event) => {{
            if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "p") {{
                event.preventDefault();
                event.stopImmediatePropagation();
            }}
        }}, true);
        document.addEventListener("contextmenu", (event) => {{
            event.preventDefault();
            event.stopImmediatePropagation();
        }}, true);
        window.addEventListener("error", (event) => {{
            postRendererHostMessage({{
                type: "renderer_error",
                message: event.message || "Unhandled renderer script error"
            }});
        }});
        window.addEventListener("unhandledrejection", (event) => {{
            const reason = event.reason instanceof Error
                ? event.reason.message
                : String(event.reason ?? "Unhandled renderer promise rejection");
            postRendererHostMessage({{ type: "renderer_error", message: reason }});
        }});
        document.addEventListener("securitypolicyviolation", (event) => {{
            postRendererHostMessage({{
                type: "renderer_error",
                message: `Content Security Policy blocked ${{event.violatedDirective}}`
            }});
        }});
        window.fetch = () => Promise.reject(new Error("Network access is disabled in the form renderer"));
        window.XMLHttpRequest = class {{ constructor() {{ throw new Error("Network access is disabled in the form renderer"); }} }};
        window.WebSocket = class {{ constructor() {{ throw new Error("Network access is disabled in the form renderer"); }} }};
        window.EventSource = class {{ constructor() {{ throw new Error("Network access is disabled in the form renderer"); }} }};

        const blockedWorkerConstructor = (capability) => class {{
            constructor() {{
                throw new Error(`${{capability}} is disabled in the form renderer`);
            }}
        }};
        const installBlockedWorkerConstructor = (capability) => {{
            const blocked = blockedWorkerConstructor(capability);
            try {{
                Object.defineProperty(window, capability, {{
                    value: blocked,
                    writable: false,
                    configurable: false
                }});
            }} catch (_error) {{
                try {{ window[capability] = blocked; }} catch (_ignored) {{}}
            }}
        }};
        installBlockedWorkerConstructor("Worker");
        installBlockedWorkerConstructor("SharedWorker");

        const blockedCapabilityConstructor = (capability) => class {{
            constructor() {{
                throw new Error(`${{capability}} is disabled in the form renderer`);
            }}
        }};
        const installBlockedWindowCapability = (capability) => {{
            const blocked = blockedCapabilityConstructor(capability);
            try {{
                Object.defineProperty(window, capability, {{
                    value: blocked,
                    writable: false,
                    configurable: false
                }});
            }} catch (_error) {{
                try {{ window[capability] = blocked; }} catch (_ignored) {{}}
            }}
        }};
        [
            "RTCPeerConnection",
            "webkitRTCPeerConnection",
            "mozRTCPeerConnection",
            "msRTCPeerConnection",
            "webkitPeerConnection00",
            "MediaRecorder"
        ].forEach(installBlockedWindowCapability);

        if (window.navigator) {{
            window.navigator.sendBeacon = () => false;
            const blockedMediaOperation = (capability) => () => Promise.reject(
                new Error(`${{capability}} is disabled in the form renderer`)
            );
            const blockedMediaDevices = Object.freeze({{
                getUserMedia: blockedMediaOperation("mediaDevices.getUserMedia"),
                getDisplayMedia: blockedMediaOperation("mediaDevices.getDisplayMedia"),
                selectAudioOutput: blockedMediaOperation("mediaDevices.selectAudioOutput"),
                enumerateDevices: () => Promise.resolve([])
            }});
            try {{
                Object.defineProperty(window.navigator, "mediaDevices", {{
                    value: blockedMediaDevices,
                    writable: false,
                    configurable: false
                }});
            }} catch (_error) {{
                try {{
                    const mediaDevices = window.navigator.mediaDevices;
                    if (mediaDevices) {{
                        for (const [operation, blocked] of Object.entries(blockedMediaDevices)) {{
                            try {{
                                Object.defineProperty(mediaDevices, operation, {{
                                    value: blocked,
                                    writable: false,
                                    configurable: false
                                }});
                            }} catch (_ignored) {{}}
                        }}
                    }}
                }} catch (_ignored) {{}}
            }}
            for (const alias of [
                "getUserMedia",
                "webkitGetUserMedia",
                "mozGetUserMedia",
                "msGetUserMedia"
            ]) {{
                try {{
                    Object.defineProperty(window.navigator, alias, {{
                        value: blockedMediaOperation(`navigator.${{alias}}`),
                        writable: false,
                        configurable: false
                    }});
                }} catch (_ignored) {{}}
            }}

            const blockedDeviceSurface = Object.freeze({{
                getDevices: () => Promise.resolve([]),
                getPorts: () => Promise.resolve([]),
                requestDevice: blockedMediaOperation("device access"),
                requestPort: blockedMediaOperation("serial access")
            }});
            for (const capability of ["bluetooth", "hid", "serial", "usb"]) {{
                try {{
                    Object.defineProperty(window.navigator, capability, {{
                        value: blockedDeviceSurface,
                        writable: false,
                        configurable: false
                    }});
                }} catch (_ignored) {{}}
            }}

            const blockedServiceWorkerOperation = () => Promise.reject(
                new Error("Service workers are disabled in the form renderer")
            );
            const blockedServiceWorker = Object.freeze({{
                register: blockedServiceWorkerOperation,
                getRegistration: blockedServiceWorkerOperation,
                getRegistrations: () => Promise.resolve([]),
                controller: null
            }});
            try {{
                Object.defineProperty(window.navigator, "serviceWorker", {{
                    value: blockedServiceWorker,
                    writable: false,
                    configurable: false
                }});
            }} catch (_error) {{
                try {{
                    const serviceWorker = window.navigator.serviceWorker;
                    if (serviceWorker) {{
                        Object.defineProperty(serviceWorker, "register", {{
                            value: blockedServiceWorkerOperation,
                            writable: false,
                            configurable: false
                        }});
                    }}
                }} catch (_ignored) {{}}
            }}
        }}
        "#
    )
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn renderer_document_identity_script(identity: &RendererDocumentIdentity) -> String {
    let document_run_id = serde_json::to_string(&identity.document_run_id)
        .expect("UUID renderer document run IDs always serialize");
    let envelope_hash = serde_json::to_string(&identity.envelope_hash)
        .expect("canonical renderer envelope hashes always serialize");
    format!(
        r#"
        Object.defineProperty(window, "__EBIR_RENDER_DOCUMENT_RUN_ID__", {{
            value: {document_run_id},
            writable: false,
            configurable: false
        }});
        Object.defineProperty(window, "__EBIR_RENDER_ENVELOPE_HASH__", {{
            value: {envelope_hash},
            writable: false,
            configurable: false
        }});
        "#
    )
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub(crate) fn native_print_preflight_script(nonce: u64) -> String {
    format!(
        r#"
        void (() => {{
            if (typeof window.prepareEbirFormForNativePrint !== "function") {{
                throw new Error("HTML renderer native print preflight is unavailable");
            }}
            window.prepareEbirFormForNativePrint({nonce});
        }})();
        "#
    )
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub(crate) fn native_output_cleanup_script() -> &'static str {
    r#"
    void (() => {
        document.documentElement.classList.remove("ebir-native-print-mode");
        window.dispatchEvent(new Event("resize"));
    })();
    "#
}

#[cfg(target_os = "macos")]
fn macos_system_print_completion_decision(success: bool) -> Result<(), String> {
    if success {
        Ok(())
    } else {
        // AppKit deliberately uses the same `NO` result for a cancelled print
        // panel and for an operation error. Do not claim either one more
        // specifically than the native API can prove.
        Err("the macOS print operation was cancelled or failed".to_string())
    }
}

#[cfg(target_os = "macos")]
struct MacPrintCompletionDelegateIvars {
    nonce: u64,
    document_identity: RendererDocumentIdentity,
    render_epoch: u64,
    bridge: Arc<Mutex<NativeBackendBridge>>,
}

#[cfg(target_os = "macos")]
define_class!(
    #[unsafe(super(NSObject))]
    #[name = "EbirHtmlPrintCompletionDelegate"]
    #[thread_kind = MainThreadOnly]
    #[ivars = MacPrintCompletionDelegateIvars]
    struct MacPrintCompletionDelegate;

    unsafe impl NSObjectProtocol for MacPrintCompletionDelegate {}

    impl MacPrintCompletionDelegate {
        #[unsafe(method(printOperationDidRun:success:contextInfo:))]
        fn print_operation_did_run(
            &self,
            _print_operation: &NSPrintOperation,
            success: bool,
            context_info: *mut c_void,
        ) {
            if let Ok(mut bridge) = self.ivars().bridge.lock() {
                bridge.record_completion(NativeBackendCompletion::SystemPrint {
                    nonce: self.ivars().nonce,
                    document_identity: self.ivars().document_identity.clone(),
                    render_epoch: self.ivars().render_epoch,
                    result: macos_system_print_completion_decision(success),
                });
            }

            if !context_info.is_null() {
                // SAFETY: `start_macos_system_print` passes exactly one +1
                // retain of this delegate as contextInfo. AppKit documents
                // exactly one completion callback for each print operation.
                drop(unsafe {
                    Retained::<MacPrintCompletionDelegate>::from_raw(context_info.cast())
                });
            }
        }
    }
);

#[cfg(target_os = "macos")]
impl MacPrintCompletionDelegate {
    fn new(
        main_thread: MainThreadMarker,
        nonce: u64,
        document_identity: RendererDocumentIdentity,
        render_epoch: u64,
        bridge: Arc<Mutex<NativeBackendBridge>>,
    ) -> Retained<Self> {
        let delegate = main_thread
            .alloc::<Self>()
            .set_ivars(MacPrintCompletionDelegateIvars {
                nonce,
                document_identity,
                render_epoch,
                bridge,
            });
        // SAFETY: the allocated object has its complete Rust ivars installed
        // and NSObject's initializer returns the retained instance.
        unsafe { msg_send![super(delegate), init] }
    }
}

#[cfg(target_os = "macos")]
fn start_macos_system_print(
    raw_webview: &wry::WebView,
    expectation: &PdfExpectation,
    nonce: u64,
    document_identity: RendererDocumentIdentity,
    render_epoch: u64,
    bridge: Arc<Mutex<NativeBackendBridge>>,
) -> Result<(), String> {
    expectation
        .validate()
        .map_err(|error| format!("invalid macOS print expectation: {error}"))?;
    let Some(main_thread) = MainThreadMarker::new() else {
        return Err("WKWebView system print must start on the macOS main thread".to_string());
    };
    if NSPrintOperation::currentOperation(main_thread).is_some() {
        return Err("another macOS print operation is already active".to_string());
    }

    let webview = raw_webview.webview();
    // SAFETY: this runtime availability query sends no state-changing message
    // and protects installations older than WKWebView printing support.
    let can_print =
        unsafe { webview.respondsToSelector(objc2_modern::sel!(printOperationWithPrintInfo:)) };
    if !can_print {
        return Err("this macOS WKWebView does not support native print operations".to_string());
    }
    let window = webview
        .window()
        .ok_or_else(|| "WKWebView is not attached to a macOS window".to_string())?;

    let print_info = NSPrintInfo::sharedPrintInfo();
    print_info.setPaperSize(CGSize::new(
        expectation.width_points,
        expectation.height_points,
    ));
    print_info.setOrientation(NSPaperOrientation::Portrait);
    print_info.setScalingFactor(1.0);
    print_info.setTopMargin(0.0);
    print_info.setRightMargin(0.0);
    print_info.setBottomMargin(0.0);
    print_info.setLeftMargin(0.0);
    print_info.setHorizontallyCentered(false);
    print_info.setVerticallyCentered(false);
    print_info.setHorizontalPagination(NSPrintingPaginationMode::Automatic);
    print_info.setVerticalPagination(NSPrintingPaginationMode::Automatic);

    // SAFETY: `webview` is the live WKWebView returned by Wry and the method
    // availability was checked immediately above on the main thread.
    let print_operation = unsafe { webview.printOperationWithPrintInfo(&print_info) };
    print_operation.setCanSpawnSeparateThread(true);

    let delegate = MacPrintCompletionDelegate::new(
        main_thread,
        nonce,
        document_identity,
        render_epoch,
        bridge,
    );
    let retained_context = Retained::into_raw(delegate.clone()).cast::<c_void>();
    // SAFETY: the window, operation, and delegate are main-thread AppKit
    // objects. The selector has AppKit's documented
    // `(NSPrintOperation *, BOOL, void *)` callback signature. The +1 context
    // retain keeps the delegate alive until that callback consumes it.
    unsafe {
        print_operation.runOperationModalForWindow_delegate_didRunSelector_contextInfo(
            &window,
            Some(&delegate),
            Some(objc2_modern::sel!(
                printOperationDidRun:success:contextInfo:
            )),
            retained_context,
        );
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn start_macos_pdf_capture(
    raw_webview: &wry::WebView,
    page_rects: &[RendererPageRect],
    nonce: u64,
    document_identity: RendererDocumentIdentity,
    render_epoch: u64,
    bridge: Arc<Mutex<NativeBackendBridge>>,
) -> Result<(), String> {
    let Some(main_thread) = MainThreadMarker::new() else {
        return Err("WKWebView PDF capture must start on the macOS main thread".to_string());
    };
    if page_rects.is_empty() {
        return Err("WKWebView PDF capture received no validated page rectangles".to_string());
    }
    if let Ok(mut bridge) = bridge.lock() {
        bridge.begin_macos_capture(nonce, document_identity, render_epoch, page_rects.len());
    } else {
        return Err("WKWebView PDF capture state is unavailable".to_string());
    }

    let webview = raw_webview.webview();
    for (page_index, page) in page_rects.iter().copied().enumerate() {
        // SAFETY: the marker proves this is the main thread; the rectangle
        // contains finite, validated DOM coordinates.
        let configuration = unsafe {
            let configuration = WKPDFConfiguration::new(main_thread);
            configuration.setRect(CGRect::new(
                CGPoint::new(page.x, page.y),
                CGSize::new(page.width, page.height),
            ));
            configuration.setAllowTransparentBackground(false);
            configuration
        };
        let callback_bridge = bridge.clone();
        let callback = RcBlock::new(move |data: *mut NSData, error: *mut NSError| {
            let result = if let Some(error) = unsafe { error.as_ref() } {
                Err(format!(
                    "WKWebView page {} capture failed: {}",
                    page_index + 1,
                    error.localizedDescription()
                ))
            } else if let Some(data) = unsafe { data.as_ref() } {
                Ok(data.to_vec())
            } else {
                Err(format!(
                    "WKWebView page {} capture returned neither PDF data nor an error",
                    page_index + 1
                ))
            };
            if let Ok(mut bridge) = callback_bridge.lock() {
                bridge.record_macos_page(nonce, page_index, result);
            }
        });
        // SAFETY: `webview` and `configuration` are live Objective-C objects,
        // the escaping block owns its Arc state, and WebKit copies completion
        // handlers for the asynchronous operation.
        unsafe {
            webview.createPDFWithConfiguration_completionHandler(Some(&configuration), &callback);
        }
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn finalize_macos_pdf_capture(
    pages: Vec<Result<Vec<u8>, String>>,
    temp_path: &Path,
    destination: &Path,
    expectation: &PdfExpectation,
) -> Result<PdfValidationReport, String> {
    let result = pages
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .and_then(|pages| merge_single_page_pdfs(&pages).map_err(|error| error.to_string()))
        .and_then(|merged| std::fs::write(temp_path, merged).map_err(|error| error.to_string()))
        .and_then(|()| {
            finalize_pdf_export(temp_path, destination, expectation)
                .map_err(|error| error.to_string())
        });
    if result.is_err() {
        // Merge and write failures happen before `finalize_pdf_export`, so the
        // macOS capture boundary must clean its registered sibling temp itself.
        // The outer view repeats this cleanup defensively when it leaves the
        // failed output state.
        let _ = discard_pdf_export_temp(temp_path);
    }
    result
}

#[cfg(target_os = "windows")]
fn create_windows_print_settings(
    raw_webview: &wry::WebView,
    expectation: &PdfExpectation,
) -> Result<ICoreWebView2PrintSettings, String> {
    let spec = windows_native_print_settings_spec(expectation)?;
    let environment = raw_webview
        .environment()
        .cast::<ICoreWebView2Environment6>()
        .map_err(|error| format!("WebView2 print settings are unavailable: {error}"))?;
    let settings = unsafe { environment.CreatePrintSettings() }
        .map_err(|error| format!("WebView2 could not create print settings: {error}"))?;
    // SAFETY: every setter is invoked on a live WebView2 print-settings COM
    // object on the UI thread. Dimensions and margins are in inches.
    unsafe {
        settings
            .SetOrientation(COREWEBVIEW2_PRINT_ORIENTATION_PORTRAIT)
            .and_then(|_| settings.SetScaleFactor(spec.scale_factor))
            .and_then(|_| settings.SetPageWidth(spec.page_width_inches))
            .and_then(|_| settings.SetPageHeight(spec.page_height_inches))
            .and_then(|_| settings.SetMarginTop(spec.margin_top_inches))
            .and_then(|_| settings.SetMarginBottom(spec.margin_bottom_inches))
            .and_then(|_| settings.SetMarginLeft(spec.margin_left_inches))
            .and_then(|_| settings.SetMarginRight(spec.margin_right_inches))
            .and_then(|_| settings.SetShouldPrintBackgrounds(spec.should_print_backgrounds))
            .and_then(|_| settings.SetShouldPrintSelectionOnly(spec.should_print_selection_only))
            .and_then(|_| {
                settings.SetShouldPrintHeaderAndFooter(spec.should_print_header_and_footer)
            })
    }
    .map_err(|error| format!("WebView2 rejected print settings: {error}"))?;
    Ok(settings)
}

#[cfg(target_os = "windows")]
fn windows_native_print_status(status: COREWEBVIEW2_PRINT_STATUS) -> WindowsNativePrintStatus {
    if status.0 == COREWEBVIEW2_PRINT_STATUS_SUCCEEDED.0 {
        WindowsNativePrintStatus::Succeeded
    } else if status.0 == COREWEBVIEW2_PRINT_STATUS_PRINTER_UNAVAILABLE.0 {
        WindowsNativePrintStatus::PrinterUnavailable
    } else if status.0 == COREWEBVIEW2_PRINT_STATUS_OTHER_ERROR.0 {
        WindowsNativePrintStatus::OtherError
    } else {
        WindowsNativePrintStatus::Unknown(status.0)
    }
}

#[cfg(target_os = "windows")]
fn start_windows_system_print(
    raw_webview: &wry::WebView,
    expectation: &PdfExpectation,
    nonce: u64,
    document_identity: RendererDocumentIdentity,
    render_epoch: u64,
    bridge: Arc<Mutex<NativeBackendBridge>>,
) -> Result<(), String> {
    let settings = create_windows_print_settings(raw_webview, expectation)?;
    let webview = raw_webview
        .webview()
        .cast::<ICoreWebView2_16>()
        .map_err(|error| format!("WebView2 native Print is unavailable: {error}"))?;
    let callback = PrintCompletedHandler::create(Box::new(move |hresult, status| {
        let result = match hresult {
            Ok(()) => webview2_print_completion_decision(true, windows_native_print_status(status)),
            Err(error) => Err(format!(
                "WebView2 Print completion failed before its status: {error}"
            )),
        };
        if let Ok(mut bridge) = bridge.lock() {
            bridge.record_completion(NativeBackendCompletion::SystemPrint {
                nonce,
                document_identity: document_identity.clone(),
                render_epoch,
                result,
            });
        }
        Ok(())
    }));

    // SAFETY: the settings and callback are live COM objects on the WebView2
    // UI thread; the callback owns all state needed after this call returns.
    unsafe { webview.Print(&settings, &callback) }
        .map_err(|error| format!("WebView2 native Print failed to start: {error}"))
}

#[cfg(target_os = "windows")]
fn start_windows_pdf_export(
    raw_webview: &wry::WebView,
    temp_path: &Path,
    expectation: &PdfExpectation,
    nonce: u64,
    document_identity: RendererDocumentIdentity,
    render_epoch: u64,
    bridge: Arc<Mutex<NativeBackendBridge>>,
) -> Result<(), String> {
    let settings = create_windows_print_settings(raw_webview, expectation)?;

    let webview = raw_webview
        .webview()
        .cast::<ICoreWebView2_7>()
        .map_err(|error| format!("WebView2 PrintToPdf is unavailable: {error}"))?;
    let mut wide_path = temp_path.as_os_str().encode_wide().collect::<Vec<_>>();
    if wide_path.contains(&0) {
        return Err("WebView2 PDF output path contains an embedded NUL".to_string());
    }
    wide_path.push(0);

    let callback_bridge = bridge;
    let callback = PrintToPdfCompletedHandler::create(Box::new(move |hresult, succeeded| {
        let result = match hresult {
            Ok(()) => webview2_pdf_completion_decision(true, succeeded),
            Err(error) => Err(format!(
                "WebView2 PrintToPdf completion failed before its result flag: {error}"
            )),
        };
        if let Ok(mut bridge) = callback_bridge.lock() {
            bridge.record_completion(NativeBackendCompletion::PdfFile {
                nonce,
                document_identity: document_identity.clone(),
                render_epoch,
                result,
            });
        }
        Ok(())
    }));

    // SAFETY: the UTF-16 path remains alive for the synchronous COM call and
    // the callback owns all state required after the call returns.
    unsafe { webview.PrintToPdf(PCWSTR(wide_path.as_ptr()), &settings, &callback) }
        .map_err(|error| format!("WebView2 PrintToPdf failed to start: {error}"))
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, Default, PartialEq)]
struct RendererState {
    document_identity: Option<RendererDocumentIdentity>,
    document_boot_accepted: bool,
    document_identity_rejected: bool,
    ready: bool,
    page_count: Option<usize>,
    page_rects: Vec<RendererPageRect>,
    geometry_reports: Option<[RendererGeometryReport; 2]>,
    geometry_print_mode: bool,
    error: Option<String>,
    render_epoch: u64,
    readiness_revision: u64,
    print_ready_nonce: Option<u64>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl RendererState {
    fn for_document(document_identity: RendererDocumentIdentity) -> Self {
        Self {
            document_identity: Some(document_identity),
            ..Self::default()
        }
    }

    fn reject_document_identity(&mut self, reason: impl Into<String>) {
        self.document_identity_rejected = true;
        self.ready = false;
        self.page_count = None;
        self.page_rects.clear();
        self.geometry_reports = None;
        self.geometry_print_mode = false;
        self.print_ready_nonce = None;
        self.error = Some(reason.into());
        self.readiness_revision = self.readiness_revision.saturating_add(1);
    }

    fn accepts_document_identity(&self, identity: &RendererDocumentIdentity) -> bool {
        !self.document_identity_rejected && self.document_identity.as_ref() == Some(identity)
    }

    fn invalidate_for_epoch(&mut self, render_epoch: u64) -> bool {
        if render_epoch == 0 || render_epoch <= self.render_epoch {
            return false;
        }
        self.render_epoch = render_epoch;
        self.ready = false;
        self.page_count = None;
        self.page_rects.clear();
        self.geometry_reports = None;
        self.geometry_print_mode = false;
        self.error = None;
        self.print_ready_nonce = None;
        self.readiness_revision = self.readiness_revision.saturating_add(1);
        true
    }

    fn accepts_epoch(&self, render_epoch: u64) -> bool {
        render_epoch != 0 && render_epoch == self.render_epoch
    }

    fn accept_print_ready(&mut self, nonce: u64, render_epoch: u64, print_mode: bool) {
        if !self.accepts_epoch(render_epoch) {
            return;
        }
        if !print_mode || !self.geometry_print_mode {
            self.print_ready_nonce = None;
            self.error =
                Some("native print preflight was not measured in explicit print mode".to_string());
            return;
        }
        match renderer_readiness_decision(self.ready, self.page_count, self.error.as_deref(), false)
        {
            RendererReadinessDecision::Ready { .. }
                if self.page_count != Some(self.page_rects.len())
                    || self.page_rects.is_empty()
                    || self.geometry_reports.is_none() =>
            {
                self.print_ready_nonce = None;
                self.error = Some(
                    "native print preflight did not retain every validated page rectangle"
                        .to_string(),
                );
            }
            RendererReadinessDecision::Ready { .. } => self.print_ready_nonce = Some(nonce),
            RendererReadinessDecision::Pending => {
                self.error =
                    Some("native print preflight completed before renderer readiness".to_string())
            }
            RendererReadinessDecision::Fallback(reason) => self.error = Some(reason),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug)]
struct PendingNativeOutput {
    kind: HtmlOutputKind,
    nonce: u64,
    destination: Option<PathBuf>,
    temp_path: Option<PathBuf>,
    started_at: Instant,
    backend_started: bool,
    binding: Option<NativeOutputRendererBinding>,
    #[cfg(feature = "dev-tools")]
    destination_before: DevelopmentDestinationSnapshotV1,
    #[cfg(feature = "dev-tools")]
    preflight_consumptions: Vec<u64>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, PartialEq)]
struct NativeOutputRendererBinding {
    document_identity: RendererDocumentIdentity,
    render_epoch: u64,
    readiness_revision: u64,
    page_rects: Vec<RendererPageRect>,
    geometry_reports: [RendererGeometryReport; 2],
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl PendingNativeOutput {
    fn validating(kind: HtmlOutputKind, nonce: u64, destination: Option<PathBuf>) -> Self {
        #[cfg(feature = "dev-tools")]
        let destination_before = destination
            .as_deref()
            .map(development_destination_snapshot)
            .unwrap_or_else(|| DevelopmentDestinationSnapshotV1::Unavailable {
                reason: "system print has no PDF destination".to_string(),
            });
        Self {
            kind,
            nonce,
            destination,
            temp_path: None,
            started_at: Instant::now(),
            backend_started: false,
            binding: None,
            #[cfg(feature = "dev-tools")]
            destination_before,
            #[cfg(feature = "dev-tools")]
            preflight_consumptions: Vec::new(),
        }
    }
}

#[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
#[derive(Debug)]
struct DevelopmentBackendObservation {
    completion: NativeBackendCompletionObservationV1,
    page_payloads: DevelopmentEvidenceAvailability<Vec<NativePdfPagePayloadObservationV1>>,
}

#[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
fn development_destination_snapshot(path: &Path) -> DevelopmentDestinationSnapshotV1 {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return DevelopmentDestinationSnapshotV1::Absent;
        }
        Err(error) => {
            return DevelopmentDestinationSnapshotV1::Unavailable {
                reason: format!("destination metadata could not be read: {error}"),
            };
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return DevelopmentDestinationSnapshotV1::Unavailable {
            reason: "destination is not a regular non-symlink file".to_string(),
        };
    }
    match hash_evidence_artifact(EvidenceArtifactSource::File(path)) {
        Ok(sha256) => DevelopmentDestinationSnapshotV1::File { sha256 },
        Err(error) => DevelopmentDestinationSnapshotV1::Unavailable {
            reason: format!("destination could not be hashed: {error}"),
        },
    }
}

#[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
fn development_renderer_bundle_hash(entry: &Path) -> DevelopmentEvidenceAvailability<String> {
    let Some(renderer_root) = entry.parent() else {
        return DevelopmentEvidenceAvailability::unavailable(
            "renderer entry point has no parent directory",
        );
    };
    match hash_evidence_artifact(EvidenceArtifactSource::Directory(renderer_root)) {
        Ok(hash) => DevelopmentEvidenceAvailability::observed(hash),
        Err(error) => DevelopmentEvidenceAvailability::unavailable(format!(
            "runtime renderer bundle could not be stably hashed: {error}"
        )),
    }
}

#[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
#[derive(Debug)]
struct DevelopmentRendererBuildIdentityBinding {
    source_revision: DevelopmentEvidenceAvailability<String>,
    expected_renderer_bundle_sha256: DevelopmentEvidenceAvailability<String>,
    binding_gap: Option<String>,
}

#[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
fn unavailable_renderer_build_identity(
    reason: impl Into<String>,
) -> DevelopmentRendererBuildIdentityBinding {
    let reason = reason.into();
    DevelopmentRendererBuildIdentityBinding {
        source_revision: DevelopmentEvidenceAvailability::unavailable(reason.clone()),
        expected_renderer_bundle_sha256: DevelopmentEvidenceAvailability::unavailable(reason),
        binding_gap: None,
    }
}

#[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
fn development_renderer_build_identity(
    entry: &Path,
    observed_bundle_hash: &DevelopmentEvidenceAvailability<String>,
) -> DevelopmentRendererBuildIdentityBinding {
    let Some(renderer_root) = entry.parent() else {
        return unavailable_renderer_build_identity(
            "renderer entry point has no parent directory for build identity",
        );
    };
    let Some(assets_root) = renderer_root.parent() else {
        return unavailable_renderer_build_identity(
            "renderer root has no assets parent for build identity",
        );
    };
    let identity_path = assets_root.join(OFFLINE_RENDERER_BUILD_IDENTITY_FILE_NAME);
    let metadata = match std::fs::symlink_metadata(&identity_path) {
        Ok(metadata) if metadata.is_file() && !metadata.file_type().is_symlink() => metadata,
        Ok(_) => {
            return unavailable_renderer_build_identity(
                "renderer build identity is not a regular non-symlink file",
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return unavailable_renderer_build_identity(
                "renderer build identity was not generated by the offline build gate",
            );
        }
        Err(error) => {
            return unavailable_renderer_build_identity(format!(
                "renderer build identity metadata could not be read: {error}"
            ));
        }
    };
    if metadata.len() > 64 * 1024 {
        return unavailable_renderer_build_identity(
            "renderer build identity exceeds the 64 KiB diagnostic limit",
        );
    }
    let bytes = match std::fs::read(&identity_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            return unavailable_renderer_build_identity(format!(
                "renderer build identity could not be read: {error}"
            ));
        }
    };
    match std::fs::read(&identity_path) {
        Ok(second) if second == bytes => {}
        Ok(_) => {
            return unavailable_renderer_build_identity(
                "renderer build identity changed while it was being read",
            );
        }
        Err(error) => {
            return unavailable_renderer_build_identity(format!(
                "renderer build identity could not be read twice: {error}"
            ));
        }
    }
    let identity = match decode_offline_renderer_build_identity(&bytes) {
        Ok(identity) => identity,
        Err(error) => {
            return unavailable_renderer_build_identity(format!(
                "renderer build identity is invalid: {error}"
            ));
        }
    };
    let expected = identity.renderer_bundle_sha256;
    match observed_bundle_hash {
        DevelopmentEvidenceAvailability::Observed { value } if value == &expected => {
            DevelopmentRendererBuildIdentityBinding {
                source_revision: identity.source_revision,
                expected_renderer_bundle_sha256: DevelopmentEvidenceAvailability::observed(
                    expected,
                ),
                binding_gap: None,
            }
        }
        DevelopmentEvidenceAvailability::Observed { .. } => {
            DevelopmentRendererBuildIdentityBinding {
                source_revision: DevelopmentEvidenceAvailability::unavailable(
                    "build-time source revision is not bound because the running renderer differs from its expected bundle hash",
                ),
                expected_renderer_bundle_sha256: DevelopmentEvidenceAvailability::observed(
                    expected,
                ),
                binding_gap: Some(
                    "running renderer bundle differs from the independently generated build identity"
                        .to_string(),
                ),
            }
        }
        DevelopmentEvidenceAvailability::Unavailable { .. } => {
            DevelopmentRendererBuildIdentityBinding {
                source_revision: DevelopmentEvidenceAvailability::unavailable(
                    "build-time source revision cannot be bound while the running renderer hash is unavailable",
                ),
                expected_renderer_bundle_sha256: DevelopmentEvidenceAvailability::observed(
                    expected,
                ),
                binding_gap: Some(
                    "running renderer hash is unavailable for build-identity comparison"
                        .to_string(),
                ),
            }
        }
    }
}

#[cfg(all(feature = "dev-tools", target_os = "macos"))]
fn development_package_hash() -> DevelopmentEvidenceAvailability<String> {
    let executable = match std::env::current_exe() {
        Ok(path) => path,
        Err(error) => {
            return DevelopmentEvidenceAvailability::unavailable(format!(
                "current executable path could not be resolved: {error}"
            ));
        }
    };
    let mut candidate = executable.as_path();
    while let Some(parent) = candidate.parent() {
        if candidate
            .extension()
            .is_some_and(|extension| extension == "app")
        {
            return match hash_evidence_artifact(EvidenceArtifactSource::Directory(candidate)) {
                Ok(hash) => DevelopmentEvidenceAvailability::observed(hash),
                Err(error) => DevelopmentEvidenceAvailability::unavailable(format!(
                    "running macOS package could not be stably hashed: {error}"
                )),
            };
        }
        candidate = parent;
    }
    DevelopmentEvidenceAvailability::unavailable(
        "cargo-run executable is not inside a macOS application package",
    )
}

#[cfg(all(feature = "dev-tools", target_os = "windows"))]
fn development_package_hash() -> DevelopmentEvidenceAvailability<String> {
    DevelopmentEvidenceAvailability::unavailable(
        "the Windows package root is not bound to the running executable yet",
    )
}

#[cfg(all(feature = "dev-tools", target_os = "macos"))]
fn development_platform() -> DevelopmentNativeOutputPlatformV1 {
    DevelopmentNativeOutputPlatformV1::Macos
}

#[cfg(all(feature = "dev-tools", target_os = "windows"))]
fn development_platform() -> DevelopmentNativeOutputPlatformV1 {
    DevelopmentNativeOutputPlatformV1::Windows
}

#[cfg(all(feature = "dev-tools", target_os = "macos"))]
fn development_backend() -> DevelopmentNativeOutputBackendV1 {
    DevelopmentNativeOutputBackendV1::WkWebViewCreatePdf
}

#[cfg(all(feature = "dev-tools", target_os = "windows"))]
fn development_backend() -> DevelopmentNativeOutputBackendV1 {
    DevelopmentNativeOutputBackendV1::WebView2PrintToPdf
}

#[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
fn development_backend_observation(
    completion: &NativeBackendCompletion,
) -> Option<DevelopmentBackendObservation> {
    match completion {
        NativeBackendCompletion::SystemPrint { .. } => None,
        #[cfg(target_os = "macos")]
        NativeBackendCompletion::CapturedPages {
            nonce,
            document_identity,
            render_epoch,
            pages,
        } => {
            let page_payloads = pages
                .iter()
                .enumerate()
                .map(|(index, page)| match page {
                    Ok(bytes) => NativePdfPagePayloadObservationV1 {
                        page_number: index + 1,
                        succeeded: true,
                        byte_count: bytes.len(),
                        sha256: Some(format!("{:x}", Sha256::digest(bytes))),
                        error: None,
                    },
                    Err(error) => NativePdfPagePayloadObservationV1 {
                        page_number: index + 1,
                        succeeded: false,
                        byte_count: 0,
                        sha256: None,
                        error: Some(error.clone()),
                    },
                })
                .collect::<Vec<_>>();
            let errors = pages
                .iter()
                .filter_map(|page| page.as_ref().err())
                .cloned()
                .collect::<Vec<_>>();
            Some(DevelopmentBackendObservation {
                completion: NativeBackendCompletionObservationV1 {
                    nonce: *nonce,
                    document_run_id: document_identity.document_run_id.clone(),
                    envelope_sha256: document_identity.envelope_hash.clone(),
                    render_epoch: *render_epoch,
                    succeeded: errors.is_empty(),
                    error: (!errors.is_empty()).then(|| errors.join("; ")),
                },
                page_payloads: DevelopmentEvidenceAvailability::observed(page_payloads),
            })
        }
        #[cfg(target_os = "windows")]
        NativeBackendCompletion::PdfFile {
            nonce,
            document_identity,
            render_epoch,
            result,
        } => Some(DevelopmentBackendObservation {
            completion: NativeBackendCompletionObservationV1 {
                nonce: *nonce,
                document_run_id: document_identity.document_run_id.clone(),
                envelope_sha256: document_identity.envelope_hash.clone(),
                render_epoch: *render_epoch,
                succeeded: result.is_ok(),
                error: result.as_ref().err().cloned(),
            },
            page_payloads: DevelopmentEvidenceAvailability::unavailable(
                "WebView2 PrintToPdf exposes only the completed PDF file, not one callback payload per page",
            ),
        }),
    }
}

#[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
fn development_evidence_dir() -> Option<PathBuf> {
    std::env::var_os("EBIR_NATIVE_OUTPUT_EVIDENCE_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

#[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
fn write_development_evidence_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .ok_or_else(|| "development evidence path has no parent".to_string())?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "development evidence directory {} could not be created: {error}",
            parent.display()
        )
    })?;
    let temporary = parent.join(format!(
        ".{}.{}.partial",
        path.file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("native-output-observation"),
        uuid::Uuid::new_v4()
    ));
    let result = (|| {
        use std::io::Write;

        let mut options = std::fs::OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary).map_err(|error| {
            format!("development evidence temp file could not be created: {error}")
        })?;
        file.write_all(bytes)
            .map_err(|error| format!("development evidence could not be written: {error}"))?;
        file.sync_all()
            .map_err(|error| format!("development evidence could not be flushed: {error}"))?;
        std::fs::rename(&temporary, path)
            .map_err(|error| format!("development evidence could not be finalized: {error}"))
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temporary);
    }
    result
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn bind_renderer_for_native_output(
    state: &RendererState,
) -> Result<NativeOutputRendererBinding, String> {
    let document_identity = state
        .document_identity
        .clone()
        .ok_or_else(|| "native output has no bound renderer document identity".to_string())?;
    if !state.document_boot_accepted || state.document_identity_rejected {
        return Err("native output renderer document identity is not valid".to_string());
    }
    if state.render_epoch == 0 {
        return Err("native output has no validated renderer epoch".to_string());
    }
    if !state.ready || !state.geometry_print_mode {
        return Err("native output renderer epoch is not ready in print mode".to_string());
    }
    if state.page_rects.is_empty() || state.page_count != Some(state.page_rects.len()) {
        return Err("native output renderer epoch has incomplete page rectangles".to_string());
    }
    let geometry_reports = state.geometry_reports.clone().ok_or_else(|| {
        "native output renderer epoch did not retain both stable geometry observations".to_string()
    })?;
    Ok(NativeOutputRendererBinding {
        document_identity,
        render_epoch: state.render_epoch,
        readiness_revision: state.readiness_revision,
        page_rects: state.page_rects.clone(),
        geometry_reports,
    })
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn renderer_binding_mismatch_reason(
    state: &RendererState,
    binding: &NativeOutputRendererBinding,
) -> Option<String> {
    if state.document_identity.as_ref() != Some(&binding.document_identity)
        || !state.document_boot_accepted
        || state.document_identity_rejected
    {
        return Some("renderer document identity changed after native output started".to_string());
    }
    if state.render_epoch != binding.render_epoch
        || state.readiness_revision != binding.readiness_revision
    {
        return Some("renderer epoch changed after native output started".to_string());
    }
    if !state.ready || !state.geometry_print_mode {
        return Some("renderer readiness was invalidated after native output started".to_string());
    }
    if state.page_count != Some(binding.page_rects.len()) || state.page_rects != binding.page_rects
    {
        return Some("renderer page geometry changed after native output started".to_string());
    }
    if state.geometry_reports.as_ref() != Some(&binding.geometry_reports) {
        return Some(
            "renderer stable geometry observations changed after native output started".to_string(),
        );
    }
    None
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn native_output_timeout_reason(
    kind: HtmlOutputKind,
    backend_started: bool,
    elapsed: Duration,
) -> Option<String> {
    match html_output_timeout_stage(
        kind,
        backend_started,
        elapsed,
        READINESS_TIMEOUT,
        PDF_EXPORT_TIMEOUT,
    ) {
        Some(HtmlOutputTimeoutStage::Preflight) => {
            Some("HTML renderer native output preflight timed out".to_string())
        }
        Some(HtmlOutputTimeoutStage::PdfExportBackend) => {
            Some("native HTML PDF export backend did not complete before its deadline".to_string())
        }
        None => None,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug)]
enum NativeBackendCompletion {
    SystemPrint {
        nonce: u64,
        document_identity: RendererDocumentIdentity,
        render_epoch: u64,
        result: Result<(), String>,
    },
    #[cfg(target_os = "macos")]
    CapturedPages {
        nonce: u64,
        document_identity: RendererDocumentIdentity,
        render_epoch: u64,
        pages: Vec<Result<Vec<u8>, String>>,
    },
    #[cfg(target_os = "windows")]
    PdfFile {
        nonce: u64,
        document_identity: RendererDocumentIdentity,
        render_epoch: u64,
        result: Result<(), String>,
    },
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct MacCaptureBatch {
    nonce: u64,
    document_identity: RendererDocumentIdentity,
    render_epoch: u64,
    pages: Vec<Option<Result<Vec<u8>, String>>>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Default)]
struct NativeBackendBridge {
    completion: Option<NativeBackendCompletion>,
    cancelled_nonces: HashSet<u64>,
    registered_temp_paths: HashMap<u64, PathBuf>,
    cancelled_temp_paths: HashMap<u64, PathBuf>,
    #[cfg(target_os = "macos")]
    mac_capture: Option<MacCaptureBatch>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl NativeBackendBridge {
    fn prepare_for_output(&mut self) {
        self.completion = None;
        #[cfg(target_os = "macos")]
        {
            self.mac_capture = None;
        }
    }

    fn register_temp_path(&mut self, nonce: u64, path: PathBuf) -> Result<(), String> {
        if self.cancelled_nonces.contains(&nonce) {
            let _ = discard_pdf_export_temp(&path);
            return Err("native output was cancelled before its backend started".to_string());
        }
        self.registered_temp_paths.insert(nonce, path);
        Ok(())
    }

    fn record_completion(&mut self, completion: NativeBackendCompletion) {
        let nonce = native_backend_completion_nonce(&completion);
        if self.cancelled_nonces.remove(&nonce) {
            self.discard_registered_temp(nonce);
            self.discard_cancelled_temp(nonce);
            return;
        }
        self.completion = Some(completion);
    }

    fn cancel_output(&mut self, nonce: u64) {
        self.cancelled_nonces.insert(nonce);
        if self
            .completion
            .as_ref()
            .is_some_and(|completion| native_backend_completion_nonce(completion) == nonce)
        {
            self.completion = None;
        }
        #[cfg(target_os = "macos")]
        if self
            .mac_capture
            .as_ref()
            .is_some_and(|capture| capture.nonce == nonce)
        {
            self.mac_capture = None;
        }
        if let Some(path) = self.registered_temp_paths.remove(&nonce) {
            let _ = discard_pdf_export_temp(&path);
            self.cancelled_temp_paths.insert(nonce, path);
        }
    }

    fn finish_output(&mut self, nonce: u64) {
        self.registered_temp_paths.remove(&nonce);
        self.cancelled_nonces.remove(&nonce);
        self.cancelled_temp_paths.remove(&nonce);
    }

    fn discard_registered_temp(&mut self, nonce: u64) {
        if let Some(path) = self.registered_temp_paths.remove(&nonce) {
            let _ = discard_pdf_export_temp(&path);
        }
    }

    fn discard_cancelled_temp(&mut self, nonce: u64) {
        if let Some(path) = self.cancelled_temp_paths.remove(&nonce) {
            let _ = discard_pdf_export_temp(&path);
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl Drop for NativeBackendBridge {
    fn drop(&mut self) {
        for (_, path) in self.registered_temp_paths.drain() {
            let _ = discard_pdf_export_temp(&path);
        }
        for (_, path) in self.cancelled_temp_paths.drain() {
            let _ = discard_pdf_export_temp(&path);
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn native_backend_completion_nonce(completion: &NativeBackendCompletion) -> u64 {
    match completion {
        NativeBackendCompletion::SystemPrint { nonce, .. } => *nonce,
        #[cfg(target_os = "macos")]
        NativeBackendCompletion::CapturedPages { nonce, .. } => *nonce,
        #[cfg(target_os = "windows")]
        NativeBackendCompletion::PdfFile { nonce, .. } => *nonce,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn native_backend_completion_render_epoch(completion: &NativeBackendCompletion) -> u64 {
    match completion {
        NativeBackendCompletion::SystemPrint { render_epoch, .. } => *render_epoch,
        #[cfg(target_os = "macos")]
        NativeBackendCompletion::CapturedPages { render_epoch, .. } => *render_epoch,
        #[cfg(target_os = "windows")]
        NativeBackendCompletion::PdfFile { render_epoch, .. } => *render_epoch,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn native_backend_completion_document_identity(
    completion: &NativeBackendCompletion,
) -> &RendererDocumentIdentity {
    match completion {
        NativeBackendCompletion::SystemPrint {
            document_identity, ..
        } => document_identity,
        #[cfg(target_os = "macos")]
        NativeBackendCompletion::CapturedPages {
            document_identity, ..
        } => document_identity,
        #[cfg(target_os = "windows")]
        NativeBackendCompletion::PdfFile {
            document_identity, ..
        } => document_identity,
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn native_backend_completion_binding_error(
    pending: &PendingNativeOutput,
    state: &RendererState,
    completion: &NativeBackendCompletion,
) -> Option<String> {
    if !pending.backend_started {
        return Some("native backend completed before its renderer epoch was bound".to_string());
    }
    let Some(binding) = pending.binding.as_ref() else {
        return Some("native backend completed without a bound renderer epoch".to_string());
    };
    if native_backend_completion_document_identity(completion) != &binding.document_identity {
        return Some("native backend completion reported a stale renderer document".to_string());
    }
    if native_backend_completion_render_epoch(completion) != binding.render_epoch {
        return Some("native backend completion reported a stale renderer epoch".to_string());
    }
    renderer_binding_mismatch_reason(state, binding)
}

#[cfg(target_os = "macos")]
impl NativeBackendBridge {
    fn begin_macos_capture(
        &mut self,
        nonce: u64,
        document_identity: RendererDocumentIdentity,
        render_epoch: u64,
        page_count: usize,
    ) {
        self.completion = None;
        self.mac_capture = Some(MacCaptureBatch {
            nonce,
            document_identity,
            render_epoch,
            pages: (0..page_count).map(|_| None).collect(),
        });
    }

    fn record_macos_page(
        &mut self,
        nonce: u64,
        page_index: usize,
        result: Result<Vec<u8>, String>,
    ) {
        if self.cancelled_nonces.remove(&nonce) {
            self.discard_registered_temp(nonce);
            self.discard_cancelled_temp(nonce);
            return;
        }
        let Some(batch) = self.mac_capture.as_mut() else {
            return;
        };
        if batch.nonce != nonce || page_index >= batch.pages.len() {
            return;
        }
        batch.pages[page_index] = Some(result);
        if batch.pages.iter().all(Option::is_some) {
            let Some(batch) = self.mac_capture.take() else {
                return;
            };
            let Some(pages) = batch.pages.into_iter().collect::<Option<Vec<_>>>() else {
                return;
            };
            self.record_completion(NativeBackendCompletion::CapturedPages {
                nonce,
                document_identity: batch.document_identity,
                render_epoch: batch.render_epoch,
                pages,
            });
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Deserialize)]
struct RendererIpcMessage {
    document_run_id: String,
    envelope_hash: String,
    #[serde(flatten)]
    message: RendererMessage,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl RendererIpcMessage {
    fn document_identity(&self) -> RendererDocumentIdentity {
        RendererDocumentIdentity {
            document_run_id: self.document_run_id.clone(),
            envelope_hash: self.envelope_hash.clone(),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RendererMessage {
    RendererBoot,
    RendererReady {
        render_epoch: u64,
    },
    RendererInvalidated {
        render_epoch: u64,
    },
    RendererError {
        render_epoch: u64,
        message: String,
    },
    PrintReady {
        nonce: u64,
        render_epoch: u64,
        print_mode: bool,
    },
    PageCount {
        render_epoch: u64,
        print_mode: bool,
        geometry_reports: [RendererGeometryReportMessage; 2],
    },
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Deserialize)]
struct RendererGeometryReportMessage {
    page_count: usize,
    page_width_pt: f64,
    page_height_pt: f64,
    pages: Vec<RendererPageRectMessage>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl RendererGeometryReportMessage {
    fn into_report(self) -> RendererGeometryReport {
        RendererGeometryReport {
            page_count: self.page_count,
            page_width_pt: self.page_width_pt,
            page_height_pt: self.page_height_pt,
            pages: self
                .pages
                .into_iter()
                .map(RendererPageRectMessage::into_rect)
                .collect(),
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn apply_renderer_ipc_message(
    state: &mut RendererState,
    ipc_message: RendererIpcMessage,
    layout_plan: &RenderLayoutPlan,
) {
    let identity = ipc_message.document_identity();
    if state.document_identity_rejected {
        return;
    }
    if !state.accepts_document_identity(&identity) {
        state.reject_document_identity(
            "renderer IPC document run ID or envelope hash did not match the host document",
        );
        return;
    }

    match ipc_message.message {
        RendererMessage::RendererBoot => {
            if state.document_boot_accepted {
                state.reject_document_identity(
                    "renderer document run ID was replayed by a reload or replacement document",
                );
            } else {
                state.document_boot_accepted = true;
            }
        }
        _ if !state.document_boot_accepted => state.reject_document_identity(
            "renderer IPC arrived before the host document identity boot handshake",
        ),
        message => apply_renderer_message(state, message, layout_plan),
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Deserialize)]
struct RendererPageRectMessage {
    x: f64,
    y: f64,
    width: f64,
    height: f64,
    client_width: f64,
    client_height: f64,
    scroll_width: f64,
    scroll_height: f64,
    descendant_overflow_x: usize,
    descendant_overflow_y: usize,
    descendant_clipped_x: usize,
    descendant_clipped_y: usize,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl RendererPageRectMessage {
    fn into_rect(self) -> RendererPageRect {
        RendererPageRect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            client_width: self.client_width,
            client_height: self.client_height,
            scroll_width: self.scroll_width,
            scroll_height: self.scroll_height,
            descendant_overflow_x: self.descendant_overflow_x,
            descendant_overflow_y: self.descendant_overflow_y,
            descendant_clipped_x: self.descendant_clipped_x,
            descendant_clipped_y: self.descendant_clipped_y,
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn apply_renderer_message(
    state: &mut RendererState,
    message: RendererMessage,
    layout_plan: &RenderLayoutPlan,
) {
    match message {
        RendererMessage::RendererBoot => {
            state.reject_document_identity(
                "renderer boot messages must pass through the document identity gate",
            );
        }
        RendererMessage::RendererReady { render_epoch } => {
            if state.accepts_epoch(render_epoch) {
                state.ready = true;
            }
        }
        RendererMessage::RendererInvalidated { render_epoch } => {
            state.invalidate_for_epoch(render_epoch);
        }
        RendererMessage::RendererError {
            render_epoch,
            message,
        } => {
            if state.accepts_epoch(render_epoch) {
                state.error = Some(message);
            }
        }
        RendererMessage::PrintReady {
            nonce,
            render_epoch,
            print_mode,
        } => state.accept_print_ready(nonce, render_epoch, print_mode),
        RendererMessage::PageCount {
            render_epoch,
            print_mode,
            geometry_reports,
        } => {
            if !state.accepts_epoch(render_epoch) {
                return;
            }
            let [first, second] = geometry_reports.map(RendererGeometryReportMessage::into_report);
            let validation = if first != second {
                Err(
                    "the renderer's two stable geometry observations were not identical"
                        .to_string(),
                )
            } else {
                validate_renderer_geometry(&first, layout_plan)
                    .and_then(|()| validate_renderer_geometry(&second, layout_plan))
            };
            match validation {
                Ok(()) => {
                    state.page_count = Some(second.page_count);
                    state.page_rects = second.pages.clone();
                    state.geometry_reports = Some([first, second]);
                    state.geometry_print_mode = print_mode;
                }
                Err(error) => {
                    state.page_count = None;
                    state.page_rects.clear();
                    state.geometry_reports = None;
                    state.geometry_print_mode = false;
                    state.error = Some(error);
                }
            }
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub struct HtmlFormPreviewView {
    prepared: PreparedHtmlPreview,
    webview: Option<Entity<WebView>>,
    bridge_state: Arc<Mutex<RendererState>>,
    native_backend_bridge: Arc<Mutex<NativeBackendBridge>>,
    renderer_state: RendererState,
    status: String,
    pdf_expectation: PdfExpectation,
    default_pdf_name: String,
    output_state: HtmlOutputState,
    next_output_nonce: u64,
    pending_output: Option<PendingNativeOutput>,
    _readiness_task: Task<()>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl HtmlFormPreviewView {
    pub fn new(prepared: PreparedHtmlPreview, window: &mut Window, cx: &mut Context<Self>) -> Self {
        use raw_window_handle::HasWindowHandle;

        let retry_prepared = prepared.clone();
        let document_identity =
            RendererDocumentIdentity::host_generated(&prepared.pdf_expectation.envelope_hash)
                .expect("prepared HTML previews always contain a canonical envelope hash");
        let renderer_state = RendererState::for_document(document_identity.clone());
        let bridge_state = Arc::new(Mutex::new(renderer_state.clone()));
        let native_backend_bridge = Arc::new(Mutex::new(NativeBackendBridge::default()));
        let ipc_state = bridge_state.clone();
        let protocol_root = prepared.entry.parent().map(PathBuf::from);
        let layout_plan = prepared.layout_plan;
        let expected_page_count = layout_plan.expected_page_count;
        let initialization_script = format!(
            "{}\n{}",
            renderer_document_identity_script(&document_identity),
            prepared.initialization_script
        );

        let result = window
            .window_handle()
            .map_err(|error| error.to_string())
            .and_then(|window_handle| {
                let builder = wry::WebViewBuilder::new();
                #[cfg(target_os = "windows")]
                let builder = builder
                    .with_browser_accelerator_keys(false)
                    .with_default_context_menus(false);
                builder
                    .with_incognito(RENDERER_WEBVIEW_IS_INCOGNITO)
                    .with_custom_protocol("ebirforms".into(), move |_webview_id, request| {
                        renderer_protocol_response(protocol_root.as_deref(), request)
                    })
                    .with_url(&prepared.url)
                    .with_initialization_script(&initialization_script)
                    .with_navigation_handler(|candidate| {
                        let Ok(url) = url::Url::parse(&candidate) else {
                            return false;
                        };
                        let is_renderer_url = (url.scheme() == "ebirforms"
                            && url.host_str() == Some("localhost"))
                            || (cfg!(target_os = "windows")
                                && url.scheme() == "http"
                                && url.host_str() == Some("ebirforms.localhost"));
                        is_renderer_url && renderer_relative_path(url.path()).is_some()
                    })
                    .with_ipc_handler(move |request| {
                        let message = match serde_json::from_str::<RendererIpcMessage>(request.body())
                        {
                            Ok(message) => message,
                            Err(_) => {
                                if let Ok(mut state) = ipc_state.lock() {
                                    state.reject_document_identity(
                                        "renderer IPC omitted or malformed the immutable document identity",
                                    );
                                }
                            tracing::warn!(
                                body_bytes = request.body().len(),
                                "ignored malformed renderer IPC"
                            );
                                return;
                            }
                        };
                        let Ok(mut state) = ipc_state.lock() else {
                            return;
                        };
                        apply_renderer_ipc_message(&mut state, message, &layout_plan);
                    })
                    .build_as_child(&window_handle)
                    .map_err(|error| error.to_string())
            });

        let (webview, status) = match result {
            Ok(webview) => (
                Some(cx.new(|cx| WebView::new(webview, window, cx))),
                "Preparing HTML print preview...".to_string(),
            ),
            Err(error) => {
                tracing::error!(
                    error_bytes = error.len(),
                    "HTML print-preview WebView construction failed"
                );
                if let Ok(mut state) = bridge_state.lock() {
                    state.error = Some(format!("WebView construction failed: {error}"));
                }
                (None, format!("HTML print preview failed: {error}"))
            }
        };

        let initial_readiness_deadline = Instant::now() + READINESS_TIMEOUT;
        let overall_initial_readiness_deadline = initial_readiness_deadline;
        let poll_state = bridge_state.clone();
        let poll_native_backend = native_backend_bridge.clone();
        let readiness_task = cx.spawn(async move |this, cx| {
            let mut reported_page_count = None;
            let mut initial_readiness_completed = false;
            let mut observed_readiness_revision = 0;
            let mut readiness_deadline = initial_readiness_deadline;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;
                let snapshot = poll_state.lock().ok().map(|state| state.clone());
                let backend_completion = poll_native_backend
                    .lock()
                    .ok()
                    .and_then(|mut bridge| bridge.completion.take());
                let readiness_was_invalidated = snapshot.as_ref().is_some_and(|snapshot| {
                    snapshot.readiness_revision != observed_readiness_revision
                });
                if readiness_was_invalidated {
                    let snapshot = snapshot
                        .as_ref()
                        .expect("readiness invalidation requires a renderer snapshot");
                    observed_readiness_revision = snapshot.readiness_revision;
                    reported_page_count = None;
                    readiness_deadline = Instant::now() + READINESS_TIMEOUT;
                }
                // Every DOM/font/layout invalidation starts a new bounded
                // readiness epoch. A preview cannot remain printable on an old
                // geometry report after its document changes.
                let now = Instant::now();
                let timed_out = renderer_readiness_timed_out(
                    reported_page_count.is_some(),
                    now >= readiness_deadline,
                    initial_readiness_completed,
                    now >= overall_initial_readiness_deadline,
                );
                let update_result = this.update(cx, |this, cx| {
                    let mut should_notify = false;
                    if let Some(snapshot) = snapshot {
                        if snapshot != this.renderer_state {
                            this.renderer_state = snapshot;
                            should_notify = true;
                        }
                    }
                    if readiness_was_invalidated {
                        this.status = "Layout changed; validating print geometry again".to_string();
                        should_notify = true;
                    }
                    let output_binding_failure = this.pending_output.as_ref().and_then(|pending| {
                        if !pending.backend_started {
                            return None;
                        }
                        match pending.binding.as_ref() {
                            Some(binding) => {
                                renderer_binding_mismatch_reason(&this.renderer_state, binding)
                            }
                            None => Some(
                                "native output started without a bound renderer epoch".to_string(),
                            ),
                        }
                    });
                    if let Some(reason) = output_binding_failure {
                        this.fail_pending_output(reason, cx);
                        should_notify = true;
                    }
                    match renderer_readiness_decision(
                        this.renderer_state.ready,
                        this.renderer_state.page_count,
                        this.renderer_state.error.as_deref(),
                        timed_out,
                    ) {
                        RendererReadinessDecision::Pending => {
                            if reported_page_count.is_some() {
                                reported_page_count = None;
                                this.status =
                                    "Layout changed; validating print geometry again".to_string();
                                should_notify = true;
                            }
                        }
                        RendererReadinessDecision::Ready { page_count } => {
                            initial_readiness_completed = true;
                            if reported_page_count != Some(page_count) {
                                this.status = format!("{page_count} printable page(s) ready");
                                reported_page_count = Some(page_count);
                                should_notify = true;
                            }
                        }
                        RendererReadinessDecision::Fallback(reason) => {
                            if this.pending_output.is_some() {
                                this.fail_pending_output(reason, cx);
                                should_notify = true;
                            } else if this.status != reason {
                                this.status = reason;
                                should_notify = true;
                            }
                        }
                    }

                    if let Some(completion) = backend_completion {
                        this.finish_native_backend(completion, cx);
                        should_notify = true;
                    }

                    let timeout_reason = this.pending_output.as_ref().and_then(|pending| {
                        native_output_timeout_reason(
                            pending.kind,
                            pending.backend_started,
                            pending.started_at.elapsed(),
                        )
                    });
                    if let Some(reason) = timeout_reason {
                        this.fail_pending_output(reason, cx);
                        should_notify = true;
                    }

                    if let Some(pending_nonce) =
                        this.pending_output.as_ref().map(|pending| pending.nonce)
                        && this.renderer_state.print_ready_nonce == Some(pending_nonce)
                    {
                        match native_print_decision(
                            this.renderer_state.ready,
                            this.renderer_state.page_count,
                            this.renderer_state.error.as_deref(),
                            this.webview.is_some(),
                        ) {
                            NativePrintDecision::StartPrint => {
                                if let Err(error) = this.start_validated_native_output(cx) {
                                    this.fail_pending_output(error, cx);
                                }
                                should_notify = true;
                            }
                            NativePrintDecision::WaitForRenderer => {}
                            NativePrintDecision::Fallback(reason) => {
                                this.fail_pending_output(reason, cx);
                                should_notify = true;
                            }
                        }
                    }
                    if should_notify {
                        cx.notify();
                    }
                });
                if update_result.is_err() {
                    break;
                }
            }
        });

        Self {
            prepared: retry_prepared,
            webview,
            bridge_state,
            native_backend_bridge,
            renderer_state,
            status,
            pdf_expectation: prepared.pdf_expectation,
            default_pdf_name: prepared.default_pdf_name,
            output_state: HtmlOutputState::Idle,
            next_output_nonce: 0,
            pending_output: None,
            _readiness_task: readiness_task,
        }
    }

    fn begin_native_output(
        &mut self,
        kind: HtmlOutputKind,
        destination: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) -> Result<(), String> {
        if self.pending_output.is_some() {
            return Err("another native HTML output operation is already running".to_string());
        }
        match native_print_decision(
            self.renderer_state.ready,
            self.renderer_state.page_count,
            self.renderer_state.error.as_deref(),
            self.webview.is_some(),
        ) {
            NativePrintDecision::WaitForRenderer => {
                return Err("HTML renderer is not ready for native output".to_string());
            }
            NativePrintDecision::Fallback(reason) => return Err(reason),
            NativePrintDecision::StartPrint => {}
        }
        let Some(webview) = self.webview.clone() else {
            return Err("HTML renderer WebView disappeared before native output".to_string());
        };

        self.next_output_nonce = self.next_output_nonce.checked_add(1).unwrap_or(1);
        let nonce = self.next_output_nonce;
        self.renderer_state.print_ready_nonce = None;
        if let Ok(mut state) = self.bridge_state.lock() {
            state.print_ready_nonce = None;
        }
        if let Ok(mut bridge) = self.native_backend_bridge.lock() {
            bridge.prepare_for_output();
        }
        self.output_state = HtmlOutputState::Validating {
            kind,
            nonce: nonce.to_string(),
            destination: destination.clone(),
        };
        self.pending_output = Some(PendingNativeOutput::validating(kind, nonce, destination));
        self.status = match kind {
            HtmlOutputKind::SystemPrint => {
                "Validating fonts and geometry for system print...".to_string()
            }
            HtmlOutputKind::PdfExport => {
                "Validating fonts and geometry for PDF export...".to_string()
            }
        };

        match webview.update(cx, |webview, _| {
            webview
                .raw()
                .evaluate_script(&native_print_preflight_script(nonce))
        }) {
            Ok(()) => {
                cx.notify();
                Ok(())
            }
            Err(error) => Err(format!(
                "HTML renderer native output preflight failed to start: {error}"
            )),
        }
    }

    fn choose_pdf_destination(&mut self, cx: &mut Context<Self>) {
        if self.pending_output.is_some() {
            return;
        }
        let default_name = self.default_pdf_name.clone();
        cx.spawn(async move |this, cx| {
            let Some(target_handle) = rfd::AsyncFileDialog::new()
                .set_file_name(&default_name)
                .add_filter("PDF", &["pdf"])
                .save_file()
                .await
            else {
                return;
            };
            let destination = target_handle.path().to_path_buf();
            let _ = this.update(cx, |this, cx| {
                if let Err(error) =
                    this.begin_native_output(HtmlOutputKind::PdfExport, Some(destination), cx)
                {
                    this.fail_pending_output(error, cx);
                    cx.notify();
                }
            });
        })
        .detach();
    }

    fn start_validated_native_output(&mut self, cx: &mut Context<Self>) -> Result<(), String> {
        let Some(pending) = self.pending_output.as_ref() else {
            return Err(
                "native output preflight completed without a pending operation".to_string(),
            );
        };
        if pending.backend_started {
            return Ok(());
        }
        let kind = pending.kind;
        let nonce = pending.nonce;
        let destination = pending.destination.clone();
        let Some(webview) = self.webview.clone() else {
            return Err("HTML renderer WebView disappeared before native output".to_string());
        };
        let binding = bind_renderer_for_native_output(&self.renderer_state)?;
        let document_identity = binding.document_identity.clone();
        let render_epoch = binding.render_epoch;
        if let Some(pending) = self.pending_output.as_mut() {
            #[cfg(feature = "dev-tools")]
            {
                if !pending.preflight_consumptions.is_empty() {
                    return Err(
                        "native output preflight nonce was already consumed for this operation"
                            .to_string(),
                    );
                }
                pending.preflight_consumptions.push(nonce);
            }
            pending.binding = Some(binding.clone());
        }

        match kind {
            HtmlOutputKind::SystemPrint => {
                if let Some(pending) = self.pending_output.as_mut() {
                    pending.backend_started = true;
                }
                self.output_state = HtmlOutputState::Running {
                    kind,
                    temp_path: None,
                };
                #[cfg(target_os = "windows")]
                {
                    let expectation = self.pdf_expectation.clone();
                    let bridge = self.native_backend_bridge.clone();
                    webview
                        .update(cx, move |webview, _| {
                            start_windows_system_print(
                                webview.raw(),
                                &expectation,
                                nonce,
                                document_identity,
                                render_epoch,
                                bridge,
                            )
                        })
                        .map_err(|error| {
                            format!("HTML renderer native print failed to start: {error}")
                        })?;
                    self.status =
                        "Sending validated form to the Windows print service...".to_string();
                    Ok(())
                }
                #[cfg(target_os = "macos")]
                {
                    let expectation = self.pdf_expectation.clone();
                    let bridge = self.native_backend_bridge.clone();
                    webview
                        .update(cx, move |webview, _| {
                            start_macos_system_print(
                                webview.raw(),
                                &expectation,
                                nonce,
                                document_identity,
                                render_epoch,
                                bridge,
                            )
                        })
                        .map_err(|error| {
                            format!("HTML renderer native print failed to start: {error}")
                        })?;
                    self.status =
                        "macOS print dialog opened; waiting for native completion...".to_string();
                    Ok(())
                }
            }
            HtmlOutputKind::PdfExport => {
                let destination = destination.ok_or_else(|| {
                    "PDF export preflight completed without a destination".to_string()
                })?;
                let temp_path = create_pdf_export_temp(&destination).map_err(|error| {
                    format!("PDF export temp file could not be created: {error}")
                })?;
                let registration = self
                    .native_backend_bridge
                    .lock()
                    .map_err(|_| "native PDF backend state is unavailable".to_string())
                    .and_then(|mut bridge| bridge.register_temp_path(nonce, temp_path.clone()));
                if let Err(error) = registration {
                    let _ = discard_pdf_export_temp(&temp_path);
                    return Err(error);
                }
                if let Some(pending) = self.pending_output.as_mut() {
                    pending.backend_started = true;
                    pending.temp_path = Some(temp_path.clone());
                }
                self.output_state = HtmlOutputState::Running {
                    kind,
                    temp_path: Some(temp_path.clone()),
                };
                self.status = "Generating validated PDF...".to_string();

                let bridge = self.native_backend_bridge.clone();
                #[cfg(target_os = "macos")]
                let start_result = {
                    let page_rects = binding.page_rects;
                    let document_identity = document_identity.clone();
                    webview.update(cx, move |webview, _| {
                        start_macos_pdf_capture(
                            webview.raw(),
                            &page_rects,
                            nonce,
                            document_identity,
                            render_epoch,
                            bridge,
                        )
                    })
                };
                #[cfg(target_os = "windows")]
                let start_result = {
                    let expectation = self.pdf_expectation.clone();
                    webview.update(cx, move |webview, _| {
                        start_windows_pdf_export(
                            webview.raw(),
                            &temp_path,
                            &expectation,
                            nonce,
                            document_identity,
                            render_epoch,
                            bridge,
                        )
                    })
                };
                start_result
                    .map_err(|error| format!("native PDF backend could not start: {error}"))?;
                Ok(())
            }
        }
    }

    fn finish_native_backend(
        &mut self,
        completion: NativeBackendCompletion,
        cx: &mut Context<Self>,
    ) {
        let completion_nonce = native_backend_completion_nonce(&completion);
        let completion_kind = match &completion {
            NativeBackendCompletion::SystemPrint { .. } => HtmlOutputKind::SystemPrint,
            #[cfg(target_os = "macos")]
            NativeBackendCompletion::CapturedPages { .. } => HtmlOutputKind::PdfExport,
            #[cfg(target_os = "windows")]
            NativeBackendCompletion::PdfFile { .. } => HtmlOutputKind::PdfExport,
        };
        let Some((pending_nonce, pending_kind)) = self
            .pending_output
            .as_ref()
            .map(|pending| (pending.nonce, pending.kind))
        else {
            return;
        };
        if pending_nonce != completion_nonce || pending_kind != completion_kind {
            return;
        }
        let binding_error = self.pending_output.as_ref().and_then(|pending| {
            native_backend_completion_binding_error(pending, &self.renderer_state, &completion)
        });
        if let Some(reason) = binding_error {
            self.fail_pending_output(reason, cx);
            return;
        }

        let completion = match completion {
            NativeBackendCompletion::SystemPrint { result, .. } => {
                match result {
                    Ok(()) => {
                        self.finish_native_output_state(completion_nonce);
                        self.pending_output = None;
                        self.output_state = HtmlOutputState::Idle;
                        self.status = if cfg!(target_os = "macos") {
                            "Validated form completed the macOS print operation".to_string()
                        } else {
                            "Validated form was accepted by the Windows print service".to_string()
                        };
                        self.leave_native_output_mode(cx);
                    }
                    Err(error) => self.fail_pending_output(
                        format!("HTML renderer system print failed: {error}"),
                        cx,
                    ),
                }
                return;
            }
            #[cfg(target_os = "macos")]
            completion @ NativeBackendCompletion::CapturedPages { .. } => completion,
            #[cfg(target_os = "windows")]
            completion @ NativeBackendCompletion::PdfFile { .. } => completion,
        };

        let Some(temp_path) = self
            .pending_output
            .as_ref()
            .and_then(|pending| pending.temp_path.clone())
        else {
            self.fail_pending_output(
                "native PDF backend completed without a temporary file".to_string(),
                cx,
            );
            return;
        };
        let Some(destination) = self
            .pending_output
            .as_ref()
            .and_then(|pending| pending.destination.clone())
        else {
            self.fail_pending_output(
                "native PDF backend completed without a destination".to_string(),
                cx,
            );
            return;
        };

        #[cfg(feature = "dev-tools")]
        let development_backend = development_backend_observation(&completion);

        #[cfg(target_os = "macos")]
        let backend_result = match completion {
            NativeBackendCompletion::CapturedPages { pages, .. } => {
                finalize_macos_pdf_capture(pages, &temp_path, &destination, &self.pdf_expectation)
            }
            NativeBackendCompletion::SystemPrint { .. } => {
                Err("system print completion reached the PDF export finalizer".to_string())
            }
        };
        #[cfg(target_os = "windows")]
        let backend_result = match completion {
            NativeBackendCompletion::PdfFile { result, .. } => result,
            NativeBackendCompletion::SystemPrint { .. } => {
                Err("system print completion reached the PDF export finalizer".to_string())
            }
        };

        #[cfg(target_os = "macos")]
        let export_result = backend_result;
        #[cfg(target_os = "windows")]
        let export_result = backend_result.and_then(|()| {
            finalize_pdf_export(&temp_path, &destination, &self.pdf_expectation)
                .map_err(|error| error.to_string())
        });
        self.leave_native_output_mode(cx);
        match export_result {
            Ok(validation) => {
                #[cfg(feature = "dev-tools")]
                if let Some(backend) = development_backend.as_ref() {
                    let evidence_result = self
                        .pending_output
                        .as_ref()
                        .ok_or_else(|| {
                            "development evidence lost the pending native output".to_string()
                        })
                        .and_then(|pending| {
                            self.write_development_pdf_observation(
                                pending,
                                backend,
                                &validation,
                                &destination,
                            )
                        });
                    match evidence_result {
                        Ok(Some(path)) => tracing::info!(
                            path = %path.display(),
                            "wrote non-promotional native-output observation"
                        ),
                        Ok(None) => {}
                        Err(error) => tracing::warn!(
                            error = %error,
                            "could not write non-promotional native-output observation"
                        ),
                    }
                }
                self.finish_native_output_state(completion_nonce);
                self.pending_output = None;
                self.output_state = HtmlOutputState::Idle;
                self.status = format!("PDF exported to {}", destination.display());
            }
            Err(error) => {
                self.fail_pending_output(format!("HTML renderer PDF export failed: {error}"), cx)
            }
        }
    }

    #[cfg(feature = "dev-tools")]
    fn write_development_pdf_observation(
        &self,
        pending: &PendingNativeOutput,
        backend: &DevelopmentBackendObservation,
        validation: &PdfValidationReport,
        destination: &Path,
    ) -> Result<Option<PathBuf>, String> {
        let Some(evidence_dir) = development_evidence_dir() else {
            return Ok(None);
        };
        let binding = pending.binding.as_ref().ok_or_else(|| {
            "development evidence has no renderer document/epoch binding".to_string()
        })?;
        let [first_report, second_report] = &binding.geometry_reports;
        let first = GeometryReportEvidenceV1::from(first_report);
        let second = GeometryReportEvidenceV1::from(second_report);
        let geometry_page_rect_sha256 =
            geometry_page_rect_sha256(&first).map_err(|error| error.to_string())?;
        let renderer_bundle_sha256 = development_renderer_bundle_hash(&self.prepared.entry);
        let renderer_build_identity =
            development_renderer_build_identity(&self.prepared.entry, &renderer_bundle_sha256);
        let package_sha256 = development_package_hash();
        let output_snapshot = development_destination_snapshot(destination);
        let output_pdf_sha256 = match &output_snapshot {
            DevelopmentDestinationSnapshotV1::File { sha256 } => {
                DevelopmentEvidenceAvailability::observed(sha256.clone())
            }
            DevelopmentDestinationSnapshotV1::Absent => {
                DevelopmentEvidenceAvailability::unavailable(
                    "finalized export destination is unexpectedly absent",
                )
            }
            DevelopmentDestinationSnapshotV1::Unavailable { reason } => {
                DevelopmentEvidenceAvailability::unavailable(format!(
                    "finalized export destination hash is unavailable: {reason}"
                ))
            }
        };

        let mut strict_verifier_gaps = vec![
            "runtime observation collector is development-only and is not attested"
                .to_string(),
            "a real failed export against a pre-existing destination was not exercised by this successful run"
                .to_string(),
            "system-print, signed-package, packaged network-denial, and rollback evidence are outside this observation"
                .to_string(),
        ];
        if matches!(
            &renderer_build_identity.source_revision,
            DevelopmentEvidenceAvailability::Unavailable { .. }
        ) {
            strict_verifier_gaps.push(
                "canonical source revision is unavailable or not bound to the running renderer"
                    .to_string(),
            );
        }
        if matches!(
            &renderer_build_identity.expected_renderer_bundle_sha256,
            DevelopmentEvidenceAvailability::Unavailable { .. }
        ) {
            strict_verifier_gaps.push(
                "independent offline renderer bundle hash is unavailable from the build identity"
                    .to_string(),
            );
        }
        if let Some(gap) = renderer_build_identity.binding_gap.clone() {
            strict_verifier_gaps.push(gap);
        }
        if matches!(
            &package_sha256,
            DevelopmentEvidenceAvailability::Unavailable { .. }
        ) {
            strict_verifier_gaps.push(
                "running package hash is unavailable for this cargo-run or unbound package layout"
                    .to_string(),
            );
        }
        if matches!(
            &renderer_bundle_sha256,
            DevelopmentEvidenceAvailability::Unavailable { .. }
        ) {
            strict_verifier_gaps
                .push("runtime renderer bundle could not be stably hashed".to_string());
        }
        if matches!(
            &backend.page_payloads,
            DevelopmentEvidenceAvailability::Unavailable { .. }
        ) {
            strict_verifier_gaps.push(
                "native backend does not expose per-page callback payload hashes".to_string(),
            );
        }
        if matches!(
            &output_pdf_sha256,
            DevelopmentEvidenceAvailability::Unavailable { .. }
        ) {
            strict_verifier_gaps.push("final output PDF hash is unavailable".to_string());
        }

        let observation = DevelopmentNativeOutputObservationV1 {
            schema_version: DEVELOPMENT_NATIVE_OUTPUT_OBSERVATION_SCHEMA_VERSION,
            scope: DevelopmentEvidenceScope::DevelopmentDiagnostic,
            promotion_eligible: false,
            platform: development_platform(),
            backend: development_backend(),
            form_code: self.pdf_expectation.form_code.clone(),
            form_revision: self.pdf_expectation.revision.clone(),
            document_run_id: binding.document_identity.document_run_id.clone(),
            envelope_sha256: binding.document_identity.envelope_hash.clone(),
            source_revision: renderer_build_identity.source_revision,
            package_sha256,
            renderer_bundle_sha256,
            independently_expected_renderer_bundle_sha256: renderer_build_identity
                .expected_renderer_bundle_sha256,
            geometry_reports: [first.clone(), second],
            geometry_page_rect_sha256,
            clipping_totals: ClippingCountersV1::from_geometry(&first),
            nonce: NativeNonceObservationV1 {
                issued_nonce: pending.nonce,
                preflight_consumptions: pending.preflight_consumptions.clone(),
                backend_completion_nonce: DevelopmentEvidenceAvailability::observed(
                    backend.completion.nonce,
                ),
            },
            render_epoch: binding.render_epoch,
            readiness_revision: binding.readiness_revision,
            backend_completion: DevelopmentEvidenceAvailability::observed(
                backend.completion.clone(),
            ),
            native_page_payloads: backend.page_payloads.clone(),
            output_pdf_sha256,
            pdf_validation: DevelopmentEvidenceAvailability::observed(
                PdfValidationEvidenceV1::from(validation),
            ),
            destination_outcome: DevelopmentDestinationOutcomeV1::ExportSucceeded {
                before: pending.destination_before.clone(),
                after: output_snapshot,
                temporary_file_remaining: pending.temp_path.as_deref().is_some_and(Path::exists),
                preservation_failure_case_exercised: false,
            },
            strict_verifier_gaps,
        };
        let encoded = encode_development_native_output_observation(&observation)
            .map_err(|error| error.to_string())?;

        std::fs::create_dir_all(&evidence_dir).map_err(|error| {
            format!(
                "development evidence directory {} could not be created: {error}",
                evidence_dir.display()
            )
        })?;
        let evidence_dir = evidence_dir.canonicalize().map_err(|error| {
            format!("development evidence directory could not be resolved: {error}")
        })?;
        if let Some(renderer_root) = self.prepared.entry.parent() {
            let renderer_root = renderer_root
                .canonicalize()
                .map_err(|error| format!("runtime renderer root could not be resolved: {error}"))?;
            if evidence_dir.starts_with(&renderer_root) {
                return Err(
                    "development evidence directory must remain outside the renderer bundle"
                        .to_string(),
                );
            }
        }
        let stem = format!(
            "native-output-{}-{}",
            binding.document_identity.document_run_id, pending.nonce
        );
        let envelope_path = evidence_dir.join(format!("{stem}.envelope.json"));
        let observed_envelope_hash = format!(
            "{:x}",
            Sha256::digest(self.prepared.envelope_json.as_bytes())
        );
        if observed_envelope_hash != binding.document_identity.envelope_hash {
            return Err(
                "immutable envelope bytes no longer match the renderer document identity"
                    .to_string(),
            );
        }
        write_development_evidence_file(&envelope_path, self.prepared.envelope_json.as_bytes())?;
        let observation_path = evidence_dir.join(format!("{stem}.observation.json"));
        write_development_evidence_file(&observation_path, &encoded)?;
        Ok(Some(observation_path))
    }

    fn leave_native_output_mode(&self, cx: &mut Context<Self>) {
        if let Some(webview) = self.webview.clone() {
            let _ = webview.update(cx, |webview, _| {
                webview
                    .raw()
                    .evaluate_script(native_output_cleanup_script())
            });
        }
    }

    fn finish_native_output_state(&self, nonce: u64) {
        if let Ok(mut bridge) = self.native_backend_bridge.lock() {
            bridge.finish_output(nonce);
        }
    }

    fn cancel_pending_output_state(&mut self) {
        let Some(pending) = self.pending_output.take() else {
            return;
        };
        if let Some(temp_path) = pending.temp_path.as_deref() {
            let _ = discard_pdf_export_temp(temp_path);
        }
        if let Ok(mut bridge) = self.native_backend_bridge.lock() {
            bridge.cancel_output(pending.nonce);
        }
        self.renderer_state.print_ready_nonce = None;
        if let Ok(mut renderer) = self.bridge_state.lock() {
            renderer.print_ready_nonce = None;
        }
    }

    fn fail_pending_output(&mut self, error: String, cx: &mut Context<Self>) {
        self.cancel_pending_output_state();
        self.output_state = HtmlOutputState::Failed(error.clone());
        self.status = error;
        self.leave_native_output_mode(cx);
    }

    fn request_retry(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.cancel_pending_output_state();
        self.leave_native_output_mode(cx);
        self.webview.take();
        let prepared = self.prepared.clone();
        *self = Self::new(prepared, window, cx);
        cx.notify();
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl Drop for HtmlFormPreviewView {
    fn drop(&mut self) {
        self.cancel_pending_output_state();
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl Render for HtmlFormPreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let native_output_enabled = matches!(
            native_print_decision(
                self.renderer_state.ready,
                self.renderer_state.page_count,
                self.renderer_state.error.as_deref(),
                self.webview.is_some(),
            ),
            NativePrintDecision::StartPrint
        ) && self.pending_output.is_none()
            && !matches!(
                &self.output_state,
                HtmlOutputState::Validating { .. } | HtmlOutputState::Running { .. }
            );

        div()
            .size_full()
            .flex()
            .flex_col()
            .child(
                div()
                    .h(px(48.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .border_b_1()
                    .border_color(cx.theme().border)
                    .child(self.status.clone())
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(
                                Button::new("html-export-pdf")
                                    .label("Export PDF")
                                    .disabled(!native_output_enabled)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.choose_pdf_destination(cx);
                                    })),
                            )
                            .child(
                                Button::new("html-print")
                                    .label("Print")
                                    .disabled(!native_output_enabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        match native_print_decision(
                                            this.renderer_state.ready,
                                            this.renderer_state.page_count,
                                            this.renderer_state.error.as_deref(),
                                            this.webview.is_some(),
                                        ) {
                                            NativePrintDecision::WaitForRenderer => {
                                                this.status = "HTML renderer is not ready to print"
                                                    .to_string();
                                                cx.notify();
                                            }
                                            NativePrintDecision::Fallback(reason) => {
                                                this.status = reason;
                                                cx.notify();
                                            }
                                            NativePrintDecision::StartPrint => {
                                                if let Err(error) = this.begin_native_output(
                                                    HtmlOutputKind::SystemPrint,
                                                    None,
                                                    cx,
                                                ) {
                                                    this.fail_pending_output(error, cx);
                                                    cx.notify();
                                                }
                                            }
                                        }
                                    })),
                            )
                            .child(
                                Button::new("html-preview-retry")
                                    .label("Retry")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        this.request_retry(window, cx);
                                    })),
                            )
                            .child(Button::new("html-preview-close").label("Close").on_click(
                                cx.listener(|this, _, window, cx| {
                                    this.cancel_pending_output_state();
                                    this.leave_native_output_mode(cx);
                                    window.remove_window();
                                }),
                            )),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .when_some(self.webview.clone(), |this, webview| this.child(webview))
                    .when(self.webview.is_none(), |this| {
                        this.p_6().child(
                            self.renderer_state
                                .error
                                .clone()
                                .unwrap_or_else(|| "HTML preview could not be initialized".into()),
                        )
                    }),
            )
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
pub(crate) fn renderer_protocol_response(
    root: Option<&Path>,
    request: wry::http::Request<Vec<u8>>,
) -> wry::http::Response<Cow<'static, [u8]>> {
    let Some(root) = root else {
        return renderer_error_response(500, "renderer root is unavailable");
    };
    let Some(relative_path) = renderer_relative_path(request.uri().path()) else {
        return renderer_error_response(403, "renderer path is not allowed");
    };
    let canonical_root = match root.canonicalize() {
        Ok(root) => root,
        Err(_) => return renderer_error_response(500, "renderer root cannot be resolved"),
    };
    let canonical_path = match canonical_root.join(relative_path).canonicalize() {
        Ok(path) if path.starts_with(&canonical_root) && path.is_file() => path,
        _ => return renderer_error_response(404, "renderer asset was not found"),
    };
    let content = match std::fs::read(&canonical_path) {
        Ok(content) => content,
        Err(_) => return renderer_error_response(500, "renderer asset cannot be read"),
    };
    wry::http::Response::builder()
        .status(200)
        .header(
            wry::http::header::CONTENT_TYPE,
            renderer_content_type(&canonical_path),
        )
        .header(wry::http::header::CACHE_CONTROL, "no-store")
        .header("Content-Security-Policy", RENDERER_CONTENT_SECURITY_POLICY)
        .header("Permissions-Policy", RENDERER_PERMISSIONS_POLICY)
        .header("X-DNS-Prefetch-Control", "off")
        .header("X-Content-Type-Options", "nosniff")
        .header("Referrer-Policy", "no-referrer")
        .body(Cow::Owned(content))
        .unwrap_or_else(|_| renderer_error_response(500, "renderer response could not be built"))
}

pub(crate) fn renderer_relative_path(uri_path: &str) -> Option<PathBuf> {
    let path = uri_path.trim_start_matches('/');
    let path = if path.is_empty() { "index.html" } else { path };
    let relative = PathBuf::from(path);
    (!relative.is_absolute()
        && !relative.components().any(|part| {
            matches!(
                part,
                std::path::Component::ParentDir | std::path::Component::Prefix(_)
            )
        }))
    .then_some(relative)
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn renderer_content_type(path: &Path) -> &'static str {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("html") => "text/html; charset=utf-8",
        Some("js" | "mjs") => "text/javascript; charset=utf-8",
        Some("css") => "text/css; charset=utf-8",
        Some("json") => "application/json; charset=utf-8",
        Some("svg") => "image/svg+xml",
        Some("png") => "image/png",
        Some("woff2") => "font/woff2",
        _ => "application/octet-stream",
    }
}

#[cfg(any(target_os = "macos", target_os = "windows", target_os = "linux"))]
fn renderer_error_response(
    status: u16,
    message: &'static str,
) -> wry::http::Response<Cow<'static, [u8]>> {
    wry::http::Response::builder()
        .status(status)
        .header(wry::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")
        .header(wry::http::header::CACHE_CONTROL, "no-store")
        .header("Content-Security-Policy", RENDERER_CONTENT_SECURITY_POLICY)
        .header("Permissions-Policy", RENDERER_PERMISSIONS_POLICY)
        .header("X-DNS-Prefetch-Control", "off")
        .header("X-Content-Type-Options", "nosniff")
        .header("Referrer-Policy", "no-referrer")
        .body(Cow::Borrowed(message.as_bytes()))
        .expect("static renderer error response is valid")
}

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub struct PreparedHtmlPreview;

#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
pub fn prepare_html_form_preview(
    _envelope: &RenderEnvelopeV1,
) -> Result<PreparedHtmlPreview, HtmlPreviewError> {
    Err(HtmlPreviewError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_preview_rejects_uncertified_routes() {
        let experimental = HtmlRendererSupport {
            html_enabled: true,
            release_ready: false,
        };

        assert!(!html_preview_route_permitted(experimental, false));
    }

    #[test]
    fn developer_preview_accepts_experimental_routes() {
        let experimental = HtmlRendererSupport {
            html_enabled: true,
            release_ready: false,
        };

        assert!(html_preview_route_permitted(experimental, true));
    }

    #[test]
    fn production_preview_accepts_certified_html_only_routes() {
        let certified = HtmlRendererSupport {
            html_enabled: true,
            release_ready: true,
        };

        assert!(html_preview_route_permitted(certified, false));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn validated_page_rect(y: f64) -> RendererPageRect {
        RendererPageRect {
            x: 0.0,
            y,
            width: 816.0,
            height: 1248.0,
            client_width: 816.0,
            client_height: 1248.0,
            scroll_width: 816.0,
            scroll_height: 1248.0,
            descendant_overflow_x: 0,
            descendant_overflow_y: 0,
            descendant_clipped_x: 0,
            descendant_clipped_y: 0,
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn ready_renderer_state(render_epoch: u64) -> RendererState {
        let mut state = RendererState::for_document(RendererDocumentIdentity::test_identity());
        state.document_boot_accepted = true;
        assert!(state.invalidate_for_epoch(render_epoch));
        state.ready = true;
        let report = validated_geometry_report();
        state.page_count = Some(report.page_count);
        state.page_rects = report.pages.clone();
        state.geometry_reports = Some([report.clone(), report]);
        state.geometry_print_mode = true;
        state
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn validated_geometry_report() -> RendererGeometryReport {
        RendererGeometryReport {
            page_count: 2,
            page_width_pt: 612.0,
            page_height_pt: 936.0,
            pages: vec![validated_page_rect(0.0), validated_page_rect(1248.0)],
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn renderer_layout_plan() -> RenderLayoutPlan {
        let provider = bir_print::html_forms::render_form_provider("2551Q", "2018")
            .expect("2551Q renderer provider");
        RenderLayoutPlan {
            provider,
            page_geometry: provider.page_geometry().expect("2551Q page geometry"),
            expected_page_count: 2,
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn renderer_page_rect_message(y: f64) -> RendererPageRectMessage {
        RendererPageRectMessage {
            x: 0.0,
            y,
            width: 816.0,
            height: 1248.0,
            client_width: 816.0,
            client_height: 1248.0,
            scroll_width: 816.0,
            scroll_height: 1248.0,
            descendant_overflow_x: 0,
            descendant_overflow_y: 0,
            descendant_clipped_x: 0,
            descendant_clipped_y: 0,
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn renderer_geometry_report_message() -> RendererGeometryReportMessage {
        RendererGeometryReportMessage {
            page_count: 2,
            page_width_pt: 612.0,
            page_height_pt: 936.0,
            pages: vec![
                renderer_page_rect_message(0.0),
                renderer_page_rect_message(1248.0),
            ],
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn renderer_ipc_message(
        identity: &RendererDocumentIdentity,
        message: RendererMessage,
    ) -> RendererIpcMessage {
        RendererIpcMessage {
            document_run_id: identity.document_run_id.clone(),
            envelope_hash: identity.envelope_hash.clone(),
            message,
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn assert_renderer_security_headers(response: &wry::http::Response<Cow<'static, [u8]>>) {
        let csp = response
            .headers()
            .get("content-security-policy")
            .and_then(|value| value.to_str().ok())
            .expect("renderer response has a CSP header");
        assert!(csp.contains("worker-src 'none'"));
        assert!(csp.contains("child-src 'none'"));
        assert!(csp.contains("frame-ancestors 'none'"));
        assert!(csp.contains("img-src 'self' data:"));
        assert!(!csp.contains("script-src 'self' data:"));
        let permissions_policy = response
            .headers()
            .get("permissions-policy")
            .and_then(|value| value.to_str().ok())
            .expect("renderer response has a Permissions-Policy header");
        assert!(permissions_policy.contains("camera=()"));
        assert!(permissions_policy.contains("microphone=()"));
        assert!(permissions_policy.contains("display-capture=()"));
        assert!(permissions_policy.contains("usb=()"));
        assert_eq!(
            response
                .headers()
                .get("x-dns-prefetch-control")
                .and_then(|value| value.to_str().ok()),
            Some("off")
        );
        assert_eq!(
            response
                .headers()
                .get("x-content-type-options")
                .and_then(|value| value.to_str().ok()),
            Some("nosniff")
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_initialization_blocks_worker_capabilities() {
        let script = renderer_initialization_script("\"{}\"");
        assert!(script.contains("installBlockedWorkerConstructor(\"Worker\")"));
        assert!(script.contains("installBlockedWorkerConstructor(\"SharedWorker\")"));
        assert!(script.contains("Object.defineProperty(window.navigator, \"serviceWorker\""));
        assert!(script.contains("Object.defineProperty(serviceWorker, \"register\""));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_initialization_blocks_peer_media_and_device_capabilities() {
        let script = renderer_initialization_script("\"{}\"");
        assert!(script.contains("\"RTCPeerConnection\""));
        assert!(script.contains("\"webkitRTCPeerConnection\""));
        assert!(script.contains("\"mozRTCPeerConnection\""));
        assert!(script.contains("Object.defineProperty(window.navigator, \"mediaDevices\""));
        assert!(script.contains("\"webkitGetUserMedia\""));
        assert!(script.contains("[\"bluetooth\", \"hid\", \"serial\", \"usb\"]"));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_initialization_blocks_unvalidated_print_entry_points() {
        let script = renderer_initialization_script("\"{}\"");
        assert!(script.contains("Object.defineProperty(window, \"print\""));
        assert!(script.contains("Script-initiated printing is disabled"));
        assert!(!script.contains("authorizeEbirNativePrint"));
        assert!(!script.contains("window.print()"));
        assert!(script.contains("printGuardInstallationFailed = true"));
        assert!(script.contains("Native print guard installation failed"));
        assert!(script.contains("window.addEventListener(\"DOMContentLoaded\""));
        assert!(script.contains("event.key.toLowerCase() === \"p\""));
        assert!(script.contains("document.addEventListener(\"contextmenu\""));
        assert!(script.contains("event.stopImmediatePropagation()"));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_initialization_binds_every_message_to_host_document_identity() {
        let first = RendererDocumentIdentity::host_generated(&"a".repeat(64))
            .expect("canonical envelope hash");
        let second = RendererDocumentIdentity::host_generated(&"a".repeat(64))
            .expect("canonical envelope hash");
        assert_ne!(first.document_run_id, second.document_run_id);
        assert_eq!(first.envelope_hash, second.envelope_hash);
        assert!(RendererDocumentIdentity::host_generated(&"A".repeat(64)).is_err());

        let identity_script = renderer_document_identity_script(&first);
        assert!(identity_script.contains(&first.document_run_id));
        assert!(identity_script.contains(&first.envelope_hash));
        assert!(identity_script.contains("writable: false"));
        assert!(identity_script.contains("configurable: false"));
        let bootstrap_script = renderer_initialization_script("\"{}\"");
        assert!(bootstrap_script.contains("document_run_id:"));
        assert!(bootstrap_script.contains("envelope_hash:"));

        let renderer_source = include_str!("../../../../apps/form-preview/src/main.tsx");
        assert!(renderer_source.contains("postRendererHostMessage({ type: \"renderer_boot\" })"));
        assert!(renderer_source.contains("document_run_id: rendererDocumentRunId"));
        assert!(renderer_source.contains("envelope_hash: rendererEnvelopeHash"));
        assert!(renderer_source.contains("geometry_reports: [previousMeasurement, measurement]"));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_ipc_fails_closed_for_identity_mismatch_or_reload_replay() {
        let plan = renderer_layout_plan();
        let identity = RendererDocumentIdentity::test_identity();
        let mut state = RendererState::for_document(identity.clone());

        apply_renderer_ipc_message(
            &mut state,
            renderer_ipc_message(&identity, RendererMessage::RendererBoot),
            &plan,
        );
        apply_renderer_ipc_message(
            &mut state,
            renderer_ipc_message(
                &identity,
                RendererMessage::RendererInvalidated { render_epoch: 1 },
            ),
            &plan,
        );
        assert!(state.document_boot_accepted);
        assert_eq!(state.render_epoch, 1);

        // A WebView reload restarts JavaScript state and repeats epoch one.
        // The one-use host run ID is rejected before that epoch can be reused.
        apply_renderer_ipc_message(
            &mut state,
            renderer_ipc_message(&identity, RendererMessage::RendererBoot),
            &plan,
        );
        assert!(state.document_identity_rejected);
        assert!(!state.ready);
        assert!(
            state
                .error
                .as_deref()
                .is_some_and(|error| error.contains("replayed"))
        );

        let mut mismatched = RendererDocumentIdentity::test_identity();
        mismatched.document_run_id = "00000000-0000-4000-8000-000000000002".to_string();
        let mut replacement_state = RendererState::for_document(identity);
        apply_renderer_ipc_message(
            &mut replacement_state,
            renderer_ipc_message(&mismatched, RendererMessage::RendererBoot),
            &plan,
        );
        assert!(replacement_state.document_identity_rejected);
        assert!(
            replacement_state
                .error
                .as_deref()
                .is_some_and(|error| error.contains("did not match"))
        );

        let expected = RendererDocumentIdentity::test_identity();
        let mut wrong_envelope = expected.clone();
        wrong_envelope.envelope_hash = "b".repeat(64);
        let mut wrong_envelope_state = RendererState::for_document(expected);
        apply_renderer_ipc_message(
            &mut wrong_envelope_state,
            renderer_ipc_message(&wrong_envelope, RendererMessage::RendererBoot),
            &plan,
        );
        assert!(wrong_envelope_state.document_identity_rejected);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_ipc_requires_boot_before_epoch_messages() {
        let plan = renderer_layout_plan();
        let identity = RendererDocumentIdentity::test_identity();
        let mut state = RendererState::for_document(identity.clone());
        apply_renderer_ipc_message(
            &mut state,
            renderer_ipc_message(
                &identity,
                RendererMessage::RendererInvalidated { render_epoch: 1 },
            ),
            &plan,
        );
        assert!(state.document_identity_rejected);
        assert_eq!(state.render_epoch, 0);
        assert!(
            state
                .error
                .as_deref()
                .is_some_and(|error| error.contains("before"))
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn native_print_requires_a_nonce_bound_renderer_preflight() {
        assert!(std::hint::black_box(RENDERER_WEBVIEW_IS_INCOGNITO));
        let script = native_print_preflight_script(42);
        assert!(script.contains("prepareEbirFormForNativePrint"));
        assert!(script.contains("(42)"));

        let identity = RendererDocumentIdentity::test_identity();
        let invalidated: RendererIpcMessage = serde_json::from_value(serde_json::json!({
            "document_run_id": identity.document_run_id,
            "envelope_hash": identity.envelope_hash,
            "type": "renderer_invalidated",
            "render_epoch": 7
        }))
        .expect("parse renderer invalidation");
        assert!(matches!(
            invalidated.message,
            RendererMessage::RendererInvalidated { render_epoch: 7 }
        ));
        let identity = RendererDocumentIdentity::test_identity();
        let print_ready: RendererIpcMessage = serde_json::from_value(serde_json::json!({
            "document_run_id": identity.document_run_id,
            "envelope_hash": identity.envelope_hash,
            "type": "print_ready",
            "nonce": 42,
            "render_epoch": 7,
            "print_mode": true
        }))
        .expect("parse print readiness");
        assert!(matches!(
            print_ready.message,
            RendererMessage::PrintReady {
                nonce: 42,
                render_epoch: 7,
                print_mode: true
            }
        ));
        let identity = RendererDocumentIdentity::test_identity();
        let renderer_error: RendererIpcMessage = serde_json::from_value(serde_json::json!({
            "document_run_id": identity.document_run_id,
            "envelope_hash": identity.envelope_hash,
            "type": "renderer_error",
            "render_epoch": 7,
            "message": "font failure"
        }))
        .expect("parse renderer error");
        assert!(matches!(
            renderer_error.message,
            RendererMessage::RendererError {
                render_epoch: 7,
                ref message
            } if message == "font failure"
        ));
        assert!(
            serde_json::from_str::<RendererIpcMessage>(
                r#"{"type":"renderer_error","render_epoch":7,"message":"unidentified failure"}"#
            )
            .is_err(),
            "renderer errors must carry the immutable document identity"
        );
        assert!(
            serde_json::from_str::<RendererIpcMessage>(&format!(
                r#"{{"envelope_hash":"{}","type":"renderer_boot"}}"#,
                "a".repeat(64)
            ))
            .is_err(),
            "renderer messages without a document run ID must be rejected"
        );
        assert!(
            serde_json::from_str::<RendererIpcMessage>(
                r#"{"document_run_id":"00000000-0000-4000-8000-000000000001","type":"renderer_boot"}"#
            )
            .is_err(),
            "renderer messages without an envelope hash must be rejected"
        );

        let mut state = ready_renderer_state(7);
        state.accept_print_ready(42, 7, true);
        assert_eq!(state.print_ready_nonce, Some(42));
        assert!(state.invalidate_for_epoch(8));
        assert!(!state.ready);
        assert_eq!(state.page_count, None);
        assert!(!state.geometry_print_mode);
        assert_eq!(state.print_ready_nonce, None);
        assert_eq!(state.render_epoch, 8);
        assert_eq!(state.readiness_revision, 2);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_ipc_ignores_stale_and_out_of_order_epochs() {
        let plan = renderer_layout_plan();
        let mut state = RendererState::default();
        apply_renderer_message(
            &mut state,
            RendererMessage::RendererInvalidated { render_epoch: 5 },
            &plan,
        );
        apply_renderer_message(
            &mut state,
            RendererMessage::RendererReady { render_epoch: 4 },
            &plan,
        );
        apply_renderer_message(
            &mut state,
            RendererMessage::PageCount {
                render_epoch: 6,
                print_mode: true,
                geometry_reports: [
                    renderer_geometry_report_message(),
                    renderer_geometry_report_message(),
                ],
            },
            &plan,
        );
        apply_renderer_message(
            &mut state,
            RendererMessage::RendererInvalidated { render_epoch: 4 },
            &plan,
        );

        assert_eq!(state.render_epoch, 5);
        assert_eq!(state.readiness_revision, 1);
        assert!(!state.ready);
        assert_eq!(state.page_count, None);

        apply_renderer_message(
            &mut state,
            RendererMessage::PageCount {
                render_epoch: 5,
                print_mode: true,
                geometry_reports: [
                    renderer_geometry_report_message(),
                    renderer_geometry_report_message(),
                ],
            },
            &plan,
        );
        apply_renderer_message(
            &mut state,
            RendererMessage::RendererReady { render_epoch: 5 },
            &plan,
        );

        assert!(state.ready);
        assert_eq!(state.page_count, Some(2));
        assert_eq!(
            state.geometry_reports,
            Some([validated_geometry_report(), validated_geometry_report(),])
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_ipc_rejects_nonidentical_geometry_observations() {
        let plan = renderer_layout_plan();
        let mut state = RendererState::default();
        assert!(state.invalidate_for_epoch(5));
        let first = renderer_geometry_report_message();
        let mut second = renderer_geometry_report_message();
        second.pages[0].x += 0.5;

        apply_renderer_message(
            &mut state,
            RendererMessage::PageCount {
                render_epoch: 5,
                print_mode: true,
                geometry_reports: [first, second],
            },
            &plan,
        );

        assert_eq!(state.page_count, None);
        assert!(state.page_rects.is_empty());
        assert!(state.geometry_reports.is_none());
        assert!(
            state
                .error
                .as_deref()
                .is_some_and(|error| error.contains("not identical"))
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_ipc_ignores_stale_error_epochs() {
        let plan = renderer_layout_plan();
        let mut state = ready_renderer_state(7);

        apply_renderer_message(
            &mut state,
            RendererMessage::RendererError {
                render_epoch: 6,
                message: "stale font failure".to_string(),
            },
            &plan,
        );
        assert_eq!(state.error, None);

        apply_renderer_message(
            &mut state,
            RendererMessage::RendererError {
                render_epoch: 7,
                message: "current font failure".to_string(),
            },
            &plan,
        );
        assert_eq!(state.error.as_deref(), Some("current font failure"));

        apply_renderer_message(
            &mut state,
            RendererMessage::RendererInvalidated { render_epoch: 8 },
            &plan,
        );
        apply_renderer_message(
            &mut state,
            RendererMessage::RendererError {
                render_epoch: 7,
                message: "late measurement failure".to_string(),
            },
            &plan,
        );
        assert_eq!(state.render_epoch, 8);
        assert_eq!(state.error, None);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn native_output_binding_rejects_invalidation_after_backend_start() {
        let mut state = ready_renderer_state(7);
        let binding = bind_renderer_for_native_output(&state).expect("bind ready renderer");
        let mut pending = PendingNativeOutput::validating(HtmlOutputKind::SystemPrint, 9, None);
        pending.backend_started = true;
        pending.binding = Some(binding);

        assert!(state.invalidate_for_epoch(8));

        let reason = renderer_binding_mismatch_reason(
            &state,
            pending.binding.as_ref().expect("pending renderer binding"),
        )
        .expect("invalidation must stale the native output");
        assert!(reason.contains("epoch changed"));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn native_backend_completion_must_match_bound_renderer_epoch() {
        let state = ready_renderer_state(7);
        let binding = bind_renderer_for_native_output(&state).expect("bind ready renderer");
        let document_identity = binding.document_identity.clone();
        let mut pending = PendingNativeOutput::validating(HtmlOutputKind::SystemPrint, 9, None);
        pending.backend_started = true;
        pending.binding = Some(binding);
        let stale_completion = NativeBackendCompletion::SystemPrint {
            nonce: 9,
            document_identity: document_identity.clone(),
            render_epoch: 6,
            result: Ok(()),
        };

        let reason = native_backend_completion_binding_error(&pending, &state, &stale_completion)
            .expect("stale completion must fail");
        assert!(reason.contains("stale renderer epoch"));

        let mut other_document = document_identity.clone();
        other_document.document_run_id = "00000000-0000-4000-8000-000000000002".to_string();
        let stale_document_completion = NativeBackendCompletion::SystemPrint {
            nonce: 9,
            document_identity: other_document,
            render_epoch: 7,
            result: Ok(()),
        };
        let reason =
            native_backend_completion_binding_error(&pending, &state, &stale_document_completion)
                .expect("completion from another document must fail");
        assert!(reason.contains("stale renderer document"));

        let current_completion = NativeBackendCompletion::SystemPrint {
            nonce: 9,
            document_identity,
            render_epoch: 7,
            result: Ok(()),
        };
        assert_eq!(
            native_backend_completion_binding_error(&pending, &state, &current_completion),
            None
        );
    }

    #[cfg(all(feature = "dev-tools", any(target_os = "macos", target_os = "windows")))]
    #[test]
    fn renderer_build_identity_binds_only_to_the_observed_bundle() {
        let root = std::env::temp_dir().join(format!(
            "ebirforms-renderer-build-identity-{}",
            uuid::Uuid::new_v4()
        ));
        let renderer = root.join("form-renderer");
        std::fs::create_dir_all(&renderer).expect("renderer fixture directory");
        let entry = renderer.join("index.html");
        std::fs::write(&entry, b"<!doctype html><p>first</p>").expect("renderer fixture entry");
        let observed = hash_evidence_artifact(EvidenceArtifactSource::Directory(&renderer))
            .expect("renderer fixture hash");
        let identity = serde_json::json!({
            "schema_version": 1,
            "scope": "build_time_non_promotional_identity",
            "promotion_eligible": false,
            "offline_verification_passed": true,
            "renderer_bundle_relative_path": "form-renderer",
            "renderer_bundle_sha256": observed,
            "source_revision": { "status": "observed", "value": "a".repeat(40) }
        });
        std::fs::write(
            root.join(OFFLINE_RENDERER_BUILD_IDENTITY_FILE_NAME),
            serde_json::to_vec_pretty(&identity).expect("identity JSON"),
        )
        .expect("renderer build identity");

        let bound = development_renderer_build_identity(
            &entry,
            &DevelopmentEvidenceAvailability::observed(observed.clone()),
        );
        assert_eq!(
            bound.source_revision,
            DevelopmentEvidenceAvailability::observed("a".repeat(40))
        );
        assert_eq!(
            bound.expected_renderer_bundle_sha256,
            DevelopmentEvidenceAvailability::observed(observed)
        );
        assert_eq!(bound.binding_gap, None);

        std::fs::write(&entry, b"<!doctype html><p>changed after build</p>")
            .expect("mutated renderer fixture entry");
        let changed = hash_evidence_artifact(EvidenceArtifactSource::Directory(&renderer))
            .expect("changed renderer fixture hash");
        let rejected = development_renderer_build_identity(
            &entry,
            &DevelopmentEvidenceAvailability::observed(changed),
        );
        assert!(matches!(
            rejected.source_revision,
            DevelopmentEvidenceAvailability::Unavailable { .. }
        ));
        assert!(
            rejected
                .binding_gap
                .as_deref()
                .is_some_and(|gap| gap.contains("differs"))
        );

        std::fs::remove_dir_all(root).expect("remove renderer build-identity fixture");
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn cancelled_system_print_ignores_late_native_completion() {
        let mut bridge = NativeBackendBridge::default();
        bridge.cancel_output(9);

        bridge.record_completion(NativeBackendCompletion::SystemPrint {
            nonce: 9,
            document_identity: RendererDocumentIdentity::test_identity(),
            render_epoch: 7,
            result: Ok(()),
        });

        assert!(bridge.completion.is_none());
        assert!(!bridge.cancelled_nonces.contains(&9));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn native_output_cleanup_leaves_print_mode_and_revalidates_geometry() {
        let script = native_output_cleanup_script();
        assert!(script.contains("classList.remove(\"ebir-native-print-mode\")"));
        assert!(script.contains("dispatchEvent(new Event(\"resize\"))"));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn native_pdf_export_backend_remains_bounded_after_it_starts() {
        let reason =
            native_output_timeout_reason(HtmlOutputKind::PdfExport, true, PDF_EXPORT_TIMEOUT);
        assert_eq!(
            reason.as_deref(),
            Some("native HTML PDF export backend did not complete before its deadline")
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn active_native_system_print_is_not_failed_by_the_pdf_export_deadline() {
        let reason = native_output_timeout_reason(
            HtmlOutputKind::SystemPrint,
            true,
            PDF_EXPORT_TIMEOUT + Duration::from_secs(1),
        );
        assert_eq!(reason, None);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn native_output_preflight_uses_the_shorter_readiness_deadline() {
        let reason =
            native_output_timeout_reason(HtmlOutputKind::PdfExport, false, READINESS_TIMEOUT);
        assert_eq!(
            reason.as_deref(),
            Some("HTML renderer native output preflight timed out")
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_system_print_completion_fails_closed_for_cancel_or_error() {
        assert_eq!(macos_system_print_completion_decision(true), Ok(()));
        assert_eq!(
            macos_system_print_completion_decision(false),
            Err("the macOS print operation was cancelled or failed".to_string())
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_page_capture_waits_for_every_page_and_preserves_order() {
        let mut bridge = NativeBackendBridge::default();
        let document_identity = RendererDocumentIdentity::test_identity();
        bridge.begin_macos_capture(9, document_identity.clone(), 7, 2);
        bridge.record_macos_page(9, 1, Ok(vec![2]));
        assert!(bridge.completion.is_none());
        bridge.record_macos_page(9, 0, Ok(vec![1]));
        let Some(NativeBackendCompletion::CapturedPages {
            nonce,
            document_identity: completed_identity,
            render_epoch,
            pages,
        }) = bridge.completion.take()
        else {
            panic!("macOS capture should complete after all page callbacks");
        };
        assert_eq!(nonce, 9);
        assert_eq!(completed_identity, document_identity);
        assert_eq!(render_epoch, 7);
        assert_eq!(pages, vec![Ok(vec![1]), Ok(vec![2])]);
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cancelled_native_output_discards_temp_and_ignores_late_completion() {
        let temp_path = std::env::temp_dir().join(format!(
            "ebirforms-cancelled-output-{}.pdf",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&temp_path, b"partial").expect("write test temp");
        let mut bridge = NativeBackendBridge::default();
        bridge
            .register_temp_path(9, temp_path.clone())
            .expect("register test temp");
        bridge.begin_macos_capture(9, RendererDocumentIdentity::test_identity(), 7, 1);

        bridge.cancel_output(9);
        bridge.prepare_for_output();
        bridge.record_macos_page(9, 0, Ok(vec![1]));

        assert!(!temp_path.exists());
        assert!(bridge.completion.is_none());
    }

    #[cfg(target_os = "macos")]
    fn captured_pdf_page(content: &str) -> Vec<u8> {
        let stream = format!("{content}\n");
        let objects = [
            "<< /Type /Catalog /Pages 2 0 R >>".to_string(),
            "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string(),
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 936] /CropBox [0 0 612 936] /Resources << >> /Contents 4 0 R >>".to_string(),
            format!(
                "<< /Length {} >>\nstream\n{}endstream",
                stream.len(),
                stream
            ),
        ];
        let mut bytes = b"%PDF-1.4\n".to_vec();
        let mut offsets = Vec::with_capacity(objects.len());
        for (index, object) in objects.iter().enumerate() {
            offsets.push(bytes.len());
            bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", index + 1).as_bytes());
        }
        let xref_offset = bytes.len();
        bytes.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        bytes.extend_from_slice(b"0000000000 65535 f \n");
        for offset in offsets {
            bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
        }
        bytes.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_offset}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );
        bytes
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_capture_pipeline_merges_validated_pages_and_binds_the_envelope() {
        let directory = std::env::temp_dir().join(format!(
            "ebirforms-macos-capture-success-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).expect("create macOS capture fixture directory");
        let destination = directory.join("selected.pdf");
        std::fs::write(&destination, b"old destination").expect("write old destination");
        let temp_path = create_pdf_export_temp(&destination).expect("create sibling temp");
        let mut expectation = pdf_expectation_with_geometry(612.0, 936.0);
        expectation.expected_page_count = 2;

        let report = finalize_macos_pdf_capture(
            vec![
                Ok(captured_pdf_page("q 1 0 0 1 1 1 cm Q")),
                Ok(captured_pdf_page("q 1 0 0 1 2 2 cm Q")),
            ],
            &temp_path,
            &destination,
            &expectation,
        )
        .expect("valid WKPDF captures should finalize");

        assert_eq!(report.page_count, 2);
        assert!(!temp_path.exists());
        bir_print::html_output::validate_pdf_file(&destination, &expectation)
            .expect("the destination must retain the immutable envelope evidence");
        let mut other_envelope = expectation.clone();
        other_envelope.envelope_hash = "b".repeat(64);
        assert!(
            bir_print::html_output::validate_pdf_file(&destination, &other_envelope).is_err(),
            "the same PDF must not validate for another immutable envelope"
        );

        std::fs::remove_dir_all(directory).expect("remove macOS capture fixture directory");
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_capture_pipeline_preserves_existing_destination_on_forced_failure() {
        let directory = std::env::temp_dir().join(format!(
            "ebirforms-macos-capture-failure-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(&directory).expect("create macOS capture fixture directory");
        let destination = directory.join("selected.pdf");
        let original = b"pre-existing destination";
        std::fs::write(&destination, original).expect("write existing destination");
        let temp_path = create_pdf_export_temp(&destination).expect("create sibling temp");

        let error = finalize_macos_pdf_capture(
            vec![
                Ok(captured_pdf_page("q 1 0 0 1 1 1 cm Q")),
                Ok(b"forced invalid WKPDF callback payload".to_vec()),
            ],
            &temp_path,
            &destination,
            &pdf_expectation_with_geometry(612.0, 936.0),
        )
        .expect_err("invalid WKPDF payload must fail closed");

        assert!(error.contains("captured page 2"));
        assert_eq!(
            std::fs::read(&destination).expect("read preserved destination"),
            original
        );
        assert!(!temp_path.exists());

        std::fs::remove_dir_all(directory).expect("remove macOS capture fixture directory");
    }

    #[cfg(all(feature = "dev-tools", target_os = "macos"))]
    #[test]
    fn macos_observation_binds_callback_payloads_to_the_renderer_document() {
        let document_identity = RendererDocumentIdentity::test_identity();
        let first = captured_pdf_page("q 1 0 0 1 1 1 cm Q");
        let second = captured_pdf_page("q 1 0 0 1 2 2 cm Q");
        let completion = NativeBackendCompletion::CapturedPages {
            nonce: 17,
            document_identity: document_identity.clone(),
            render_epoch: 23,
            pages: vec![Ok(first.clone()), Ok(second.clone())],
        };

        let observation = development_backend_observation(&completion)
            .expect("captured pages should produce a diagnostic observation");
        assert_eq!(observation.completion.nonce, 17);
        assert_eq!(
            observation.completion.document_run_id,
            document_identity.document_run_id
        );
        assert_eq!(
            observation.completion.envelope_sha256,
            document_identity.envelope_hash
        );
        assert_eq!(observation.completion.render_epoch, 23);
        assert!(observation.completion.succeeded);
        let DevelopmentEvidenceAvailability::Observed { value: payloads } =
            observation.page_payloads
        else {
            panic!("WKPDF callback payload hashes must remain directly observed");
        };
        assert_eq!(payloads.len(), 2);
        assert_eq!(payloads[0].page_number, 1);
        assert_eq!(payloads[0].byte_count, first.len());
        assert_eq!(
            payloads[0].sha256.as_deref(),
            Some(format!("{:x}", Sha256::digest(&first)).as_str())
        );
        assert_eq!(payloads[1].page_number, 2);
        assert_eq!(payloads[1].byte_count, second.len());
        assert_eq!(
            payloads[1].sha256.as_deref(),
            Some(format!("{:x}", Sha256::digest(&second)).as_str())
        );
    }

    #[test]
    fn webview2_pdf_requires_both_hresult_and_success_result() {
        assert!(webview2_pdf_completion_decision(true, true).is_ok());
        assert!(webview2_pdf_completion_decision(false, true).is_err());
        assert!(webview2_pdf_completion_decision(true, false).is_err());
        assert!(webview2_pdf_completion_decision(false, false).is_err());
    }

    fn pdf_expectation_with_geometry(width_points: f64, height_points: f64) -> PdfExpectation {
        PdfExpectation {
            form_code: "TEST".to_string(),
            revision: "2018".to_string(),
            envelope_hash: "a".repeat(64),
            expected_page_count: 1,
            width_points,
            height_points,
        }
    }

    #[test]
    fn windows_print_settings_convert_legal_points_to_exact_inches() {
        let expectation = pdf_expectation_with_geometry(612.0, 936.0);
        let settings = windows_native_print_settings_spec(&expectation)
            .expect("legal paper geometry should produce WebView2 settings");
        assert_eq!(
            settings,
            WindowsNativePrintSettingsSpec {
                page_width_inches: 8.5,
                page_height_inches: 13.0,
                scale_factor: 1.0,
                margin_top_inches: 0.0,
                margin_bottom_inches: 0.0,
                margin_left_inches: 0.0,
                margin_right_inches: 0.0,
                should_print_backgrounds: true,
                should_print_selection_only: false,
                should_print_header_and_footer: false,
            }
        );
    }

    #[test]
    fn windows_print_settings_follow_provider_specific_paper_height() {
        let letter =
            windows_native_print_settings_spec(&pdf_expectation_with_geometry(612.0, 792.0))
                .expect("letter geometry should produce WebView2 settings");
        let fourteen_inch =
            windows_native_print_settings_spec(&pdf_expectation_with_geometry(612.0, 1_008.0))
                .expect("fourteen-inch geometry should produce WebView2 settings");
        assert_eq!(
            (letter.page_height_inches, fourteen_inch.page_height_inches),
            (11.0, 14.0)
        );
    }

    #[test]
    fn windows_print_settings_reject_invalid_expectations() {
        let expectation = pdf_expectation_with_geometry(f64::NAN, 936.0);
        assert!(windows_native_print_settings_spec(&expectation).is_err());
    }

    #[test]
    fn webview2_system_print_requires_successful_hresult_and_status() {
        assert!(
            webview2_print_completion_decision(true, WindowsNativePrintStatus::Succeeded).is_ok()
        );
        assert!(
            webview2_print_completion_decision(false, WindowsNativePrintStatus::Succeeded).is_err()
        );
        assert!(
            webview2_print_completion_decision(true, WindowsNativePrintStatus::PrinterUnavailable)
                .is_err()
        );
        assert!(
            webview2_print_completion_decision(true, WindowsNativePrintStatus::OtherError).is_err()
        );
        assert!(
            webview2_print_completion_decision(true, WindowsNativePrintStatus::Unknown(99))
                .is_err()
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn print_ready_rejects_missing_page_rectangles() {
        let mut state = RendererState {
            ready: true,
            page_count: Some(2),
            geometry_print_mode: true,
            render_epoch: 7,
            ..RendererState::default()
        };
        state.accept_print_ready(42, 7, true);
        assert_eq!(state.print_ready_nonce, None);
        assert!(
            state
                .error
                .as_deref()
                .is_some_and(|error| error.contains("page rectangle"))
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn print_ready_signal_is_rejected_without_fresh_geometry() {
        let mut state = RendererState {
            geometry_print_mode: true,
            render_epoch: 7,
            ..RendererState::default()
        };
        state.accept_print_ready(7, 7, true);
        assert_eq!(state.print_ready_nonce, None);
        assert!(
            state
                .error
                .as_deref()
                .is_some_and(|error| error.contains("before renderer readiness"))
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn print_ready_signal_requires_explicit_print_mode_geometry() {
        let mut state = RendererState {
            ready: true,
            page_count: Some(2),
            render_epoch: 7,
            ..RendererState::default()
        };
        state.accept_print_ready(7, 7, false);
        assert_eq!(state.print_ready_nonce, None);
        assert!(
            state
                .error
                .as_deref()
                .is_some_and(|error| error.contains("explicit print mode"))
        );
    }

    #[test]
    fn initial_readiness_deadline_cannot_be_extended_by_invalidation() {
        assert!(renderer_readiness_timed_out(false, false, false, true));
        assert!(!renderer_readiness_timed_out(false, false, true, true));
        assert!(renderer_readiness_timed_out(false, true, true, true));
        assert!(!renderer_readiness_timed_out(true, true, false, true));
    }

    #[test]
    fn print_css_hides_unvalidated_media_and_exposes_explicit_print_mode() {
        let css = include_str!("../../../../packages/form-renderer/src/print.css");
        assert!(css.contains(":root.ebir-native-print-mode .form-document"));
        assert!(css.contains(
            ":root:not(.ebir-native-print-mode) .form-document { display: none !important; }"
        ));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_protocol_responses_send_security_headers() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock is after Unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "ebirforms-renderer-protocol-{}-{unique}",
            std::process::id()
        ));
        std::fs::create_dir_all(&root).expect("create renderer fixture directory");
        std::fs::write(root.join("index.html"), b"<!doctype html>")
            .expect("write renderer fixture");

        let request = wry::http::Request::builder()
            .uri("ebirforms://localhost/index.html")
            .body(Vec::new())
            .expect("build renderer request");
        let response = renderer_protocol_response(Some(&root), request);
        assert_eq!(response.status(), 200);
        assert_renderer_security_headers(&response);
        assert_renderer_security_headers(&renderer_error_response(404, "missing"));

        std::fs::remove_dir_all(root).expect("remove renderer fixture directory");
    }

    #[test]
    fn renderer_paths_reject_parent_traversal() {
        assert_eq!(
            renderer_relative_path("/"),
            Some(PathBuf::from("index.html"))
        );
        assert_eq!(
            renderer_relative_path("/assets/index.js"),
            Some(PathBuf::from("assets/index.js"))
        );
        assert_eq!(renderer_relative_path("/../secrets"), None);
    }

    #[test]
    fn native_print_waits_until_the_renderer_is_geometry_ready() {
        assert_eq!(
            native_print_decision(false, None, None, true),
            NativePrintDecision::WaitForRenderer
        );
        assert_eq!(
            native_print_decision(true, None, None, true),
            NativePrintDecision::WaitForRenderer
        );
        assert_eq!(
            native_print_decision(true, Some(2), None, true),
            NativePrintDecision::StartPrint
        );
    }

    #[test]
    fn native_print_failures_remain_fail_closed() {
        assert!(matches!(
            native_print_decision(true, Some(2), Some("late renderer failure"), true),
            NativePrintDecision::Fallback(reason) if reason.contains("late renderer failure")
        ));
        assert!(matches!(
            native_print_decision(true, Some(2), None, false),
            NativePrintDecision::Fallback(reason) if reason.contains("without an available native WebView")
        ));
        assert!(matches!(
            native_print_decision(true, Some(0), None, true),
            NativePrintDecision::Fallback(reason) if reason.contains("zero printable pages")
        ));
    }
}
