//! Windows half of `os_login`: Windows Hello through `UserConsentVerifier`, with the prompt owned by the app's window.
//!
//! PLAN.md §E.3 and OS-LOGIN-INTERFACE.md "Windows details":
//! - Availability first. Only `Available` gives `WindowsHello`, and only `Available` at the moment of the request
//!   prompts. Asking without it can hang (robius-authentication's own note).
//! - The prompt's owner is the app's window, found without the kit (`lock_window`). No window, no prompt.
//! - Never a password box. Where Hello is not set up the answer is `None` / `Unavailable`, and the caller's page
//!   shows its own path (the support sentence on the legacy unlock).
//! - Each call runs on the caller's own fresh thread in the multithreaded apartment. A thread that is already
//!   single-threaded (STA) answers `None` / `Unavailable` without touching WinRT: blocking on an IAsyncOperation
//!   there, with no message pump, can hang.
//!
//! The rules that need no OS (`pick_window`, `map_result`, `decide`) are plain functions over plain values, tested on
//! every OS. The OS calls are in `os` below, Windows only.

#[cfg(windows)]
use super::OwnerCheck;
use super::OwnerCheckResult;

/// The window class of every NSH window (nsh-platform-windows `window.rs`).
const NSH_WINDOW_CLASS: &str = "NshWindow";

/// One top-level window as `lock_window` sees it.
#[derive(Clone, Debug, PartialEq, Eq)]
struct WindowRecord {
    hwnd: isize,
    pid: u32,
    class: String,
    visible: bool,
    /// It has an owner window (a dialog, a panel). The app's own main window has none.
    owned: bool,
    /// Its title, read only for our own NSH windows (empty for every other window).
    title: String,
}

/// Which window owns the prompt (§E.3 point 2). A candidate is ours, an NSH window, visible, unowned, and titled
/// exactly `expected_title` (the main window's; the print preview is also an unowned NshWindow, titled with the
/// taxpayer, and must never own the sheet):
/// - the foreground window, when it is a candidate;
/// - otherwise our one candidate, only when exactly one matches;
/// - otherwise none, and the caller does not prompt.
fn pick_window(
    foreground: Option<&WindowRecord>,
    all: &[WindowRecord],
    our_pid: u32,
    expected_title: &str,
) -> Option<isize> {
    let candidate = |window: &WindowRecord| {
        window.pid == our_pid
            && window.class == NSH_WINDOW_CLASS
            && window.visible
            && !window.owned
            && window.title == expected_title
    };
    if let Some(front) = foreground.filter(|front| candidate(front)) {
        return Some(front.hwnd);
    }
    let mut candidates = all.iter().filter(|window| candidate(window));
    match (candidates.next(), candidates.next()) {
        (Some(only), None) => Some(only.hwnd),
        _ => None,
    }
}

// `UserConsentVerifierAvailability` and `UserConsentVerificationResult` values (Windows.Security.Credentials.UI),
// as plain numbers so the rules below are tested off Windows too. A Windows test checks them against the crate.
const AVAILABLE: i32 = 0;
const VERIFIED: i32 = 0;
const RETRIES_EXHAUSTED: i32 = 5;
const CANCELED: i32 = 6;

/// One verification result as the page's answer. `DeviceBusy`, `DeviceNotPresent`, `NotConfiguredForUser`,
/// `DisabledByPolicy` and anything unknown are `Unavailable`: the PIN or the page's own path stays.
fn map_result(result: i32) -> OwnerCheckResult {
    match result {
        VERIFIED => OwnerCheckResult::Verified,
        CANCELED => OwnerCheckResult::Cancelled,
        RETRIES_EXHAUSTED => OwnerCheckResult::LockedOut,
        _ => OwnerCheckResult::Unavailable,
    }
}

/// The order of one `prove_owner`, apart from the OS: availability, then the window, then the prompt. "Not
/// available" and "no window" never reach `ask`.
fn decide(
    available: impl FnOnce() -> bool,
    window: impl FnOnce() -> Option<isize>,
    ask: impl FnOnce(isize) -> OwnerCheckResult,
) -> OwnerCheckResult {
    if !available() {
        return OwnerCheckResult::Unavailable;
    }
    match window() {
        Some(hwnd) => ask(hwnd),
        None => OwnerCheckResult::Unavailable,
    }
}

#[cfg(windows)]
pub(super) fn owner_check() -> OwnerCheck {
    match os::Apartment::enter() {
        Some(_apartment) if os::hello_available() => OwnerCheck::WindowsHello,
        _ => OwnerCheck::None,
    }
}

