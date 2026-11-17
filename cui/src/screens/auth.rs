//! The account gate, first-run setup and the sign-in screen (port of `nsh/screens/auth/`).
//! State lives here, one owner; the frame's session changes only through `AppState`.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use serde_json::{Value, json};

use crate::bir;
use crate::lib::errors::error_text;
use crate::state::{AppState, Session, ViewName, ViewParams};
use crate::store;
use crate::ui::kit::UiExt;
use cui::{Align, Justify, Size, Ui};

// ---------------------------------------------------------------- logic (logic.ts)

/// The account gate's line before anything happened.
pub const GATE_DEFAULT: &str = "Sign in with your Buwiz account to file from this computer.";

/// Whose TIN `tin` (digits) is: the admin's root, a taxpayer's root among `roots`, or why neither.
pub enum SignInTarget {
    Error(String),
    Root { root: String, admin: bool },
}

pub fn sign_in_target(tin: &str, admin_root: Option<&str>, roots: &[String]) -> SignInTarget {
    if tin.len() != 9 {
        return SignInTarget::Error("Type all 9 digits of the TIN.".to_owned());
    }
    if Some(tin) == admin_root {
        return SignInTarget::Root {
            root: tin.to_owned(),
            admin: true,
        };
    }
    if !roots.iter().any(|r| r == tin) {
        return SignInTarget::Error("No taxpayer has that TIN on this computer.".to_owned());
    }
    SignInTarget::Root {
        root: tin.to_owned(),
        admin: false,
    }
}

/// The line under "Sign in": what to type, for the method and the "Forgot PIN?" path.
pub fn sign_in_line(forgot: bool, for_admin: bool, helpdesk: bool) -> &'static str {
    if forgot {
        if for_admin {
            "Use your recovery key, or your Mac login."
        } else {
            "The admin can open your account with their PIN."
        }
    } else if helpdesk {
        "Enter your TIN and your PIN. The help desk admin signs in with TIN 000-000-000."
    } else {
        "Enter your TIN and your PIN."
    }
}

/// Why the code as typed cannot be checked yet; `None` when it is complete.
pub fn code_length_error(totp: bool, code: &str) -> Option<&'static str> {
    if code.len() == if totp { 6 } else { 4 } {
        return None;
    }
    Some(if totp {
        "Type all 6 digits of the code."
    } else {
        "Type all 4 digits of the PIN."
    })
}

/// The answer to a sign-in code that does not match.
pub fn wrong_code(totp: bool, taxpayer_has_no_code: bool) -> String {
    if taxpayer_has_no_code {
        "That taxpayer has no PIN yet. Ask the admin to open it.".to_owned()
    } else if totp {
        "Wrong TIN or code. Try again.".to_owned()
    } else {
        "Wrong TIN or PIN. Try again.".to_owned()
    }
}

/// The gate's line while the browser approves this computer.
pub fn waiting_detail(started: &Value) -> String {
    let code = started
        .get("user_code")
        .and_then(Value::as_str)
        .map(|c| format!(" Code {c}."))
        .unwrap_or_default();
    let opened = started
        .get("browser_opened")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    if opened {
        format!("Finish in the browser, then return here.{code}")
    } else {
        let url = started
            .get("verification_uri_complete")
            .and_then(Value::as_str)
            .unwrap_or("");
        format!("Open {url} and approve this computer.{code}")
    }
}

// ---------------------------------------------------------------- state (state.ts)

/// The auth screens' state, one owner. Lives beside `AppState` (the frame's session is there).
#[derive(Default)]
pub struct AuthState {
    // the account gate
    pub gate_busy: bool,
    pub gate_detail: String,
    /// A sign-in/sign-up flow in flight: the host call that will answer (polled in tick).
    pending_flow: Option<PendingFlow>,
    /// A mac_login reset in flight (the OS prompt runs off the engine thread).
    pub pending_mac: Option<std::sync::mpsc::Receiver<Result<Value, String>>>,
    // sign-in
    pub tin: String,
    pub method_totp: bool,
    pub forgot: bool,
    pub busy: bool,
    pub error: Option<String>,
    pub tp_pin: String,
    pub tp_totp: String,
    pub admin_pin: String,
    pub admin_totp: String,
    pub recovery: String,
    // first launch
    pub setup_pin: String,
    pub setup_key: Option<String>,
    /// Each digit-cells field's caret, keyed by field id ("tin", "pin", ...).
    pub cells: HashMap<String, CellState>,
    /// The cells field holding the keys, like DigitCells/PinCells `focus`.
    pub cells_focus: Option<String>,
    /// Escape let the keys go for good; the TIN does not grab them back (first-focus seed).
    pub cells_blurred: bool,
}

/// A digit-cells field's caret.
#[derive(Default, Clone, Copy)]
pub struct CellState {
    pub caret: usize,
}

#[derive(Debug)]
enum PendingFlow {
    /// cloud_account_begin_sign_in/up: the answer carries the user code and verification URI.
    Begin {
        rx: std::sync::mpsc::Receiver<Result<Value, String>>,
    },
    /// cloud_account_finish_sign_in: the approval arrived.
    Finish {
        rx: std::sync::mpsc::Receiver<Result<Value, String>>,
    },
}

pub type SharedAuth = Rc<RefCell<AuthState>>;

pub fn shared() -> SharedAuth {
    Rc::new(RefCell::new(AuthState::default()))
}

impl AuthState {
    fn clear_codes(&mut self) {
        self.tp_pin.clear();
        self.tp_totp.clear();
        self.admin_pin.clear();
        self.admin_totp.clear();
        self.recovery.clear();
        self.cells.clear();
        self.cells_focus = None;
        self.cells_blurred = false;
    }

    fn fail(&mut self, message: String) {
        self.error = Some(message);
        self.clear_codes();
    }

