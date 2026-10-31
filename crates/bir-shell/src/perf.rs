//! Performance tooling for the shell window: a jank log, an on-screen
//! monitor, a per-frame trace and a scroll bench. All of it is off by
//! default; a normal launch builds none of it and the main window opens
//! exactly as before (the one thing left in a normal build is a relaxed
//! atomic load per screen-form host call, see [`timed`]).
//!
//! | variable | what it turns on |
//! | --- | --- |
//! | `BIR_SHELL_FPS=1` | gpui-fps's HUD (top right) and the frame breakdown panel (bottom right); also the jank log |
//! | `BIR_SHELL_JANK=1` | the jank log alone |
//! | `BIR_SHELL_JANK_LOG=<path>` | where the jank log goes (default `~/Library/Logs/eBIRForms/jank.log` on macOS, `%LOCALAPPDATA%\eBIRForms\Logs\jank.log` on Windows, `$XDG_STATE_HOME/ebirforms/jank.log` on Linux); setting it turns the log on |
//! | `BIR_SHELL_TRACE=1` | every frame, not only the slow ones, with its script renders and materializations, in the jank log |
//! | `BIR_SHELL_SCROLL_BENCH=<pt per frame>` | the scroll bench (below); also the jank log |
//! | `BIR_SHELL_TYPE_BENCH=<x>,<y>,<keys>` | the typing bench (below); also the jank log |
//!
//! # Jank log
//!
//! Every frame whose work exceeds the display's refresh interval (8.3 ms at
//! 120 Hz, 16.7 ms at 60 Hz; GPUI never judges below 8.3 ms) appends a line:
//!
//! ```text
//! 13:41:21.008 frame 116.3ms (budget 8.3) [scroll] script 0.0 (0 renders) build 25.7 (111 views) layout 96.8 paint 14.4 present 0.7 before 1.8 host 0.0 | dirty->present 116.9ms
//! ```
//!
//! * `[trigger]` — what made the frame: `scroll`, `key`, `mouse` (an input
//!   event since the previous frame), `notify` (no input, but a script render
//!   ran: a timer, a task, a host reply), `redraw` (no input and no script
//!   render: an animation or a retained view re-materializing).
//! * `script` — QuickJS `render` passes (host calls made inside them
//!   included) and how many there were.
//! * `build` — gpui-shell materializing script descriptions into GPUI
//!   elements, and how many view snapshots it materialized. gpui-shell does
//!   not expose a node count to the host, so the count is views, not nodes.
//! * `views` / `layout` / `paint` — GPUI's own draw phases (request layout,
//!   prepaint incl. Taffy layout, paint) with the script and build time
//!   taken out of whichever phase they ran in. A dirty script view renders
//!   inside the prepaint of the cached shell root, so most script and build
//!   time comes out of `layout`.
//! * `present` — submitting the frame; `before` — main-thread time between
//!   the frame becoming dirty and its draw (input and timer handlers,
//!   JavaScript outside render).
//! * `host` — every host function the script called since the previous
//!   frame, inside a render or not; the synchronous screen-form calls are
//!   named with their own time (`host 9.8 (screen_form_fill 9.1)`).
//!
//! # Scroll bench
//!
//! `BIR_SHELL_SCROLL_BENCH=<pt per frame>` waits for the page the
//! `BIR_SHELL_START` route opens to settle, then scrolls the form down by
//! `BIR_SHELL_BENCH_DISTANCE` points (default 12000) and back up at that
//! speed, one synthetic wheel event per frame from a frame callback, over the
//! middle of the form column. It writes a summary to the jank log, between
//! `---- scroll bench ... ----` and `==== bench end ====`: frames, frames over
//! budget, mean/p95/worst frame, steps more than 1.5 refresh periods apart,
//! the longest gap, frames per second and materializations per second.
//! `scripts/bench-shell-scroll.sh` builds, seeds, runs and reads it.
//!
//! # Typing bench
//!
//! `BIR_SHELL_TYPE_BENCH=<x>,<y>,<keys>` waits for the page to settle,
//! clicks the window point `x,y` (logical pixels; a text box), then types
//! `keys` through GPUI's own keystroke path (`Window::dispatch_keystroke`),
//! one every `BIR_SHELL_TYPE_INTERVAL_MS` (default 150); `|` in `keys` is
//! Tab. It writes, between `---- typing bench ----` and `==== typing end
//! ====`: each key's latency from dispatch to the end of the next present,
//! the frames while typing (and the debounced fill/validate after it), and
//! the named host calls with their counts and times.
//!
//! GPUI clamps a scroll offset at layout, so a wheel event past the end of
//! the form still re-renders the page: a distance longer than the form
//! keeps measuring the per-event cost at the bottom.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::io::Write as _;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use gpui_kit::profiler::{FrameEvent, FrameTiming, FrameTimingCollector};
use gpui_kit::{
    AnyView, App, AppContext as _, Context, DispatchPhase, Entity, IntoElement, KeyDownEvent,
    Modifiers, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, ParentElement,
    PlatformInput, Render, ScrollDelta, ScrollWheelEvent, Styled, TouchPhase, Window, WindowId,
    canvas, div, hsla, point, px,
};
use gpui_shell::{HostArguments, HostResult, RuntimeMetrics, ShellRoot, ShellRuntime};

