//! Experimental native host for the owned HTML renderer.
//!
//! This view is intentionally not the normal Print Preview path. It exists so
//! the 2551Q renderer can be calibrated in a real native WebView while the
//! manifest still has `release_ready: false`.

use bir_print::html::RenderEnvelopeV1;
use bir_print::html_support::{
    RendererGeometryReport, RendererPageRect, RendererReadinessDecision,
    bundled_html_renderer_support, expected_2551q_page_count, renderer_readiness_decision,
    validate_2551q_renderer_geometry,
};
use std::path::PathBuf;

#[cfg(any(target_os = "macos", target_os = "windows"))]
use {
    bir_print::html::serialize_envelope,
    gpui::prelude::FluentBuilder,
    gpui::{
        AppContext, Context, Entity, EventEmitter, IntoElement, ParentElement, Render, Styled,
        Task, Window, div, px,
    },
    gpui_component::ActiveTheme,
    gpui_component::Disableable,
    gpui_component::button::{Button, ButtonVariants},
    gpui_wry::WebView,
    serde::Deserialize,
    std::borrow::Cow,
    std::path::Path,
    std::sync::{Arc, Mutex},
    std::time::{Duration, Instant},
};

#[cfg(target_os = "windows")]
use wry::WebViewBuilderExtWindows;

#[cfg(any(target_os = "macos", target_os = "windows"))]
const READINESS_TIMEOUT: Duration = Duration::from_secs(5);

#[cfg(any(target_os = "macos", target_os = "windows"))]
const RENDERER_WEBVIEW_IS_INCOGNITO: bool = true;

#[cfg(any(target_os = "macos", target_os = "windows"))]
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
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

