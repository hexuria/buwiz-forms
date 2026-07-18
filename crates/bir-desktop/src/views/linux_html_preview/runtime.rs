use super::{
    LinuxDisplayEnvironment, LinuxHostLifecycle, LinuxHostLifecycleEvent, LinuxHtmlHostStrategy,
    LinuxLifecycleError, LinuxNativePrintContract, LinuxRendererRetryAction,
    linux_native_print_contract, linux_renderer_retry_action, select_linux_html_host,
};
use crate::views::html_form_preview::{
    PreparedHtmlPreview, RendererDocumentIdentity, native_output_cleanup_script,
    native_print_preflight_script, prepare_html_form_preview, renderer_document_identity_script,
    renderer_protocol_response, renderer_relative_path,
};
use bir_print::html::RenderEnvelopeV1;
use bir_print::html_forms::RenderLayoutPlan;
use bir_print::html_output::{
    HtmlOutputKind, HtmlOutputTimeoutStage, PdfExpectation, create_pdf_export_temp,
    discard_pdf_export_temp, finalize_pdf_export, html_output_timeout_stage,
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
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::time::{Duration, Instant};
use webkit2gtk::{
    PrintOperationExt as WebKitPrintOperationExt, SettingsExt as WebKitSettingsExt,
    WebViewExt as WebKitWebViewExt,
};
use wry::{WebViewBuilderExtUnix, WebViewExtUnix};

const READINESS_TIMEOUT: Duration = Duration::from_secs(5);
const PDF_EXPORT_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_TOMBSTONED_OUTPUT_NONCES: usize = 64;

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
    document_identity: Option<RendererDocumentIdentity>,
    document_boot_accepted: bool,
    document_identity_rejected: bool,
    ready: bool,
    page_count: Option<usize>,
    print_mode: bool,
    error: Option<String>,
    render_epoch: u64,
    readiness_revision: u64,
    print_ready_nonce: Option<u64>,
    last_geometry: Option<RendererGeometryReport>,
}

impl LinuxRendererState {
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
        self.print_mode = false;
        self.error = Some(reason.into());
        self.print_ready_nonce = None;
        self.last_geometry = None;
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
        self.print_mode = false;
        self.error = None;
        self.print_ready_nonce = None;
        self.last_geometry = None;
        self.readiness_revision = self.readiness_revision.saturating_add(1);
        true
    }

    fn accepts_epoch(&self, render_epoch: u64) -> bool {
        render_epoch != 0 && render_epoch == self.render_epoch
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
    binding: Option<LinuxOutputRendererBinding>,
}

#[derive(Debug, Clone, PartialEq)]
struct LinuxOutputRendererBinding {
    document_identity: RendererDocumentIdentity,
    render_epoch: u64,
    readiness_revision: u64,
    geometry: RendererGeometryReport,
}

#[derive(Debug, Clone)]
struct LinuxOutputCompletion {
    nonce: u64,
    result: Result<String, String>,
}

#[derive(Debug)]
struct LinuxHostBridge {
    renderer: LinuxRendererState,
    next_output_nonce: u64,
    pending_output: Option<PendingLinuxOutput>,
    completion: Option<LinuxOutputCompletion>,
    tombstoned_output_nonces: HashSet<u64>,
    readiness_started_at: Instant,
}

impl Default for LinuxHostBridge {
    fn default() -> Self {
        Self {
            renderer: LinuxRendererState::default(),
            next_output_nonce: 0,
            pending_output: None,
            completion: None,
            tombstoned_output_nonces: HashSet::new(),
            readiness_started_at: Instant::now(),
        }
    }
}

fn bind_renderer_for_linux_output(
    state: &LinuxRendererState,
) -> Result<LinuxOutputRendererBinding, String> {
    let document_identity = state
        .document_identity
        .clone()
        .ok_or_else(|| "Linux native output has no immutable document identity".to_string())?;
    if !state.document_boot_accepted || state.document_identity_rejected {
        return Err(
            "Linux native output document identity handshake is incomplete or rejected".to_string(),
        );
    }
    if state.render_epoch == 0 {
        return Err("Linux native output has no validated renderer epoch".to_string());
    }
    if !state.print_mode || !matches!(state.decision(), RendererReadinessDecision::Ready { .. }) {
        return Err("Linux native output renderer epoch is not ready in print mode".to_string());
    }
    let geometry = state
        .last_geometry
        .clone()
        .ok_or_else(|| "Linux native output has no validated page geometry".to_string())?;
    if geometry.pages.is_empty()
        || state.page_count != Some(geometry.page_count)
        || geometry.page_count != geometry.pages.len()
    {
        return Err("Linux native output has incomplete page rectangles".to_string());
    }
    Ok(LinuxOutputRendererBinding {
        document_identity,
        render_epoch: state.render_epoch,
        readiness_revision: state.readiness_revision,
        geometry,
    })
}

fn linux_renderer_binding_mismatch_reason(
    state: &LinuxRendererState,
    binding: &LinuxOutputRendererBinding,
) -> Option<String> {
    if state.document_identity.as_ref() != Some(&binding.document_identity)
        || !state.document_boot_accepted
        || state.document_identity_rejected
    {
        return Some(
            "Linux renderer document identity changed after native output started".to_string(),
        );
    }
    if state.render_epoch != binding.render_epoch
        || state.readiness_revision != binding.readiness_revision
    {
        return Some("Linux renderer epoch changed after native output started".to_string());
    }
    if !state.print_mode || !matches!(state.decision(), RendererReadinessDecision::Ready { .. }) {
        return Some(
            "Linux renderer readiness was invalidated after native output started".to_string(),
        );
    }
    if state.last_geometry.as_ref() != Some(&binding.geometry) {
        return Some(
            "Linux renderer page geometry changed after native output started".to_string(),
        );
    }
    None
}

fn pending_output_binding_error(
    pending: &PendingLinuxOutput,
    state: &LinuxRendererState,
    completion_render_epoch: u64,
) -> Option<String> {
    if !pending.backend_started {
        return Some(
            "Linux native backend completed before its renderer epoch was bound".to_string(),
        );
    }
    let Some(binding) = pending.binding.as_ref() else {
        return Some("Linux native backend completed without a renderer binding".to_string());
    };
    if completion_render_epoch != binding.render_epoch {
        return Some("Linux native backend completion used a stale renderer epoch".to_string());
    }
    linux_renderer_binding_mismatch_reason(state, binding)
}

fn cancel_pending_output_locked(bridge: &mut LinuxHostBridge, reason: String) {
    let Some(pending) = bridge.pending_output.take() else {
        return;
    };
    let nonce = pending.nonce;
    tombstone_output_nonce(bridge, nonce);
    if let Some(temp_path) = pending.temp_path {
        let _ = discard_pdf_export_temp(&temp_path);
    }
    bridge.renderer.print_ready_nonce = None;
    bridge.completion = Some(LinuxOutputCompletion {
        nonce,
        result: Err(reason),
    });
}

fn tombstone_output_nonce(bridge: &mut LinuxHostBridge, nonce: u64) {
    bridge.tombstoned_output_nonces.insert(nonce);
    while bridge.tombstoned_output_nonces.len() > MAX_TOMBSTONED_OUTPUT_NONCES {
        let Some(oldest) = bridge.tombstoned_output_nonces.iter().copied().min() else {
            break;
        };
        bridge.tombstoned_output_nonces.remove(&oldest);
    }
}

fn cancel_output_if_renderer_binding_changed(bridge: &mut LinuxHostBridge) {
    let reason = bridge.pending_output.as_ref().and_then(|pending| {
        if !pending.backend_started {
            return None;
        }
        match pending.binding.as_ref() {
            Some(binding) => linux_renderer_binding_mismatch_reason(&bridge.renderer, binding),
            None => Some("Linux native output started without a renderer binding".to_string()),
        }
    });
    if let Some(reason) = reason {
        cancel_pending_output_locked(bridge, reason);
    }
}