#[cfg(windows)]
pub(super) fn prove_owner(reason: &str, window_title: &str) -> OwnerCheckResult {
    let Some(_apartment) = os::Apartment::enter() else {
        return OwnerCheckResult::Unavailable;
    };
    decide(
        os::hello_available,
        || os::lock_window(window_title),
        |hwnd| os::request(hwnd, reason),
    )
}

#[cfg(windows)]
mod os {
    use ::windows::Security::Credentials::UI::{
        UserConsentVerificationResult, UserConsentVerifier,
    };
    use ::windows::Win32::Foundation::{HWND, LPARAM};
    use ::windows::Win32::System::Threading::GetCurrentProcessId;
    use ::windows::Win32::System::WinRT::{
        IUserConsentVerifierInterop, RO_INIT_MULTITHREADED, RoInitialize, RoUninitialize,
    };
    use ::windows::Win32::UI::WindowsAndMessaging::{
        EnumWindows, GW_OWNER, GetClassNameW, GetForegroundWindow, GetWindow,
        GetWindowThreadProcessId, InternalGetWindowText, IsWindowVisible,
    };
    use ::windows::core::{BOOL, HSTRING, factory};
    use windows_future::IAsyncOperation;

    use super::{
        AVAILABLE, NSH_WINDOW_CLASS, OwnerCheckResult, WindowRecord, map_result, pick_window,
    };

    /// This thread in the multithreaded apartment for the life of the value. `None` when the thread is already
    /// single-threaded (`RPC_E_CHANGED_MODE`) or WinRT cannot start: the caller then answers without WinRT.
    /// Not `Send`: it must be dropped on the thread that entered.
    pub(super) struct Apartment(std::marker::PhantomData<*const ()>);

    impl Apartment {
        pub(super) fn enter() -> Option<Self> {
            // SAFETY: RoInitialize on the calling thread; every success (S_OK or S_FALSE) is balanced by the Drop.
            match unsafe { RoInitialize(RO_INIT_MULTITHREADED) } {
                Ok(()) => Some(Self(std::marker::PhantomData)),
                Err(error) => {
                    tracing::warn!(code = ?error.code(), "os_login: no multithreaded apartment on this thread");
                    None
                }
            }
        }
    }

    impl Drop for Apartment {
        fn drop(&mut self) {
            // SAFETY: balances the successful RoInitialize in `enter`, on the same thread (the value is not Send).
            unsafe { RoUninitialize() };
        }
    }

    /// `CheckAvailabilityAsync` answered `Available`. Any error is "not available".
    pub(super) fn hello_available() -> bool {
        UserConsentVerifier::CheckAvailabilityAsync()
            .and_then(|operation| operation.join())
            .is_ok_and(|availability| availability.0 == AVAILABLE)
    }

    /// The Hello prompt, owned by `hwnd`, with `reason` as its text. Blocks until the person answers.
    pub(super) fn request(hwnd: isize, reason: &str) -> OwnerCheckResult {
        let asked =
            factory::<UserConsentVerifier, IUserConsentVerifierInterop>().and_then(|interop| {
                // SAFETY: a live top-level window handle of this process (from `lock_window`) and a valid HSTRING.
                let operation: IAsyncOperation<UserConsentVerificationResult> = unsafe {
                    interop.RequestVerificationForWindowAsync(
                        HWND(hwnd as *mut _),
                        &HSTRING::from(reason),
                    )
                }?;
                operation.join()
            });
        match asked {
            Ok(result) => map_result(result.0),
            Err(error) => {
                tracing::warn!(code = ?error.code(), "os_login: the Hello request failed");
                OwnerCheckResult::Unavailable
            }
        }
    }

    /// The app's window for the prompt, titled `expected_title`: the foreground window if it qualifies, else the one
    /// match of `EnumWindows`.
    pub(super) fn lock_window(expected_title: &str) -> Option<isize> {
        // SAFETY: plain queries.
        let our_pid = unsafe { GetCurrentProcessId() };
        let foreground = unsafe { GetForegroundWindow() };
        let foreground = (!foreground.is_invalid()).then(|| record(foreground, our_pid));
        let mut all = (our_pid, Vec::<WindowRecord>::new());
        // SAFETY: the callback only reads each window and pushes into `all`, which outlives the call.
        let _ = unsafe {
            EnumWindows(
                Some(collect),
                LPARAM(&mut all as *mut (u32, Vec<WindowRecord>) as isize),
            )
        };
        pick_window(foreground.as_ref(), &all.1, our_pid, expected_title)
    }

