//! Window + runtime bootstrap, mirroring `crates/shell/src/host.rs::run` from
//! the monorepo but embedding the `bir` host module and our capability policy
//! instead of the CLI's local-app defaults.

use std::ffi::OsString;
use std::path::PathBuf;
use std::rc::Rc;

// GPUI comes through the Kit so the `gpui-fast` feature switches this host
// along with gpui-shell and the components (one engine in the build).
use gpui_kit::{
    App, Bounds, Pixels, Size, TitlebarOptions, Window, WindowBackgroundAppearance, WindowBounds,
    WindowDecorations, WindowOptions, point, px, size,
};
use gpui_shell::{
    AppAssets, Capabilities, ShellRuntime, export_module, init_with_components, on_exit_request,
    resolve_app_root, set_bundle_id, set_capabilities, set_development_mode,
};

use crate::bir_module;
use crate::packaged;

/// The JavaScript application directory.
///
/// Lookup order:
/// 1. `BIR_SHELL_APP` — tests and overrides always win.
/// 2. The packaged layout beside the running executable
///    ([`crate::packaged`]): `Contents/Resources/app` in a macOS bundle,
///    `app/` beside the executable in a Windows/Linux install or an unpacked
///    archive, and `/usr/share/ebirforms/app` for deb/rpm/pacman installs.
/// 3. Development checkout `<repo>/app` — debug builds only, because a
///    shipped binary must not resolve a path baked in at build time.
pub fn app_root() -> PathBuf {
    app_root_from(
        std::env::var_os("BIR_SHELL_APP"),
        std::env::current_exe().ok(),
    )
}

fn app_root_from(env_override: Option<OsString>, executable: Option<PathBuf>) -> PathBuf {
    if let Some(dir) = env_override {
        return PathBuf::from(dir);
    }
    if let Some(dir) = packaged::dir("app", executable.as_deref(), packaged::application_present) {
        return dir;
    }
    dev_app_root()
}

/// The checkout's `app/` for development runs and tests.
#[cfg(debug_assertions)]
fn dev_app_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app")
}

/// No packaged `app/` was found: answer the location it would have occupied
/// beside the executable so the load failure names the real path.
#[cfg(not(debug_assertions))]
fn dev_app_root() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|exe| exe.parent().map(|dir| dir.join("app")))
        .unwrap_or_else(|| PathBuf::from("app"))
}

/// Capabilities the `app/` JavaScript may exercise. Everything not listed is
/// denied — the manifest can narrow this set, never widen it.
fn capabilities() -> Capabilities {
    Capabilities::new()
        // Local settings store (store.json under the bundle id's data dir).
        .storage(true)
        .clipboard_read(true)
        .clipboard_write(true)
        // `process.exit` quits after the submission worker stops and the
        // database lock is released. The window close button hides to the
        // tray instead of exiting.
        .exit(true)
        // The JS reads configuration env, never arbitrary secrets.
        .env_variables(
            [
                "BIR_DATABASE_PATH",
                "BIR_SFTP_DRY_RUN",
                "EBIR_TEST_ENV",
                "BIR_SHELL_START",
                // Test hook: install a verified update this many seconds
                // after the prompt appears, without a click.
                "BIR_UPDATE_INSTALL_AFTER_SECS",
            ]
            .iter()
            .map(|name| name.to_string()),
        )
}