/// What the environment asked for. `None` from [`config`] when nothing was.
#[derive(Clone, Debug, Default)]
pub(crate) struct Config {
    fps: bool,
    trace: bool,
    jank_log: PathBuf,
    bench: Option<BenchConfig>,
    typing: Option<TypeConfig>,
}

#[derive(Clone, Debug)]
struct TypeConfig {
    x: f32,
    y: f32,
    keys: String,
    interval: Duration,
    wait: Duration,
}

#[derive(Clone, Copy, Debug)]
struct BenchConfig {
    speed: f32,
    distance: f32,
    /// The earliest the bench starts, after the window opened.
    wait: Duration,
}

fn flag(name: &str) -> bool {
    std::env::var(name)
        .map(|value| matches!(value.trim(), "1" | "true" | "on" | "yes"))
        .unwrap_or(false)
}

fn number(name: &str) -> Option<f32> {
    std::env::var(name)
        .ok()
        .and_then(|v| v.trim().parse::<f32>().ok())
        .filter(|v| *v > 0.0)
}

/// The tooling the environment turns on, or `None` for a normal launch.
pub(crate) fn config() -> Option<Config> {
    let fps = flag("BIR_SHELL_FPS");
    let trace = flag("BIR_SHELL_TRACE");
    let log_override = std::env::var_os("BIR_SHELL_JANK_LOG").map(PathBuf::from);
    let bench = number("BIR_SHELL_SCROLL_BENCH").map(|speed| BenchConfig {
        speed,
        distance: number("BIR_SHELL_BENCH_DISTANCE").unwrap_or(12000.0),
        wait: Duration::from_millis(number("BIR_SHELL_BENCH_WAIT_MS").unwrap_or(6000.0) as u64),
    });
    let typing = std::env::var("BIR_SHELL_TYPE_BENCH").ok().and_then(|raw| {
        let mut parts = raw.splitn(3, ',');
        let x = parts.next()?.trim().parse().ok()?;
        let y = parts.next()?.trim().parse().ok()?;
        Some(TypeConfig {
            x,
            y,
            keys: parts.next()?.to_string(),
            interval: Duration::from_millis(
                number("BIR_SHELL_TYPE_INTERVAL_MS").unwrap_or(150.0) as u64
            ),
            wait: Duration::from_millis(number("BIR_SHELL_BENCH_WAIT_MS").unwrap_or(6000.0) as u64),
        })
    });
    let on = fps
        || trace
        || bench.is_some()
        || typing.is_some()
        || log_override.is_some()
        || flag("BIR_SHELL_JANK");
    on.then(|| Config {
        fps,
        trace,
        jank_log: log_override.unwrap_or_else(default_log_path),
        bench,
        typing,
    })
}

#[cfg(target_os = "macos")]
fn default_log_path() -> PathBuf {
    let home = std::env::var_os("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library/Logs/eBIRForms/jank.log")
}

#[cfg(windows)]
fn default_log_path() -> PathBuf {
    std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir)
        .join("eBIRForms")
        .join("Logs")
        .join("jank.log")
}

#[cfg(not(any(target_os = "macos", windows)))]
fn default_log_path() -> PathBuf {
    std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default()).join(".local/state")
        })
        .join("ebirforms")
        .join("jank.log")
}

// ---------------------------------------------------------------- host calls

/// Set while the tooling runs; host-call timing is skipped otherwise.
static TIMING: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Named host calls since the last frame was painted.
    static HOST_CALLS: RefCell<Vec<(&'static str, Duration)>> = const { RefCell::new(Vec::new()) };
}

/// Wraps a synchronous host function so the jank log can name it. With the
/// tooling off this is one relaxed atomic load before the call.
pub(crate) fn timed(
    name: &'static str,
    body: impl Fn(&HostArguments) -> HostResult + 'static,
) -> impl Fn(&HostArguments) -> HostResult + 'static {
    move |arguments| {
        if !TIMING.load(Ordering::Relaxed) {
            return body(arguments);
        }
        let started = Instant::now();
        let result = body(arguments);
        let took = started.elapsed();
        HOST_CALLS.with(|calls| {
            let mut calls = calls.borrow_mut();
            match calls.iter_mut().find(|(n, _)| *n == name) {
                Some((_, total)) => *total += took,
                None => calls.push((name, took)),
            }
        });
        result
    }
}