    /// Signed in: the frame takes over; the screen is left clean for the next lock.
    fn begin(&mut self, app: &mut AppState, session: Session) {
        self.error = None;
        self.forgot = false;
        self.method_totp = false;
        self.tin.clear();
        self.clear_codes();
        app.start_session(session);
    }

    /// Sign in or Create account: begin the device flow (the host opens the browser), show the code,
    /// wait for the approval. Async: the begin call runs on the engine; the finish follows.
    pub fn start_account_flow(&mut self, app: &mut AppState, kind: &str) {
        if self.gate_busy || self.pending_flow.is_some() {
            return;
        }
        self.gate_busy = true;
        app.gate_error = None;
        self.gate_detail = "Opening your browser…".to_owned();
        let name = if kind == "create" {
            "cloud_account_begin_sign_up"
        } else {
            "cloud_account_begin_sign_in"
        };
        let rx = bir::call_async_spawn(name, vec![]);
        self.pending_flow = Some(PendingFlow::Begin { rx });
    }

    /// One tick of the gate's async flow.
    pub fn tick_flow(auth: &SharedAuth, app: &mut AppState) {
        let mut this = auth.borrow_mut();
        let Some(flow) = this.pending_flow.take() else {
            return;
        };
        match flow {
            PendingFlow::Begin { rx } => match rx.try_recv() {
                Ok(Ok(started)) => {
                    this.gate_detail = waiting_detail(&started);
                    let rx2 = bir::call_async_spawn("cloud_account_finish_sign_in", vec![]);
                    this.pending_flow = Some(PendingFlow::Finish { rx: rx2 });
                }
                Ok(Err(e)) => {
                    app.gate_error = Some(error_text(&e));
                    this.gate_busy = false;
                    this.gate_detail.clear();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    this.pending_flow = Some(PendingFlow::Begin { rx });
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    this.gate_busy = false;
                    this.gate_detail.clear();
                }
            },
            PendingFlow::Finish { rx } => match rx.try_recv() {
                Ok(Ok(_)) => {
                    drop(this);
                    if app.finish_unlock() {
                        let mut t = auth.borrow_mut();
                        t.gate_busy = false;
                        t.gate_detail.clear();
                    } else {
                        let mut t = auth.borrow_mut();
                        t.gate_busy = false;
                        t.gate_detail.clear();
                    }
                }
                Ok(Err(e)) => {
                    app.gate_error = Some(error_text(&e));
                    this.gate_busy = false;
                    this.gate_detail.clear();
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {
                    this.pending_flow = Some(PendingFlow::Finish { rx });
                }
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    this.gate_busy = false;
                    this.gate_detail.clear();
                }
            },
        }
    }

    fn target(&self, app: &AppState) -> SignInTarget {
        let roots: Vec<String> = store::taxpayers()
            .iter()
            .map(|t| t.tin_root.clone())
            .collect();
        sign_in_target(&self.tin, app.auth_state().admin_root.as_deref(), &roots)
    }

    /// TIN plus that account's own PIN or authenticator code.
    pub fn submit_sign_in(&mut self, app: &mut AppState) {
        let t = self.target(app);
        let SignInTarget::Root { root, admin } = t else {
            if let SignInTarget::Error(e) = t {
                self.fail(e);
            }
            return;
        };
        let totp = self.method_totp;
        let code = if totp {
            self.tp_totp.clone()
        } else {
            self.tp_pin.clone()
        };
        if let Some(short) = code_length_error(totp, &code) {
            self.fail(short.to_owned());
            return;
        }
        let ok = (|| -> Result<bool, String> {
            if admin {
                if totp {
                    bir::call1("verify_admin_totp", json!(code))
                        .map(|v| v.as_bool().unwrap_or(false))
                } else {
                    bir::call1("verify_admin_pin", json!(code))
                        .map(|v| v.as_bool().unwrap_or(false))
                }
            } else if totp {
                bir::call("verify_taxpayer_totp", vec![json!(root), json!(code)])
                    .map(|v| v.as_bool().unwrap_or(false))
            } else {
                bir::call("verify_taxpayer_pin", vec![json!(root), json!(code)])
                    .map(|v| v.as_bool().unwrap_or(false))
            }
        })();
        match ok {
            Ok(true) => {
                let session = if admin {
                    Session::Admin
                } else {
                    Session::Taxpayer { root }
                };
                self.begin(app, session);
            }
            Ok(false) => {
                let none = if admin {
                    false
                } else {
                    // The taxpayer's own credentials: no PIN and no TOTP.
                    bir::call1("taxpayer_auth", json!(root))
                        .ok()
                        .and_then(|v| serde_json::from_value::<crate::types::ProfileAuth>(v).ok())
                        .is_some_and(|c| !c.has_pin && !c.has_totp)
                };
                self.fail(wrong_code(totp, none));
            }
            Err(e) => self.fail(error_text(&e)),
        }
    }

    /// Why the admin's PIN or authenticator code as typed does not open; `None` when one matches.
    fn admin_code_error(&mut self) -> Option<String> {
        let a = crate::store::auth_state();
        let check = |name: &str, code: &str| -> Result<bool, String> {
            bir::call1(name, json!(code)).map(|v| v.as_bool().unwrap_or(false))
        };
        if a.has_admin_pin && self.admin_pin.len() == 4 {
            match check("verify_admin_pin", &self.admin_pin.clone()) {
                Ok(true) => return None,
                Ok(false) => {}
                Err(e) => return Some(error_text(&e)),
            }
        }
        if a.has_admin_totp && self.admin_totp.len() == 6 {
            match check("verify_admin_totp", &self.admin_totp.clone()) {
                Ok(true) => return None,
                Ok(false) => {}
                Err(e) => return Some(error_text(&e)),
            }
        }
        Some("Wrong admin PIN or code. Try again.".to_owned())
    }