fn linux_output_timeout_reason(pending: &PendingLinuxOutput) -> Option<String> {
    match html_output_timeout_stage(
        pending.kind,
        pending.backend_started,
        pending.requested_at.elapsed(),
        READINESS_TIMEOUT,
        PDF_EXPORT_TIMEOUT,
    ) {
        Some(HtmlOutputTimeoutStage::Preflight) => {
            Some("Linux native output preflight timed out".to_string())
        }
        Some(HtmlOutputTimeoutStage::PdfExportBackend) => {
            Some("Linux native PDF export backend did not complete before its deadline".to_string())
        }
        None => None,
    }
}

#[derive(Debug, Deserialize)]
struct RendererIpcMessage {
    document_run_id: String,
    envelope_hash: String,
    #[serde(flatten)]
    message: RendererMessage,
}

impl RendererIpcMessage {
    fn document_identity(&self) -> RendererDocumentIdentity {
        RendererDocumentIdentity {
            document_run_id: self.document_run_id.clone(),
            envelope_hash: self.envelope_hash.clone(),
        }
    }
}

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

#[derive(Debug, Deserialize)]
struct RendererGeometryReportMessage {
    page_count: usize,
    page_width_pt: f64,
    page_height_pt: f64,
    pages: Vec<RendererPageRectMessage>,
}

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