fn take_host_calls() -> Vec<(&'static str, Duration)> {
    HOST_CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
}

// ---------------------------------------------------------------- frames

/// What made a frame.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Trigger {
    Scroll,
    Key,
    Mouse,
    Notify,
    #[default]
    Redraw,
}

impl Trigger {
    fn as_str(self) -> &'static str {
        match self {
            Trigger::Scroll => "scroll",
            Trigger::Key => "key",
            Trigger::Mouse => "mouse",
            Trigger::Notify => "notify",
            Trigger::Redraw => "redraw",
        }
    }
}

/// One frame, as the panel, the log and the bench read it. Milliseconds.
#[derive(Clone, Debug, Default)]
struct Frame {
    end: Option<Instant>,
    work: f64,
    budget: f64,
    trigger: Trigger,
    script: f64,
    renders: u64,
    build: f64,
    views_built: u64,
    host: f64,
    named: Vec<(&'static str, f64)>,
    views: f64,
    layout: f64,
    paint: f64,
    present: f64,
    before: f64,
    latency: Option<f64>,
}

impl Frame {
    fn line(&self) -> String {
        let latency = self
            .latency
            .map(|ms| format!(" | dirty->present {ms:.1}ms"))
            .unwrap_or_default();
        let named = if self.named.is_empty() {
            String::new()
        } else {
            let calls: Vec<String> = self
                .named
                .iter()
                .map(|(name, ms)| format!("{name} {ms:.1}"))
                .collect();
            format!(" ({})", calls.join(", "))
        };
        format!(
            "frame {:.1}ms (budget {:.1}) [{}] script {:.1} ({} renders) build {:.1} ({} views) views {:.1} layout {:.1} paint {:.1} present {:.1} before {:.1} host {:.1}{named}{latency}",
            self.work,
            self.budget,
            self.trigger.as_str(),
            self.script,
            self.renders,
            self.build,
            self.views_built,
            self.views,
            self.layout,
            self.paint,
            self.present,
            self.before,
            self.host,
        )
    }
}

/// Counters read at one frame's paint: what they gained since the previous
/// paint, and the input seen in between.
struct Mark {
    at: Instant,
    delta: RuntimeMetrics,
    calls: Vec<(&'static str, Duration)>,
    input: Option<Trigger>,
}

/// The jank log file, opened on first write.
struct JankLog {
    path: PathBuf,
    file: Option<std::fs::File>,
}

impl JankLog {
    fn write(&mut self, line: &str) {
        if self.file.is_none() {
            if let Some(dir) = self.path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            self.file = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)
                .ok();
        }
        let stamp = clock();
        if let Some(file) = self.file.as_mut() {
            let _ = writeln!(file, "{stamp} {line}");
        }
        tracing::info!(target: "bir_shell::frames", "{line}");
    }
}

/// Wall-clock time of day (UTC), for lines two runs can be lined up by.
fn clock() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs() % 86_400;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60,
        now.subsec_millis()
    )
}

/// The typing bench while it runs.
#[derive(Default)]
struct TypingRun {
    /// Keys dispatched whose frame has not been presented yet.
    pending: VecDeque<Instant>,
    latencies: Vec<f64>,
    frames: Vec<Frame>,
}

/// The scroll bench while it runs.
struct BenchRun {
    started: Instant,
    frames: Vec<Frame>,
    materializations: u64,
    steps: u32,
    gaps_over: u32,
    longest_gap: f64,
    last_step: Option<Instant>,
}

struct Probe {
    config: Config,
    window: WindowId,
    opened: Instant,
    runtime: Rc<ShellRuntime>,
    collector: FrameTimingCollector,
    last_metrics: RuntimeMetrics,
    marks: VecDeque<Mark>,
    /// Input seen since the last paint.
    input: Option<Trigger>,
    /// A drawn frame waiting for its present event.
    pending: Option<FrameTiming>,
    /// The last second of frames, for the panel.
    recent: VecDeque<Frame>,
    /// When the panel last asked for a frame of its own, so that frame is
    /// left out of the readings.
    own_notify: Option<Instant>,
    panel: Vec<String>,
    log: JankLog,
    bench: Option<BenchRun>,
    typing: Option<TypingRun>,
    /// The last frame that did any work, for the bench's settle check.
    last_busy: Instant,
}

impl Probe {
    fn note_input(&mut self, kind: Trigger) {
        // Scroll and keys say more about a frame than the pointer moving.
        let rank = |t: Option<Trigger>| match t {
            Some(Trigger::Scroll) | Some(Trigger::Key) => 2,
            Some(Trigger::Mouse) => 1,
            _ => 0,
        };
        if rank(Some(kind)) >= rank(self.input) {
            self.input = Some(kind);
        }
    }