    /// "Forgot PIN? Ask the admin": the admin's code opens the taxpayer typed above.
    pub fn submit_override(&mut self, app: &mut AppState) {
        let t = self.target(app);
        let SignInTarget::Root { root, admin } = t else {
            if let SignInTarget::Error(e) = t {
                self.fail(e);
            }
            return;
        };
        if admin {
            self.fail(
                "That is the admin's TIN. Use the recovery key or your Mac login below.".to_owned(),
            );
            return;
        }
        if let Some(why) = self.admin_code_error() {
            self.fail(why);
            return;
        }
        self.begin(app, Session::Taxpayer { root });
        app.toast(
            "Opened by the admin. Set a new PIN in Settings.",
            Some("lock"),
            None,
        );
    }

    fn recovered_admin(&mut self, app: &mut AppState, message: &str) {
        self.begin(app, Session::Admin);
        app.go(ViewName::Settings, ViewParams::new());
        app.toast(message, Some("lock"), None);
    }

    /// The admin's one-time recovery key.
    pub fn submit_recovery_key(&mut self, app: &mut AppState) {
        let key = self.recovery.trim().to_owned();
        if key.is_empty() {
            self.fail(
                "Type the recovery key you wrote down when the admin PIN was set.".to_owned(),
            );
            return;
        }
        match store::write("use_recovery_key", vec![json!(key)]) {
            Ok(v) if v.as_bool().unwrap_or(false) => {
                self.recovered_admin(app, "Recovery key used. It works only once: set a new admin PIN and make a new key.");
            }
            Ok(_) => {
                self.fail("That recovery key does not match. Check it and try again.".to_owned())
            }
            Err(e) => self.fail(error_text(&e)),
        }
    }

    /// Collects a pending mac_login answer.
    pub fn tick_mac_login(&mut self, app: &mut AppState) {
        let Some(rx) = self.pending_mac.take() else {
            return;
        };
        match rx.try_recv() {
            Ok(result) => {
                self.busy = false;
                self.pending_mac = None;
                match result {
                    Ok(v) if v.as_bool().unwrap_or(false) => {
                        self.recovered_admin(
                            app,
                            "Signed in with your Mac login. Set a new admin PIN now.",
                        );
                    }
                    Ok(_) => self.fail("Mac login was cancelled or did not match.".to_owned()),
                    Err(e) => self.fail(error_text(&e)),
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                self.pending_mac = Some(rx);
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                self.busy = false;
            }
        }
    }

    /// The admin has no code, so there is nothing to sign in with: the owner is at the keyboard.
    pub fn open_without_sign_in(&mut self, app: &mut AppState) {
        if app.read_access() != Some(false) {
            return;
        }
        self.begin(app, Session::Open);
    }

    pub fn toggle_method(&mut self) {
        self.method_totp = !self.method_totp;
        self.error = None;
        self.clear_codes();
    }

    pub fn toggle_forgot(&mut self) {
        self.forgot = !self.forgot;
        self.error = None;
        self.clear_codes();
    }

    // ---- first launch

    /// "Just me or my business": the owner's taxpayer becomes the admin account.
    pub fn choose_solo(&mut self, app: &mut AppState) {
        if let Some(first) = store::taxpayers().into_iter().next() {
            if let Err(e) = store::write("setup_solo", vec![json!(first.tin_root)]) {
                app.toast(error_text(&e), Some("triangle-alert"), None);
            }
            return;
        }
        // No taxpayer yet: New taxpayer, with no chrome; Create finishes the setup.
        app.go(ViewName::NewTaxpayer, ViewParams::new());
        app.setup_pending = Some("solo".to_owned());
        app.setup_step = "choose".to_owned();
    }

    /// The New taxpayer page was left during first launch: back to the choice.
    pub fn cancel_setup(&mut self, app: &mut AppState) {
        self.error = None;
        app.setup_pending = None;
        app.setup_step = "choose".to_owned();
    }

    /// The first taxpayer exists and first launch chose "solo": it is the owner, the admin.
    pub fn finish_solo_setup(&mut self, app: &mut AppState, root: &str) {
        if app.setup_pending.as_deref() != Some("solo") {
            return;
        }
        if let Err(e) = store::write("setup_solo", vec![json!(root)]) {
            app.toast(error_text(&e), Some("triangle-alert"), None);
        }
        app.setup_pending = None;
        app.setup_step = "choose".to_owned();
    }

    /// "I help other taxpayers": choose the admin PIN, then the recovery key is shown once.
    pub fn choose_helpdesk(&mut self, app: &mut AppState) {
        self.setup_pin.clear();
        self.error = None;
        app.setup_pending = None;
        app.setup_step = "helpdesk-pin".to_owned();
    }

    pub fn submit_helpdesk_pin(&mut self, app: &mut AppState) {
        let pin = self.setup_pin.clone();
        if pin.len() != 4 {
            self.fail("Type all 4 digits of the admin PIN.".to_owned());
            return;
        }
        match store::write("setup_helpdesk", vec![json!(pin)]) {
            Ok(v) => {
                self.setup_key = v
                    .get("recovery_key")
                    .and_then(Value::as_str)
                    .map(str::to_owned);
                self.error = None;
                app.setup_pending = None;
                app.setup_step = "helpdesk-key".to_owned();
            }
            Err(e) => {
                self.error = Some(error_text(&e));
            }
        }
    }

    /// The recovery key was written down: the admin starts on an empty Home.
    pub fn finish_helpdesk_setup(&mut self, app: &mut AppState) {
        if app.setup_step != "helpdesk-key" {
            return;
        }
        self.setup_key = None;
        self.setup_pin.clear();
        app.setup_pending = None;
        app.setup_step = "choose".to_owned();
        self.begin(app, Session::Admin);
    }
}

// ---------------------------------------------------------------- drawing

// ---- the auth card chrome (screens/auth/parts.tsx): AuthCard, Heading, Labelled, ErrorLine,
// TextLink and Actions. CUI has no OTP widget, so DigitCells/PinCells are drawn by hand below.

/// What the window shows the auth screens in: a scroll-less centered column holding the
/// 400pt card (p-6, p-8 from 600pt).
fn auth_card(ui: &mut Ui, app: &AppState, f: impl FnOnce(&mut Ui)) {
    let pad = if app.narrow() { 24.0 } else { 32.0 };
    let p = crate::ui::kit::frame_palette();
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            height: cui::Size::FILL,
            align: cui::Align::Center,
            justify: cui::Justify::Center,
            pad_x: 16.0,
            pad_y: 24.0,
            ..Default::default()
        },
        |ui| {
            ui.box_(
                &cui::BoxOpts {
                    width: cui::Size::FILL,
                    max_width: 400.0,
                    dir: cui::Dir::Column,
                    gap: crate::theme::space(4),
                    pad,
                    bg: p.card,
                    border: 1.0,
                    border_color: p.border,
                    radius: crate::theme::radius::DIALOG,
                    ..Default::default()
                },
                f,
            );
        },
    );
}

