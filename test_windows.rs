#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM, BOOL};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{EnumWindows, GetWindowThreadProcessId, ShowWindow, SW_HIDE, SW_SHOW};
#[cfg(target_os = "windows")]
use windows::Win32::System::Threading::GetCurrentProcessId;

fn main() {
    println!("Testing windows crate");
}