    /// Called from the frame's paint phase.
    fn mark(&mut self) {
        let now = self.runtime.read_metrics();
        let delta = now.since(&self.last_metrics);
        self.last_metrics = now;
        self.marks.push_back(Mark {
            at: Instant::now(),
            delta,
            calls: take_host_calls(),
            input: self.input.take(),
        });
        while self.marks.len() > 512 {
            self.marks.pop_front();
        }
    }

    fn mark_for(&mut self, timing: &FrameTiming) -> Option<Mark> {
        while self
            .marks
            .front()
            .is_some_and(|mark| mark.at < timing.draw_start)
        {
            self.marks.pop_front();
        }
        match self.marks.front() {
            Some(mark) if mark.at <= timing.draw_end => self.marks.pop_front(),
            _ => None,
        }
    }

    fn is_own(&self, timing: &FrameTiming) -> bool {
        match (self.own_notify, timing.dirty_at) {
            (Some(notified), Some(dirty)) => timing.invalidations <= 1 && dirty >= notified,
            _ => false,
        }
    }

    fn finish(&mut self, timing: FrameTiming, present: Option<(Instant, Instant)>) {
        let own = self.is_own(&timing);
        let mark = self.mark_for(&timing);
        if own {
            self.own_notify = None;
            return;
        }
        let ms = |d: Duration| d.as_secs_f64() * 1000.0;
        let (metrics, calls, input) = mark
            .map(|m| (m.delta, m.calls, m.input))
            .unwrap_or_default();
        let submitted = present.map_or(timing.draw_end, |(start, _)| start);
        let draw = ms(timing.draw_duration());
        let work = timing.work(submitted).map_or(draw, ms);
        let script = ms(metrics.script_render_time());
        let build = ms(metrics.materialize_time());
        // Script and build ran inside one of GPUI's phases: request layout
        // for a root render, prepaint for a view under a cached boundary.
        let request_layout = ms(timing.phases.request_layout);
        let inside = script + build;
        let in_request = inside.min(request_layout);
        let trigger = input.unwrap_or(if metrics.script_renders() > 0 {
            Trigger::Notify
        } else {
            Trigger::Redraw
        });
        let frame = Frame {
            end: Some(present.map_or(timing.draw_end, |(_, end)| end)),
            work,
            budget: ms(timing.budget()),
            trigger,
            script,
            renders: metrics.script_renders(),
            build,
            views_built: metrics.materializations(),
            host: ms(metrics.native_time()),
            named: calls.iter().map(|(name, d)| (*name, ms(*d))).collect(),
            views: request_layout - in_request,
            layout: (ms(timing.phases.prepaint) - (inside - in_request)).max(0.0),
            paint: ms(timing.phases.paint),
            present: present.map_or(0.0, |(start, end)| ms(end.duration_since(start))),
            before: timing.work_start().map_or(0.0, |start| {
                ms(timing.draw_start.saturating_duration_since(start))
            }),
            latency: timing.dirty_at.map(|dirty| {
                ms(present
                    .map_or(timing.draw_end, |(_, end)| end)
                    .duration_since(dirty))
            }),
        };
        // The page is busy while scripts render or views re-materialize;
        // the HUD's own redraws do neither.
        if frame.renders > 0 || frame.views_built > 0 {
            self.last_busy = Instant::now();
        }
        if frame.work > frame.budget || self.config.trace {
            let prefix = if frame.work > frame.budget {
                ""
            } else {
                "trace "
            };
            self.log.write(&format!("{prefix}{}", frame.line()));
        }
        if let Some(bench) = self.bench.as_mut() {
            bench.materializations += frame.views_built;
            bench.frames.push(frame.clone());
        }
        if let Some(typing) = self.typing.as_mut() {
            if let Some(end) = frame.end {
                while typing.pending.front().is_some_and(|key| *key <= end) {
                    let key = typing.pending.pop_front().unwrap_or(end);
                    typing
                        .latencies
                        .push(end.duration_since(key).as_secs_f64() * 1000.0);
                }
            }
            typing.frames.push(frame.clone());
        }
        self.recent.push_back(frame);
    }

    /// Takes the frames GPUI recorded since the last poll.
    fn collect(&mut self) {
        for event in self.collector.collect_unseen() {
            match event {
                FrameEvent::Draw(timing) if timing.window_id == self.window => {
                    if let Some(previous) = self.pending.take() {
                        self.finish(previous, None);
                    }
                    self.pending = Some(timing);
                }
                FrameEvent::Present(present) if present.window_id == self.window => {
                    if let Some(timing) = self.pending.take() {
                        self.finish(timing, Some((present.present_start, present.present_end)));
                    }
                }
                _ => {}
            }
        }
        let cutoff = Instant::now() - Duration::from_secs(1);
        while self
            .recent
            .front()
            .is_some_and(|frame| frame.end.is_some_and(|end| end < cutoff))
        {
            self.recent.pop_front();
        }
    }