#[derive(Debug, Clone)]
pub enum HtmlFormPreviewEvent {
    LegacyFallbackRequested(String),
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn emit_legacy_fallback_then_close<T>(reason: String, window: &mut Window, cx: &mut Context<T>)
where
    T: EventEmitter<HtmlFormPreviewEvent> + 'static,
{
    cx.emit(HtmlFormPreviewEvent::LegacyFallbackRequested(reason));
    // `Context::emit` is dispatched at the end of GPUI's current effect cycle.
    // Removing this window synchronously drops the emitter before its parent
    // can receive the fallback event, so close it only after that event runs.
    cx.defer_in(window, |_, window, _| window.remove_window());
}

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
    #[error("HTML renderer bundle was not found at {0}")]
    AssetsNotFound(PathBuf),
    #[error("HTML renderer entry point was not found at {0}")]
    MissingEntryPoint(PathBuf),
    #[error("failed to serialize the renderer envelope: {0}")]
    Serialization(#[from] bir_print::html::HtmlRendererError),
    #[error("failed to encode the renderer envelope for WebView injection: {0}")]
    EnvelopeEncoding(#[source] serde_json::Error),
    #[error("HTML preview is not enabled on this platform")]
    UnsupportedPlatform,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub struct PreparedHtmlPreview {
    entry: PathBuf,
    url: String,
    initialization_script: String,
    print_authorization_token: String,
    expected_page_count: usize,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
pub fn prepare_html_form_preview(
    envelope: &RenderEnvelopeV1,
) -> Result<PreparedHtmlPreview, HtmlPreviewError> {
    let support = bundled_html_renderer_support(&envelope.form.code, &envelope.form.version);
    if !support.permits_experimental_preview() {
        return Err(HtmlPreviewError::Disabled {
            code: envelope.form.code.clone(),
            revision: envelope.form.version.clone(),
        });
    }

    let renderer_dir = crate::platform::find_resource_dir("assets").join("form-renderer");
    if !renderer_dir.is_dir() {
        return Err(HtmlPreviewError::AssetsNotFound(renderer_dir));
    }
    let entry = renderer_dir.join("index.html");
    if !entry.is_file() {
        return Err(HtmlPreviewError::MissingEntryPoint(entry));
    }

    let envelope_json = serialize_envelope(envelope)?;
    let encoded_json =
        serde_json::to_string(&envelope_json).map_err(HtmlPreviewError::EnvelopeEncoding)?;
    let print_authorization_token = uuid::Uuid::new_v4().simple().to_string();
    let initialization_script =
        renderer_initialization_script(&encoded_json, &print_authorization_token);

    Ok(PreparedHtmlPreview {
        entry,
        url: "ebirforms://localhost/index.html".to_string(),
        initialization_script,
        print_authorization_token,
        expected_page_count: expected_2551q_page_count(
            envelope
                .schedules
                .iter()
                .find(|schedule| schedule.id == "schedule_1")
                .map_or(0, |schedule| schedule.rows.len()),
        ),
    })
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn renderer_initialization_script(encoded_json: &str, print_authorization_token: &str) -> String {
    let encoded_print_authorization_token = serde_json::to_string(print_authorization_token)
        .expect("a string is always JSON encodable");
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
                const originalRendererPrint = window.print.bind(window);
                const nativePrintAuthorizationToken = {encoded_print_authorization_token};
                let authorizedNativePrintNonce = null;
                const authorizeEbirNativePrint = (token, nonce) => {{
                    if (token !== nativePrintAuthorizationToken
                        || !Number.isSafeInteger(nonce)
                        || nonce < 1) {{
                        const message = "Native print authorization was rejected";
                        postRendererHostMessage({{ type: "renderer_error", message }});
                        throw new Error(message);
                    }}
                    authorizedNativePrintNonce = nonce;
                }};
                const guardedRendererPrint = () => {{
                    if (!Number.isSafeInteger(authorizedNativePrintNonce)) {{
                        const message = "Script-initiated printing is disabled; use the native validated Print button";
                        postRendererHostMessage({{ type: "renderer_error", message }});
                        throw new Error(message);
                    }}
                    authorizedNativePrintNonce = null;
                    return originalRendererPrint();
                }};
                // Install the guard first. If the authorization hook cannot be
                // installed afterward, direct printing remains blocked.
                Object.defineProperty(window, "print", {{
                    value: guardedRendererPrint,
                    writable: false,
                    configurable: false
                }});
                Object.defineProperty(window, "authorizeEbirNativePrint", {{
                    value: authorizeEbirNativePrint,
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
fn native_print_preflight_script(nonce: u64) -> String {
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn native_authorized_print_script(print_authorization_token: &str, nonce: u64) -> String {
    let encoded_token = serde_json::to_string(print_authorization_token)
        .expect("a native print authorization token is always JSON encodable");
    format!(
        r#"
        void (() => {{
            if (typeof window.authorizeEbirNativePrint !== "function") {{
                throw new Error("HTML renderer native print authorization is unavailable");
            }}
            window.authorizeEbirNativePrint({encoded_token}, {nonce});
            window.print();
        }})();
        "#
    )
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct RendererState {
    ready: bool,
    page_count: Option<usize>,
    geometry_print_mode: bool,
    error: Option<String>,
    readiness_revision: u64,
    print_ready_nonce: Option<u64>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl RendererState {
    fn invalidate(&mut self) {
        self.ready = false;
        self.page_count = None;
        self.geometry_print_mode = false;
        self.print_ready_nonce = None;
        self.readiness_revision = self.readiness_revision.saturating_add(1);
    }

    fn accept_print_ready(&mut self, nonce: u64, print_mode: bool) {
        if !print_mode || !self.geometry_print_mode {
            self.print_ready_nonce = None;
            self.error =
                Some("native print preflight was not measured in explicit print mode".to_string());
            return;
        }
        match renderer_readiness_decision(self.ready, self.page_count, self.error.as_deref(), false)
        {
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
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum RendererMessage {
    RendererReady,
    RendererInvalidated,
    RendererError {
        message: String,
    },
    PrintReady {
        nonce: u64,
        print_mode: bool,
    },
    PageCount {
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
pub struct HtmlFormPreviewView {
    webview: Option<Entity<WebView>>,
    renderer_state: RendererState,
    status: String,
    print_authorization_token: String,
    next_print_nonce: u64,
    pending_print_nonce: Option<u64>,
    pending_print_started_at: Option<Instant>,
    _readiness_task: Task<()>,
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl EventEmitter<HtmlFormPreviewEvent> for HtmlFormPreviewView {}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl HtmlFormPreviewView {
    pub fn new(prepared: PreparedHtmlPreview, window: &mut Window, cx: &mut Context<Self>) -> Self {
        use raw_window_handle::HasWindowHandle;

        let bridge_state = Arc::new(Mutex::new(RendererState::default()));
        let ipc_state = bridge_state.clone();
        let protocol_root = prepared.entry.parent().map(PathBuf::from);
        let preview_window_handle = gpui::Window::window_handle(window);
        let expected_page_count = prepared.expected_page_count;

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
                        match message {
                            RendererMessage::RendererReady => state.ready = true,
                            RendererMessage::RendererInvalidated => state.invalidate(),
                            RendererMessage::RendererError { message } => {
                                state.error = Some(message)
                            }
                            RendererMessage::PrintReady { nonce, print_mode } => {
                                state.accept_print_ready(nonce, print_mode)
                            }
                            RendererMessage::PageCount {
                                page_count,
                                page_width_pt,
                                page_height_pt,
                                print_mode,
                                pages,
                            } => {
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
                                match validate_2551q_renderer_geometry(&report, expected_page_count)
                                {
                                    Ok(()) => {
                                        state.page_count = Some(page_count);
                                        state.geometry_print_mode = print_mode;
                                    }
                                    Err(error) => {
                                        state.page_count = None;
                                        state.geometry_print_mode = false;
                                        state.error = Some(error);
                                    }
                                }
                            }
                        }
                    })
                    .build_as_child(&window_handle)
                    .map_err(|error| error.to_string())
            });

        let (webview, status) = match result {
            Ok(webview) => (
                Some(cx.new(|cx| WebView::new(webview, window, cx))),
                "Experimental HTML renderer: preparing preview...".to_string(),
            ),
            Err(error) => {
                tracing::error!(
                    error_bytes = error.len(),
                    "experimental HTML WebView construction failed"
                );
                if let Ok(mut state) = bridge_state.lock() {
                    state.error = Some(format!("WebView construction failed: {error}"));
                }
                (None, format!("Experimental HTML renderer failed: {error}"))
            }
        };

        let initial_readiness_deadline = Instant::now() + READINESS_TIMEOUT;
        let overall_initial_readiness_deadline = initial_readiness_deadline;
        let poll_state = bridge_state;
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
                let mut fallback_reason = None;
                let update_result = this.update(cx, |this, cx| {
                    let mut should_notify = false;
                    if let Some(snapshot) = snapshot {
                        if snapshot != this.renderer_state {
                            this.renderer_state = snapshot;
                            should_notify = true;
                        }
                    }
                    if readiness_was_invalidated {
                        this.status =
                            "Experimental HTML renderer — layout changed; revalidating"
                                .to_string();
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
                                this.status = "Experimental HTML renderer — layout changed; revalidating"
                                    .to_string();
                                should_notify = true;
                            }
                        }
                        RendererReadinessDecision::Ready { page_count } => {
                            initial_readiness_completed = true;
                            if reported_page_count != Some(page_count) {
                                this.status = format!(
                                    "Experimental HTML renderer — {page_count} page(s) ready; not release-ready"
                                );
                                reported_page_count = Some(page_count);
                                should_notify = true;
                            }
                        }
                        RendererReadinessDecision::Fallback(reason) => {
                            this.status = reason.clone();
                            fallback_reason = Some(reason);
                            should_notify = true;
                        }
                    }

                    if let Some(started_at) = this.pending_print_started_at
                        && started_at.elapsed() >= READINESS_TIMEOUT
                    {
                        fallback_reason = Some(
                            "HTML renderer native print preflight timed out".to_string(),
                        );
                    }

                    if fallback_reason.is_none()
                        && let Some(pending_nonce) = this.pending_print_nonce
                        && this.renderer_state.print_ready_nonce == Some(pending_nonce)
                    {
                        match native_print_decision(
                            this.renderer_state.ready,
                            this.renderer_state.page_count,
                            this.renderer_state.error.as_deref(),
                            this.webview.is_some(),
                        ) {
                            NativePrintDecision::StartPrint => {
                                let Some(webview) = this.webview.clone() else {
                                    fallback_reason = Some(
                                        "HTML renderer WebView disappeared before printing"
                                            .to_string(),
                                    );
                                    if should_notify {
                                        cx.notify();
                                    }
                                    return;
                                };
                                #[cfg(target_os = "windows")]
                                let print_result = {
                                    let script = native_authorized_print_script(
                                        &this.print_authorization_token,
                                        pending_nonce,
                                    );
                                    webview.update(cx, move |webview, _| {
                                        webview.raw().evaluate_script(&script)
                                    })
                                };
                                #[cfg(target_os = "macos")]
                                let print_result =
                                    webview.update(cx, |webview, _| webview.raw().print());
                                match print_result {
                                    Ok(()) => {
                                        this.pending_print_nonce = None;
                                        this.pending_print_started_at = None;
                                        this.status = "Experimental HTML renderer — freshly validated native print dialog requested; not release-ready"
                                            .to_string();
                                        should_notify = true;
                                    }
                                    Err(error) => {
                                        fallback_reason = Some(format!(
                                            "HTML renderer native print failed to start: {error}"
                                        ));
                                    }
                                }
                            }
                            NativePrintDecision::WaitForRenderer => {}
                            NativePrintDecision::Fallback(reason) => {
                                fallback_reason = Some(reason)
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
                if let Some(reason) = fallback_reason {
                    let _ = this.update(cx, |_, cx| {
                        cx.emit(HtmlFormPreviewEvent::LegacyFallbackRequested(reason));
                    });
                    let _ = cx.update_window(preview_window_handle, |_, window, _| {
                        window.remove_window();
                    });
                    break;
                }
            }
        });

        Self {
            webview,
            renderer_state: RendererState::default(),
            status,
            print_authorization_token: prepared.print_authorization_token,
            next_print_nonce: 0,
            pending_print_nonce: None,
            pending_print_started_at: None,
            _readiness_task: readiness_task,
        }
    }

    fn request_legacy_fallback(
        &mut self,
        reason: String,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.status = reason.clone();
        emit_legacy_fallback_then_close(reason, window, cx);
        cx.notify();
    }
}

#[cfg(any(target_os = "macos", target_os = "windows"))]
impl Render for HtmlFormPreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let print_enabled = matches!(
            native_print_decision(
                self.renderer_state.ready,
                self.renderer_state.page_count,
                self.renderer_state.error.as_deref(),
                self.webview.is_some(),
            ),
            NativePrintDecision::StartPrint
        ) && self.pending_print_nonce.is_none();

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
                                Button::new("experimental-html-print")
                                    .label("Print (Experimental)")
                                    .disabled(!print_enabled)
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        match native_print_decision(
                                            this.renderer_state.ready,
                                            this.renderer_state.page_count,
                                            this.renderer_state.error.as_deref(),
                                            this.webview.is_some(),
                                        ) {
                                            NativePrintDecision::WaitForRenderer => {
                                                this.status = "Experimental HTML renderer is not ready to print"
                                                    .to_string();
                                                cx.notify();
                                            }
                                            NativePrintDecision::Fallback(reason) => {
                                                this.request_legacy_fallback(reason, window, cx);
                                            }
                                            NativePrintDecision::StartPrint => {
                                                let Some(webview) = this.webview.clone() else {
                                                    this.request_legacy_fallback(
                                                        "HTML renderer WebView disappeared before printing"
                                                            .to_string(),
                                                        window,
                                                        cx,
                                                    );
                                                    return;
                                                };
                                                this.next_print_nonce = this
                                                    .next_print_nonce
                                                    .checked_add(1)
                                                    .unwrap_or(1);
                                                let nonce = this.next_print_nonce;
                                                this.pending_print_nonce = Some(nonce);
                                                this.pending_print_started_at = Some(Instant::now());
                                                this.status = "Experimental HTML renderer — validating fonts and layout for native print..."
                                                    .to_string();
                                                match webview.update(cx, |webview, _| {
                                                    webview.raw().evaluate_script(
                                                        &native_print_preflight_script(nonce),
                                                    )
                                                }) {
                                                    Ok(()) => {
                                                        cx.notify();
                                                    }
                                                    Err(error) => {
                                                        this.pending_print_nonce = None;
                                                        this.pending_print_started_at = None;
                                                        this.request_legacy_fallback(
                                                            format!(
                                                                "HTML renderer native print preflight failed to start: {error}"
                                                            ),
                                                            window,
                                                            cx,
                                                        );
                                                    }
                                                }
                                            }
                                        }
                                    })),
                            )
                            .child(
                                Button::new("experimental-html-open-legacy")
                                    .label("Open Legacy Preview")
                                    .outline()
                                    .on_click(cx.listener(|this, _, window, cx| {
                                        let reason = "Legacy preview requested from the experimental HTML toolbar".to_string();
                                        this.request_legacy_fallback(reason, window, cx);
                                    })),
                            )
                            .child(
                                Button::new("experimental-html-close")
                                    .label("Close")
                                    .on_click(|_, window, _| window.remove_window()),
                            ),
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
fn renderer_protocol_response(
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

fn renderer_relative_path(uri_path: &str) -> Option<PathBuf> {
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
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

#[cfg(any(target_os = "macos", target_os = "windows"))]
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

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub struct PreparedHtmlPreview;

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
pub fn prepare_html_form_preview(
    _envelope: &RenderEnvelopeV1,
) -> Result<PreparedHtmlPreview, HtmlPreviewError> {
    Err(HtmlPreviewError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    struct FallbackTestEmitter;

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    impl EventEmitter<HtmlFormPreviewEvent> for FallbackTestEmitter {}

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    impl Render for FallbackTestEmitter {
        fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
            div()
        }
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    struct FallbackTestObserver {
        _subscription: gpui::Subscription,
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[gpui::test]
    fn legacy_fallback_event_is_delivered_before_the_preview_window_closes(
        cx: &mut gpui::TestAppContext,
    ) {
        use std::sync::atomic::{AtomicBool, Ordering};

        let preview_window = cx.add_window(|_, _| FallbackTestEmitter);
        let emitter = preview_window
            .root(cx)
            .expect("fallback test preview has a root emitter");
        let event_delivered = Arc::new(AtomicBool::new(false));
        let delivered_for_subscription = event_delivered.clone();
        let _observer = cx.new(|cx| {
            let subscription =
                cx.subscribe(&emitter, move |_, _, event: &HtmlFormPreviewEvent, _| {
                    let HtmlFormPreviewEvent::LegacyFallbackRequested(reason) = event;
                    assert_eq!(reason, "manual fallback");
                    delivered_for_subscription.store(true, Ordering::SeqCst);
                });
            FallbackTestObserver {
                _subscription: subscription,
            }
        });

        preview_window
            .update(cx, |_, window, cx| {
                emit_legacy_fallback_then_close("manual fallback".to_string(), window, cx);
            })
            .expect("fallback test preview remains open for the event update");
        cx.run_until_parked();

        assert!(event_delivered.load(Ordering::SeqCst));
        assert!(preview_window.root(cx).is_err());
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
        let script = renderer_initialization_script("\"{}\"", "test-token");
        assert!(script.contains("installBlockedWorkerConstructor(\"Worker\")"));
        assert!(script.contains("installBlockedWorkerConstructor(\"SharedWorker\")"));
        assert!(script.contains("Object.defineProperty(window.navigator, \"serviceWorker\""));
        assert!(script.contains("Object.defineProperty(serviceWorker, \"register\""));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn renderer_initialization_blocks_peer_media_and_device_capabilities() {
        let script = renderer_initialization_script("\"{}\"", "test-token");
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
        let script = renderer_initialization_script("\"{}\"", "test-token");
        assert!(script.contains("Object.defineProperty(window, \"print\""));
        assert!(script.contains("const originalRendererPrint = window.print.bind(window)"));
        assert!(script.contains("Object.defineProperty(window, \"authorizeEbirNativePrint\""));
        assert!(script.contains("authorizedNativePrintNonce = null"));
        assert!(script.contains("printGuardInstallationFailed = true"));
        assert!(script.contains("Native print guard installation failed"));
        assert!(script.contains("window.addEventListener(\"DOMContentLoaded\""));
        assert!(script.contains("event.key.toLowerCase() === \"p\""));
        assert!(script.contains("document.addEventListener(\"contextmenu\""));
        assert!(script.contains("event.stopImmediatePropagation()"));

        let authorized_print = native_authorized_print_script("test-token", 42);
        assert!(authorized_print.contains("window.authorizeEbirNativePrint(\"test-token\", 42)"));
        assert!(authorized_print.contains("window.print()"));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn native_print_requires_a_nonce_bound_renderer_preflight() {
        assert!(std::hint::black_box(RENDERER_WEBVIEW_IS_INCOGNITO));
        let script = native_print_preflight_script(42);
        assert!(script.contains("prepareEbirFormForNativePrint"));
        assert!(script.contains("(42)"));

        let invalidated: RendererMessage =
            serde_json::from_str(r#"{"type":"renderer_invalidated"}"#)
                .expect("parse renderer invalidation");
        assert!(matches!(invalidated, RendererMessage::RendererInvalidated));
        let print_ready: RendererMessage =
            serde_json::from_str(r#"{"type":"print_ready","nonce":42,"print_mode":true}"#)
                .expect("parse print readiness");
        assert!(matches!(
            print_ready,
            RendererMessage::PrintReady {
                nonce: 42,
                print_mode: true
            }
        ));

        let mut state = RendererState {
            ready: true,
            page_count: Some(2),
            geometry_print_mode: true,
            ..RendererState::default()
        };
        state.accept_print_ready(42, true);
        assert_eq!(state.print_ready_nonce, Some(42));
        state.invalidate();
        assert!(!state.ready);
        assert_eq!(state.page_count, None);
        assert!(!state.geometry_print_mode);
        assert_eq!(state.print_ready_nonce, None);
        assert_eq!(state.readiness_revision, 1);
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    #[test]
    fn print_ready_signal_is_rejected_without_fresh_geometry() {
        let mut state = RendererState {
            geometry_print_mode: true,
            ..RendererState::default()
        };
        state.accept_print_ready(7, true);
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
            ..RendererState::default()
        };
        state.accept_print_ready(7, false);
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
    fn native_print_failures_route_to_the_legacy_fallback() {
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