fn accept_renderer_message(
    bridge: &Arc<Mutex<LinuxHostBridge>>,
    layout_plan: &RenderLayoutPlan,
    body: &str,
) {
    let message = match serde_json::from_str::<RendererIpcMessage>(body) {
        Ok(message) => message,
        Err(_) => {
            if let Ok(mut bridge) = bridge.lock() {
                bridge.renderer.reject_document_identity(
                    "Linux renderer IPC omitted or malformed the immutable document identity",
                );
                cancel_output_if_renderer_binding_changed(&mut bridge);
            }
            tracing::warn!(
                body_bytes = body.len(),
                "ignored malformed Linux renderer IPC"
            );
            return;
        }
    };
    let Ok(mut bridge) = bridge.lock() else {
        return;
    };

    let identity = message.document_identity();
    if bridge.renderer.document_identity_rejected {
        return;
    }
    if !bridge.renderer.accepts_document_identity(&identity) {
        bridge.renderer.reject_document_identity(
            "Linux renderer IPC document run ID or envelope hash did not match the host document",
        );
        cancel_output_if_renderer_binding_changed(&mut bridge);
        return;
    }
    let message = match message.message {
        RendererMessage::RendererBoot => {
            if bridge.renderer.document_boot_accepted {
                bridge.renderer.reject_document_identity(
                    "Linux renderer document run ID was replayed by a reload or replacement document",
                );
            } else {
                bridge.renderer.document_boot_accepted = true;
            }
            cancel_output_if_renderer_binding_changed(&mut bridge);
            return;
        }
        _ if !bridge.renderer.document_boot_accepted => {
            bridge.renderer.reject_document_identity(
                "Linux renderer IPC arrived before the host document identity boot handshake",
            );
            cancel_output_if_renderer_binding_changed(&mut bridge);
            return;
        }
        message => message,
    };

    match message {
        RendererMessage::RendererBoot => unreachable!("renderer boot handled by identity gate"),
        RendererMessage::RendererReady { render_epoch } => {
            if bridge.renderer.accepts_epoch(render_epoch) {
                bridge.renderer.ready = true;
            }
        }
        RendererMessage::RendererInvalidated { render_epoch } => {
            if bridge.renderer.invalidate_for_epoch(render_epoch) {
                bridge.readiness_started_at = Instant::now();
            }
        }
        RendererMessage::RendererError {
            render_epoch,
            message,
        } => {
            if bridge.renderer.accepts_epoch(render_epoch) {
                bridge.renderer.error = Some(message);
            }
        }
        RendererMessage::PrintReady {
            nonce,
            render_epoch,
            print_mode,
        } => {
            if !bridge.renderer.accepts_epoch(render_epoch)
                || bridge.tombstoned_output_nonces.contains(&nonce)
            {
                return;
            }
            match bridge.pending_output.as_ref().map(|pending| pending.nonce) {
                None => return,
                Some(pending_nonce) if nonce < pending_nonce => return,
                _ => {}
            }
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
            render_epoch,
            print_mode,
            geometry_reports,
        } => {
            if !bridge.renderer.accepts_epoch(render_epoch) {
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
                    bridge.renderer.page_count = Some(second.page_count);
                    bridge.renderer.print_mode = print_mode;
                    bridge.renderer.last_geometry = Some(second);
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
    cancel_output_if_renderer_binding_changed(&mut bridge);
}

fn configured_webview_builder(
    prepared: &PreparedHtmlPreview,
    bridge: Arc<Mutex<LinuxHostBridge>>,
    document_identity: &RendererDocumentIdentity,
) -> wry::WebViewBuilder<'static> {
    let protocol_root = prepared.entry.parent().map(PathBuf::from);
    let layout_plan = prepared.layout_plan;
    let initialization_script = format!(
        "{}\n{}",
        renderer_document_identity_script(document_identity),
        prepared.initialization_script
    );
    wry::WebViewBuilder::new()
        .with_incognito(true)
        .with_custom_protocol("ebirforms".into(), move |_webview_id, request| {
            renderer_protocol_response(protocol_root.as_deref(), request)
        })
        .with_url(prepared.url.clone())
        .with_initialization_script(initialization_script)
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

fn harden_webkit(webview: &webkit2gtk::WebView) -> Result<(), String> {
    let settings = WebKitWebViewExt::settings(webview)
        .ok_or_else(|| "WebKitGTK did not expose renderer settings".to_string())?;
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
    if !settings.is_print_backgrounds() {
        return Err("WebKitGTK refused to enable printed CSS backgrounds".to_string());
    }
    Ok(())
}

fn evaluate_script(webview: &webkit2gtk::WebView, script: &str) {
    webview.run_javascript(script, gio::Cancellable::NONE, |_| {});
}

fn custom_paper(contract: LinuxNativePrintContract) -> gtk::PaperSize {
    gtk::PaperSize::new_custom(
        "ebirforms-folio",
        "eBIRForms 8.5 x 13 in",
        contract.width_points,
        contract.height_points,
        gtk::Unit::Points,
    )
}

fn point_value_matches(actual: f64, expected: f64) -> bool {
    actual.is_finite() && (actual - expected).abs() <= 0.001
}

fn page_setup(contract: LinuxNativePrintContract) -> Result<gtk::PageSetup, String> {
    let paper = custom_paper(contract);
    let setup = gtk::PageSetup::new();
    setup.set_orientation(gtk::PageOrientation::Portrait);
    setup.set_paper_size(&paper);
    setup.set_top_margin(contract.margin_top_points, gtk::Unit::Points);
    setup.set_right_margin(contract.margin_right_points, gtk::Unit::Points);
    setup.set_bottom_margin(contract.margin_bottom_points, gtk::Unit::Points);
    setup.set_left_margin(contract.margin_left_points, gtk::Unit::Points);
    if setup.orientation() != gtk::PageOrientation::Portrait
        || !point_value_matches(setup.paper_width(gtk::Unit::Points), contract.width_points)
        || !point_value_matches(
            setup.paper_height(gtk::Unit::Points),
            contract.height_points,
        )
        || !point_value_matches(
            setup.top_margin(gtk::Unit::Points),
            contract.margin_top_points,
        )
        || !point_value_matches(
            setup.right_margin(gtk::Unit::Points),
            contract.margin_right_points,
        )
        || !point_value_matches(
            setup.bottom_margin(gtk::Unit::Points),
            contract.margin_bottom_points,
        )
        || !point_value_matches(
            setup.left_margin(gtk::Unit::Points),
            contract.margin_left_points,
        )
    {
        return Err(
            "GTK did not retain the required 612 x 936 point paper and zero margins".to_string(),
        );
    }
    Ok(setup)
}

fn print_settings(contract: LinuxNativePrintContract) -> Result<gtk::PrintSettings, String> {
    let settings = gtk::PrintSettings::new();
    settings.set_orientation(gtk::PageOrientation::Portrait);
    settings.set_paper_size(&custom_paper(contract));
    settings.set_paper_width(contract.width_points, gtk::Unit::Points);
    settings.set_paper_height(contract.height_points, gtk::Unit::Points);
    settings.set_page_set(gtk::PageSet::All);
    settings.set_print_pages(gtk::PrintPages::All);
    settings.set_n_copies(1);
    settings.set_number_up(1);
    settings.set_scale(contract.scale_percent);
    settings.set_use_color(true);
    // CSS backgrounds are a WebKitSettings property and are verified by
    // `harden_webkit`; `GtkPrintSettings` has no standard background key.
    // WebKitGTK's print operation also exposes no browser header/footer API,
    // so there is no synthetic setting name to write here.
    if settings.orientation() != gtk::PageOrientation::Portrait
        || !point_value_matches(
            settings.paper_width(gtk::Unit::Points),
            contract.width_points,
        )
        || !point_value_matches(
            settings.paper_height(gtk::Unit::Points),
            contract.height_points,
        )
        || settings.page_set() != gtk::PageSet::All
        || settings.print_pages() != gtk::PrintPages::All
        || settings.n_copies() != 1
        || settings.number_up() != 1
        || !point_value_matches(settings.scale(), contract.scale_percent)
        || !settings.uses_color()
        || !contract.print_backgrounds
        || contract.webkitgtk_exposes_header_footer_control
    {
        return Err("GTK did not retain the required native print settings".to_string());
    }
    Ok(settings)
}

fn native_print_configuration(
    expectation: &PdfExpectation,
) -> Result<(gtk::PageSetup, gtk::PrintSettings), String> {
    let contract = linux_native_print_contract(expectation.width_points, expectation.height_points)
        .map_err(|error| error.to_string())?;
    Ok((page_setup(contract)?, print_settings(contract)?))
}

fn begin_system_print(
    webview: &webkit2gtk::WebView,
    parent: Option<&gtk::Window>,
    pending: PendingLinuxOutput,
    expectation: &PdfExpectation,
    bridge: Arc<Mutex<LinuxHostBridge>>,
) {
    let Some(render_epoch) = pending.binding.as_ref().map(|binding| binding.render_epoch) else {
        complete_output(
            &bridge,
            pending.nonce,
            0,
            Err("Linux system print started without a renderer binding".to_string()),
        );
        return;
    };
    let (page_setup, print_settings) = match native_print_configuration(expectation) {
        Ok(configuration) => configuration,
        Err(error) => {
            complete_output(&bridge, pending.nonce, render_epoch, Err(error));
            return;
        }
    };
    let operation = webkit2gtk::PrintOperation::new(webview);
    operation.set_page_setup(&page_setup);
    operation.set_print_settings(&print_settings);

    let failed_bridge = bridge.clone();
    let nonce = pending.nonce;
    operation.connect_failed(move |_, error| {
        complete_output(
            &failed_bridge,
            nonce,
            render_epoch,
            Err(format!("WebKitGTK system print failed: {error}")),
        );
    });
    let finished_bridge = bridge.clone();
    operation.connect_finished(move |_| {
        complete_output(
            &finished_bridge,
            nonce,
            render_epoch,
            Ok("System print job handed to GTK".to_string()),
        );
    });

    match operation.run_dialog(parent) {
        webkit2gtk::PrintOperationResponse::Print => {}
        webkit2gtk::PrintOperationResponse::Cancel => complete_output(
            &bridge,
            nonce,
            render_epoch,
            Err("System print was cancelled".to_string()),
        ),
        _ => complete_output(
            &bridge,
            nonce,
            render_epoch,
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
    let render_epoch = pending
        .binding
        .as_ref()
        .map(|binding| binding.render_epoch)
        .ok_or_else(|| "Linux PDF export started without a renderer binding".to_string())?;
    let destination = pending
        .destination
        .as_ref()
        .ok_or_else(|| "PDF export destination is missing".to_string())?;
    let temp_path = create_pdf_export_temp(destination).map_err(|error| error.to_string())?;
    register_linux_temp_path(&bridge, pending.nonce, temp_path.clone())?;
    let output_uri = url::Url::from_file_path(&temp_path)
        .map_err(|_| format!("cannot convert {} to a file URI", temp_path.display()))?;

    let (page_setup, settings) = native_print_configuration(&expectation)?;
    settings.set_printer("Print to File");
    settings.set(
        gtk::PRINT_SETTINGS_OUTPUT_URI.as_str(),
        Some(output_uri.as_str()),
    );
    settings.set(gtk::PRINT_SETTINGS_OUTPUT_FILE_FORMAT.as_str(), Some("pdf"));

    let operation = webkit2gtk::PrintOperation::new(webview);
    operation.set_page_setup(&page_setup);
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
            render_epoch,
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
            render_epoch,
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
    let Some(pending) = bridge.pending_output.as_ref() else {
        let _ = discard_pdf_export_temp(&temp_path);
        return Err("Linux PDF output was cancelled before its backend started".to_string());
    };
    if pending.nonce != nonce || !pending.backend_started {
        let _ = discard_pdf_export_temp(&temp_path);
        return Err("Linux PDF output no longer matches its validated nonce".to_string());
    }
    let binding_error = pending
        .binding
        .as_ref()
        .and_then(|binding| linux_renderer_binding_mismatch_reason(&bridge.renderer, binding));
    if let Some(reason) = binding_error {
        let _ = discard_pdf_export_temp(&temp_path);
        cancel_pending_output_locked(&mut bridge, reason.clone());
        return Err(reason);
    }
    if let Some(pending) = bridge.pending_output.as_mut() {
        pending.temp_path = Some(temp_path);
    }
    Ok(())
}

fn finish_linux_pdf_export(
    bridge: &Arc<Mutex<LinuxHostBridge>>,
    nonce: u64,
    render_epoch: u64,
    temp_path: &Path,
    destination: &Path,
    expectation: &PdfExpectation,
) {
    let Ok(mut bridge) = bridge.lock() else {
        let _ = discard_pdf_export_temp(temp_path);
        return;
    };
    if bridge.tombstoned_output_nonces.contains(&nonce) {
        let _ = discard_pdf_export_temp(temp_path);
        return;
    }
    let is_active = bridge
        .pending_output
        .as_ref()
        .is_some_and(|pending| pending.nonce == nonce && pending.backend_started);
    if !is_active {
        let _ = discard_pdf_export_temp(temp_path);
        return;
    }
    let binding_error = bridge
        .pending_output
        .as_ref()
        .and_then(|pending| pending_output_binding_error(pending, &bridge.renderer, render_epoch));
    if let Some(reason) = binding_error {
        let _ = discard_pdf_export_temp(temp_path);
        cancel_pending_output_locked(&mut bridge, reason);
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
    render_epoch: u64,
    result: Result<String, String>,
) {
    if let Ok(mut bridge) = bridge.lock() {
        if bridge.tombstoned_output_nonces.contains(&nonce) {
            return;
        }
        let is_active = bridge
            .pending_output
            .as_ref()
            .is_some_and(|pending| pending.nonce == nonce);
        if !is_active {
            return;
        }
        let binding_error = bridge.pending_output.as_ref().and_then(|pending| {
            pending_output_binding_error(pending, &bridge.renderer, render_epoch)
        });
        if let Some(reason) = binding_error {
            cancel_pending_output_locked(&mut bridge, reason);
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
        bridge.completion = Some(LinuxOutputCompletion { nonce, result });
    }
}

fn abandon_pending_output(bridge: &Arc<Mutex<LinuxHostBridge>>) {
    let Ok(mut bridge) = bridge.lock() else {
        return;
    };
    if let Some(pending) = bridge.pending_output.take() {
        tombstone_output_nonce(&mut bridge, pending.nonce);
        if let Some(temp_path) = pending.temp_path {
            let _ = discard_pdf_export_temp(&temp_path);
        }
    }
    bridge.renderer.print_ready_nonce = None;
    bridge.completion = None;
}

fn renderer_retry_action(
    bridge: &Arc<Mutex<LinuxHostBridge>>,
    webview_available: bool,
) -> LinuxRendererRetryAction {
    bridge
        .lock()
        .map(|bridge| {
            linux_renderer_retry_action(
                matches!(
                    bridge.renderer.decision(),
                    RendererReadinessDecision::Fallback(_)
                ),
                webview_available,
            )
        })
        .unwrap_or(LinuxRendererRetryAction::Disabled)
}

fn reset_linux_renderer_for_retry(bridge: &Arc<Mutex<LinuxHostBridge>>) -> Result<(), String> {
    let mut bridge = bridge
        .lock()
        .map_err(|_| "Linux renderer state is unavailable".to_string())?;
    if let Some(temp_path) = bridge
        .pending_output
        .as_ref()
        .and_then(|pending| pending.temp_path.as_deref())
        && let Err(error) = discard_pdf_export_temp(temp_path)
    {
        let message = format!("Failed to clean the pending Linux PDF export: {error}");
        bridge.renderer.error = Some(message.clone());
        return Err(message);
    }

    if let Some(pending) = bridge.pending_output.take() {
        tombstone_output_nonce(&mut bridge, pending.nonce);
    }
    bridge.completion = None;
    if bridge.renderer.document_boot_accepted || bridge.renderer.document_identity_rejected {
        return Err(
            "Secure Linux renderer retry requires closing and reopening the preview so the host can mint a new document run ID"
                .to_string(),
        );
    }
    let document_identity = bridge
        .renderer
        .document_identity
        .clone()
        .ok_or_else(|| "Linux renderer retry has no immutable document identity".to_string())?;
    bridge.renderer = LinuxRendererState::for_document(document_identity);
    bridge.readiness_started_at = Instant::now();
    Ok(())
}

fn retry_linux_renderer(
    webview: &webkit2gtk::WebView,
    bridge: &Arc<Mutex<LinuxHostBridge>>,
) -> Result<(), String> {
    reset_linux_renderer_for_retry(bridge)?;
    evaluate_script(webview, native_output_cleanup_script());
    WebKitWebViewExt::stop_loading(webview);
    // Reloading retains the original initialization script, immutable envelope,
    // navigation allowlist, and `ebirforms://` custom protocol. No external URL
    // or mutable draft state participates in retry.
    WebKitWebViewExt::reload(webview);
    Ok(())
}

fn expire_renderer_readiness(bridge: &Arc<Mutex<LinuxHostBridge>>) {
    if let Ok(mut bridge) = bridge.lock()
        && bridge.renderer.last_geometry.is_none()
        && bridge.renderer.error.is_none()
        && bridge.readiness_started_at.elapsed() >= READINESS_TIMEOUT
    {
        bridge.renderer.error = Some("Linux HTML renderer readiness timed out".to_string());
    }
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
    if let Some(pending) = bridge.pending_output.take() {
        tombstone_output_nonce(&mut bridge, pending.nonce);
        if let Some(temp_path) = pending.temp_path {
            let _ = discard_pdf_export_temp(&temp_path);
        }
    }
    bridge.renderer.print_ready_nonce = None;
    bridge.completion = Some(LinuxOutputCompletion {
        nonce,
        result: Err(reason.to_string()),
    });
}

fn schedule_output_deadlines(
    bridge: &Arc<Mutex<LinuxHostBridge>>,
    nonce: u64,
    kind: HtmlOutputKind,
) {
    let preflight_bridge = bridge.clone();
    glib::timeout_add_once(READINESS_TIMEOUT, move || {
        expire_pending_output(
            &preflight_bridge,
            nonce,
            true,
            "Linux native output preflight timed out",
        );
    });
    if kind != HtmlOutputKind::PdfExport {
        return;
    }
    let backend_bridge = bridge.clone();
    glib::timeout_add_once(PDF_EXPORT_TIMEOUT, move || {
        expire_pending_output(
            &backend_bridge,
            nonce,
            false,
            "Linux native PDF export backend did not complete before its deadline",
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
            binding: None,
        });
        nonce
    };
    schedule_output_deadlines(bridge, nonce, kind);
    evaluate_script(webview, &native_print_preflight_script(nonce));
    Ok(nonce)
}

fn take_ready_output(bridge: &Arc<Mutex<LinuxHostBridge>>) -> Option<PendingLinuxOutput> {
    let mut bridge = bridge.lock().ok()?;
    let pending = bridge.pending_output.clone()?;
    if let Some(reason) = linux_output_timeout_reason(&pending) {
        cancel_pending_output_locked(&mut bridge, reason);
        return None;
    }
    if pending.backend_started || bridge.renderer.print_ready_nonce != Some(pending.nonce) {
        return None;
    }

    let binding = match bind_renderer_for_linux_output(&bridge.renderer) {
        Ok(binding) => binding,
        Err(reason) => {
            cancel_pending_output_locked(&mut bridge, reason);
            return None;
        }
    };
    bridge.renderer.print_ready_nonce = None;
    if let Some(stored) = bridge.pending_output.as_mut() {
        stored.backend_started = true;
        stored.binding = Some(binding.clone());
    }
    let mut started = pending;
    started.backend_started = true;
    started.binding = Some(binding);
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
    let startup_cancelled = Arc::new(AtomicBool::new(false));
    let thread_cancelled = startup_cancelled.clone();
    std::thread::Builder::new()
        .name("ebirforms-html-preview-gtk".to_string())
        .spawn(move || {
            let result =
                run_gtk_top_level(prepared, lifecycle, &startup_tx, thread_cancelled.as_ref());
            if let Err(error) = result {
                let _ = startup_tx.send(Err(error));
            }
        })
        .map_err(LinuxHtmlPreviewError::ThreadStart)?;

    match startup_rx.recv_timeout(READINESS_TIMEOUT) {
        Ok(Ok(())) => Ok(()),
        Ok(Err(error)) => Err(LinuxHtmlPreviewError::Startup(error)),
        Err(mpsc::RecvTimeoutError::Timeout) => {
            startup_cancelled.store(true, Ordering::Release);
            Err(LinuxHtmlPreviewError::Startup(
                "GTK/WebKit host initialization timed out".to_string(),
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => {
            startup_cancelled.store(true, Ordering::Release);
            Err(LinuxHtmlPreviewError::StartupDisconnected)
        }
    }
}

fn run_gtk_top_level(
    prepared: PreparedHtmlPreview,
    lifecycle: LinuxHostLifecycle,
    startup_tx: &mpsc::SyncSender<Result<(), String>>,
    startup_cancelled: &AtomicBool,
) -> Result<(), String> {
    gtk::init().map_err(|error| format!("GTK initialization failed: {error}"))?;
    if startup_cancelled.load(Ordering::Acquire) {
        return Err("GTK/WebKit host startup was cancelled".to_string());
    }
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
    let retry_button = gtk::Button::with_label("Retry");
    let close_button = gtk::Button::with_label("Close");
    export_button.set_sensitive(false);
    print_button.set_sensitive(false);
    retry_button.set_no_show_all(true);
    retry_button.set_visible(false);
    toolbar.pack_start(&status, true, true, 0);
    toolbar.pack_end(&close_button, false, false, 0);
    toolbar.pack_end(&retry_button, false, false, 0);
    toolbar.pack_end(&print_button, false, false, 0);
    toolbar.pack_end(&export_button, false, false, 0);
    root.pack_start(&toolbar, false, false, 0);

    let webview_container = gtk::Box::new(gtk::Orientation::Vertical, 0);
    root.pack_start(&webview_container, true, true, 0);
    window.add(&root);

    let document_identity =
        RendererDocumentIdentity::host_generated(&prepared.pdf_expectation.envelope_hash)
            .expect("prepared HTML previews always contain a canonical envelope hash");
    let bridge = Arc::new(Mutex::new(LinuxHostBridge {
        renderer: LinuxRendererState::for_document(document_identity.clone()),
        ..LinuxHostBridge::default()
    }));
    let webview = configured_webview_builder(&prepared, bridge.clone(), &document_identity)
        .build_gtk(&webview_container)
        .map_err(|error| format!("WebKitGTK host construction failed: {error}"))?;
    let webview = Rc::new(webview);
    let webkit = webview.webview();
    harden_webkit(&webkit)
        .map_err(|error| format!("WebKitGTK renderer hardening failed: {error}"))?;

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

    let retry_webkit = webkit.clone();
    let retry_bridge = bridge.clone();
    let retry_status = status.clone();
    retry_button.connect_clicked(move |_| {
        match retry_linux_renderer(&retry_webkit, &retry_bridge) {
            Ok(()) => retry_status.set_text("HTML renderer: retrying preview..."),
            Err(error) => retry_status.set_text(&error),
        }
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

    let startup_shutdown_bridge = bridge.clone();
    let poll_bridge = bridge;
    let poll_webkit = webkit;
    let poll_window = window.clone();
    let poll_status = status;
    let poll_export_button = export_button;
    let poll_print_button = print_button;
    let poll_retry_button = retry_button;
    let expectation = prepared.pdf_expectation;
    glib::timeout_add_local(Duration::from_millis(100), move || {
        // Retain the Wry owner for as long as the GTK window is alive.
        let _webview_owner = &webview;
        if !poll_window.is_visible() {
            return glib::ControlFlow::Break;
        }

        expire_renderer_readiness(&poll_bridge);
        let (decision, output_pending) = poll_bridge
            .lock()
            .map(|bridge| (bridge.renderer.decision(), bridge.pending_output.is_some()))
            .unwrap_or_else(|_| {
                (
                    RendererReadinessDecision::Fallback(
                        "Linux renderer state is unavailable".to_string(),
                    ),
                    true,
                )
            });

        let can_output =
            matches!(decision, RendererReadinessDecision::Ready { .. }) && !output_pending;
        poll_export_button.set_sensitive(can_output);
        poll_print_button.set_sensitive(can_output);
        match renderer_retry_action(&poll_bridge, true) {
            LinuxRendererRetryAction::Hidden => poll_retry_button.set_visible(false),
            LinuxRendererRetryAction::Disabled => {
                poll_retry_button.set_visible(true);
                poll_retry_button.set_sensitive(false);
            }
            LinuxRendererRetryAction::Enabled => {
                poll_retry_button.set_visible(true);
                poll_retry_button.set_sensitive(true);
            }
        }
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
                        let render_epoch = pending
                            .binding
                            .as_ref()
                            .map_or(0, |binding| binding.render_epoch);
                        complete_output(&poll_bridge, pending.nonce, render_epoch, Err(error));
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

    if startup_cancelled.load(Ordering::Acquire) {
        abandon_pending_output(&startup_shutdown_bridge);
        return Err("GTK/WebKit host startup was cancelled".to_string());
    }
    // The launcher waits only for construction, not for renderer readiness.
    // If its receiver has already timed out, do not leave a late ghost window
    // running in an otherwise failed launch.
    startup_tx
        .send(Ok(()))
        .map_err(|_| "GTK/WebKit host launcher disconnected".to_string())?;
    window.show_all();
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

        let document_identity =
            RendererDocumentIdentity::host_generated(&prepared.pdf_expectation.envelope_hash)
                .expect("prepared HTML previews always contain a canonical envelope hash");
        let bridge = Arc::new(Mutex::new(LinuxHostBridge {
            renderer: LinuxRendererState::for_document(document_identity.clone()),
            ..LinuxHostBridge::default()
        }));
        let result = window
            .window_handle()
            .map_err(|error| error.to_string())
            .and_then(|window_handle| {
                configured_webview_builder(&prepared, bridge.clone(), &document_identity)
                    .build_as_child(&window_handle)
                    .map_err(|error| error.to_string())
            });

        let (webview, status) = match result {
            Ok(webview) => match harden_webkit(&webview.webview()) {
                Ok(()) => (
                    Some(cx.new(|cx| GpuiWebView::new(webview, window, cx))),
                    "HTML renderer: preparing X11 child preview...".to_string(),
                ),
                Err(error) => {
                    if let Ok(mut bridge) = bridge.lock() {
                        bridge.renderer.error =
                            Some(format!("X11 WebKit renderer hardening failed: {error}"));
                    }
                    (None, format!("Linux HTML renderer failed: {error}"))
                }
            },
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
        let poll_task = cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(100))
                    .await;

                expire_renderer_readiness(&poll_bridge);

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
        let render_epoch = pending
            .binding
            .as_ref()
            .map_or(0, |binding| binding.render_epoch);
        let Some(webview) = self.webview.clone() else {
            complete_output(
                &self.bridge,
                pending.nonce,
                render_epoch,
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
                render_epoch,
                Err(error.to_string()),
            );
        }
    }

    fn request_retry(&mut self, cx: &mut Context<Self>) {
        let Some(webview) = self.webview.clone() else {
            self.status = "Linux renderer WebView is unavailable".to_string();
            cx.notify();
            return;
        };
        let bridge = self.bridge.clone();
        match webview.update(cx, |webview, _| {
            retry_linux_renderer(&webview.raw().webview(), &bridge)
        }) {
            Ok(()) => self.status = "HTML renderer: retrying X11 child preview...".to_string(),
            Err(error) => self.status = error,
        }
        cx.notify();
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
        let retry_action = renderer_retry_action(&self.bridge, self.webview.is_some());
        // Spin the refresh control while the renderer is still working, but
        // never when it failed — the user must be able to click to retry.
        let refreshing = self
            .bridge
            .lock()
            .is_ok_and(|bridge| !bridge.renderer.ready && bridge.renderer.error.is_none());

        div()
            .size_full()
            .flex()
            .flex_col()
            // Without an explicit background the preview window falls through
            // to black in both themes; follow the app theme instead.
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(
                div()
                    .h(px(48.))
                    .px_3()
                    .flex()
                    .items_center()
                    .justify_between()
                    .bg(cx.theme().secondary)
                    .text_sm()
                    .text_color(cx.theme().foreground)
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
                                    .primary()
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
                                    .primary()
                                    .disabled(!output_enabled)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.request_output(HtmlOutputKind::SystemPrint, None, cx);
                                    })),
                            )
                            .when(
                                retry_action != LinuxRendererRetryAction::Hidden,
                                |toolbar| {
                                    toolbar.child(
                                        Button::new("linux-html-preview-retry")
                                            .icon(
                                                gpui_component::Icon::empty()
                                                    .path("svg/refresh.svg"),
                                            )
                                            .tooltip("Refresh preview")
                                            .outline()
                                            .loading(refreshing)
                                            .disabled(
                                                retry_action != LinuxRendererRetryAction::Enabled,
                                            )
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.request_retry(cx);
                                            })),
                                    )
                                },
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

    fn test_document_identity() -> RendererDocumentIdentity {
        RendererDocumentIdentity::test_identity()
    }

    fn test_bridge() -> Arc<Mutex<LinuxHostBridge>> {
        Arc::new(Mutex::new(LinuxHostBridge {
            renderer: LinuxRendererState::for_document(test_document_identity()),
            ..LinuxHostBridge::default()
        }))
    }

    fn identified_message(identity: &RendererDocumentIdentity, body: &str) -> serde_json::Value {
        let mut message =
            serde_json::from_str::<serde_json::Value>(body).expect("renderer test message JSON");
        let object = message
            .as_object_mut()
            .expect("renderer test messages are JSON objects");
        object.insert(
            "document_run_id".to_string(),
            serde_json::json!(identity.document_run_id),
        );
        object.insert(
            "envelope_hash".to_string(),
            serde_json::json!(identity.envelope_hash),
        );
        message
    }

    // Existing protocol tests use this helper so every message crosses the
    // same immutable identity gate as production. Security-negative tests call
    // `super::accept_renderer_message` directly.
    fn accept_renderer_message(
        bridge: &Arc<Mutex<LinuxHostBridge>>,
        layout_plan: &RenderLayoutPlan,
        body: &str,
    ) {
        let identity = {
            let mut bridge = bridge.lock().expect("bridge");
            let identity = bridge
                .renderer
                .document_identity
                .clone()
                .unwrap_or_else(test_document_identity);
            if bridge.renderer.document_identity.is_none() {
                bridge.renderer = LinuxRendererState::for_document(identity.clone());
            }
            identity
        };
        let needs_boot = !bridge
            .lock()
            .expect("bridge")
            .renderer
            .document_boot_accepted;
        if needs_boot {
            super::accept_renderer_message(
                bridge,
                layout_plan,
                &identified_message(&identity, r#"{"type":"renderer_boot"}"#).to_string(),
            );
        }
        super::accept_renderer_message(
            bridge,
            layout_plan,
            &identified_message(&identity, body).to_string(),
        );
    }

    fn geometry_message(render_epoch: u64, print_mode: bool, overflow_x: usize) -> String {
        geometry_message_with_x(render_epoch, print_mode, overflow_x, 0.0)
    }

    fn geometry_message_with_x(
        render_epoch: u64,
        print_mode: bool,
        overflow_x: usize,
        x: f64,
    ) -> String {
        let measurement = serde_json::json!({
            "page_count": 2,
            "page_width_pt": 612.0,
            "page_height_pt": 936.0,
            "pages": [
                {
                    "x": x,
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
                    "x": x,
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
        });
        serde_json::json!({
            "type": "page_count",
            "render_epoch": render_epoch,
            "print_mode": print_mode,
            "geometry_reports": [measurement.clone(), measurement]
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
            binding: None,
        }
    }

    fn test_pdf_expectation(width_points: f64, height_points: f64) -> PdfExpectation {
        PdfExpectation {
            form_code: "2551Q".to_string(),
            revision: "2018".to_string(),
            envelope_hash: "a".repeat(64),
            expected_page_count: 2,
            width_points,
            height_points,
        }
    }

    #[test]
    #[ignore = "requires a GTK display; run under Xvfb in headless Linux CI"]
    fn gtk_native_print_configuration_retains_exact_folio_contract() {
        gtk::init().expect("GTK display initializes");
        let (setup, settings) = native_print_configuration(&test_pdf_expectation(612.0, 936.0))
            .expect("GTK retains the native BIR print contract");

        assert_eq!(setup.orientation(), gtk::PageOrientation::Portrait);
        assert!(point_value_matches(
            setup.paper_width(gtk::Unit::Points),
            612.0
        ));
        assert!(point_value_matches(
            setup.paper_height(gtk::Unit::Points),
            936.0
        ));
        assert!(point_value_matches(
            setup.top_margin(gtk::Unit::Points),
            0.0
        ));
        assert!(point_value_matches(
            setup.right_margin(gtk::Unit::Points),
            0.0
        ));
        assert!(point_value_matches(
            setup.bottom_margin(gtk::Unit::Points),
            0.0
        ));
        assert!(point_value_matches(
            setup.left_margin(gtk::Unit::Points),
            0.0
        ));
        assert_eq!(settings.orientation(), gtk::PageOrientation::Portrait);
        assert!(point_value_matches(
            settings.paper_width(gtk::Unit::Points),
            612.0
        ));
        assert!(point_value_matches(
            settings.paper_height(gtk::Unit::Points),
            936.0
        ));
        assert_eq!(settings.page_set(), gtk::PageSet::All);
        assert_eq!(settings.print_pages(), gtk::PrintPages::All);
        assert_eq!(settings.n_copies(), 1);
        assert_eq!(settings.number_up(), 1);
        assert!(point_value_matches(settings.scale(), 100.0));
        assert!(settings.uses_color());
    }

    #[test]
    fn gtk_native_print_configuration_rejects_non_folio_geometry() {
        let error = native_print_configuration(&test_pdf_expectation(612.0, 792.0))
            .expect_err("letter paper must fail closed");
        assert!(error.contains("exactly 612 x 936 point"));
    }

    fn ready_renderer(bridge: &Arc<Mutex<LinuxHostBridge>>, plan: &RenderLayoutPlan, epoch: u64) {
        accept_renderer_message(
            bridge,
            plan,
            &format!(r#"{{"type":"renderer_invalidated","render_epoch":{epoch}}}"#),
        );
        accept_renderer_message(bridge, plan, &geometry_message(epoch, true, 0));
        accept_renderer_message(
            bridge,
            plan,
            &format!(r#"{{"type":"renderer_ready","render_epoch":{epoch}}}"#),
        );
    }

    #[test]
    fn retry_reset_cleans_in_flight_output_and_restarts_renderer_readiness() {
        let bridge = test_bridge();
        let temp_path = std::env::temp_dir().join(format!(
            "ebirforms-linux-retry-output-{}.pdf",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&temp_path, b"partial").expect("write retry temp");
        let mut pending = pending_output(17);
        pending.kind = HtmlOutputKind::PdfExport;
        pending.backend_started = true;
        pending.temp_path = Some(temp_path.clone());
        {
            let mut state = bridge.lock().expect("bridge");
            state.next_output_nonce = 17;
            state.renderer.ready = true;
            state.renderer.page_count = Some(2);
            state.renderer.print_mode = true;
            state.renderer.print_ready_nonce = Some(17);
            state.renderer.error = Some("renderer failed during output".to_string());
            state.pending_output = Some(pending);
            state.completion = Some(LinuxOutputCompletion {
                nonce: 17,
                result: Err("stale completion".to_string()),
            });
            state.readiness_started_at = Instant::now() - READINESS_TIMEOUT;
        }

        assert_eq!(
            renderer_retry_action(&bridge, true),
            LinuxRendererRetryAction::Enabled
        );
        let reset_started_at = Instant::now();
        reset_linux_renderer_for_retry(&bridge).expect("retry state resets");

        assert!(!temp_path.exists());
        let state = bridge.lock().expect("bridge");
        assert_eq!(state.next_output_nonce, 17);
        assert!(state.pending_output.is_none());
        assert!(state.completion.is_none());
        assert_eq!(
            state.renderer,
            LinuxRendererState::for_document(test_document_identity())
        );
        assert!(state.readiness_started_at >= reset_started_at);
        assert!(matches!(
            state.renderer.decision(),
            RendererReadinessDecision::Pending
        ));
        drop(state);

        expire_renderer_readiness(&bridge);
        assert!(bridge.lock().expect("bridge").renderer.error.is_none());
        complete_output(&bridge, 17, 7, Ok("late output".to_string()));
        assert!(bridge.lock().expect("bridge").completion.is_none());
    }

    #[test]
    fn renderer_ipc_requires_the_host_identity_boot_handshake() {
        let plan = test_layout_plan();
        let identity = test_document_identity();
        let bridge = test_bridge();

        super::accept_renderer_message(
            &bridge,
            &plan,
            &identified_message(
                &identity,
                r#"{"type":"renderer_invalidated","render_epoch":1}"#,
            )
            .to_string(),
        );

        let state = bridge.lock().expect("bridge");
        assert!(state.renderer.document_identity_rejected);
        assert!(!state.renderer.ready);
        assert_eq!(state.renderer.render_epoch, 0);
        assert!(
            state
                .renderer
                .error
                .as_deref()
                .is_some_and(|error| error.contains("before"))
        );
    }

    #[test]
    fn renderer_ipc_rejects_mismatch_reload_replay_and_missing_identity() {
        let plan = test_layout_plan();
        let identity = test_document_identity();

        let replay_bridge = test_bridge();
        let boot = identified_message(&identity, r#"{"type":"renderer_boot"}"#).to_string();
        super::accept_renderer_message(&replay_bridge, &plan, &boot);
        super::accept_renderer_message(&replay_bridge, &plan, &boot);
        {
            let state = replay_bridge.lock().expect("bridge");
            assert!(state.renderer.document_identity_rejected);
            assert!(
                state
                    .renderer
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("replayed"))
            );
        }

        let mismatch_bridge = test_bridge();
        let mut wrong_run = identity.clone();
        wrong_run.document_run_id = "00000000-0000-4000-8000-000000000002".to_string();
        super::accept_renderer_message(
            &mismatch_bridge,
            &plan,
            &identified_message(&wrong_run, r#"{"type":"renderer_boot"}"#).to_string(),
        );
        {
            let state = mismatch_bridge.lock().expect("bridge");
            assert!(state.renderer.document_identity_rejected);
            assert!(
                state
                    .renderer
                    .error
                    .as_deref()
                    .is_some_and(|error| error.contains("did not match"))
            );
        }

        let missing_bridge = test_bridge();
        super::accept_renderer_message(&missing_bridge, &plan, r#"{"type":"renderer_boot"}"#);
        let state = missing_bridge.lock().expect("bridge");
        assert!(state.renderer.document_identity_rejected);
        assert!(
            state
                .renderer
                .error
                .as_deref()
                .is_some_and(|error| error.contains("omitted or malformed"))
        );
    }

    #[test]
    fn output_binding_is_cancelled_when_document_identity_changes() {
        let bridge = test_bridge();
        let plan = test_layout_plan();
        ready_renderer(&bridge, &plan, 7);
        bridge.lock().expect("bridge").pending_output = Some(pending_output(42));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":42,"render_epoch":7,"print_mode":true}"#,
        );

        let started = take_ready_output(&bridge).expect("identity-bound backend starts");
        assert_eq!(
            started
                .binding
                .as_ref()
                .expect("renderer binding")
                .document_identity,
            test_document_identity()
        );

        let mut replacement = test_document_identity();
        replacement.document_run_id = "00000000-0000-4000-8000-000000000002".to_string();
        super::accept_renderer_message(
            &bridge,
            &plan,
            &identified_message(
                &replacement,
                r#"{"type":"renderer_ready","render_epoch":7}"#,
            )
            .to_string(),
        );

        let completion = take_completion(&bridge).expect("identity change cancels output");
        assert!(
            completion
                .result
                .is_err_and(|error| error.contains("document identity changed"))
        );
        assert!(
            bridge
                .lock()
                .expect("bridge")
                .tombstoned_output_nonces
                .contains(&42)
        );
    }

    #[test]
    fn booted_linux_document_cannot_reuse_its_run_id_for_in_place_retry() {
        let bridge = test_bridge();
        let plan = test_layout_plan();
        ready_renderer(&bridge, &plan, 7);
        bridge.lock().expect("bridge").renderer.error = Some("failed".to_string());

        let error = reset_linux_renderer_for_retry(&bridge)
            .expect_err("booted documents need a newly constructed WebView and run ID");
        assert!(error.contains("closing and reopening"));
        let state = bridge.lock().expect("bridge");
        assert!(state.renderer.document_boot_accepted);
        assert_eq!(
            state.renderer.document_identity,
            Some(test_document_identity())
        );
    }

    #[test]
    fn print_preflight_consumes_a_matching_nonce_only_once() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        ready_renderer(&bridge, &plan, 7);
        bridge.lock().expect("bridge").pending_output = Some(pending_output(42));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":42,"render_epoch":7,"print_mode":true}"#,
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
        let binding = started.binding.expect("native output renderer binding");
        assert_eq!(binding.render_epoch, 7);
        assert_eq!(binding.readiness_revision, 1);
        assert_eq!(binding.geometry.page_count, 2);
        assert_eq!(binding.geometry.pages.len(), 2);
    }

    #[test]
    fn successful_output_does_not_retain_a_cancellation_tombstone() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        ready_renderer(&bridge, &plan, 7);
        bridge.lock().expect("bridge").pending_output = Some(pending_output(42));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":42,"render_epoch":7,"print_mode":true}"#,
        );
        take_ready_output(&bridge).expect("matching nonce starts output");

        complete_output(&bridge, 42, 7, Ok("print complete".to_string()));

        let completion = take_completion(&bridge).expect("successful completion");
        assert!(completion.result.is_ok());
        let state = bridge.lock().expect("bridge");
        assert!(state.pending_output.is_none());
        assert!(state.tombstoned_output_nonces.is_empty());
        drop(state);

        complete_output(&bridge, 42, 7, Ok("duplicate callback".to_string()));
        assert!(take_completion(&bridge).is_none());
    }

    #[test]
    fn cancellation_tombstones_are_bounded() {
        let mut bridge = LinuxHostBridge::default();
        for nonce in 1..=(MAX_TOMBSTONED_OUTPUT_NONCES as u64 + 1) {
            tombstone_output_nonce(&mut bridge, nonce);
        }

        assert_eq!(
            bridge.tombstoned_output_nonces.len(),
            MAX_TOMBSTONED_OUTPUT_NONCES
        );
        assert!(!bridge.tombstoned_output_nonces.contains(&1));
        assert!(
            bridge
                .tombstoned_output_nonces
                .contains(&(MAX_TOMBSTONED_OUTPUT_NONCES as u64 + 1))
        );
    }

    #[test]
    fn renderer_ipc_ignores_stale_and_out_of_order_epochs() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"renderer_invalidated","render_epoch":5}"#,
        );
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"renderer_ready","render_epoch":4}"#,
        );
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"renderer_error","render_epoch":4,"message":"stale failure"}"#,
        );
        accept_renderer_message(&bridge, &plan, &geometry_message(6, true, 0));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"renderer_invalidated","render_epoch":4}"#,
        );

        {
            let state = bridge.lock().expect("bridge");
            assert_eq!(state.renderer.render_epoch, 5);
            assert_eq!(state.renderer.readiness_revision, 1);
            assert!(!state.renderer.ready);
            assert_eq!(state.renderer.page_count, None);
        }

        accept_renderer_message(&bridge, &plan, &geometry_message(5, true, 0));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"renderer_ready","render_epoch":5}"#,
        );
        let state = bridge.lock().expect("bridge");
        assert!(state.renderer.ready);
        assert_eq!(state.renderer.page_count, Some(2));
        assert_eq!(state.renderer.render_epoch, 5);
        assert_eq!(state.renderer.readiness_revision, 1);
        drop(state);

        bridge.lock().expect("bridge").pending_output = Some(pending_output(88));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":88,"render_epoch":6,"print_mode":true}"#,
        );
        {
            let state = bridge.lock().expect("bridge");
            assert_eq!(state.renderer.print_ready_nonce, None);
            assert_eq!(state.renderer.error, None);
        }
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":88,"render_epoch":5,"print_mode":true}"#,
        );
        assert_eq!(
            bridge.lock().expect("bridge").renderer.print_ready_nonce,
            Some(88)
        );
    }

    #[test]
    fn invalidation_after_backend_start_tombstones_late_completion() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        ready_renderer(&bridge, &plan, 7);
        bridge.lock().expect("bridge").pending_output = Some(pending_output(42));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":42,"render_epoch":7,"print_mode":true}"#,
        );
        let started = take_ready_output(&bridge).expect("backend starts from epoch 7");
        assert!(started.backend_started);

        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"renderer_invalidated","render_epoch":8}"#,
        );

        let completion = take_completion(&bridge).expect("stale output is rejected");
        assert!(
            completion
                .result
                .is_err_and(|error| error.contains("epoch changed"))
        );
        assert!(bridge.lock().expect("bridge").pending_output.is_none());
        assert!(
            bridge
                .lock()
                .expect("bridge")
                .tombstoned_output_nonces
                .contains(&42)
        );

        complete_output(&bridge, 42, 7, Ok("late print completion".to_string()));
        assert!(take_completion(&bridge).is_none());
    }

    #[test]
    fn geometry_change_after_backend_start_tombstones_output() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        ready_renderer(&bridge, &plan, 7);
        bridge.lock().expect("bridge").pending_output = Some(pending_output(43));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":43,"render_epoch":7,"print_mode":true}"#,
        );
        take_ready_output(&bridge).expect("backend starts from validated geometry");

        accept_renderer_message(&bridge, &plan, &geometry_message_with_x(7, true, 0, 1.0));

        let completion = take_completion(&bridge).expect("changed geometry rejects output");
        assert!(
            completion
                .result
                .is_err_and(|error| error.contains("page geometry changed"))
        );
        assert!(
            bridge
                .lock()
                .expect("bridge")
                .tombstoned_output_nonces
                .contains(&43)
        );
    }

    #[test]
    fn renderer_error_after_backend_start_tombstones_output() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        ready_renderer(&bridge, &plan, 7);
        bridge.lock().expect("bridge").pending_output = Some(pending_output(44));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":44,"render_epoch":7,"print_mode":true}"#,
        );
        take_ready_output(&bridge).expect("backend starts from a ready renderer");

        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"renderer_error","render_epoch":7,"message":"font readiness changed"}"#,
        );

        let completion = take_completion(&bridge).expect("readiness failure rejects output");
        assert!(
            completion
                .result
                .is_err_and(|error| error.contains("readiness was invalidated"))
        );
        assert!(
            bridge
                .lock()
                .expect("bridge")
                .tombstoned_output_nonces
                .contains(&44)
        );
    }

    #[test]
    fn print_preflight_ignores_stale_nonce_and_rejects_future_nonce() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        ready_renderer(&bridge, &plan, 7);
        bridge.lock().expect("bridge").pending_output = Some(pending_output(42));
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":41,"render_epoch":7,"print_mode":true}"#,
        );

        {
            let state = bridge.lock().expect("bridge");
            assert_eq!(state.renderer.print_ready_nonce, None);
            assert_eq!(state.renderer.error, None);
        }
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":43,"render_epoch":7,"print_mode":true}"#,
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
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"renderer_invalidated","render_epoch":7}"#,
        );
        accept_renderer_message(&bridge, &plan, &geometry_message(7, false, 1));

        let bridge = bridge.lock().expect("bridge");
        assert_eq!(bridge.renderer.page_count, None);
        assert!(matches!(
            bridge.renderer.decision(),
            RendererReadinessDecision::Fallback(_)
        ));
    }

    #[test]
    fn non_identical_stable_geometry_reports_fail_closed() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"renderer_invalidated","render_epoch":7}"#,
        );
        let mut message = serde_json::from_str::<serde_json::Value>(&geometry_message(7, true, 0))
            .expect("geometry message JSON");
        message["geometry_reports"][1]["pages"][0]["x"] = serde_json::json!(1.0);
        accept_renderer_message(&bridge, &plan, &message.to_string());

        let bridge = bridge.lock().expect("bridge");
        assert_eq!(bridge.renderer.page_count, None);
        assert!(
            bridge
                .renderer
                .error
                .as_deref()
                .is_some_and(|error| error.contains("were not identical"))
        );
        assert!(matches!(
            bridge.renderer.decision(),
            RendererReadinessDecision::Fallback(_)
        ));
    }

    #[test]
    fn running_pdf_export_expires_at_the_overall_output_deadline() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let mut pending = pending_output(9);
        pending.kind = HtmlOutputKind::PdfExport;
        pending.requested_at = Instant::now() - PDF_EXPORT_TIMEOUT;
        pending.backend_started = true;
        bridge.lock().expect("bridge").pending_output = Some(pending);

        assert!(take_ready_output(&bridge).is_none());
        let completion = take_completion(&bridge).expect("timeout completion");
        assert!(
            completion
                .result
                .is_err_and(|error| error.contains("PDF export backend did not complete"))
        );
    }

    #[test]
    fn running_system_print_does_not_expire_at_the_pdf_export_deadline() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let mut pending = pending_output(9);
        pending.requested_at = Instant::now() - PDF_EXPORT_TIMEOUT;
        pending.backend_started = true;
        bridge.lock().expect("bridge").pending_output = Some(pending);

        assert!(take_ready_output(&bridge).is_none());
        assert!(take_completion(&bridge).is_none());
        assert!(bridge.lock().expect("bridge").pending_output.is_some());
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

        finish_linux_pdf_export(&bridge, 12, 7, &temp_path, &destination, &expectation);

        assert_eq!(
            std::fs::read(&destination).expect("read existing destination"),
            b"existing"
        );
        assert!(!temp_path.exists());
        std::fs::remove_file(destination).expect("remove test destination");
    }

    #[test]
    fn invalidated_pdf_backend_cannot_replace_existing_destination() {
        let bridge = Arc::new(Mutex::new(LinuxHostBridge::default()));
        let plan = test_layout_plan();
        ready_renderer(&bridge, &plan, 7);
        let suffix = uuid::Uuid::new_v4();
        let temp_path =
            std::env::temp_dir().join(format!("ebirforms-linux-invalidated-output-{suffix}.pdf"));
        let destination = std::env::temp_dir().join(format!(
            "ebirforms-linux-preserved-destination-{suffix}.pdf"
        ));
        std::fs::write(&destination, b"existing").expect("write existing destination");

        let mut pending = pending_output(13);
        pending.kind = HtmlOutputKind::PdfExport;
        pending.destination = Some(destination.clone());
        bridge.lock().expect("bridge").pending_output = Some(pending);
        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"print_ready","nonce":13,"render_epoch":7,"print_mode":true}"#,
        );
        take_ready_output(&bridge).expect("PDF backend starts from epoch 7");
        std::fs::write(&temp_path, b"partial backend bytes").expect("write partial output");
        bridge
            .lock()
            .expect("bridge")
            .pending_output
            .as_mut()
            .expect("pending output")
            .temp_path = Some(temp_path.clone());

        accept_renderer_message(
            &bridge,
            &plan,
            r#"{"type":"renderer_invalidated","render_epoch":8}"#,
        );
        assert!(!temp_path.exists());
        std::fs::write(&temp_path, b"late backend bytes").expect("simulate late backend write");
        let expectation = PdfExpectation {
            form_code: "2551Q".to_string(),
            revision: "2018".to_string(),
            envelope_hash: "a".repeat(64),
            expected_page_count: 2,
            width_points: 612.0,
            height_points: 936.0,
        };

        finish_linux_pdf_export(&bridge, 13, 7, &temp_path, &destination, &expectation);

        assert_eq!(
            std::fs::read(&destination).expect("read existing destination"),
            b"existing"
        );
        assert!(!temp_path.exists());
        std::fs::remove_file(destination).expect("remove test destination");
    }
}