    /// The panel's lines for the frames of the last second.
    fn summary(&self) -> Vec<String> {
        let frames: Vec<&Frame> = self.recent.iter().collect();
        let Some(last) = frames.last() else {
            return vec!["idle (no frames in the last second)".into()];
        };
        let (p95, worst) = percentile(frames.iter().map(|f| f.work));
        let n = frames.len() as f64;
        let mean = |pick: fn(&Frame) -> f64| frames.iter().map(|f| pick(f)).sum::<f64>() / n;
        let late = frames.iter().filter(|f| f.work > f.budget).count();
        let mut lines = vec![
            format!(
                "{} frames/s  late {}  budget {:.1}",
                frames.len(),
                late,
                last.budget
            ),
            format!(
                "frame last {:.1}  p95 {:.1}  worst {:.1}",
                last.work, p95, worst
            ),
            format!(
                "avg script {:.1} build {:.1} host {:.1}",
                mean(|f| f.script),
                mean(|f| f.build),
                mean(|f| f.host)
            ),
            format!(
                "avg views {:.1} layout {:.1} paint {:.1}",
                mean(|f| f.views),
                mean(|f| f.layout),
                mean(|f| f.paint)
            ),
            format!(
                "avg present {:.1} before {:.1}",
                mean(|f| f.present),
                mean(|f| f.before)
            ),
        ];
        if let Some(slow) = frames
            .iter()
            .max_by(|a, b| a.work.total_cmp(&b.work))
            .filter(|f| f.work > f.budget)
        {
            lines.push(format!(
                "worst [{}]: script {:.1} build {:.1} layout {:.1} paint {:.1}",
                slow.trigger.as_str(),
                slow.script,
                slow.build,
                slow.layout,
                slow.paint
            ));
        }
        lines
    }

    fn bench_summary(&mut self, config: &BenchConfig) {
        let Some(bench) = self.bench.take() else {
            return;
        };
        let frames = &bench.frames;
        let secs = bench.started.elapsed().as_secs_f64().max(0.001);
        let (p95, worst) = percentile(frames.iter().map(|f| f.work));
        let n = frames.len().max(1) as f64;
        let mean = frames.iter().map(|f| f.work).sum::<f64>() / n;
        let over = frames.iter().filter(|f| f.work > f.budget).count();
        let budget = frames.first().map_or(8.3, |f| f.budget);
        let avg = |pick: fn(&Frame) -> f64| frames.iter().map(|f| pick(f)).sum::<f64>() / n;
        let lines = [
            format!(
                "bench: {} frames in {:.1}s ({:.1} frames/s), {} over {:.1} ms budget",
                frames.len(),
                secs,
                frames.len() as f64 / secs,
                over,
                budget
            ),
            format!("bench: frame mean {mean:.1} ms, p95 {p95:.1} ms, worst {worst:.1} ms"),
            format!(
                "bench: avg script {:.1} build {:.1} views {:.1} layout {:.1} paint {:.1} present {:.1} host {:.1}",
                avg(|f| f.script),
                avg(|f| f.build),
                avg(|f| f.views),
                avg(|f| f.layout),
                avg(|f| f.paint),
                avg(|f| f.present),
                avg(|f| f.host)
            ),
            format!(
                "bench: materializations {} ({:.0}/s, {:.1} per frame)",
                bench.materializations,
                bench.materializations as f64 / secs,
                bench.materializations as f64 / n
            ),
            format!(
                "frame gaps: {} steps, {} over {:.1} ms, longest {:.1} ms",
                bench.steps,
                bench.gaps_over,
                budget * 1.5,
                bench.longest_gap
            ),
            format!(
                "==== bench end ==== {:.0}pt/frame, {:.0}pt down and back",
                config.speed, config.distance
            ),
        ];
        for line in lines {
            self.log.write(&line);
        }
    }
}

