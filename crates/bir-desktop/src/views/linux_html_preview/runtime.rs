use super::{
    LinuxDisplayEnvironment, LinuxHostLifecycle, LinuxHostLifecycleEvent, LinuxHtmlHostStrategy,
    LinuxLifecycleError, select_linux_html_host,
};
use crate::views::html_form_preview::{
    PreparedHtmlPreview, native_output_cleanup_script, native_print_preflight_script,
    prepare_html_form_preview, renderer_protocol_response, renderer_relative_path,
};
use bir_print::html::RenderEnvelopeV1;
use bir_print::html_forms::RenderLayoutPlan;
use bir_print::html_output::{
    HtmlOutputKind, PdfExpectation, create_pdf_export_temp, discard_pdf_export_temp,
    finalize_pdf_export,
};
use bir_print::html_support::{
    RendererGeometryReport, RendererPageRect, RendererReadinessDecision,
    renderer_readiness_decision, validate_renderer_geometry,
};
use gpui::prelude::FluentBuilder;
use gpui::{
    AppContext, Context, Entity, IntoElement, ParentElement, Render, Styled, Task, Window, div, px,
};
use gpui_component::ActiveTheme;
use gpui_component::Disableable;
use gpui_component::button::{Button, ButtonVariants};
use gpui_wry::WebView as GpuiWebView;
use gtk::prelude::*;
use serde::Deserialize;
use std::cell::Cell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use webkit2gtk::{
    PrintOperationExt as WebKitPrintOperationExt, SettingsExt as WebKitSettingsExt,
    WebViewExt as WebKitWebViewExt,
};
use wry::{WebViewBuilderExtUnix, WebViewExtUnix};