/// The card's head: a 44pt round badge with the icon, the title (page) and a line (meta fg2),
/// all centred (parts.tsx Heading).
fn auth_heading(ui: &mut Ui, icon: &str, title: &str, line: &str) {
    let p = crate::ui::kit::frame_palette();
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            gap: crate::theme::space(2),
            align: cui::Align::Center,
            ..Default::default()
        },
        |ui| {
            ui.box_(
                &cui::BoxOpts {
                    width: cui::Size::px(44.0),
                    height: cui::Size::px(44.0),
                    bg: p.hover,
                    radius: crate::theme::radius::FULL,
                    align: cui::Align::Center,
                    justify: cui::Justify::Center,
                    ..Default::default()
                },
                |ui| {
                    ui.icon(crate::ui::icons::glyph(icon), 20.0, p.muted_foreground);
                },
            );
            ui.text_with(
                title,
                &cui::Text {
                    size: crate::theme::r#type::PAGE.0,
                    font: cui::font::MEDIUM,
                    align: cui::TextAlign::Center,
                    width: cui::Size::FILL,
                    ..Default::default()
                },
            );
            ui.text_with(
                line,
                &cui::Text {
                    size: crate::theme::r#type::META.0,
                    color: p.muted_foreground,
                    align: cui::TextAlign::Center,
                    width: cui::Size::FILL,
                    ..Default::default()
                },
            );
        },
    );
}

/// A label (meta fg2) over its control, 8pt apart (parts.tsx Labelled).
fn labelled<R>(ui: &mut Ui, label: &str, f: impl FnOnce(&mut Ui) -> R) -> R {
    let p = crate::ui::kit::frame_palette();
    let mut out = None;
    ui.column(
        &cui::Column {
            width: cui::Size::FILL,
            gap: crate::theme::space(2),
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                label,
                &cui::Text {
                    size: crate::theme::r#type::META.0,
                    color: p.muted_foreground,
                    ..Default::default()
                },
            );
            out = Some(f(ui));
        },
    );
    out.unwrap()
}

/// The error under the boxes, in danger; nothing when there is none (parts.tsx ErrorLine).
fn error_line(ui: &mut Ui, message: &Option<String>) {
    if let Some(m) = message {
        ui.danger_text(m);
    }
}

/// A link: meta-strong in the accent, its words the press target (parts.tsx TextLink).
fn text_link(ui: &mut Ui, id: &str, label: &str) -> bool {
    let p = crate::ui::kit::frame_palette();
    let wid = ui.make_id(id);
    let hovered = crate::ui::kit::press_hovered(id, ui.mouse_pos().x, ui.mouse_pos().y);
    ui.box_(
        &cui::BoxOpts {
            id: Some(id),
            pad_y: 4.0,
            pad_x: 2.0,
            bg: if hovered { p.hover } else { p.card },
            radius: crate::theme::radius::CTL,
            ..Default::default()
        },
        |ui| {
            ui.text_with(
                label,
                &cui::Text {
                    size: crate::theme::r#type::META_STRONG.0,
                    color: p.accent_blue,
                    ..Default::default()
                },
            );
        },
    );
    let rect = ui.get_rect(wid);
    crate::ui::kit::press_remember(id, rect);
    let mouse = ui.mouse_pos();
    ui.mouse_clicked(cui::MouseButton::Left) && crate::ui::kit::press_hit(rect, mouse.x, mouse.y)
}

// ---- digit cells (ui/fields.tsx DigitCells + PinCells): one box per digit, groups with a
// dash between them, a caret in the focused cell, masked dots for PINs. CUI has no OTP
// widget, so the cells take the keys themselves (digits, Backspace, arrows, Enter, Esc).

/// What a cells frame did.
#[derive(Default)]
struct CellsOut {
    /// The held digits changed this frame.
    changed: bool,
    /// Every cell filled (onComplete).
    complete: bool,
    /// Enter in the cells (onSubmit).
    submit: bool,
    /// A click landed in cell `usize` (focus that field, caret there).
    click: Option<usize>,
    /// Escape/Tab let the keys go.
    blur: bool,
    /// Backspace in an empty box at its start: the keys go back to the TIN (SignIn.tsx keymap).
    retreat: bool,
}