impl Probe {
    fn typing_summary(&mut self) {
        let Some(run) = self.typing.take() else {
            return;
        };
        let frames = &run.frames;
        let (lat_p95, lat_worst) = percentile(run.latencies.iter().copied());
        let lat_mean = run.latencies.iter().sum::<f64>() / run.latencies.len().max(1) as f64;
        let (p95, worst) = percentile(frames.iter().map(|f| f.work));
        let over = frames.iter().filter(|f| f.work > f.budget).count();
        let total: f64 = frames.iter().map(|f| f.work + f.before).sum();
        let renders: u64 = frames.iter().map(|f| f.renders).sum();
        let views: u64 = frames.iter().map(|f| f.views_built).sum();
        let script: f64 = frames.iter().map(|f| f.script).sum();
        let build: f64 = frames.iter().map(|f| f.build).sum();
        let layout: f64 = frames.iter().map(|f| f.layout + f.views).sum();
        let paint: f64 = frames.iter().map(|f| f.paint).sum();
        let mut calls: Vec<(&'static str, u32, f64, f64)> = Vec::new();
        for frame in frames {
            for (name, ms) in &frame.named {
                match calls.iter_mut().find(|c| c.0 == *name) {
                    Some(c) => {
                        c.1 += 1;
                        c.2 += ms;
                        c.3 = c.3.max(*ms);
                    }
                    None => calls.push((name, 1, *ms, *ms)),
                }
            }
        }
        let calls: Vec<String> = calls
            .iter()
            .map(|(name, n, total, worst)| format!("{name} x{n} total {total:.1} worst {worst:.1}"))
            .collect();
        let lines = [
            format!(
                "typing: {} keys, key->present mean {lat_mean:.1} ms, p95 {lat_p95:.1} ms, worst {lat_worst:.1} ms",
                run.latencies.len()
            ),
            format!(
                "typing: {} frames, {over} over budget, frame p95 {p95:.1} ms, worst {worst:.1} ms, main thread {total:.0} ms",
                frames.len()
            ),
            format!(
                "typing: script {script:.1} ms ({renders} renders), build {build:.1} ms ({views} views), layout {layout:.1} ms, paint {paint:.1} ms"
            ),
            format!(
                "typing: host {}",
                if calls.is_empty() {
                    "-".to_string()
                } else {
                    calls.join(", ")
                }
            ),
            "==== typing end ====".to_string(),
        ];
        for line in lines {
            self.log.write(&line);
        }
    }
}

/// Clicks the configured point, then types the keys, one per interval.
async fn run_typing_bench(
    config: TypeConfig,
    probe: Rc<RefCell<Probe>>,
    cx: &mut gpui_kit::AsyncWindowContext,
) {
    // Settle as the scroll bench does.
    loop {
        cx.background_executor()
            .timer(Duration::from_millis(250))
            .await;
        let p = probe.borrow();
        let waited = p.opened.elapsed();
        if waited >= config.wait
            && (p.last_busy.elapsed() >= Duration::from_millis(1000)
                || waited >= config.wait + Duration::from_secs(10))
        {
            break;
        }
    }
    let at = point(px(config.x), px(config.y));
    let clicked = cx.update(|window, cx| {
        let size = window.viewport_size();
        probe.borrow_mut().log.write(&format!(
            "typing bench window {:.0}x{:.0}",
            f32::from(size.width),
            f32::from(size.height)
        ));
        for input in [
            PlatformInput::MouseMove(MouseMoveEvent {
                position: at,
                pressed_button: None,
                modifiers: Modifiers::default(),
            }),
            PlatformInput::MouseDown(MouseDownEvent {
                button: MouseButton::Left,
                position: at,
                modifiers: Modifiers::default(),
                click_count: 1,
                first_mouse: false,
            }),
            PlatformInput::MouseUp(MouseUpEvent {
                button: MouseButton::Left,
                position: at,
                modifiers: Modifiers::default(),
                click_count: 1,
            }),
        ] {
            window.dispatch_event(input, cx);
        }
    });
    if clicked.is_err() {
        return;
    }
    cx.background_executor()
        .timer(Duration::from_millis(500))
        .await;
    {
        let mut p = probe.borrow_mut();
        p.collect();
        let line = format!(
            "---- typing bench at {:.0},{:.0}: {:?} every {} ms ----",
            config.x,
            config.y,
            config.keys,
            config.interval.as_millis()
        );
        p.log.write(&line);
        p.typing = Some(TypingRun::default());
    }
    for key in config.keys.chars() {
        let name = if key == '|' {
            "tab".to_string()
        } else {
            key.to_string()
        };
        let Ok(keystroke) = gpui_kit::Keystroke::parse(&name) else {
            continue;
        };
        if let Some(run) = probe.borrow_mut().typing.as_mut() {
            run.pending.push_back(Instant::now());
        }
        if cx
            .update(|window, cx| {
                window.dispatch_keystroke(keystroke, cx);
            })
            .is_err()
        {
            return;
        }
        cx.background_executor().timer(config.interval).await;
    }
    // The debounced fill (300 ms) and the validation after it.
    cx.background_executor()
        .timer(Duration::from_millis(2000))
        .await;
    let mut p = probe.borrow_mut();
    p.collect();
    p.typing_summary();
}

/// p95 and worst of the values.
fn percentile(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let mut values: Vec<f64> = values.collect();
    if values.is_empty() {
        return (0.0, 0.0);
    }
    values.sort_by(|a, b| a.total_cmp(b));
    let at = ((values.len() as f64 * 0.95).ceil() as usize).saturating_sub(1);
    (values[at], values[values.len() - 1])
}

// ---------------------------------------------------------------- window root

/// The window root while the tooling runs: the real root, the breakdown
/// panel when the HUD is on, and a paint hook that reads the runtime's
/// counters once per frame and listens for the input that triggers frames.
pub(crate) struct PerfFrame<V: 'static> {
    inner: Entity<V>,
    probe: Rc<RefCell<Probe>>,
}