const READINESS_TIMEOUT: Duration = Duration::from_secs(5);
const OUTPUT_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum LinuxHtmlPreviewError {
    #[error(transparent)]
    Selection(#[from] super::LinuxHostSelectionError),
    #[error(transparent)]
    Preparation(#[from] crate::views::html_form_preview::HtmlPreviewError),
    #[error(transparent)]
    Lifecycle(#[from] LinuxLifecycleError),
    #[error("failed to start the Linux HTML preview thread: {0}")]
    ThreadStart(std::io::Error),
    #[error("Linux HTML preview startup failed: {0}")]
    Startup(String),
    #[error("Linux HTML preview startup did not respond")]
    StartupDisconnected,
}

pub(crate) enum LinuxHtmlPreviewLaunch {
    Embedded(Box<PreparedHtmlPreview>),
    GtkTopLevel,
}

pub(crate) fn launch_linux_html_preview(
    envelope: &RenderEnvelopeV1,
) -> Result<LinuxHtmlPreviewLaunch, LinuxHtmlPreviewError> {
    let environment = LinuxDisplayEnvironment::from_process();
    let strategy = select_linux_html_host(&environment)?;
    let prepared = prepare_html_form_preview(envelope)?;
    let lifecycle = LinuxHostLifecycle::Starting { strategy };

    match strategy {
        LinuxHtmlHostStrategy::GpuiWryChild => {
            let _ready = lifecycle.transition(LinuxHostLifecycleEvent::Started)?;
            Ok(LinuxHtmlPreviewLaunch::Embedded(Box::new(prepared)))
        }
        LinuxHtmlHostStrategy::GtkTopLevel => {
            launch_gtk_top_level(prepared, lifecycle)?;
            Ok(LinuxHtmlPreviewLaunch::GtkTopLevel)
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
struct LinuxRendererState {
    ready: bool,
    page_count: Option<usize>,
    print_mode: bool,
    error: Option<String>,
    print_ready_nonce: Option<u64>,
    last_geometry: Option<RendererGeometryReport>,
}

impl LinuxRendererState {
    fn invalidate(&mut self) {
        self.ready = false;
        self.page_count = None;
        self.print_mode = false;
        self.print_ready_nonce = None;
        self.last_geometry = None;
    }

    fn decision(&self) -> RendererReadinessDecision {
        renderer_readiness_decision(self.ready, self.page_count, self.error.as_deref(), false)
    }
}

#[derive(Debug, Clone)]
struct PendingLinuxOutput {
    kind: HtmlOutputKind,
    nonce: u64,
    destination: Option<PathBuf>,
    temp_path: Option<PathBuf>,
    requested_at: Instant,
    backend_started: bool,
}

#[derive(Debug, Clone)]
struct LinuxOutputCompletion {
    nonce: u64,
    result: Result<String, String>,
}

#[derive(Debug, Default)]
struct LinuxHostBridge {
    renderer: LinuxRendererState,
    next_output_nonce: u64,
    pending_output: Option<PendingLinuxOutput>,
    completion: Option<LinuxOutputCompletion>,
}

fn linux_output_timeout_reason(pending: &PendingLinuxOutput) -> Option<String> {
    let elapsed = pending.requested_at.elapsed();
    if !pending.backend_started && elapsed >= READINESS_TIMEOUT {
        return Some("Linux native output preflight timed out".to_string());
    }
    if elapsed >= OUTPUT_TIMEOUT {
        return Some(
            "Linux native output backend did not complete before its deadline".to_string(),
        );
    }
    None
}

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

fn accept_renderer_message(
    bridge: &Arc<Mutex<LinuxHostBridge>>,
    layout_plan: &RenderLayoutPlan,
    body: &str,
) {
    let Ok(message) = serde_json::from_str::<RendererMessage>(body) else {
        tracing::warn!(
            body_bytes = body.len(),
            "ignored malformed Linux renderer IPC"
        );
        return;
    };
    let Ok(mut bridge) = bridge.lock() else {
        return;
    };

    match message {
        RendererMessage::RendererReady => bridge.renderer.ready = true,
        RendererMessage::RendererInvalidated => bridge.renderer.invalidate(),
        RendererMessage::RendererError { message } => bridge.renderer.error = Some(message),
        RendererMessage::PrintReady { nonce, print_mode } => {
            if !print_mode || !bridge.renderer.print_mode {
                bridge.renderer.error =
                    Some("Linux output preflight was not measured in print mode".to_string());
                bridge.renderer.print_ready_nonce = None;
            } else if bridge
                .pending_output
                .as_ref()
                .is_some_and(|pending| pending.nonce == nonce)
                && matches!(
                    bridge.renderer.decision(),
                    RendererReadinessDecision::Ready { .. }
                )
            {
                bridge.renderer.print_ready_nonce = Some(nonce);
            } else {
                bridge.renderer.error =
                    Some("Linux output preflight returned an unexpected nonce".to_string());
                bridge.renderer.print_ready_nonce = None;
            }
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
            match validate_renderer_geometry(&report, layout_plan) {
                Ok(()) => {
                    bridge.renderer.page_count = Some(page_count);
                    bridge.renderer.print_mode = print_mode;
                    bridge.renderer.last_geometry = Some(report);
                }
                Err(error) => {
                    bridge.renderer.page_count = None;
                    bridge.renderer.print_mode = false;
                    bridge.renderer.last_geometry = None;
                    bridge.renderer.error = Some(error);
                }
            }
        }
    }
}

fn configured_webview_builder(
    prepared: &PreparedHtmlPreview,
    bridge: Arc<Mutex<LinuxHostBridge>>,
) -> wry::WebViewBuilder<'static> {
    let protocol_root = prepared.entry.parent().map(PathBuf::from);
    let layout_plan = prepared.layout_plan;
    wry::WebViewBuilder::new()
        .with_incognito(true)
        .with_custom_protocol("ebirforms".into(), move |_webview_id, request| {
            renderer_protocol_response(protocol_root.as_deref(), request)
        })
        .with_url(prepared.url.clone())
        .with_initialization_script(prepared.initialization_script.clone())
        .with_navigation_handler(|candidate| {
            let Ok(url) = url::Url::parse(&candidate) else {
                return false;
            };
            url.scheme() == "ebirforms"
                && url.host_str() == Some("localhost")
                && renderer_relative_path(url.path()).is_some()
        })
        .with_ipc_handler(move |request| {
            accept_renderer_message(&bridge, &layout_plan, request.body())
        })
}

fn harden_webkit(webview: &webkit2gtk::WebView) {
    let Some(settings) = WebKitWebViewExt::settings(webview) else {
        return;
    };
    settings.set_print_backgrounds(true);
    settings.set_enable_developer_extras(false);
    settings.set_enable_dns_prefetching(false);
    settings.set_enable_html5_database(false);
    settings.set_enable_html5_local_storage(false);
    settings.set_enable_hyperlink_auditing(false);
    settings.set_enable_media(false);
    settings.set_enable_media_stream(false);
    settings.set_enable_mediasource(false);
    settings.set_enable_offline_web_application_cache(false);
    settings.set_enable_page_cache(false);
    settings.set_enable_webaudio(false);
    settings.set_enable_webgl(false);
    settings.set_enable_webrtc(false);
    settings.set_javascript_can_access_clipboard(false);
    settings.set_javascript_can_open_windows_automatically(false);
}

fn evaluate_script(webview: &webkit2gtk::WebView, script: &str) {
    webview.run_javascript(script, gio::Cancellable::NONE, |_| {});
}

fn page_setup(expectation: &PdfExpectation) -> gtk::PageSetup {
    let paper = gtk::PaperSize::new_custom(
        "ebirforms-folio",
        "eBIRForms 8.5 x 13 in",
        expectation.width_points,
        expectation.height_points,
        gtk::Unit::Points,
    );
    let setup = gtk::PageSetup::new();
    setup.set_orientation(gtk::PageOrientation::Portrait);
    setup.set_paper_size(&paper);
    setup.set_top_margin(0.0, gtk::Unit::Points);
    setup.set_right_margin(0.0, gtk::Unit::Points);
    setup.set_bottom_margin(0.0, gtk::Unit::Points);
    setup.set_left_margin(0.0, gtk::Unit::Points);
    setup
}

fn print_settings(expectation: &PdfExpectation) -> gtk::PrintSettings {
    let settings = gtk::PrintSettings::new();
    settings.set_orientation(gtk::PageOrientation::Portrait);
    settings.set_paper_width(expectation.width_points, gtk::Unit::Points);
    settings.set_paper_height(expectation.height_points, gtk::Unit::Points);
    settings.set_scale(100.0);
    settings.set_use_color(true);
    settings.set_bool("print-backgrounds", true);
    settings
}

fn begin_system_print(
    webview: &webkit2gtk::WebView,
    parent: Option<&gtk::Window>,
    pending: PendingLinuxOutput,
    expectation: &PdfExpectation,
    bridge: Arc<Mutex<LinuxHostBridge>>,
) {
    let operation = webkit2gtk::PrintOperation::new(webview);
    operation.set_page_setup(&page_setup(expectation));
    operation.set_print_settings(&print_settings(expectation));

    let failed_bridge = bridge.clone();
    let nonce = pending.nonce;
    operation.connect_failed(move |_, error| {
        complete_output(
            &failed_bridge,
            nonce,
            Err(format!("WebKitGTK system print failed: {error}")),
        );
    });
    let finished_bridge = bridge.clone();
    operation.connect_finished(move |_| {
        complete_output(
            &finished_bridge,
            nonce,
            Ok("System print job handed to GTK".to_string()),
        );
    });

    match operation.run_dialog(parent) {
        webkit2gtk::PrintOperationResponse::Print => {}
        webkit2gtk::PrintOperationResponse::Cancel => complete_output(
            &bridge,
            nonce,
            Err("System print was cancelled".to_string()),
        ),
        _ => complete_output(
            &bridge,
            nonce,
            Err("WebKitGTK returned an unknown print response".to_string()),
        ),
    }
}

fn begin_pdf_export(
    webview: &webkit2gtk::WebView,
    pending: PendingLinuxOutput,
    expectation: PdfExpectation,
    bridge: Arc<Mutex<LinuxHostBridge>>,
) -> Result<(), String> {
    let destination = pending
        .destination
        .as_ref()
        .ok_or_else(|| "PDF export destination is missing".to_string())?;
    let temp_path = create_pdf_export_temp(destination).map_err(|error| error.to_string())?;
    register_linux_temp_path(&bridge, pending.nonce, temp_path.clone())?;
    let output_uri = url::Url::from_file_path(&temp_path)
        .map_err(|_| format!("cannot convert {} to a file URI", temp_path.display()))?;

    let settings = print_settings(&expectation);
    settings.set_printer("Print to File");
    settings.set(
        gtk::PRINT_SETTINGS_OUTPUT_URI.as_str(),
        Some(output_uri.as_str()),
    );
    settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT.as_str(), Some("pdf"));

    let operation = webkit2gtk::PrintOperation::new(webview);
    operation.set_page_setup(&page_setup(&expectation));
    operation.set_print_settings(&settings);

    let completed = Rc::new(Cell::new(false));
    let failed_completed = completed.clone();
    let failed_bridge = bridge.clone();
    let failed_temp_path = temp_path.clone();
    let nonce = pending.nonce;
    operation.connect_failed(move |_, error| {
        if failed_completed.replace(true) {
            return;
        }
        let _ = discard_pdf_export_temp(&failed_temp_path);
        complete_output(
            &failed_bridge,
            nonce,
            Err(format!("WebKitGTK PDF export failed: {error}")),
        );
    });

    let finished_completed = completed;
    let finished_bridge = bridge;
    let finished_destination = destination.clone();
    operation.connect_finished(move |_| {
        if finished_completed.replace(true) {
            return;
        }
        finish_linux_pdf_export(
            &finished_bridge,
            nonce,
            &temp_path,
            &finished_destination,
            &expectation,
        );
    });
    operation.print();
    Ok(())
}

fn register_linux_temp_path(
    bridge: &Arc<Mutex<LinuxHostBridge>>,
    nonce: u64,
    temp_path: PathBuf,
) -> Result<(), String> {
    let mut bridge = match bridge.lock() {
        Ok(bridge) => bridge,
        Err(_) => {
            let _ = discard_pdf_export_temp(&temp_path);
            return Err("Linux renderer state is unavailable".to_string());
        }
    };
    let Some(pending) = bridge.pending_output.as_mut() else {
        let _ = discard_pdf_export_temp(&temp_path);
        return Err("Linux PDF output was cancelled before its backend started".to_string());
    };
    if pending.nonce != nonce || !pending.backend_started {
        let _ = discard_pdf_export_temp(&temp_path);
        return Err("Linux PDF output no longer matches its validated nonce".to_string());
    }
    pending.temp_path = Some(temp_path);
    Ok(())
}

fn finish_linux_pdf_export(
    bridge: &Arc<Mutex<LinuxHostBridge>>,
    nonce: u64,
    temp_path: &Path,
    destination: &Path,
    expectation: &PdfExpectation,
) {
    let Ok(mut bridge) = bridge.lock() else {
        let _ = discard_pdf_export_temp(temp_path);
        return;
    };
    let is_active = bridge
        .pending_output
        .as_ref()
        .is_some_and(|pending| pending.nonce == nonce && pending.backend_started);
    if !is_active {
        let _ = discard_pdf_export_temp(temp_path);
        return;
    }

    // Serialize destination replacement with timeout and window-close
    // cancellation. Whichever path acquires the bridge first owns the nonce;
    // a late WebKit callback can therefore never replace a destination.
    let result = finalize_pdf_export(temp_path, destination, expectation)
        .map(|report| {
            format!(
                "Exported {} page(s) to {}",
                report.page_count,
                destination.display()
            )
        })
        .map_err(|error| error.to_string());
    bridge.pending_output = None;
    bridge.renderer.print_ready_nonce = None;
    bridge.completion = Some(LinuxOutputCompletion { nonce, result });
}

fn complete_output(
    bridge: &Arc<Mutex<LinuxHostBridge>>,
    nonce: u64,
    result: Result<String, String>,
) {
    if let Ok(mut bridge) = bridge.lock()
        && bridge
            .pending_output
            .as_ref()
            .is_some_and(|pending| pending.nonce == nonce)
    {
        if let Some(temp_path) = bridge
            .pending_output
            .take()
            .and_then(|pending| pending.temp_path)
        {
            let _ = discard_pdf_export_temp(&temp_path);
        }
        bridge.renderer.print_ready_nonce = None;
        bridge.completion = Some(LinuxOutputCompletion { nonce, result });
    }
}

fn abandon_pending_output(bridge: &Arc<Mutex<LinuxHostBridge>>) {
    let Ok(mut bridge) = bridge.lock() else {
        return;
    };
    if let Some(temp_path) = bridge
        .pending_output
        .take()
        .and_then(|pending| pending.temp_path)
    {
        let _ = discard_pdf_export_temp(&temp_path);
    }
    bridge.renderer.print_ready_nonce = None;
    bridge.completion = None;
}

fn expire_pending_output(
    bridge: &Arc<Mutex<LinuxHostBridge>>,
    nonce: u64,
    preflight_only: bool,
    reason: &str,
) {
    let Ok(mut bridge) = bridge.lock() else {
        return;
    };
    let should_expire = bridge.pending_output.as_ref().is_some_and(|pending| {
        pending.nonce == nonce && (!preflight_only || !pending.backend_started)
    });
    if !should_expire {
        return;
    }
    if let Some(temp_path) = bridge
        .pending_output
        .take()
        .and_then(|pending| pending.temp_path)
    {
        let _ = discard_pdf_export_temp(&temp_path);
    }
    bridge.renderer.print_ready_nonce = None;
    bridge.completion = Some(LinuxOutputCompletion {
        nonce,
        result: Err(reason.to_string()),
    });
}

fn schedule_output_deadlines(bridge: &Arc<Mutex<LinuxHostBridge>>, nonce: u64) {
    let preflight_bridge = bridge.clone();
    glib::timeout_add_once(READINESS_TIMEOUT, move || {
        expire_pending_output(
            &preflight_bridge,
            nonce,
            true,
            "Linux native output preflight timed out",
        );
    });
    let backend_bridge = bridge.clone();
    glib::timeout_add_once(OUTPUT_TIMEOUT, move || {
        expire_pending_output(
            &backend_bridge,
            nonce,
            false,
            "Linux native output backend did not complete before its deadline",
        );
    });
}

fn request_output_preflight(
    webview: &webkit2gtk::WebView,
    bridge: &Arc<Mutex<LinuxHostBridge>>,
    kind: HtmlOutputKind,
    destination: Option<PathBuf>,
) -> Result<u64, String> {
    let nonce = {
        let mut bridge = bridge
            .lock()
            .map_err(|_| "Linux renderer state is unavailable".to_string())?;
        if bridge.pending_output.is_some() {
            return Err("another print or export operation is already running".to_string());
        }
        if !matches!(
            bridge.renderer.decision(),
            RendererReadinessDecision::Ready { .. }
        ) {
            return Err("HTML renderer is not ready for print or export".to_string());
        }
        bridge.next_output_nonce = bridge.next_output_nonce.saturating_add(1).max(1);
        let nonce = bridge.next_output_nonce;
        bridge.renderer.print_ready_nonce = None;
        bridge.pending_output = Some(PendingLinuxOutput {
            kind,
            nonce,
            destination,
            temp_path: None,
            requested_at: Instant::now(),
            backend_started: false,
        });
        nonce
    };
    schedule_output_deadlines(bridge, nonce);
    evaluate_script(webview, &native_print_preflight_script(nonce));
    Ok(nonce)
}

fn take_ready_output(bridge: &Arc<Mutex<LinuxHostBridge>>) -> Option<PendingLinuxOutput> {
    let mut bridge = bridge.lock().ok()?;
    let pending = bridge.pending_output.clone()?;
    if let Some(reason) = linux_output_timeout_reason(&pending) {
        let nonce = pending.nonce;
        if let Some(temp_path) = bridge
            .pending_output
            .take()
            .and_then(|pending| pending.temp_path)
        {
            let _ = discard_pdf_export_temp(&temp_path);
        }
        bridge.renderer.print_ready_nonce = None;
        bridge.completion = Some(LinuxOutputCompletion {
            nonce,
            result: Err(reason),
        });
        return None;
    }
    if pending.backend_started || bridge.renderer.print_ready_nonce != Some(pending.nonce) {
        return None;
    }

    bridge.renderer.print_ready_nonce = None;
    if let Some(stored) = bridge.pending_output.as_mut() {
        stored.backend_started = true;
    }
    let mut started = pending;
    started.backend_started = true;
    Some(started)
}

fn take_completion(bridge: &Arc<Mutex<LinuxHostBridge>>) -> Option<LinuxOutputCompletion> {
    bridge.lock().ok()?.completion.take()
}

fn launch_gtk_top_level(
    prepared: PreparedHtmlPreview,
    lifecycle: LinuxHostLifecycle,
) -> Result<(), LinuxHtmlPreviewError> {
    let (startup_tx, startup_rx) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("ebirforms-html-preview-gtk".to_string())
        .spawn(move || {
            let result = run_gtk_top_level(prepared, lifecycle, &startup_tx);
            if let Err(error) = result {
                let _ = startup_tx.send(Err(error));
            }
        })
        .map_err(LinuxHtmlPreviewError::ThreadStart)?;

    match startup_rx.recv_timeout(READINESS_TIMEOUT) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(LinuxHtmlPreviewError::Startup(error)),
        Err(mpsc::RecvTimeoutError::Timeout) => Err(LinuxHtmlPreviewError::Startup(
            "GTK/WebKit host initialization timed out".to_string(),
        )),
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            Err(LinuxHtmlPreviewError::StartupDisconnected)
        }
    }
}

fn run_gtk_top_level(
    prepared: PreparedHtmlPreview,
    lifecycle: LinuxHostLifecycle,
    startup_tx: &mpsc::SyncSender<Result<(), String>>,
) -> Result<(), String> {
    gtk::init().map_err(|error| format!("GTK initialization failed: {error}"))?;
    let lifecycle = lifecycle
        .transition(LinuxHostLifecycleEvent::Started)
        .map_err(|error| error.to_string())?;
    let lifecycle = Rc::new(std::cell::RefCell::new(Some(lifecycle)));

    let window = gtk::Window::new(gtk::WindowType::Toplevel);
    window.set_title("HTML Form Preview");
    window.set_default_size(1200, 900);

    let root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    let toolbar = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    toolbar.set_margin_top(8);
    toolbar.set_margin_end(8);
    toolbar.set_margin_bottom(8);
    toolbar.set_margin_start(8);
    let status = gtk::Label::new(Some("HTML renderer: preparing preview..."));
    status.set_xalign(0.0);
    status.set_hexpand(true);
    let export_button = gtk::Button::with_label("Export PDF");
    let print_button = gtk::Button::with_label("Print");
    let close_button = gtk::Button::with_label("Close");
    export_button.set_sensitive(false);
    print_button.set_sensitive(false);
    toolbar.pack_start(&status, true, true, 0);
    toolbar.pack_end(&close_button, false, false, 0);
    toolbar.pack_end(&print_button, false, false, 0);
    toolbar.pack_end(&export_button, false, false, 0);
    root.pack_start(&toolbar, false, false, 0);

    let webview_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.pack_start(&webview_container, true, true, 0);
    window.add(&root);

    let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
    let webview = configured_webview_builder(&prepared, bridge.clone())
        .build_gtk(&webview_container)
        .map_err(|error| format!("WebKitGTK host construction failed: {error}"))?;
    let webview = Rc::new(webview);
    let webkit = webview.webview();
    harden_webkit(&webkit);

    let print_webkit = webkit.clone();
    let print_bridge = bridge.clone();
    let print_status = status.clone();
    print_button.connect_clicked(move |_| {
        match request_output_preflight(
            &print_webkit,
            &print_bridge,
            HtmlOutputKind::SystemPrint,
            None,
        ) {
            Ok(_) => print_status.set_text("Validating fonts and geometry for system print..."),
            Err(error) => print_status.set_text(&error),
        }
    });

    let export_webkit = webkit.clone();
    let export_bridge = bridge.clone();
    let export_status = status.clone();
    let export_parent = window.clone();
    let default_pdf_name = prepared.default_pdf_name.clone();
    export_button.connect_clicked(move |_| {
        let chooser = gtk::FileChooserNative::new(
            Some("Export BIR Form as PDF"),
            Some(&export_parent),
            gtk::FileChooserAction::Save,
            Some("Export"),
            Some("Cancel"),
        );
        chooser.set_current_name(&default_pdf_name);
        chooser.set_do_overwrite_confirmation(true);
        let response_webkit = export_webkit.clone();
        let response_bridge = export_bridge.clone();
        let response_status = export_status.clone();
        chooser.connect_response(move |chooser, response| {
            if response == gtk::ResponseType::Accept {
                match chooser.filename() {
                    Some(destination) => match request_output_preflight(
                        &response_webkit,
                        &response_bridge,
                        HtmlOutputKind::PdfExport,
                        Some(destination),
                    ) {
                        Ok(_) => response_status
                            .set_text("Validating fonts and geometry for PDF export..."),
                        Err(error) => response_status.set_text(&error),
                    },
                    None => response_status.set_text("No PDF destination was selected"),
                }
            }
            chooser.destroy();
        });
        chooser.show();
    });

    let close_window = window.clone();
    close_button.connect_clicked(move |_| close_window.close());

    let close_lifecycle = lifecycle.clone();
    let close_bridge = bridge.clone();
    window.connect_delete_event(move |_, _| {
        abandon_pending_output(&close_bridge);
        if let Some(current) = close_lifecycle.borrow_mut().take() {
            let next = current
                .transition(LinuxHostLifecycleEvent::CloseRequested)
                .unwrap_or_else(|error| LinuxHostLifecycle::Failed(error.to_string()));
            close_lifecycle.borrow_mut().replace(next);
        }
        glib::Propagation::Proceed
    });
    let destroyed_lifecycle = lifecycle;
    let destroyed_bridge = bridge.clone();
    window.connect_destroy(move |_| {
        abandon_pending_output(&destroyed_bridge);
        if let Some(current) = destroyed_lifecycle.borrow_mut().take() {
            let next = current
                .transition(LinuxHostLifecycleEvent::Closed)
                .unwrap_or_else(|error| LinuxHostLifecycle::Failed(error.to_string()));
            destroyed_lifecycle.borrow_mut().replace(next);
        }
        gtk::main_quit();
    });

    let poll_bridge = bridge;
    let poll_webkit = webkit;
    let poll_window = window.clone();
    let poll_status = status;
    let poll_export_button = export_button;
    let poll_print_button = print_button;
    let expectation = prepared.pdf_expectation;
    let readiness_started_at = Instant::now();
    glib::timeout_add_local(Duration::from_millis(100), move || {
        // Retain the Wry owner for as long as the GTK window is alive.
        let _webview_owner = &webview;
        if !poll_window.is_visible() {
            return glib::ControlFlow::Break;
        }

        let (decision, output_pending, has_geometry) = poll_bridge
            .lock()
            .map(|bridge| {
                (
                    bridge.renderer.decision(),
                    bridge.pending_output.is_some(),
                    bridge.renderer.last_geometry.is_some(),
                )
            })
            .unwrap_or_else(|_| {
                (
                    RendererReadinessDecision::Fallback(
                        "Linux renderer state is unavailable".to_string(),
                    ),
                    true,
                    false,
                )
            });

        if !has_geometry && readiness_started_at.elapsed() >= READINESS_TIMEOUT {
            if let Ok(mut bridge) = poll_bridge.lock() {
                bridge.renderer.error = Some("Linux HTML renderer readiness timed out".to_string());
            }
        }

        let can_output =
            matches!(decision, RendererReadinessDecision::Ready { .. }) && !output_pending;
        poll_export_button.set_sensitive(can_output);
        poll_print_button.set_sensitive(can_output);
        match decision {
            RendererReadinessDecision::Pending if !output_pending => {
                poll_status.set_text("HTML renderer: preparing preview...")
            }
            RendererReadinessDecision::Ready { page_count } if !output_pending => {
                poll_status.set_text(&format!("HTML renderer ready — {page_count} page(s)"))
            }
            RendererReadinessDecision::Fallback(error) => poll_status.set_text(&error),
            _ => {}
        }

        if let Some(pending) = take_ready_output(&poll_bridge) {
            match pending.kind {
                HtmlOutputKind::SystemPrint => begin_system_print(
                    &poll_webkit,
                    Some(&poll_window),
                    pending,
                    &expectation,
                    poll_bridge.clone(),
                ),
                HtmlOutputKind::PdfExport => {
                    if let Err(error) = begin_pdf_export(
                        &poll_webkit,
                        pending.clone(),
                        expectation.clone(),
                        poll_bridge.clone(),
                    ) {
                        complete_output(&poll_bridge, pending.nonce, Err(error));
                    }
                }
            }
        }

        if let Some(completion) = take_completion(&poll_bridge) {
            evaluate_script(&poll_webkit, native_output_cleanup_script());
            match completion.result {
                Ok(message) => poll_status.set_text(&message),
                Err(error) => poll_status.set_text(&error),
            }
        }

        glib::ControlFlow::Continue
    });

    window.show_all();
    // The launcher waits only for construction, not for renderer readiness.
    let _ = startup_tx.send(Ok(()));
    gtk::main();
    Ok(())
}

pub(crate) struct LinuxEmbeddedHtmlPreviewView {
    webview: Option<Entity<GpuiWebView>>,
    bridge: Arc<Mutex<LinuxHostBridge>>,
    status: String,
    pdf_expectation: PdfExpectation,
    default_pdf_name: String,
    _poll_task: Task<()>,
}

impl LinuxEmbeddedHtmlPreviewView {
    pub fn new(prepared: PreparedHtmlPreview, window: &mut Window, cx: &mut Context<Self>) -> Self {
        use raw_window_handle::HasWindowHandle;

        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let result = window
            .window_handle()
            .map_err(|error| error.to_string())
            .and_then(|window_handle| {
                configured_webview_builder(&prepared, bridge.clone())
                    .build_as_child(&window_handle)
                    .map_err(|error| error.to_string())
            });

        let (webview, status) = match result {
            Ok(webview) => {
                harden_webkit(&webview.webview());
                (
                    Some(cx.new(|cx| GpuiWebView::new(webview, window, cx))),
                    "HTML renderer: preparing X11 child preview...".to_string(),
                )
            }
            Err(error) => {
                if let Ok(mut bridge) = bridge.lock() {
                    bridge.renderer.error = Some(format!(
                        "X11 Wry child construction failed; select the GTK host with \
                         EBIRFORMS_HTML_LINUX_HOST=gtk: {error}"
                    ));
                }
                (None, format!("Linux HTML renderer failed: {error}"))
            }
        };

        let poll_bridge = bridge.clone();
        let readiness_started_at = Instant::now();
        let poll_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;

                if readiness_started_at.elapsed() >= READINESS_TIMEOUT {
                    if let Ok(mut bridge) = poll_bridge.lock()
                        && bridge.renderer.last_geometry.is_none()
                        && bridge.renderer.error.is_none()
                    {
                        bridge.renderer.error =
                            Some("Linux HTML renderer readiness timed out".to_string());
                    }
                }

                let ready_output = take_ready_output(&poll_bridge);
                let completion = take_completion(&poll_bridge);
                let snapshot = poll_bridge
                    .lock()
                    .ok()
                    .map(|bridge| (bridge.renderer.decision(), bridge.pending_output.is_some()));
                let update = this.update(cx, |this, cx| {
                    if let Some(pending) = ready_output {
                        this.begin_output(pending, cx);
                    }
                    if let Some(completion) = completion {
                        this.cleanup_print_mode(cx);
                        this.status = completion
                            .result
                            .unwrap_or_else(|error| format!("Linux output failed: {error}"));
                    } else if let Some((decision, output_pending)) = snapshot {
                        match decision {
                            RendererReadinessDecision::Pending if !output_pending => {
                                this.status =
                                    "HTML renderer: preparing X11 child preview...".to_string()
                            }
                            RendererReadinessDecision::Ready { page_count } if !output_pending => {
                                this.status = format!("HTML renderer ready — {page_count} page(s)")
                            }
                            RendererReadinessDecision::Fallback(error) => this.status = error,
                            _ => {}
                        }
                    }
                    cx.notify();
                });
                if update.is_err() {
                    break;
                }
            }
        });

        Self {
            webview,
            bridge,
            status,
            pdf_expectation: prepared.pdf_expectation,
            default_pdf_name: prepared.default_pdf_name,
            _poll_task: poll_task,
        }
    }

    fn request_output(
        &mut self,
        kind: HtmlOutputKind,
        destination: Option<PathBuf>,
        cx: &mut Context<Self>,
    ) {
        let Some(webview) = self.webview.clone() else {
            self.status = "Linux renderer WebView is unavailable".to_string();
            cx.notify();
            return;
        };
        let bridge = self.bridge.clone();
        let result = webview.update(cx, |webview, _| {
            request_output_preflight(&webview.raw().webview(), &bridge, kind, destination)
        });
        match result {
            Ok(_) => {
                self.status = match kind {
                    HtmlOutputKind::SystemPrint => {
                        "Validating fonts and geometry for system print...".to_string()
                    }
                    HtmlOutputKind::PdfExport => {
                        "Validating fonts and geometry for PDF export...".to_string()
                    }
                }
            }
            Err(error) => self.status = error,
        }
        cx.notify();
    }

    fn begin_output(&mut self, pending: PendingLinuxOutput, cx: &mut Context<Self>) {
        let Some(webview) = self.webview.clone() else {
            complete_output(
                &self.bridge,
                pending.nonce,
                Err("Linux renderer WebView disappeared before output".to_string()),
            );
            return;
        };
        let bridge = self.bridge.clone();
        let expectation = self.pdf_expectation.clone();
        let pending_for_error = pending.clone();
        let result = webview.update(cx, |webview, _| {
            let webkit = webview.raw().webview();
            match pending.kind {
                HtmlOutputKind::SystemPrint => {
                    begin_system_print(&webkit, None, pending, &expectation, bridge.clone());
                    Ok(())
                }
                HtmlOutputKind::PdfExport => {
                    begin_pdf_export(&webkit, pending, expectation, bridge.clone())
                }
            }
        });
        if let Err(error) = result {
            complete_output(
                &self.bridge,
                pending_for_error.nonce,
                Err(error.to_string()),
            );
        }
    }

    fn cleanup_print_mode(&self, cx: &mut Context<Self>) {
        if let Some(webview) = self.webview.clone() {
            let _ = webview.update(cx, |webview, _| {
                webview
                    .raw()
                    .evaluate_script(native_output_cleanup_script())
            });
        }
    }
}

