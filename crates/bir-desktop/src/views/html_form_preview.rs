//! Native preview, system-print, and direct-PDF host for owned HTML forms.

use bir_print::html::RenderEnvelopeV1;
use bir_print::html_forms::RenderLayoutPlan;
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
    pub(crate) layout_plan: RenderLayoutPlan,
    pub(crate) pdf_expectation: PdfExpectation,
    pub(crate) default_pdf_name: String,
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
                window.ipc.postMessage(JSON.stringify(message));
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
        render_epoch: u64,
        bridge: Arc<Mutex<NativeBackendBridge>>,
    ) -> Retained<Self> {
        let delegate = main_thread
            .alloc::<Self>()
            .set_ivars(MacPrintCompletionDelegateIvars {
                nonce,
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

    let delegate = MacPrintCompletionDelegate::new(main_thread, nonce, render_epoch, bridge);
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
        bridge.begin_macos_capture(nonce, render_epoch, page_rects.len());
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
    ready: bool,
    page_count: Option<usize>,
    page_rects: Vec<RendererPageRect>,
    geometry_print_mode: bool,
    error: Option<String>,
    render_epoch: u64,
    readiness_revision: u64,
    print_ready_nonce: Option<u64>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl RendererState {
    fn invalidate_for_epoch(&mut self, render_epoch: u64) -> bool {
        if render_epoch == 0 || render_epoch <= self.render_epoch {
            return false;
        }
        self.render_epoch = render_epoch;
        self.ready = false;
        self.page_count = None;
        self.page_rects.clear();
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
                if self.page_count != Some(self.page_rects.len()) || self.page_rects.is_empty() =>
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
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, PartialEq)]
struct NativeOutputRendererBinding {
    render_epoch: u64,
    readiness_revision: u64,
    page_rects: Vec<RendererPageRect>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl PendingNativeOutput {
    fn validating(kind: HtmlOutputKind, nonce: u64, destination: Option<PathBuf>) -> Self {
        Self {
            kind,
            nonce,
            destination,
            temp_path: None,
            started_at: Instant::now(),
            backend_started: false,
            binding: None,
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn bind_renderer_for_native_output(
    state: &RendererState,
) -> Result<NativeOutputRendererBinding, String> {
    if state.render_epoch == 0 {
        return Err("native output has no validated renderer epoch".to_string());
    }
    if !state.ready || !state.geometry_print_mode {
        return Err("native output renderer epoch is not ready in print mode".to_string());
    }
    if state.page_rects.is_empty() || state.page_count != Some(state.page_rects.len()) {
        return Err("native output renderer epoch has incomplete page rectangles".to_string());
    }
    Ok(NativeOutputRendererBinding {
        render_epoch: state.render_epoch,
        readiness_revision: state.readiness_revision,
        page_rects: state.page_rects.clone(),
    })
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn renderer_binding_mismatch_reason(
    state: &RendererState,
    binding: &NativeOutputRendererBinding,
) -> Option<String> {
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
        render_epoch: u64,
        result: Result<(), String>,
    },
    #[cfg(target_os = "macos")]
    CapturedPages {
        nonce: u64,
        render_epoch: u64,
        pages: Vec<Result<Vec<u8>, String>>,
    },
    #[cfg(target_os = "windows")]
    PdfFile {
        nonce: u64,
        render_epoch: u64,
        result: Result<(), String>,
    },
}

#[cfg(target_os = "macos")]
#[derive(Debug)]
struct MacCaptureBatch {
    nonce: u64,
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
    if native_backend_completion_render_epoch(completion) != binding.render_epoch {
        return Some("native backend completion reported a stale renderer epoch".to_string());
    }
    renderer_binding_mismatch_reason(state, binding)
}

#[cfg(target_os = "macos")]
impl NativeBackendBridge {
    fn begin_macos_capture(&mut self, nonce: u64, render_epoch: u64, page_count: usize) {
        self.completion = None;
        self.mac_capture = Some(MacCaptureBatch {
            nonce,
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
                render_epoch: batch.render_epoch,
                pages,
            });
        }
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RendererMessage {
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
        page_count: usize,
        page_width_pt: f64,
        page_height_pt: f64,
        print_mode: bool,
        pages: Vec<RendererPageRectMessage>,
    },
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
fn apply_renderer_message(
    state: &mut RendererState,
    message: RendererMessage,
    layout_plan: &RenderLayoutPlan,
) {
    match message {
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
            page_count,
            page_width_pt,
            page_height_pt,
            print_mode,
            pages,
        } => {
            if !state.accepts_epoch(render_epoch) {
                return;
            }
            let report = RendererGeometryReport {
                page_count,
                page_width_pt,
                page_height_pt,
                pages: pages
                    .into_iter()
                    .map(|page| RendererPageRect {
                        x: page.x,
                        y: page.y,
                        width: page.width,
                        height: page.height,
                        client_width: page.client_width,
                        client_height: page.client_height,
                        scroll_width: page.scroll_width,
                        scroll_height: page.scroll_height,
                        descendant_overflow_x: page.descendant_overflow_x,
                        descendant_overflow_y: page.descendant_overflow_y,
                        descendant_clipped_x: page.descendant_clipped_x,
                        descendant_clipped_y: page.descendant_clipped_y,
                    })
                    .collect(),
            };
            match validate_renderer_geometry(&report, layout_plan) {
                Ok(()) => {
                    state.page_count = Some(page_count);
                    state.page_rects = report.pages;
                    state.geometry_print_mode = print_mode;
                }
                Err(error) => {
                    state.page_count = None;
                    state.page_rects.clear();
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
        let bridge_state = Arc::new(Mutex::new(RendererState::default()));
        let native_backend_bridge = Arc::new(Mutex::new(NativeBackendBridge::default()));
        let ipc_state = bridge_state.clone();
        let protocol_root = prepared.entry.parent().map(PathBuf::from);
        let layout_plan = prepared.layout_plan;
        let expected_page_count = layout_plan.expected_page_count;

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
                    .with_initialization_script(&prepared.initialization_script)
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
                        let Ok(message) = serde_json::from_str::<RendererMessage>(request.body())
                        else {
                            tracing::warn!(
                                body_bytes = request.body().len(),
                                "ignored malformed renderer IPC"
                            );
                            return;
                        };
                        let Ok(mut state) = ipc_state.lock() else {
                            return;
                        };
                        apply_renderer_message(&mut state, message, &layout_plan);
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
            renderer_state: RendererState::default(),
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
        let render_epoch = binding.render_epoch;
        if let Some(pending) = self.pending_output.as_mut() {
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
                    webview.update(cx, move |webview, _| {
                        start_macos_pdf_capture(
                            webview.raw(),
                            &page_rects,
                            nonce,
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

        #[cfg(target_os = "macos")]
        let backend_result = match completion {
            NativeBackendCompletion::CapturedPages { pages, .. } => {
                let page_bytes = pages.into_iter().collect::<Result<Vec<_>, _>>();
                page_bytes
                    .and_then(|pages| {
                        merge_single_page_pdfs(&pages).map_err(|error| error.to_string())
                    })
                    .and_then(|merged| {
                        std::fs::write(&temp_path, merged).map_err(|error| error.to_string())
                    })
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

        let export_result = backend_result.and_then(|()| {
            finalize_pdf_export(&temp_path, &destination, &self.pdf_expectation)
                .map(|_| ())
                .map_err(|error| error.to_string())
        });
        self.leave_native_output_mode(cx);
        match export_result {
            Ok(()) => {
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
        let mut state = RendererState::default();
        assert!(state.invalidate_for_epoch(render_epoch));
        state.ready = true;
        state.page_count = Some(2);
        state.page_rects = vec![validated_page_rect(0.0), validated_page_rect(1248.0)];
        state.geometry_print_mode = true;
        state
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
    fn native_print_requires_a_nonce_bound_renderer_preflight() {
        assert!(std::hint::black_box(RENDERER_WEBVIEW_IS_INCOGNITO));
        let script = native_print_preflight_script(42);
        assert!(script.contains("prepareEbirFormForNativePrint"));
        assert!(script.contains("(42)"));

        let invalidated: RendererMessage =
            serde_json::from_str(r#"{"type":"renderer_invalidated","render_epoch":7}"#)
                .expect("parse renderer invalidation");
        assert!(matches!(
            invalidated,
            RendererMessage::RendererInvalidated { render_epoch: 7 }
        ));
        let print_ready: RendererMessage = serde_json::from_str(
            r#"{"type":"print_ready","nonce":42,"render_epoch":7,"print_mode":true}"#,
        )
        .expect("parse print readiness");
        assert!(matches!(
            print_ready,
            RendererMessage::PrintReady {
                nonce: 42,
                render_epoch: 7,
                print_mode: true
            }
        ));
        let renderer_error: RendererMessage = serde_json::from_str(
            r#"{"type":"renderer_error","render_epoch":7,"message":"font failure"}"#,
        )
        .expect("parse renderer error");
        assert!(matches!(
            renderer_error,
            RendererMessage::RendererError {
                render_epoch: 7,
                ref message
            } if message == "font failure"
        ));
        assert!(
            serde_json::from_str::<RendererMessage>(
                r#"{"type":"renderer_error","message":"unversioned failure"}"#
            )
            .is_err(),
            "renderer errors must be causally bound to an epoch"
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
                page_count: 2,
                page_width_pt: 612.0,
                page_height_pt: 936.0,
                print_mode: true,
                pages: vec![
                    renderer_page_rect_message(0.0),
                    renderer_page_rect_message(1248.0),
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
                page_count: 2,
                page_width_pt: 612.0,
                page_height_pt: 936.0,
                print_mode: true,
                pages: vec![
                    renderer_page_rect_message(0.0),
                    renderer_page_rect_message(1248.0),
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
        let mut pending = PendingNativeOutput::validating(HtmlOutputKind::SystemPrint, 9, None);
        pending.backend_started = true;
        pending.binding = Some(binding);
        let stale_completion = NativeBackendCompletion::SystemPrint {
            nonce: 9,
            render_epoch: 6,
            result: Ok(()),
        };

        let reason = native_backend_completion_binding_error(&pending, &state, &stale_completion)
            .expect("stale completion must fail");
        assert!(reason.contains("stale renderer epoch"));

        let current_completion = NativeBackendCompletion::SystemPrint {
            nonce: 9,
            render_epoch: 7,
            result: Ok(()),
        };
        assert_eq!(
            native_backend_completion_binding_error(&pending, &state, &current_completion),
            None
        );
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn cancelled_system_print_ignores_late_native_completion() {
        let mut bridge = NativeBackendBridge::default();
        bridge.cancel_output(9);

        bridge.record_completion(NativeBackendCompletion::SystemPrint {
            nonce: 9,
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
        bridge.begin_macos_capture(9, 7, 2);
        bridge.record_macos_page(9, 1, Ok(vec![2]));
        assert!(bridge.completion.is_none());
        bridge.record_macos_page(9, 0, Ok(vec![1]));
        let Some(NativeBackendCompletion::CapturedPages {
            nonce,
            render_epoch,
            pages,
        }) = bridge.completion.take()
        else {
            panic!("macOS capture should complete after all page callbacks");
        };
        assert_eq!(nonce, 9);
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
        bridge.begin_macos_capture(9, 7, 1);

        bridge.cancel_output(9);
        bridge.prepare_for_output();
        bridge.record_macos_page(9, 0, Ok(vec![1]));

        assert!(!temp_path.exists());
        assert!(bridge.completion.is_none());
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