impl<V: 'static> PerfFrame<V> {
    /// The root it wraps (a platform menu frame looks itself up through it).
    #[cfg_attr(target_os = "macos", allow(dead_code))]
    pub(crate) fn inner(&self) -> &Entity<V> {
        &self.inner
    }
}

impl<V: Render + 'static> Render for PerfFrame<V> {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        let probe = self.probe.clone();
        let (show_panel, lines) = {
            let probe = self.probe.borrow();
            (probe.config.fps, probe.panel.clone())
        };
        let hook = canvas(
            |_, _, _| {},
            move |_, _, window, _| {
                probe.borrow_mut().mark();
                // Listeners last one frame; the hook runs every frame.
                let p = probe.clone();
                window.on_mouse_event(move |_: &ScrollWheelEvent, phase, _, _| {
                    if phase == DispatchPhase::Capture {
                        p.borrow_mut().note_input(Trigger::Scroll);
                    }
                });
                let p = probe.clone();
                window.on_mouse_event(move |_: &MouseDownEvent, phase, _, _| {
                    if phase == DispatchPhase::Capture {
                        p.borrow_mut().note_input(Trigger::Mouse);
                    }
                });
                let p = probe.clone();
                window.on_mouse_event(move |_: &MouseMoveEvent, phase, _, _| {
                    if phase == DispatchPhase::Capture {
                        p.borrow_mut().note_input(Trigger::Mouse);
                    }
                });
                let p = probe.clone();
                window.on_key_event(move |_: &KeyDownEvent, phase, _, _| {
                    if phase == DispatchPhase::Capture {
                        p.borrow_mut().note_input(Trigger::Key);
                    }
                });
            },
        )
        .absolute()
        .top_0()
        .left_0()
        .size(px(1.));
        div()
            .relative()
            .size_full()
            .child(AnyView::from(self.inner.clone()))
            .child(hook)
            .children(show_panel.then(|| {
                div()
                    .absolute()
                    .bottom(px(8.))
                    .right(px(8.))
                    .p(px(6.))
                    .rounded(px(6.))
                    .bg(hsla(0., 0., 0.08, 0.82))
                    .text_color(hsla(0., 0., 0.95, 1.))
                    .font_family("Menlo")
                    .text_size(px(10.))
                    .line_height(px(13.))
                    .flex()
                    .flex_col()
                    .children(lines.into_iter().map(|line| div().child(line)))
            }))
    }
}

