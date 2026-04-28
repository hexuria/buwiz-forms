#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

//! eBIRForms Background Daemon — Menu Bar Tray App
//!
//! Runs as a persistent macOS menu bar agent (no Dock icon).
//! Provides:
//!   - System tray icon with menu (Open App, Start/Stop Cron, Quit)
//!   - Background cron job execution in a Tokio runtime
//!   - Notification click → opens the main eBIRForms app

use bir_core::db::Database;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tao::{
    event::Event,
    event_loop::{ControlFlow, EventLoopBuilder},
};
use tray_icon::{
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
    TrayIconBuilder, TrayIconEvent,
};
use tracing::{error, info};

enum UserEvent {
    TrayIconEvent(tray_icon::TrayIconEvent),
    MenuEvent(tray_icon::menu::MenuEvent),
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();
    info!("Starting eBIRForms background daemon (tray)...");

    // ── Database ─────────────────────────────────────────────────────────
    let db_path = bir_core::db::default_database_path();
    if let Some(parent) = db_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let (db, _recovered) = match Database::open_or_recreate(&db_path) {
        Ok(db) => db,
        Err(e) => {
            error!(
                "Daemon failed to open database at {}: {}",
                db_path.display(),
                e
            );
            std::process::exit(1);
        }
    };
    let db = Arc::new(Mutex::new(db));

    // ── Cron control flag ────────────────────────────────────────────────
    let cron_running = Arc::new(AtomicBool::new(true));

    // ── Spawn Tokio cron loop on a background thread ─────────────────────
    {
        let db = db.clone();
        let cron_running = cron_running.clone();
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("Failed to build tokio runtime");

            rt.block_on(async move {
                loop {
                    tokio::time::sleep(std::time::Duration::from_secs(60)).await;

                    if !cron_running.load(Ordering::Relaxed) {
                        continue; // paused
                    }

                    // Run one iteration of the cron tick
                    bir_core::background_cron::run_cron_tick(db.clone()).await;
                }
            });
        });
    }

    // ── macOS: hide from Dock ────────────────────────────────────────────
    #[cfg(target_os = "macos")]
    {
        // Must be set before the event loop is created/run.
        // Accessory = shows in menu bar only, no Dock icon.
        // NOTE: tao's EventLoopBuilder doesn't expose set_activation_policy
        // so we use the Cocoa API directly.
        unsafe {
            use std::ffi::c_void;
            #[link(name = "AppKit", kind = "framework")]
            unsafe extern "C" {
                fn NSApplicationLoad() -> bool;
            }
            #[link(name = "objc", kind = "dylib")]
            unsafe extern "C" {
                fn objc_getClass(name: *const u8) -> *mut c_void;
                fn sel_registerName(name: *const u8) -> *mut c_void;
                fn objc_msgSend(receiver: *mut c_void, sel: *mut c_void, ...) -> *mut c_void;
            }
            NSApplicationLoad();
            let ns_app_class = objc_getClass(b"NSApplication\0".as_ptr());
            let shared_app_sel = sel_registerName(b"sharedApplication\0".as_ptr());
            let app = objc_msgSend(ns_app_class, shared_app_sel);
            let set_policy_sel = sel_registerName(b"setActivationPolicy:\0".as_ptr());
            // 1 = NSApplicationActivationPolicyAccessory (no Dock icon)
            objc_msgSend(app, set_policy_sel, 1i64);
        }
    }

    // ── Event Loop ───────────────────────────────────────────────────────
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();

    // Forward tray events to the event loop
    let proxy = event_loop.create_proxy();
    TrayIconEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::TrayIconEvent(event));
    }));

    // Forward menu events to the event loop
    let proxy = event_loop.create_proxy();
    MenuEvent::set_event_handler(Some(move |event| {
        let _ = proxy.send_event(UserEvent::MenuEvent(event));
    }));

    // ── Menu ─────────────────────────────────────────────────────────────
    let tray_menu = Menu::new();
    let open_app_i = MenuItem::new("Open eBIRForms", true, None);
    let start_cron_i = MenuItem::new("Start Cron", true, None);
    let stop_cron_i = MenuItem::new("Stop Cron", true, None);
    let quit_i = MenuItem::new("Quit", true, None);

    let _ = tray_menu.append_items(&[
        &open_app_i,
        &PredefinedMenuItem::separator(),
        &start_cron_i,
        &stop_cron_i,
        &PredefinedMenuItem::separator(),
        &quit_i,
    ]);

    // ── Tray Icon ────────────────────────────────────────────────────────
    let mut tray_icon = None;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;

        match event {
            Event::NewEvents(tao::event::StartCause::Init) => {
                let icon = load_tray_icon();

                tray_icon = Some(
                    TrayIconBuilder::new()
                        .with_menu(Box::new(tray_menu.clone()))
                        .with_tooltip("eBIRForms — Background Services")
                        .with_icon(icon)
                        .build()
                        .unwrap(),
                );

                // Wake up the macOS run loop so the icon appears immediately
                #[cfg(target_os = "macos")]
                {
                    unsafe extern "C" {
                        fn CFRunLoopGetMain() -> *const std::ffi::c_void;
                        fn CFRunLoopWakeUp(rl: *const std::ffi::c_void);
                    }
                    unsafe {
                        let rl = CFRunLoopGetMain();
                        if !rl.is_null() {
                            CFRunLoopWakeUp(rl);
                        }
                    }
                }

                info!("Tray icon created successfully");
            }

            Event::UserEvent(UserEvent::MenuEvent(event)) => {
                if event.id == open_app_i.id() {
                    open_main_app();
                } else if event.id == start_cron_i.id() {
                    info!("Cron jobs started via tray menu");
                    cron_running.store(true, Ordering::Relaxed);
                } else if event.id == stop_cron_i.id() {
                    info!("Cron jobs stopped via tray menu");
                    cron_running.store(false, Ordering::Relaxed);
                } else if event.id == quit_i.id() {
                    info!("Quit requested via tray menu");
                    tray_icon.take();
                    *control_flow = ControlFlow::Exit;
                }
            }

            Event::UserEvent(UserEvent::TrayIconEvent(event)) => {
                // Double-click on the tray icon opens the main app
                if let tray_icon::TrayIconEvent::DoubleClick { .. } = event {
                    open_main_app();
                }
            }

            _ => {}
        }
    });
}