/// `length` cells in `groups` groups (a dash between), `cell_w`×`cell_h`, `gap` apart inside
/// a group and `group_gap` between groups. Digits only; masked shows a dot per digit.
#[allow(clippy::too_many_arguments)]
fn digit_cells(
    ui: &mut Ui,
    id: &str,
    value: &mut String,
    st: &mut CellState,
    focused: bool,
    length: usize,
    groups: usize,
    masked: bool,
    error: bool,
    pin_style: bool,
    cell_w: f32,
    cell_h: f32,
    gap: f32,
    group_gap: f32,
) -> CellsOut {
    let mut out = CellsOut::default();
    let p = crate::ui::kit::frame_palette();
    // The digits held, capped at the cell count.
    let digits: Vec<char> = value
        .chars()
        .filter(|c| c.is_ascii_digit())
        .take(length)
        .collect();
    let caret = st.caret.min(length);
    let per = length.div_ceil(groups.max(1));
    let mouse = ui.mouse_pos();

    // One row: cell groups with a dash between them (the wider group gap).
    ui.row(
        &cui::Row {
            gap: 0.0,
            align: Align::Center,
            height: Size::px(cell_h),
            ..Default::default()
        },
        |ui| {
            for g in 0..groups {
                if g > 0 {
                    ui.box_(
                        &cui::BoxOpts {
                            width: Size::px(group_gap),
                            height: Size::px(cell_h),
                            align: Align::Center,
                            justify: Justify::Center,
                            ..Default::default()
                        },
                        |ui| {
                            ui.text_with(
                                "-",
                                &cui::Text {
                                    size: crate::theme::r#type::EMPH.0,
                                    color: p.subtle_foreground,
                                    ..Default::default()
                                },
                            );
                        },
                    );
                }
                ui.row(
                    &cui::Row {
                        gap,
                        align: Align::Center,
                        height: Size::px(cell_h),
                        ..Default::default()
                    },
                    |ui| {
                        let start = g * per;
                        let end = ((g + 1) * per).min(length);
                        for i in start..end {
                            let cell_id = format!("{id}-{i}");
                            let wid = ui.make_id(&cell_id);
                            let is_caret = focused && caret == i;
                            let border = if error {
                                p.danger
                            } else if is_caret {
                                p.accent_blue
                            } else {
                                p.border_strong
                            };
                            let ch = digits.get(i).copied();
                            ui.box_(
                                &cui::BoxOpts {
                                    id: Some(&cell_id),
                                    width: Size::px(cell_w),
                                    height: Size::px(cell_h),
                                    bg: p.card,
                                    border: 1.0,
                                    border_color: border,
                                    // PinCells draw rounded-row cells, DigitCells rounded-ctl.
                                    radius: if pin_style {
                                        crate::theme::radius::ROW
                                    } else {
                                        crate::theme::radius::CTL
                                    },
                                    align: Align::Center,
                                    justify: Justify::Center,
                                    ..Default::default()
                                },
                                |ui| {
                                    if let Some(c) = ch {
                                        ui.text_with(
                                            &if masked {
                                                "•".to_owned()
                                            } else {
                                                c.to_string()
                                            },
                                            &cui::Text {
                                                size: crate::theme::r#type::EMPH.0,
                                                font: cui::font::MEDIUM,
                                                tabular: true,
                                                ..Default::default()
                                            },
                                        );
                                    } else if is_caret {
                                        // The caret: a 2×14 bar in the empty focused cell.
                                        ui.box_(
                                            &cui::BoxOpts {
                                                width: Size::px(2.0),
                                                // PinCells' caret is h-4, DigitCells' h-3.5.
                                                height: Size::px(if pin_style {
                                                    16.0
                                                } else {
                                                    14.0
                                                }),
                                                bg: p.foreground,
                                                radius: 1.0,
                                                ..Default::default()
                                            },
                                            |_| {},
                                        );
                                    }
                                },
                            );
                            let rect = ui.get_rect(wid);
                            crate::ui::kit::press_remember(&cell_id, rect);
                        }
                    },
                );
            }
        },
    );

    // A click in a cell puts the caret there and gives it the keys.
    if ui.mouse_clicked(cui::MouseButton::Left) {
        for i in 0..length {
            let cell_id = format!("{id}-{i}");
            if crate::ui::kit::press_hovered(&cell_id, mouse.x, mouse.y) {
                out.click = Some(i.min(digits.len()));
                break;
            }
        }
    }

    // The keys: only while focused and no real text field holds the keyboard.
    if !focused || ui.wants_keyboard() {
        out.complete = digits.len() == length && length > 0;
        return out;
    }
    let mut chars = digits;
    let mut changed = false;
    let caret = st.caret.min(length);
    let digit_key = |d: u8| match d {
        0 => cui::Key::Num0,
        1 => cui::Key::Num1,
        2 => cui::Key::Num2,
        3 => cui::Key::Num3,
        4 => cui::Key::Num4,
        5 => cui::Key::Num5,
        6 => cui::Key::Num6,
        7 => cui::Key::Num7,
        8 => cui::Key::Num8,
        _ => cui::Key::Num9,
    };
    let pressed_digit =
        (0..=9u8).find_map(|d| ui.key_pressed(digit_key(d)).then(|| char::from(b'0' + d)));
    if let Some(c) = pressed_digit {
        if caret < length {
            if caret < chars.len() {
                chars[caret] = c;
                changed = true;
            } else if chars.len() < length {
                chars.insert(caret, c);
                changed = true;
            }
            if changed {
                st.caret = (caret + 1).min(length);
            }
        }
    } else if ui.key_pressed(cui::Key::Backspace) {
        if caret > 0 {
            st.caret = caret - 1;
            if st.caret < chars.len() {
                chars.remove(st.caret);
                changed = true;
            }
        } else if chars.is_empty() {
            out.retreat = true;
        }
    } else if ui.key_pressed(cui::Key::Delete) {
        if caret < chars.len() {
            chars.remove(caret);
            changed = true;
        }
    } else if ui.key_pressed(cui::Key::Left) {
        st.caret = caret.saturating_sub(1);
    } else if ui.key_pressed(cui::Key::Right) {
        st.caret = (caret + 1).min(length);
    } else if ui.key_pressed(cui::Key::Enter) {
        out.submit = true;
    } else if ui.key_pressed(cui::Key::Escape) || ui.key_pressed(cui::Key::Tab) {
        out.blur = true;
    }
    if changed {
        *value = chars.into_iter().collect();
        out.changed = true;
    }
    out.complete = value.chars().filter(|c| c.is_ascii_digit()).count() == length && length > 0;
    out
}