/// Turns the tooling on for the main window and returns the root to open it
/// with. `shell_root` is the shell's script root, which shows gpui-fps's HUD.
pub(crate) fn wrap<V: Render + 'static>(
    config: Config,
    inner: Entity<V>,
    shell_root: &Entity<ShellRoot>,
    runtime: Rc<ShellRuntime>,
    window: &mut Window,
    cx: &mut App,
) -> Entity<PerfFrame<V>> {
    gpui_kit::profiler::set_trace_enabled(true);
    TIMING.store(true, Ordering::Relaxed);
    if config.fps {
        shell_root.update(cx, |root, cx| {
            root.show_fps_monitor(gpui_shell::root::FpsHudRequest::default(), cx);
        });
    }
    let mut log = JankLog {
        path: config.jank_log.clone(),
        file: None,
    };
    log.write(&format!(
        "---- bir-shell {} pid {} (fps {}, trace {}, bench {:?}) ----",
        env!("CARGO_PKG_VERSION"),
        std::process::id(),
        config.fps,
        config.trace,
        config.bench.map(|b| b.speed)
    ));
    let bench = config.bench;
    let typing = config.typing.clone();
    let last_metrics = runtime.read_metrics();
    let probe = Rc::new(RefCell::new(Probe {
        config,
        window: window.window_handle().window_id(),
        opened: Instant::now(),
        runtime,
        collector: FrameTimingCollector::new(),
        last_metrics,
        marks: VecDeque::new(),
        input: None,
        pending: None,
        recent: VecDeque::new(),
        own_notify: None,
        panel: vec!["frame monitor: waiting for frames".into()],
        log,
        bench: None,
        typing: None,
        last_busy: Instant::now(),
    }));
    let frame = cx.new(|_| PerfFrame {
        inner,
        probe: probe.clone(),
    });
    if let Some(typing) = typing {
        let probe = probe.clone();
        window
            .spawn(cx, async move |cx| {
                run_typing_bench(typing, probe, cx).await
            })
            .detach();
    }
    let weak = frame.downgrade();
    window
        .spawn(cx, async move |cx| {
            let mut bench_state = BenchState::Waiting;
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(250))
                    .await;
                let (changed, start_bench) = {
                    let mut probe = probe.borrow_mut();
                    probe.collect();
                    let mut changed = false;
                    if probe.config.fps {
                        let lines = probe.summary();
                        if lines != probe.panel {
                            probe.panel = lines;
                            probe.own_notify = Some(Instant::now());
                            changed = true;
                        }
                    }
                    let start_bench = match (bench, bench_state) {
                        (Some(config), BenchState::Waiting) => {
                            // Settled: a second with no script work, or
                            // ten seconds past the wait regardless.
                            let waited = probe.opened.elapsed();
                            waited >= config.wait
                                && (probe.last_busy.elapsed() >= Duration::from_millis(1000)
                                    || waited >= config.wait + Duration::from_secs(10))
                        }
                        _ => false,
                    };
                    (changed, start_bench)
                };
                if changed && weak.update(cx, |_, cx| cx.notify()).is_err() {
                    break;
                }
                if let Some(config) = bench {
                    match bench_state {
                        BenchState::Waiting if start_bench => {
                            bench_state = BenchState::Running;
                            let probe = probe.clone();
                            let done = cx.update(|window, _| {
                                start_scroll_bench(config, probe, window);
                            });
                            if done.is_err() {
                                break;
                            }
                        }
                        BenchState::Running => {
                            let finished = probe
                                .borrow()
                                .bench
                                .as_ref()
                                .is_some_and(|b| b.steps > 0 && b.last_step.is_none());
                            if finished {
                                // One more poll so the last frames are in.
                                cx.background_executor()
                                    .timer(Duration::from_millis(300))
                                    .await;
                                let mut probe = probe.borrow_mut();
                                probe.collect();
                                probe.bench_summary(&config);
                                bench_state = BenchState::Done;
                            }
                        }
                        _ => {}
                    }
                }
            }
        })
        .detach();
    frame
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum BenchState {
    Waiting,
    Running,
    Done,
}

/// Starts the scroll: one wheel event per frame, from frame callbacks.
fn start_scroll_bench(config: BenchConfig, probe: Rc<RefCell<Probe>>, window: &mut Window) {
    {
        let mut p = probe.borrow_mut();
        p.collect();
        let size = window.viewport_size();
        p.log.write(&format!(
            "---- scroll bench {:.0}pt/frame, {:.0}pt down and back, window {:.0}x{:.0} ----",
            config.speed,
            config.distance,
            f32::from(size.width),
            f32::from(size.height)
        ));
        p.bench = Some(BenchRun {
            started: Instant::now(),
            frames: Vec::new(),
            materializations: 0,
            steps: 0,
            gaps_over: 0,
            longest_gap: 0.0,
            last_step: None,
        });
    }
    let viewport = window.viewport_size();
    // Over the form column: right of the part navigator, mid-height.
    let at = point(viewport.width * 0.7, viewport.height * 0.6);
    scroll_step(config, probe, at, 0.0, -1.0, window);
}

fn scroll_step(
    config: BenchConfig,
    probe: Rc<RefCell<Probe>>,
    at: gpui_kit::Point<gpui_kit::Pixels>,
    travelled: f32,
    direction: f32,
    window: &Window,
) {
    window.on_next_frame(move |window, cx| {
        let now = Instant::now();
        {
            let mut p = probe.borrow_mut();
            let budget = p.recent.back().map_or(1000.0 / 120.0, |f| f.budget);
            if let Some(bench) = p.bench.as_mut() {
                if let Some(last) = bench.last_step {
                    let gap = now.duration_since(last).as_secs_f64() * 1000.0;
                    if gap > budget * 1.5 {
                        bench.gaps_over += 1;
                    }
                    bench.longest_gap = bench.longest_gap.max(gap);
                }
                bench.last_step = Some(now);
                bench.steps += 1;
            }
        }
        // Negative y moves the content up (scrolls down), as a wheel does.
        let event = ScrollWheelEvent {
            position: at,
            delta: ScrollDelta::Pixels(point(px(0.), px(config.speed * direction))),
            modifiers: Modifiers::default(),
            touch_phase: TouchPhase::Moved,
        };
        window.dispatch_event(PlatformInput::ScrollWheel(event), cx);
        let travelled = travelled + config.speed;
        if travelled < config.distance {
            scroll_step(config, probe, at, travelled, direction, window);
        } else if direction < 0.0 {
            scroll_step(config, probe, at, 0.0, 1.0, window);
        } else if let Some(bench) = probe.borrow_mut().bench.as_mut() {
            bench.last_step = None;
        }
    });
}