    unsafe extern "system" fn collect(hwnd: HWND, all: LPARAM) -> BOOL {
        // SAFETY: `all` is the `&mut (pid, Vec)` passed by `lock_window` for the duration of EnumWindows.
        let (our_pid, all) = unsafe { &mut *(all.0 as *mut (u32, Vec<WindowRecord>)) };
        all.push(record(hwnd, *our_pid));
        BOOL(1)
    }

    /// One window's record. Its title is read only when it is one of our NSH windows: another process's window is
    /// never asked for its text.
    fn record(hwnd: HWND, our_pid: u32) -> WindowRecord {
        let mut pid = 0u32;
        let mut class = [0u16; 64];
        // SAFETY: plain queries on a window handle; GetClassNameW writes at most `class.len()` units.
        let (length, visible, owner) = unsafe {
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            (
                GetClassNameW(hwnd, &mut class),
                IsWindowVisible(hwnd).as_bool(),
                GetWindow(hwnd, GW_OWNER),
            )
        };
        let class = String::from_utf16_lossy(&class[..usize::try_from(length).unwrap_or(0)]);
        let title = if pid == our_pid && class == NSH_WINDOW_CLASS {
            let mut text = [0u16; 256];
            // InternalGetWindowText reads the stored title without a WM_GETTEXT round trip to the UI thread, so a busy
            // UI thread never holds this up.
            // SAFETY: it writes at most `text.len()` units into the buffer.
            let length = unsafe { InternalGetWindowText(hwnd, &mut text) };
            String::from_utf16_lossy(&text[..usize::try_from(length).unwrap_or(0)])
        } else {
            String::new()
        };
        WindowRecord {
            hwnd: hwnd.0 as isize,
            pid,
            class,
            visible,
            owned: owner.is_ok_and(|owner| !owner.is_invalid()),
            title,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OURS: u32 = 4242;
    const PREVIEW_TITLE: &str = "BIR Form 2551Q · Q4 2026 — Juana Santos";

    /// The main window's title, from the build (nsh.json `window.title`), never a copy of the string.
    fn main_title() -> &'static str {
        super::super::APP_WINDOW_TITLE
    }

    fn window(
        hwnd: isize,
        pid: u32,
        class: &str,
        visible: bool,
        owned: bool,
        title: &str,
    ) -> WindowRecord {
        WindowRecord {
            hwnd,
            pid,
            class: class.to_owned(),
            visible,
            owned,
            title: title.to_owned(),
        }
    }

    fn pick(foreground: Option<&WindowRecord>, all: &[WindowRecord]) -> Option<isize> {
        pick_window(foreground, all, OURS, main_title())
    }

    #[test]
    fn windows_handle_is_ours_or_none() {
        let main = window(10, OURS, NSH_WINDOW_CLASS, true, false, main_title());
        let other_app = window(20, 7, "Chrome_WidgetWin_1", true, false, "");
        let other_nsh = window(30, 7, NSH_WINDOW_CLASS, true, false, main_title());

        // The foreground window is ours: taken.
        assert_eq!(
            pick(Some(&main), &[main.clone(), other_app.clone()]),
            Some(10)
        );
        // The foreground window is another process's (even another eBIRForms): our one window from the list.
        assert_eq!(
            pick(Some(&other_app), &[other_app.clone(), main.clone()]),
            Some(10)
        );
        assert_eq!(
            pick(Some(&other_nsh), &[other_nsh.clone(), main.clone()]),
            Some(10)
        );
        // Two of ours that both qualify, neither in front: none.
        let second_main = window(11, OURS, NSH_WINDOW_CLASS, true, false, main_title());
        assert_eq!(pick(Some(&other_app), &[main.clone(), second_main]), None);
        // An owned window (a dialog of ours) and a hidden one are skipped, in front too.
        let dialog = window(12, OURS, NSH_WINDOW_CLASS, true, true, main_title());
        let hidden = window(13, OURS, NSH_WINDOW_CLASS, false, false, main_title());
        assert_eq!(
            pick(Some(&dialog), &[dialog.clone(), main.clone()]),
            Some(10)
        );
        assert_eq!(
            pick(Some(&hidden), &[hidden.clone(), main.clone()]),
            Some(10)
        );
        assert_eq!(pick(None, &[dialog, hidden, main.clone()]), Some(10));
        // Our process but not an NSH window: not taken.
        let ours_other_class = window(14, OURS, "#32770", true, false, main_title());
        assert_eq!(
            pick(
                Some(&ours_other_class),
                std::slice::from_ref(&ours_other_class)
            ),
            None
        );
        // Nothing: none.
        assert_eq!(pick(None, &[]), None);
        assert_eq!(
            pick(Some(&other_app), std::slice::from_ref(&other_app)),
            None
        );
    }

    /// The print preview is its own visible, unowned NshWindow, titled with the taxpayer (#398's window list,
    /// try/pin-save-panel-398 enumwin-pass-a.json). It never owns the sheet, in front or in the list.
    #[test]
    fn a_preview_in_front_never_owns_the_sheet() {
        let main = window(10, OURS, NSH_WINDOW_CLASS, true, false, main_title());
        let preview = window(11, OURS, NSH_WINDOW_CLASS, true, false, PREVIEW_TITLE);
        // In front: the list decides, and the main window matches by title.
        assert_eq!(
            pick(Some(&preview), &[preview.clone(), main.clone()]),
            Some(10)
        );
        // In the list beside the main window: it does not count, so the main window is the one match.
        assert_eq!(pick(None, &[main.clone(), preview.clone()]), Some(10));
        // Only the preview: no window has the title, so none and no prompt.
        assert_eq!(pick(Some(&preview), std::slice::from_ref(&preview)), None);
        // A title that only starts with the main title is not it.
        let near_title = format!("{} - Juana Santos", main_title());
        let near = window(15, OURS, NSH_WINDOW_CLASS, true, false, &near_title);
        assert_eq!(pick(Some(&near), std::slice::from_ref(&near)), None);
    }

    #[test]
    fn each_verification_result_maps_to_one_answer() {
        assert_eq!(map_result(0), OwnerCheckResult::Verified); // Verified
        assert_eq!(map_result(6), OwnerCheckResult::Cancelled); // Canceled
        assert_eq!(map_result(5), OwnerCheckResult::LockedOut); // RetriesExhausted
        for stays_on_the_page in [1, 2, 3, 4, 99, -1] {
            // DeviceNotPresent, NotConfiguredForUser, DisabledByPolicy, DeviceBusy, anything unknown.
            assert_eq!(map_result(stays_on_the_page), OwnerCheckResult::Unavailable);
        }
    }

    #[test]
    fn not_available_or_no_window_never_asks() {
        let mut asked = false;
        assert_eq!(
            decide(
                || false,
                || Some(10),
                |_| {
                    asked = true;
                    OwnerCheckResult::Verified
                }
            ),
            OwnerCheckResult::Unavailable
        );
        assert_eq!(
            decide(
                || true,
                || None,
                |_| {
                    asked = true;
                    OwnerCheckResult::Verified
                }
            ),
            OwnerCheckResult::Unavailable
        );
        assert!(!asked);
    }

    #[test]
    fn available_with_a_window_asks_with_that_window() {
        assert_eq!(
            decide(
                || true,
                || Some(10),
                |hwnd| map_result(if hwnd == 10 { 0 } else { 6 })
            ),
            OwnerCheckResult::Verified
        );
    }

    #[cfg(windows)]
    #[test]
    fn the_numbers_match_the_windows_crate() {
        use ::windows::Security::Credentials::UI::{
            UserConsentVerificationResult, UserConsentVerifierAvailability,
        };
        assert_eq!(UserConsentVerifierAvailability::Available.0, AVAILABLE);
        assert_eq!(UserConsentVerificationResult::Verified.0, VERIFIED);
        assert_eq!(
            UserConsentVerificationResult::RetriesExhausted.0,
            RETRIES_EXHAUSTED
        );
        assert_eq!(UserConsentVerificationResult::Canceled.0, CANCELED);
        // DeviceBusy stays on the page (Unavailable): pinned so a renumbering would show.
        assert_eq!(UserConsentVerificationResult::DeviceBusy.0, 4);
        assert_eq!(
            map_result(UserConsentVerificationResult::DeviceBusy.0),
            OwnerCheckResult::Unavailable
        );
    }

    /// `lock_window` on real windows: one visible, unowned `NshWindow` of this process with the expected title is
    /// found; one with another title (as the print preview has) never is; a second one with the title makes the
    /// answer none unless it is in front. The windows sit off screen and are destroyed at the end.
    #[cfg(windows)]
    #[test]
    fn lock_window_finds_our_one_nsh_window_and_refuses_two() {
        use ::windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
        use ::windows::Win32::System::LibraryLoader::GetModuleHandleW;
        use ::windows::Win32::UI::WindowsAndMessaging::{
            CreateWindowExW, DefWindowProcW, DestroyWindow, RegisterClassW, WINDOW_EX_STYLE,
            WNDCLASSW, WS_OVERLAPPEDWINDOW, WS_VISIBLE,
        };
        use ::windows::core::{HSTRING, PCWSTR};

        unsafe extern "system" fn procedure(
            hwnd: HWND,
            message: u32,
            wparam: WPARAM,
            lparam: LPARAM,
        ) -> LRESULT {
            // SAFETY: the default handling for a window of this test.
            unsafe { DefWindowProcW(hwnd, message, wparam, lparam) }
        }

        let class = HSTRING::from(NSH_WINDOW_CLASS);
        let title = HSTRING::from("os_login test window");
        let other_title = HSTRING::from("os_login test preview");
        // SAFETY: a class and three windows of this test process, destroyed before it returns.
        unsafe {
            let instance = GetModuleHandleW(PCWSTR::null())
                .expect("module handle")
                .into();
            let description = WNDCLASSW {
                lpfnWndProc: Some(procedure),
                hInstance: instance,
                lpszClassName: PCWSTR(class.as_ptr()),
                ..Default::default()
            };
            assert_ne!(
                RegisterClassW(&description),
                0,
                "the test registers an NshWindow class"
            );
            let make = |title: &HSTRING| {
                CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    PCWSTR(class.as_ptr()),
                    PCWSTR(title.as_ptr()),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    -32000,
                    -32000,
                    200,
                    100,
                    None,
                    None,
                    Some(instance),
                    None,
                )
                .expect("a test window")
            };
            let first = make(&title);
            // Our NshWindow with another title (as the print preview is) never qualifies, in front or not.
            let preview = make(&other_title);
            assert_eq!(
                os::lock_window("os_login test window"),
                Some(first.0 as isize)
            );
            assert_eq!(os::lock_window("no window has this title"), None);
            let second = make(&title);
            let both = os::lock_window("os_login test window");
            // Creating a window may bring it to the front; then the front one is the answer, by rule.
            let front = ::windows::Win32::UI::WindowsAndMessaging::GetForegroundWindow();
            let expected = [first, second]
                .into_iter()
                .find(|window| *window == front)
                .map(|window| window.0 as isize);
            let _ = DestroyWindow(second);
            let _ = DestroyWindow(preview);
            let _ = DestroyWindow(first);
            assert_eq!(
                both, expected,
                "two of our NSH windows: the one in front, or none"
            );
        }
    }