impl Drop for LinuxEmbeddedHtmlPreviewView {
    fn drop(&mut self) {
        abandon_pending_output(&self.bridge);
    }
}

impl Render for LinuxEmbeddedHtmlPreviewView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let output_enabled = self.bridge.lock().is_ok_and(|bridge| {
            matches!(
                bridge.renderer.decision(),
                RendererReadinessDecision::Ready { .. }
            ) && bridge.pending_output.is_none()
                && self.webview.is_some()
        });

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
                                Button::new("linux-html-export-pdf")
                                    .label("Export PDF")
                                    .disabled(!output_enabled)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let destination = rfd::FileDialog::new()
                                            .set_title("Export BIR Form as PDF")
                                            .set_file_name(&this.default_pdf_name)
                                            .add_filter("PDF document", &["pdf"])
                                            .save_file();
                                        if let Some(destination) = destination {
                                            this.request_output(
                                                HtmlOutputKind::PdfExport,
                                                Some(destination),
                                                cx,
                                            );
                                        }
                                    })),
                            )
                            .child(
                                Button::new("linux-html-print")
                                    .label("Print")
                                    .disabled(!output_enabled)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_output(HtmlOutputKind::SystemPrint, None, cx);
                                    })),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .when_some(self.webview.clone(), |this, webview| this.child(webview))
                    .when(self.webview.is_none(), |this| {
                        this.p_6()
                            .child("Linux HTML preview could not be initialized")
                    }),
            )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bir_print::html_forms::{RenderFixtureKind, render_form_provider};
    use bir_print::html_support::renderer_host_plan;

    fn test_layout_plan() -> RenderLayoutPlan {
        let provider = render_form_provider("2551Q", "2018").expect("2551Q provider");
        let envelope = (provider.fixtures)()
            .expect("provider fixtures")
            .into_iter()
            .find(|fixture| fixture.kind == RenderFixtureKind::Minimum)
            .expect("minimum fixture")
            .envelope;
        renderer_host_plan(&envelope).expect("2551Q layout plan")
    }

    fn geometry_message(print_mode: bool, overflow_x: usize) -> String {
        serde_json::json!({
            "type": "page_count",
            "page_count": 2,
            "page_width_pt": 612.0,
            "page_height_pt": 936.0,
            "print_mode": print_mode,
            "pages": [
                {
                    "x": 0.0,
                    "y": 0.0,
                    "width": 816.0,
                    "height": 1248.0,
                    "client_width": 816.0,
                    "client_height": 1248.0,
                    "scroll_width": 816.0,
                    "scroll_height": 1248.0,
                    "descendant_overflow_x": overflow_x,
                    "descendant_overflow_y": 0,
                    "descendant_clipped_x": 0,
                    "descendant_clipped_y": 0
                },
                {
                    "x": 0.0,
                    "y": 1248.0,
                    "width": 816.0,
                    "height": 1248.0,
                    "client_width": 816.0,
                    "client_height": 1248.0,
                    "scroll_width": 816.0,
                    "scroll_height": 1248.0,
                    "descendant_overflow_x": 0,
                    "descendant_overflow_y": 0,
                    "descendant_clipped_x": 0,
                    "descendant_clipped_y": 0
                }
            ]
        })
        .to_string()
    }

    fn pending_output(nonce: u64) -> PendingLinuxOutput {
        PendingLinuxOutput {
            kind: HtmlOutputKind::SystemPrint,
            nonce,
            destination: None,
            temp_path: None,
            requested_at: Instant::now(),
            backend_started: false,
        }
    }

    #[test]
    fn print_preflight_consumes_a_matching_nonce_only_once() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        accept_renderer_message(&bridge, &plan, r#"{"type":"renderer_ready"}"#);
        accept_renderer_message(&bridge, &plan, &geometry_message(true, 0));
        bridge.lock().expect("bridge").pending_output = Some(pending_output(42));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":42,"print_mode":true}"#,
        );

        let started = take_ready_output(&bridge).expect("matching nonce starts output");
        assert_eq!(started.nonce, 42);
        assert!(started.backend_started);
        assert!(take_ready_output(&bridge).is_none());
        assert!(
            bridge
                .lock()
                .expect("bridge")
                .pending_output
                .as_ref()
                .is_some_and(|pending| pending.backend_started)
        );
    }

    #[test]
    fn print_preflight_rejects_a_mismatched_nonce() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        accept_renderer_message(&bridge, &plan, r#"{"type":"renderer_ready"}"#);
        accept_renderer_message(&bridge, &plan, &geometry_message(true, 0));
        bridge.lock().expect("bridge").pending_output = Some(pending_output(42));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":41,"print_mode":true}"#,
        );

        let bridge = bridge.lock().expect("bridge");
        assert_eq!(bridge.renderer.print_ready_nonce, None);
        assert!(
            bridge
                .renderer
                .error
                .as_deref()
                .is_some_and(|error| error.contains("unexpected nonce"))
        );
    }

    #[test]
    fn geometry_with_descendant_overflow_fails_closed() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        accept_renderer_message(&bridge, &plan, r#"{"type":"renderer_ready"}"#);
        accept_renderer_message(&bridge, &plan, &geometry_message(false, 1));

        let bridge = bridge.lock().expect("bridge");
        assert_eq!(bridge.renderer.page_count, None);
        assert!(matches!(
            bridge.renderer.decision(),
            RendererReadinessDecision::Fallback(_)
        ));
    }

    #[test]
    fn running_backend_expires_at_the_overall_output_deadline() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let mut pending = pending_output(9);
        pending.requested_at = Instant::now() - OUTPUT_TIMEOUT;
        pending.backend_started = true;
        bridge.lock().expect("bridge").pending_output = Some(pending);

        assert!(take_ready_output(&bridge).is_none());
        let completion = take_completion(&bridge).expect("timeout completion");
        assert!(
            completion
                .result
                .is_err_and(|error| error.contains("backend did not complete"))
        );
    }

    #[test]
    fn preflight_expires_at_the_shorter_readiness_deadline() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let mut pending = pending_output(10);
        pending.requested_at = Instant::now() - READINESS_TIMEOUT;
        bridge.lock().expect("bridge").pending_output = Some(pending);

        assert!(take_ready_output(&bridge).is_none());
        let completion = take_completion(&bridge).expect("timeout completion");
        assert!(
            completion
                .result
                .is_err_and(|error| error.contains("preflight timed out"))
        );
    }

    #[test]
    fn abandoning_output_discards_its_registered_temp_file() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let temp_path = std::env::temp_dir().join(format!(
            "ebirforms-linux-cancelled-output-{}.pdf",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&temp_path, b"partial").expect("write test temp");
        let mut pending = pending_output(11);
        pending.temp_path = Some(temp_path.clone());
        bridge.lock().expect("bridge").pending_output = Some(pending);

        abandon_pending_output(&bridge);

        assert!(!temp_path.exists());
    }

    #[test]
    fn late_pdf_callback_after_abandon_cannot_replace_destination() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let suffix = uuid::Uuid::new_v4();
        let temp_path =
            std::env::temp_dir().join(format!("ebirforms-linux-late-output-{suffix}.pdf"));
        let destination =
            std::env::temp_dir().join(format!("ebirforms-linux-existing-destination-{suffix}.pdf"));
        std::fs::write(&destination, b"existing").expect("write existing destination");
        let mut pending = pending_output(12);
        pending.backend_started = true;
        pending.temp_path = Some(temp_path.clone());
        bridge.lock().expect("bridge").pending_output = Some(pending);
        abandon_pending_output(&bridge);
        std::fs::write(&temp_path, b"late backend bytes").expect("simulate late backend write");
        let expectation = PdfExpectation {
            form_code: "2551Q".to_string(),
            revision: "2018".to_string(),
            envelope_hash: "a".repeat(64),
            expected_page_count: 2,
            width_points: 612.0,
            height_points: 936.0,
        };

        finish_linux_pdf_export(&bridge, 12, &temp_path, &destination, &expectation);

        assert_eq!(
            std::fs::read(&destination).expect("read existing destination"),
            b"existing"
        );
        assert!(!temp_path.exists());
        std::fs::remove_file(destination).expect("remove test destination");
    }
}