/// The account gate card.
fn draw_gate(ui: &mut Ui, app: &mut AppState, auth: &SharedAuth) {
    let busy = auth.borrow().gate_busy;
    let detail = app
        .gate_error
        .clone()
        .or_else(|| {
            let a = auth.borrow();
            (!a.gate_detail.is_empty()).then(|| a.gate_detail.clone())
        })
        .unwrap_or_else(|| GATE_DEFAULT.to_owned());
    let danger = app.gate_error.is_some();
    let mut flow = None::<&'static str>;
    auth_card(ui, app, |ui| {
        auth_heading(
            ui,
            "lock",
            "Welcome to eBIRForms",
            if danger {
                "Sign-in did not finish."
            } else {
                &detail
            },
        );
        if danger {
            ui.danger_text(&detail);
        }
        if ui.primary_button(if busy { "Waiting…" } else { "Sign in" }, !busy) {
            flow = Some("sign-in");
        }
        if ui.secondary_button(if busy { "Waiting…" } else { "Create account" }, !busy) {
            flow = Some("create");
        }
    });
    if let Some(f) = flow {
        auth.borrow_mut().start_account_flow(app, f);
    }
}

/// First launch: who uses this computer.
fn draw_setup(ui: &mut Ui, app: &mut AppState, auth: &SharedAuth) {
    let step = app.setup_step.clone();
    let mut act = None::<&'static str>;
    match step.as_str() {
        "helpdesk-pin" => {
            auth_card(ui, app, |ui| {
                auth_heading(
                    ui,
                    "lock",
                    "Choose the admin PIN",
                    "The help desk admin signs in with TIN 000-000-000 and this PIN. It also opens any taxpayer who forgot theirs.",
                );
                let (err, cell_submit) = {
                    let mut a = auth.borrow_mut();
                    if a.cells_focus.is_none() && !a.cells_blurred {
                        a.cells_focus = Some("setup-pin".to_owned());
                    }
                    let focused = a.cells_focus.as_deref() == Some("setup-pin");
                    let failed = a.error.is_some();
                    let err = a.error.clone();
                    let AuthState {
                        setup_pin,
                        cells,
                        cells_focus,
                        cells_blurred,
                        ..
                    } = &mut *a;
                    let out = cells_field(
                        ui,
                        "PIN",
                        "setup-pin",
                        setup_pin,
                        cells,
                        focused,
                        4,
                        1,
                        true,
                        failed,
                        true,
                    );
                    if let Some(i) = out.click {
                        *cells_focus = Some("setup-pin".to_owned());
                        if let Some(c) = cells.get_mut("setup-pin") {
                            c.caret = i;
                        }
                    }
                    if out.blur {
                        *cells_focus = None;
                        *cells_blurred = true;
                    }
                    (err, out.submit)
                };
                if cell_submit {
                    act = Some("continue");
                }
                if let Some(e) = err {
                    ui.danger_text(&e);
                }
                ui.row(
                    &cui::Row {
                        width: Size::FILL,
                        gap: crate::theme::space(2),
                        ..Default::default()
                    },
                    |ui| {
                        ui.spacer();
                        if crate::ui::kit::ghost_button(ui, "Back", true) {
                            act = Some("back");
                        }
                        if crate::ui::kit::primary_button(ui, "Continue", true) {
                            act = Some("continue");
                        }
                    },
                );
            });
        }
        "helpdesk-key" => {
            auth_card(ui, app, |ui| {
                auth_heading(
                    ui,
                    "key",
                    "Write down your recovery key",
                    "If you forget the admin PIN, this key signs you in once. It is shown only now. Keep it away from this computer.",
                );
                if let Some(key) = auth.borrow().setup_key.clone() {
                    ui.code(&key);
                }
                ui.muted(
                    "Lost both? Whoever can log in to this computer can still reset the admin PIN.",
                );
                ui.column(
                    &cui::Column {
                        width: Size::FILL,
                        ..Default::default()
                    },
                    |ui| {
                        if crate::ui::kit::primary_button_block(ui, "I wrote it down", true) {
                            act = Some("done");
                        }
                    },
                );
            });
        }
        _ => {
            auth_card(ui, app, |ui| {
                auth_heading(
                    ui,
                    "users",
                    "Who will use eBIRForms on this computer?",
                    "This decides who the admin is. Nobody can lock the others out.",
                );
                ui.card(&cui::Card { muted: true, ..Default::default() }, |ui| {
                    ui.text("Just me or my business");
                    ui.muted("You are the admin. Your own tax profile is the admin account, with all its branches.");
                });
                if crate::ui::kit::primary_button_block(ui, "Just me or my business", true) {
                    act = Some("solo");
                }
                ui.card(&cui::Card { muted: true, ..Default::default() }, |ui| {
                    ui.text("I help other taxpayers");
                    ui.muted("An RDO help desk, an accountant or a bookkeeper. You get a separate admin account, and each taxpayer signs in as themselves.");
                });
                if crate::ui::kit::secondary_button(ui, "I help other taxpayers", true) {
                    act = Some("helpdesk");
                }
            });
        }
    }
    match act {
        Some("back") => auth.borrow_mut().cancel_setup(app),
        Some("continue") => auth.borrow_mut().submit_helpdesk_pin(app),
        Some("done") => auth.borrow_mut().finish_helpdesk_setup(app),
        Some("solo") => auth.borrow_mut().choose_solo(app),
        Some("helpdesk") => auth.borrow_mut().choose_helpdesk(app),
        _ => {}
    }
}