    /// Real OS calls, outside the test-environment rule (this module has none of its own). Ignored in CI; run on a
    /// Devin Windows machine with `cargo test -p <app crate> os_login -- --ignored --nocapture`. On Server 2022 (no
    /// Hello) it prints `None` and `Unavailable`, with no prompt and no hang. Where Hello IS set up it prompts.
    #[cfg(windows)]
    #[test]
    #[ignore = "real OS calls; run by hand on a Windows machine"]
    fn real_windows_answers() {
        let started = std::time::Instant::now();
        let check = owner_check();
        let checked_in = started.elapsed();
        let window = os::lock_window(main_title());
        let proved = prove_owner("move your tax profiles to your Buwiz account", main_title());
        println!(
            "owner_check={check:?} ({checked_in:?}) lock_window={window:?} prove_owner={proved:?} total {:?}",
            started.elapsed()
        );
        if check == OwnerCheck::None {
            assert_eq!(
                proved,
                OwnerCheckResult::Unavailable,
                "no Hello: never a prompt"
            );
        }
    }

    /// A thread that is already single-threaded answers without WinRT, at once.
    #[cfg(windows)]
    #[test]
    fn an_sta_thread_answers_none_without_winrt() {
        use ::windows::Win32::System::Com::{
            COINIT_APARTMENTTHREADED, CoInitializeEx, CoUninitialize,
        };
        std::thread::spawn(|| {
            // SAFETY: this fresh thread becomes STA, and is uninitialized before it ends.
            unsafe {
                CoInitializeEx(None, COINIT_APARTMENTTHREADED)
                    .ok()
                    .expect("STA")
            };
            let check = owner_check();
            let proved = prove_owner("move your tax profiles to your Buwiz account", main_title());
            unsafe { CoUninitialize() };
            assert_eq!(check, OwnerCheck::None);
            assert_eq!(proved, OwnerCheckResult::Unavailable);
        })
        .join()
        .expect("the STA thread");
    }
}
