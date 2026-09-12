//! In-window scrolled screenshot job. Semantic scroll via `ScrollHandle`
//! (and print-preview WebView `scrollTo`) — no OS HID and no virtual wheel.
//!
//! Tile capture is macOS-only; other OSes keep the types so the mailbox
//! path can fail closed without a second job struct.

#![cfg_attr(not(target_os = "macos"), allow(dead_code))]

use gpui_agent::mailbox::MailboxRequest;
use gpui_agent::{ScrollMetrics, TileSpec};

pub const METRICS_WAIT_FRAMES: u32 = 90;
/// How long one WebView tile may take before the job fails closed. Wall clock,
/// not frames: the poll runs once per paint, and paints are not 60 Hz while
/// the window is idle.
pub const SNAPSHOT_WAIT: std::time::Duration = std::time::Duration::from_secs(10);

pub enum ScrolledPhase {
    WaitMetrics,
    WaitPaint,
    /// A `WKWebView.takeSnapshot` is in flight for the current tile
    /// (print preview only); its result lands in `ScrolledShotJob::snapshot`.
    WaitSnapshot,
}

pub struct ScrolledShotJob {
    pub reply: MailboxRequest,
    pub dest_client: String,
    pub target: String,
    pub original_offset: f32,
    pub tiles: Vec<TileSpec>,
    pub next: usize,
    pub captured: Vec<gpui_agent::RgbaImage>,
    pub metrics: Option<ScrollMetrics>,
    pub window_size: (f32, f32),
    pub window_id: u32,
    pub phase: ScrolledPhase,
    pub frames_waited: u32,
    /// Skip the first `WaitPaint` tick after `set_offset` so the previous
    /// frame (with the new offset) can paint before `screencapture`.
    pub awaiting_paint: bool,
    /// The in-flight WebView tile (print preview) and how long it has been waited for.
    #[cfg(all(feature = "agent", target_os = "macos"))]
    pub snapshot: Option<crate::agent::macos_webview_snapshot::SnapshotSlot>,
    pub snapshot_started: Option<std::time::Instant>,
}

pub fn known_scroll_target(target: &str) -> bool {
    target == crate::agent::ids::FORM_1601C_SCROLL
        || target == crate::agent::ids::PRINT_PREVIEW_SCROLL
}