/// What the sign-in's buttons, links and cells asked for this frame.
#[derive(Clone, Copy, PartialEq)]
enum SignInPending {
    None,
    SignIn,
    Override,
    Recovery,
    Mac,
    NoSignIn,
    ToggleMethod,
    ToggleForgot,
    /// A cells field took the keys at caret `usize` (click or TIN's last digit).
    Focus(&'static str, usize),
    /// Escape/Tab let the keys go.
    Blur,
}

/// One labelled digit-cells field; `focused` is the screen's single cells focus.
fn cells_field(
    ui: &mut Ui,
    label: &str,
    id: &str,
    value: &mut String,
    cells: &mut HashMap<String, CellState>,
    focused: bool,
    length: usize,
    groups: usize,
    masked: bool,
    error: bool,
    pin_style: bool,
) -> CellsOut {
    let st = cells.entry(id.to_owned()).or_default();
    labelled(ui, label, |ui| {
        digit_cells(
            ui,
            id,
            value,
            st,
            focused,
            length,
            groups,
            masked,
            error,
            pin_style,
            if length <= 6 { 32.0 } else { 24.0 },
            32.0,
            if length <= 6 { 6.0 } else { 3.0 },
            if length <= 6 { 12.0 } else { 17.0 },
        )
    })
}

/// The sign-in screen (locked): the AuthCard — heading, TIN cells, the code boxes the path
/// calls for, the error line, links left and the main button right (SignIn.tsx).
fn draw_sign_in(ui: &mut Ui, app: &mut AppState, auth: &SharedAuth) {
    let mut pending = SignInPending::None;
    let mut cells_changed = false;
    {
        let mut guard = auth.borrow_mut();
        let a = &mut *guard;
        // The first DigitCells of the screen takes the keys until the user lets go (Esc).
        if a.cells_focus.is_none() && !a.cells_blurred {
            a.cells_focus = Some("tin".to_owned());
        }
        let helpdesk = app.is_helpdesk();
        let target = a.target(app);
        let for_admin = matches!(target, SignInTarget::Root { admin: true, .. });
        let line = sign_in_line(a.forgot, for_admin, helpdesk).to_owned();
        let auth_st = app.auth_state();
        let failed = a.error.is_some();
        let method_totp = a.method_totp;
        let forgot = a.forgot;
        let busy = a.busy;
        let mac_pending = a.pending_mac.is_some();
        let no_code = !app.admin_has_code();
        let error = a.error.clone();
        let focus = a.cells_focus.clone();
        let tin_len = a.tin.chars().filter(|c| c.is_ascii_digit()).count();
        let pin_len = a.tp_pin.chars().count();
        let totp_len = a.tp_totp.chars().count();
        let admin_pin_len = a.admin_pin.chars().count();
        let admin_totp_len = a.admin_totp.chars().count();

        let AuthState {
            tin,
            tp_pin,
            tp_totp,
            admin_pin,
            admin_totp,
            recovery,
            cells,
            ..
        } = &mut *a;

        // A cells outcome becomes a pending focus move (only the last one wins a frame).
        macro_rules! cells_out {
            ($out:expr, $id:expr) => {{
                let o = $out;
                cells_changed |= o.changed;
                if let Some(i) = o.click {
                    pending = SignInPending::Focus($id, i);
                }
                if o.blur {
                    pending = SignInPending::Blur;
                }
                if o.retreat && $id != "tin" {
                    pending = SignInPending::Focus("tin", 9);
                }
                o
            }};
        }

        auth_card(ui, app, |ui| {
            auth_heading(ui, "lock", "Sign in", &line);

            // TIN: nine cells in three groups (000-000-000); its last digit moves the keys on.
            let tin_out = cells_out!(
                cells_field(
                    ui,
                    "TIN",
                    "tin",
                    tin,
                    cells,
                    focus.as_deref() == Some("tin"),
                    9,
                    3,
                    false,
                    failed,
                    false,
                ),
                "tin"
            );
            if tin_out.complete && tin_len < 9 {
                // nextBox(): the code box the path calls for (the recovery field is a text
                // input, which cells focus cannot reach).
                let next = if forgot {
                    if for_admin {
                        None
                    } else if auth_st.has_admin_pin {
                        Some("admin-pin")
                    } else {
                        Some("admin-totp")
                    }
                } else {
                    Some(if method_totp { "totp" } else { "pin" })
                };
                if let Some(id) = next {
                    pending = SignInPending::Focus(id, 0);
                }
            }
            if tin_out.submit && !forgot {
                pending = SignInPending::SignIn;
            }

            if !forgot {
                if method_totp {
                    let out = cells_out!(
                        cells_field(
                            ui,
                            "Authenticator code",
                            "totp",
                            tp_totp,
                            cells,
                            focus.as_deref() == Some("totp"),
                            6,
                            2,
                            false,
                            failed,
                            true,
                        ),
                        "totp"
                    );
                    if (out.complete && totp_len < 6) || out.submit {
                        pending = SignInPending::SignIn;
                    }
                } else {
                    let out = cells_out!(
                        cells_field(
                            ui,
                            "PIN",
                            "pin",
                            tp_pin,
                            cells,
                            focus.as_deref() == Some("pin"),
                            4,
                            1,
                            true,
                            failed,
                            true,
                        ),
                        "pin"
                    );
                    if (out.complete && pin_len < 4) || out.submit {
                        pending = SignInPending::SignIn;
                    }
                }
            } else if for_admin {
                labelled(ui, "Recovery key", |ui| {
                    let mut v = recovery.clone();
                    if v.capacity() < 64 {
                        v.reserve(64);
                    }
                    let submitted = ui
                        .input_with(
                            "auth-recovery",
                            &mut v,
                            &cui::Input {
                                placeholder: Some("XXXX-XXXX-XXXX-XXXX"),
                                ..Default::default()
                            },
                        )
                        .submitted();
                    if v != *recovery {
                        *recovery = v;
                    }
                    if submitted {
                        pending = SignInPending::Recovery;
                    }
                });
            } else {
                // The admin's code opens the taxpayer: PIN and/or authenticator boxes.
                if auth_st.has_admin_pin {
                    let out = cells_out!(
                        cells_field(
                            ui,
                            "Admin PIN",
                            "admin-pin",
                            admin_pin,
                            cells,
                            focus.as_deref() == Some("admin-pin"),
                            4,
                            1,
                            true,
                            failed,
                            true,
                        ),
                        "admin-pin"
                    );
                    if (out.complete && admin_pin_len < 4) || out.submit {
                        pending = SignInPending::Override;
                    }
                }
                if auth_st.has_admin_totp {
                    let out = cells_out!(
                        cells_field(
                            ui,
                            "Admin authenticator code",
                            "admin-totp",
                            admin_totp,
                            cells,
                            focus.as_deref() == Some("admin-totp"),
                            6,
                            2,
                            false,
                            failed,
                            true,
                        ),
                        "admin-totp"
                    );
                    if (out.complete && admin_totp_len < 6) || out.submit {
                        pending = SignInPending::Override;
                    }
                }
            }

            error_line(ui, &error);

            // Actions: links on the left, the main button on the right (parts.tsx Actions).
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    align: Align::Center,
                    gap: crate::theme::space(2),
                    ..Default::default()
                },
                |ui| {
                    ui.column(
                        &cui::Column {
                            gap: crate::theme::space(1),
                            ..Default::default()
                        },
                        |ui| {
                            if !forgot
                                && text_link(
                                    ui,
                                    "auth-method",
                                    if method_totp {
                                        "Use the PIN instead"
                                    } else {
                                        "Use an authenticator code"
                                    },
                                )
                            {
                                pending = SignInPending::ToggleMethod;
                            }
                            if text_link(
                                ui,
                                "auth-forgot",
                                if forgot {
                                    "I remember my PIN"
                                } else {
                                    "Forgot PIN?"
                                },
                            ) {
                                pending = SignInPending::ToggleForgot;
                            }
                        },
                    );
                    ui.spacer();
                    let (label, act) = if !forgot {
                        ("Sign in", SignInPending::SignIn)
                    } else if for_admin {
                        ("Use key", SignInPending::Recovery)
                    } else {
                        ("Open account", SignInPending::Override)
                    };
                    if crate::ui::kit::primary_button(ui, label, true) {
                        pending = act;
                    }
                },
            );

            if forgot && for_admin && auth_st.mac_login_available {
                ui.column(
                    &cui::Column {
                        width: Size::FILL,
                        align: Align::Center,
                        ..Default::default()
                    },
                    |ui| {
                        if crate::ui::kit::small_button(
                            ui,
                            if busy || mac_pending {
                                "Waiting for your Mac login…"
                            } else {
                                "Use my Mac login instead"
                            },
                            cui::ButtonVariant::Ghost,
                            Some("laptop"),
                        ) {
                            pending = SignInPending::Mac;
                        }
                    },
                );
            }
            if no_code {
                ui.column(
                    &cui::Column {
                        width: Size::FILL,
                        align: Align::Center,
                        ..Default::default()
                    },
                    |ui| {
                        if crate::ui::kit::small_button(
                            ui,
                            "Open without signing in",
                            cui::ButtonVariant::Ghost,
                            None,
                        ) {
                            pending = SignInPending::NoSignIn;
                        }
                    },
                );
            }
        });
    }

    let mut a = auth.borrow_mut();
    if cells_changed {
        a.error = None;
    }
    match pending {
        SignInPending::SignIn => a.submit_sign_in(app),
        SignInPending::Override => a.submit_override(app),
        SignInPending::Recovery => a.submit_recovery_key(app),
        SignInPending::Mac => {
            a.pending_mac = Some(bir::call_async_spawn(
                "mac_login",
                vec![json!("reset the eBIRForms admin PIN")],
            ));
            a.busy = true;
        }
        SignInPending::NoSignIn => a.open_without_sign_in(app),
        SignInPending::ToggleMethod => a.toggle_method(),
        SignInPending::ToggleForgot => a.toggle_forgot(),
        SignInPending::Focus(id, caret) => {
            a.cells_focus = Some(id.to_owned());
            if let Some(c) = a.cells.get_mut(id) {
                c.caret = caret;
            }
        }
        SignInPending::Blur => {
            a.cells_focus = None;
            a.cells_blurred = true;
        }
        SignInPending::None => {}
    }
}

