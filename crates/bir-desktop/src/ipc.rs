use directories::ProjectDirs;
use gpui::App;
use smol::net::UdpSocket;
use std::net::UdpSocket as StdUdpSocket;
use std::time::Duration;

fn get_port_file_path() -> Option<std::path::PathBuf> {
    if let Some(proj_dirs) = ProjectDirs::from("com", "goldcoders", "eBIRForms") {
        let data_dir = proj_dirs.data_local_dir();
        if !data_dir.exists() {
            let _ = std::fs::create_dir_all(data_dir);
        }
        Some(data_dir.join("instance.port"))
    } else {
        None
    }
}

/// Checks if another instance is running. If yes, pings it and exits the process.
/// This runs synchronously BEFORE GPUI starts.
pub fn prevent_multiple_instances() {
    let port_file = match get_port_file_path() {
        Some(path) => path,
        None => return,
    };

    if let Ok(port_str) = std::fs::read_to_string(&port_file) {
        if let Ok(port) = port_str.trim().parse::<u16>() {
            if let Ok(socket) = StdUdpSocket::bind("127.0.0.1:0") {
                #[cfg(target_os = "windows")]
                {
                    unsafe {
                        let _ = windows::Win32::UI::WindowsAndMessaging::AllowSetForegroundWindow(
                            0xFFFFFFFF,
                        );
                    }
                }
                let _ = socket.set_read_timeout(Some(Duration::from_millis(100)));
                let _ = socket.send_to(b"SHOW", format!("127.0.0.1:{}", port));
                let mut buf = [0; 4];
                if let Ok(_) = socket.recv_from(&mut buf) {
                    // Received ACK, another instance is running.
                    std::process::exit(0);
                }
            }
        }
    }
}

/// Starts the background listener for IPC requests to show the app.
pub fn start_ipc_listener(cx: &mut App) {
    let port_file = match get_port_file_path() {
        Some(path) => path,
        None => return,
    };

    cx.spawn(async move |cx| {
        if let Ok(socket) = UdpSocket::bind("127.0.0.1:0").await {
            if let Ok(addr) = socket.local_addr() {
                let _ = std::fs::write(&port_file, addr.port().to_string());
                let mut buf = [0; 1024];
                while let Ok((len, src)) = socket.recv_from(&mut buf).await {
                    if &buf[..len] == b"SHOW" {
                        let _ = socket.send_to(b"ACK", src).await;
                        let _ = cx.update(|cx| {
                            crate::platform::show_in_dock();
                            cx.activate(true);
                        });
                    } else if &buf[..len] == b"TOGGLE" {
                        let _ = socket.send_to(b"ACK", src).await;
                        let _ = cx.update(|cx| {
                            crate::platform::toggle_app_visibility();
                        });
                    }
                }
            }
        }
    })
    .detach();
}