/// Opens the main window and mounts the JS application inside it. Returns
/// `Err` only when the platform itself cannot start; a broken `app/` bundle
/// renders the shell's built-in failure surface instead of exiting.
pub fn run(root: PathBuf) -> anyhow::Result<()> {
    let root = resolve_app_root(&root, "main.js").unwrap_or(root);

    // Linux tray menus panic unless GTK is initialized before GPUI connects.
    crate::tray::init_gtk();

    let app = gpui_kit::platform::application().with_assets(AppAssets::new(root.clone()));
    // Dock / taskbar click after a hide brings the windows back.
    // `on_reopen` borrows the application; `run` takes it.
    app.on_reopen(crate::tray::show_app);
    app.run(move |cx| {
        let components =
            gpui_shell::components::components().expect("component catalog must build");
        init_with_components(cx, &components);
        // The account gate decides the menu and whether the filing database
        // opens. A locked launch paints only the welcome screen. Checking a
        // saved sign-in is a network refresh, so it runs after the window
        // opens (`restore_in_background` below) and unlocks from there.
        let restore_session = crate::cloud_account::begin_launch();
        crate::app_menu::install(cx);

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
        // Open the database only after the account gate says the session is
        // good. That start is also what runs the submission worker. A locked
        // launch waits until sign-in calls `cloud_account_session_opened`.
        if crate::account_gate::is_unlocked() {
            bir_module::open_database_at_launch();
            crate::updater::start();
        }

        on_exit_request(|request, _window, _cx| {
            let code = request.code();
            crate::tray::drop_tray();
            bir_module::shutdown_for_quit();
            std::process::exit(code);
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
        if restore_session {
            crate::cloud_account::restore_in_background(cx);
        }
        crate::print_preview_window::start(cx);
        cx.activate(true);
    });
    Ok(())
}

/// Linux client decorations paint a shadow in the inset, so the surface has
/// to be transparent there. Other platforms keep the opaque default.
pub(crate) fn window_background() -> WindowBackgroundAppearance {
    #[cfg(target_os = "linux")]
    {
        WindowBackgroundAppearance::Transparent
    }
    #[cfg(not(target_os = "linux"))]
    {
        WindowBackgroundAppearance::Opaque
    }
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
        // layout"): the system bar is hidden and the macOS traffic lights sit
        // vertically centred in it (12 px lights at y 14). Linux asks for
        // client-side decorations. The menu row draws minimize, zoom, and
        // close when the compositor grants them (`linux_csd`); a compositor
        // that forces a server title bar is left as the only title bar.
        // A transparent surface lets the Linux shadow inset composite.
        titlebar: Some(TitlebarOptions {
            title: Some("eBIRForms".into()),
            appears_transparent: true,
            traffic_light_position: Some(point(px(14.0), px(14.0))),
        }),
        window_decorations: Some(WindowDecorations::Client),
        window_background: window_background(),
        window_min_size: Some(size(px(420.0), px(480.0))),
        ..Default::default()
    };
    cx.open_window(options, move |window: &mut Window, cx: &mut App| {
        if let Some(size) = requested_window_size() {
            hold_window_size(window, cx, size);
        }
        if std::env::var_os("BIR_SHELL_METRICS").is_some() {
            log_runtime_metrics(window, cx, runtime.clone());
        }
        let shell_root = runtime.load(&root, window, cx);
        #[cfg(feature = "agent")]
        crate::agent::start(window, cx, shell_root.clone());
        // The menu frame is the window root on Linux and Windows. The agent
        // keeps the script root, which is what the menu frame wraps.
        crate::app_menu::attach_window(shell_root, window, cx)
    })
    .expect("main window must open");
}

/// Development aid: `BIR_SHELL_METRICS=1` logs what the script runtime spent
/// over each second that had activity — how many times JavaScript `render`
/// ran and what one run cost, and how many times a description was turned
/// back into GPUI elements (scrolling and hover do this without running any
/// script). Compare debug and release, or two versions of a screen, with it.
fn log_runtime_metrics(window: &mut Window, cx: &mut App, runtime: Rc<ShellRuntime>) {
    window
        .spawn(cx, async move |cx| {
            tracing::info!(target: "bir_shell::metrics", "runtime metrics: logging each active second");
            let mut last = runtime.read_metrics();
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                let now = runtime.read_metrics();
                let delta = now.since(&last);
                last = now;
                if delta.script_renders() == 0 && delta.materializations() == 0 {
                    continue;
                }
                tracing::info!(
                    target: "bir_shell::metrics",
                    "script renders {} (mean {:.2} ms, slowest {:.2} ms, host calls {:.2} ms) | materializations {} (mean {:.2} ms, total {:.1} ms)",
                    delta.script_renders(),
                    ms(delta.mean_script_render()),
                    ms(delta.slowest_script_render()),
                    ms(delta.mean_native()),
                    delta.materializations(),
                    ms(delta.mean_materialize()),
                    ms(delta.materialize_time()),
                );
            }
        })
        .detach();
}

fn ms(duration: std::time::Duration) -> f64 {
    duration.as_secs_f64() * 1000.0
}

/// Development aid for side-by-side checks against the design prototype:
/// `BIR_SHELL_WINDOW_SIZE=800x900` sizes the window. A tiling window manager
/// may re-tile it when it opens, so the size is re-applied for a few seconds
/// (long enough to float the window there).
fn requested_window_size() -> Option<Size<Pixels>> {
    let raw = std::env::var("BIR_SHELL_WINDOW_SIZE").ok()?;
    let (w, h) = raw.split_once('x')?;
    Some(size(px(w.trim().parse().ok()?), px(h.trim().parse().ok()?)))
}

fn hold_window_size(window: &mut Window, cx: &mut App, target: Size<Pixels>) {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir().join(format!(
                "bir-shell-host-{}-{}",
                std::process::id(),
                name
            ));
            let _ = fs::remove_dir_all(&root);
            fs::create_dir_all(&root).expect("temp tree");
            // The resolver canonicalizes the executable path, so compare
            // against the resolved root (/var -> /private/var on macOS).
            let root = root.canonicalize().unwrap_or(root);
            Self(root)
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn env_override_wins_over_every_layout() {
        let tree = Tree::new("env-wins");
        let packaged_app = tree.0.join("install-dir/app");
        fs::create_dir_all(&packaged_app).expect("packaged app");
        fs::write(packaged_app.join("main.js"), "// entry").expect("entry");
        let exe = tree.0.join("install-dir/bir");
        fs::write(&exe, "").expect("exe");

        let chosen = PathBuf::from("/override/app");
        assert_eq!(
            app_root_from(Some(chosen.clone().into_os_string()), Some(exe)),
            chosen
        );
    }

    #[test]
    fn a_packaged_app_beside_the_executable_is_used() {
        let tree = Tree::new("packaged-app");
        let app = tree.0.join("eBIRForms.app/Contents/Resources/app");
        fs::create_dir_all(&app).expect("bundled app");
        fs::write(app.join("main.js"), "// entry").expect("entry");
        let exe = tree.0.join("eBIRForms.app/Contents/MacOS/bir");
        fs::create_dir_all(exe.parent().unwrap()).expect("MacOS dir");
        fs::write(&exe, "").expect("exe");

        assert_eq!(app_root_from(None, Some(exe)), app);
    }

    #[cfg(debug_assertions)]
    #[test]
    fn nothing_packaged_falls_back_to_the_checkout() {
        let tree = Tree::new("dev-fallback");
        let exe = tree.0.join("install-dir/bir");
        fs::create_dir_all(exe.parent().unwrap()).expect("exe dir");
        fs::write(&exe, "").expect("exe");

        let expected = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../app");
        assert_eq!(app_root_from(None, Some(exe)), expected);
        assert!(expected.join("main.js").is_file());
    }
}