/// Which auth surface the frame shows, if any.
pub enum AuthSurface {
    /// The Buwiz account gate.
    Gate,
    /// First-run setup (or solo's New taxpayer page, which the frame routes itself).
    Setup,
    /// The sign-in lock.
    SignIn,
    /// "Just me" on an empty computer: the New taxpayer page with no chrome.
    SoloTaxpayer,
    /// Nothing in front: the filing chrome.
    None,
}

/// What stands in front of the frame (app.ts chromeVisible's inverse).
pub fn surface(app: &AppState) -> AuthSurface {
    if !app.booted {
        return AuthSurface::None;
    }
    if app.cloud_locked {
        return AuthSurface::Gate;
    }
    if app.needs_setup() {
        if app.setup_pending.as_deref() == Some("solo")
            && app.taxpayers().is_empty()
            && app.route.view == ViewName::NewTaxpayer
        {
            return AuthSurface::SoloTaxpayer;
        }
        return AuthSurface::Setup;
    }
    if app.app_locked || std::env::var_os("CUI_FORCE_SIGNIN").is_some() {
        return AuthSurface::SignIn;
    }
    AuthSurface::None
}

/// Draws whatever stands in front of the frame. Returns true when it drew (the shell is hidden).
pub fn draw(ui: &mut Ui, app: &mut AppState, auth: &SharedAuth) -> bool {
    match surface(app) {
        AuthSurface::Gate => {
            draw_gate(ui, app, auth);
            true
        }
        AuthSurface::Setup => {
            draw_setup(ui, app, auth);
            true
        }
        AuthSurface::SignIn => {
            draw_sign_in(ui, app, auth);
            true
        }
        AuthSurface::SoloTaxpayer => {
            // The New taxpayer page with no chrome (screens/taxpayer).
            crate::screens::taxpayer::draw_new(ui, app, auth);
            true
        }
        AuthSurface::None => false,
    }
}
