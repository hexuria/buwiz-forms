//! Window + runtime bootstrap, mirroring `crates/shell/src/host.rs::run` from
//! the monorepo but embedding the `bir` host module and our capability policy
//! instead of the CLI's local-app defaults.

use std::path::PathBuf;
use std::rc::Rc;

use gpui::{
    App, Bounds, TitlebarOptions, Window, WindowBounds, WindowDecorations, WindowOptions, point,
    px, size,
};
use gpui_shell::{
    AppAssets, Capabilities, ShellRuntime, export_module, init_with_components, on_exit_request,
    resolve_app_root, set_bundle_id, set_capabilities, set_development_mode,
};

use crate::bir_module;

/// The JavaScript application directory.
///
/// `BIR_SHELL_APP` wins (used by tests and packaged layouts). Development
/// builds default to `<repo>/app`; the packaged layout will install a fixed
/// relative path in S7.
pub fn app_root() -> PathBuf {
    if let Some(dir) = std::env::var_os("BIR_SHELL_APP") {
        return PathBuf::from(dir);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app")
}

/// Capabilities the `app/` JavaScript may exercise. Everything not listed is
/// denied — the manifest can narrow this set, never widen it.
fn capabilities() -> Capabilities {
    Capabilities::new()
        // Local settings store (store.json under the bundle id's data dir).
        .storage(true)
        .clipboard_read(true)
        .clipboard_write(true)
        // `process.exit` quits the app — the same authority the window close
        // button already has.
        .exit(true)
        // The JS reads configuration env, never arbitrary secrets.
        .env_variables(
            ["BIR_DATABASE_PATH", "BIR_SFTP_DRY_RUN", "EBIR_TEST_ENV"]
                .iter()
                .map(|name| name.to_string()),
        )
}

/// Opens the main window and mounts the JS application inside it. Returns
/// `Err` only when the platform itself cannot start; a broken `app/` bundle
/// renders the shell's built-in failure surface instead of exiting.
pub fn run(root: PathBuf) -> anyhow::Result<()> {
    let root = resolve_app_root(&root, "main.js").unwrap_or(root);

    gpui_platform::application()
        .with_assets(AppAssets::new(root.clone()))
        .run(move |cx| {
            let components =
                gpui_shell::components::components().expect("component catalog must build");
            init_with_components(cx, &components);

            // Debug builds get eval + unfrozen prototypes for console work;
            // release stays closed.
            set_development_mode(cfg!(debug_assertions));

            // Storage identity — same key painted `bir` would use so the JS
            // `storage` API lands in the app's own data dir.
            if let Err(error) = set_bundle_id("com.buwiz.bir") {
                tracing::warn!("storage unavailable: {error}");
            }
            set_capabilities(capabilities());
            export_module(bir_module::module()).expect("bir module must export");

            on_exit_request(|request, _window, _cx| {
                std::process::exit(request.code());
            });

            let runtime = match ShellRuntime::new_with_components(cx, components) {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("failed to start the script runtime: {error}");
                    std::process::exit(1);
                }
            };
            runtime.set_editor_files(true);

            open_main_window(cx, runtime, root);
            cx.activate(true);
        });
    Ok(())
}

fn open_main_window(cx: &mut App, runtime: Rc<ShellRuntime>, root: PathBuf) {
    let options = WindowOptions {
        window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
            None,
            size(px(1440.0), px(900.0)),
            cx,
        ))),
        app_id: Some("com.buwiz.bir".to_owned()),
        // The redesign draws its own 40 px title bar (DESIGN_SYSTEM.md "Shell
        // layout"): the system bar is hidden, the macOS traffic lights sit
        // vertically centred in it (12 px lights at y 14), and Linux gets
        // client-side decorations so the same bar is drawn there.
        titlebar: Some(TitlebarOptions {
            title: Some("eBIRForms".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.0), px(14.0))),
        }),
        window_decorations: Some(WindowDecorations::Client),
        window_min_size: Some(size(px(420.0), px(480.0))),
        ..Default::default()
    };
    cx.open_window(options, move |window: &mut Window, cx: &mut App| {
        if let Some(size) = requested_window_size() {
            hold_window_size(window, cx, size);
        }
        runtime.load(&root, window, cx)
    })
    .expect("main window must open");
}

/// Development aid for side-by-side checks against the design prototype:
/// `BIR_SHELL_WINDOW_SIZE=800x900` sizes the window. A tiling window manager
/// may re-tile it when it opens, so the size is re-applied for a few seconds
/// (long enough to float the window there).
fn requested_window_size() -> Option<gpui::Size<gpui::Pixels>> {
    let raw = std::env::var("BIR_SHELL_WINDOW_SIZE").ok()?;
    let (w, h) = raw.split_once('x')?;
    Some(size(px(w.trim().parse().ok()?), px(h.trim().parse().ok()?)))
}

fn hold_window_size(window: &mut Window, cx: &mut App, target: gpui::Size<gpui::Pixels>) {
    window
        .spawn(cx, async move |cx| {
            for _ in 0..12 {
                cx.background_executor()
                    .timer(std::time::Duration::from_millis(500))
                    .await;
                if cx.update(|window, _| window.resize(target)).is_err() {
                    break;
                }
            }
        })
        .detach();
}