/// Open the main eBIRForms application.
fn open_main_app() {
    info!("Opening main eBIRForms app...");
    #[cfg(target_os = "macos")]
    {
        // Use `open -a` to launch by app name. This ensures macOS launches
        // the main `bir` executable (CFBundleExecutable) rather than
        // potentially activating the daemon process.
        // Try the installed path first, then fall back to app name.
        let exe = std::env::current_exe().unwrap_or_default();
        // bir-daemon lives at eBIRForms.app/Contents/MacOS/bir-daemon
        // so the .app bundle is 3 levels up
        let app_bundle = exe
            .parent() // MacOS/
            .and_then(|p| p.parent()) // Contents/
            .and_then(|p| p.parent()); // eBIRForms.app/

        if let Some(bundle) = app_bundle {
            if bundle.extension().map(|e| e == "app").unwrap_or(false) {
                let _ = std::process::Command::new("open")
                    .arg(bundle)
                    .spawn();
                return;
            }
        }
        // Fallback: open by name
        let _ = std::process::Command::new("open")
            .arg("-a")
            .arg("eBIRForms")
            .spawn();
    }
    #[cfg(not(target_os = "macos"))]
    {
        // On Linux/Windows, try to find and run the bir binary
        if let Ok(exe) = std::env::current_exe() {
            let main_exe = exe.parent().unwrap().join("bir");
            let _ = std::process::Command::new(main_exe).spawn();
        }
    }
}

/// Load the tray icon from the assets directory.
fn load_tray_icon() -> tray_icon::Icon {
    // Try to load from the app bundle first, then fall back to cargo manifest path
    let icon_path = if let Ok(exe) = std::env::current_exe() {
        let bundle_path = exe
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .join("Resources/assets/tray-icon.png");
        if bundle_path.exists() {
            bundle_path
        } else {
            // Fallback for `cargo run`
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/tray-icon.png")
        }
    } else {
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets/tray-icon.png")
    };

    if icon_path.exists() {
        let img = image::open(&icon_path)
            .expect("Failed to open tray icon")
            .into_rgba8();
        let (w, h) = img.dimensions();
        tray_icon::Icon::from_rgba(img.into_raw(), w, h).expect("Failed to create tray icon")
    } else {
        // Generate a minimal fallback icon (16x16 blue square)
        info!("Tray icon not found at {:?}, using fallback", icon_path);
        let size = 16u32;
        let mut rgba = vec![0u8; (size * size * 4) as usize];
        for pixel in rgba.chunks_exact_mut(4) {
            pixel[0] = 0x1E; // R
            pixel[1] = 0x40; // G
            pixel[2] = 0xAF; // B
            pixel[3] = 0xFF; // A
        }
        tray_icon::Icon::from_rgba(rgba, size, size).expect("Failed to create fallback icon")
    }
}
