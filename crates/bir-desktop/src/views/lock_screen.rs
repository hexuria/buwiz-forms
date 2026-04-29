use bir_core::db::Database;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::Disableable;
use gpui_component::Sizable;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{InputEvent, OtpInput, OtpState};
use std::sync::{Arc, Mutex};

pub struct LockScreenView {
    otp_state: Entity<OtpState>,
    db: Arc<Mutex<Database>>,
    has_error: bool,
    failed_attempts: u8,
    os_auth_triggered: bool,
    os_auth_error: Option<String>,
    should_clear_otp: bool,
}

pub enum LockScreenEvent {
    Unlocked,
}

impl EventEmitter<LockScreenEvent> for LockScreenView {}

impl LockScreenView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        let otp_state = cx.new(|cx| {
            let mut state = OtpState::new(4, window, cx);
            state = state.masked(true);
            state.focus(window, cx);
            state
        });

        let view = Self {
            otp_state: otp_state.clone(),
            db: db.clone(),
            has_error: false,
            failed_attempts: 0,
            os_auth_triggered: false,
            os_auth_error: None,
            should_clear_otp: false,
        };

        cx.subscribe_in(
            &otp_state,
            window,
            |this: &mut Self, _entity, event: &InputEvent, _window, cx| {
                if let InputEvent::Change = event {
                    let pin = this.otp_state.read(cx).value().to_string();
                    if pin.len() == 4 {
                        let db = this.db.clone();

                        cx.spawn(async move |this, cx| {
                            let hashed_pin = cx
                                .background_executor()
                                .spawn(async move { bir_core::crypto::hash_pin(&pin) })
                                .await;

                            cx.update(|cx| {
                                if let Some(this) = this.upgrade() {
                                    this.update(cx, |this, cx| {
                                        let mut is_valid = false;
                                        if let Ok(db_guard) = db.lock() {
                                            if let Ok(Some(saved_hash)) =
                                                db_guard.get_setting("app_lock_pin_hash")
                                            {
                                                is_valid = saved_hash == hashed_pin;
                                            } else {
                                                is_valid = true;
                                            }
                                        }

                                        if is_valid {
                                            this.has_error = false;
                                            this.failed_attempts = 0;
                                            cx.emit(LockScreenEvent::Unlocked);
                                        } else {
                                            this.has_error = true;
                                            this.failed_attempts += 1;
                                            this.should_clear_otp = true;
                                            cx.notify();
                                        }
                                    });
                                }
                            });
                        })
                        .detach();
                    }
                }
            },
        )
        .detach();

        view
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn trigger_os_auth(&mut self, cx: &mut Context<Self>) {
        self.os_auth_triggered = true;
        self.os_auth_error = None;
        cx.notify();

        let db = self.db.clone();

        cx.spawn(async move |this, cx| {
            use robius_authentication::{
                AndroidText, BiometricStrength, Context, PolicyBuilder, Text, WindowsText,
            };

            let policy = match PolicyBuilder::new()
                .biometrics(Some(BiometricStrength::Strong))
                .password(true)
                .watch(true)
                .build()
            {
                Some(p) => p,
                None => return,
            };

            let text = Text {
                android: AndroidText {
                    title: "Unlock App",
                    subtitle: None,
                    description: None,
                },
                apple: "Unlock e-BIRForms Session",
                windows: WindowsText::new_truncated(
                    "Unlock e-BIRForms",
                    "Please authenticate to unlock the session.",
                ),
            };

            let success = Context::new(()).authenticate(text, &policy).await.is_ok();

            let _ = this.update(cx, |this, cx| {
                this.os_auth_triggered = false;
                if success {
                    if let Ok(db_guard) = db.lock() {
                        let _ = db_guard.set_setting("app_lock_enabled", "false");
                    }
                    this.has_error = false;
                    this.failed_attempts = 0;
                    cx.emit(LockScreenEvent::Unlocked);
                } else {
                    this.os_auth_error =
                        Some("Operating System Authentication failed or was canceled.".to_string());
                }
                cx.notify();
            });
        })
        .detach();
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn trigger_os_auth(&mut self, cx: &mut Context<Self>) {
        self.os_auth_triggered = false;
        self.os_auth_error =
            Some("Operating System Authentication is not supported on this platform.".to_string());
        cx.notify();
    }
}

impl Render for LockScreenView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        if self.should_clear_otp {
            self.should_clear_otp = false;
            self.otp_state.update(cx, |state, cx| {
                state.set_value("", window, cx);
                state.focus(window, cx);
            });
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_6()
                    .child(
                        gpui::img("images/ebirforms.png")
                            .w(px(200.))
                            .h(px(60.))
                            .object_fit(gpui::ObjectFit::Contain),
                    )
                    .child(
                        div()
                            .text_xl()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().foreground)
                            .child("Enter PIN to unlock e-BIRForms"),
                    )
                    .child(
                        OtpInput::new(&self.otp_state)
                            .groups(1)
                            .large()
                            .disabled(self.failed_attempts >= 6 || self.os_auth_triggered),
                    )
                    .when(self.has_error && self.failed_attempts < 6, |this| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().danger)
                                .child("Incorrect PIN. Please try again."),
                        )
                    })
                    .when(self.failed_attempts >= 6, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .items_center()
                                .gap_2()
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().danger)
                                        .child("Maximum PIN attempts exceeded."),
                                )
                                .child(
                                    gpui_component::button::Button::new("os_auth_btn")
                                        .label(if self.os_auth_triggered {
                                            "Waiting for OS..."
                                        } else {
                                            "Unlock with Computer Password"
                                        })
                                        .disabled(self.os_auth_triggered)
                                        .on_click(cx.listener(|this, _ev, _window, cx| {
                                            this.trigger_os_auth(cx);
                                        })),
                                )
                                .when_some(self.os_auth_error.clone(), |this, err| {
                                    this.child(
                                        div()
                                            .text_sm()
                                            .mt_2()
                                            .text_color(cx.theme().danger)
                                            .child(err),
                                    )
                                }),
                        )
                    })
                    .when(self.failed_attempts < 6, |this| {
                        this.child(
                            gpui_component::button::Button::new("forgot_pin_btn")
                                .label("Forgot PIN?")
                                .ghost()
                                .small()
                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                    this.trigger_os_auth(cx);
                                })),
                        )
                    }),
            )
            .into_any_element()
    }
}
