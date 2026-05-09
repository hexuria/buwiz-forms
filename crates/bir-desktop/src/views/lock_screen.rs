use bir_core::db::Database;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::ActiveTheme;
use gpui_component::Disableable;
use gpui_component::Sizable;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{InputEvent, OtpInput, OtpState};
use std::sync::{Arc, Mutex};

use crate::components::otp_paste::paste_otp_value;
use crate::components::rate_limiter::RateLimiter;

/// Which single authentication method is active for the lock screen.
#[derive(Clone, Copy, PartialEq)]
enum ActiveAuthMethod {
    Pin,  // 4-digit masked
    Totp, // 6-digit from authenticator app
}

/// The current phase of the lock screen authentication flow.
#[derive(Clone, Copy, PartialEq)]
enum AuthPhase {
    /// Biometrics prompt is active (auto-triggered on load).
    Biometric,
    /// Biometrics dismissed/failed — show TOTP or PIN input.
    CodeEntry,
    /// Too many failed code attempts — only biometrics/password can unlock.
    LockedOut,
}

pub struct LockScreenView {
    otp_state: Entity<OtpState>,
    db: Arc<Mutex<Database>>,
    has_error: bool,
    rate_limiter: RateLimiter,
    os_auth_triggered: bool,
    os_auth_error: Option<String>,
    should_clear_otp: bool,
    auth_method: ActiveAuthMethod,
    phase: AuthPhase,
    /// Subscription for a 1-second tick to update lockout countdown + TOTP timer.
    _tick_sub: Option<gpui::Task<()>>,
}

pub enum LockScreenEvent {
    Unlocked,
}

impl EventEmitter<LockScreenEvent> for LockScreenView {}

impl LockScreenView {
    pub fn new(db: Arc<Mutex<Database>>, window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Determine which method is configured. Mutual exclusion is enforced in Settings.
        let auth_method = if let Ok(db_guard) = db.lock() {
            if db_guard
                .get_setting("app_totp_secret")
                .ok()
                .flatten()
                .is_some()
            {
                ActiveAuthMethod::Totp
            } else {
                ActiveAuthMethod::Pin
            }
        } else {
            ActiveAuthMethod::Pin
        };

        let digit_count = match auth_method {
            ActiveAuthMethod::Pin => 4,
            ActiveAuthMethod::Totp => 6,
        };

        let otp_state = cx.new(|cx| {
            let mut state = OtpState::new(digit_count, window, cx);
            state = state.masked(auth_method == ActiveAuthMethod::Pin);
            // Don't focus yet — biometrics will be shown first
            state
        });

        let mut view = Self {
            otp_state: otp_state.clone(),
            db: db.clone(),
            has_error: false,
            rate_limiter: RateLimiter::new(),
            os_auth_triggered: false,
            os_auth_error: None,
            should_clear_otp: false,
            auth_method,
            phase: AuthPhase::Biometric,
            _tick_sub: None,
        };

        // Subscribe to input changes for validation
        cx.subscribe_in(
            &otp_state,
            window,
            move |this: &mut Self, _entity, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    if this.rate_limiter.is_locked() {
                        this.otp_state
                            .update(cx, |s, cx| s.set_value("", window, cx));
                        return;
                    }

                    let value = this.otp_state.read(cx).value().to_string();
                    let expected_len = match this.auth_method {
                        ActiveAuthMethod::Pin => 4,
                        ActiveAuthMethod::Totp => 6,
                    };

                    if value.len() == expected_len {
                        match this.auth_method {
                            ActiveAuthMethod::Pin => {
                                let db = this.db.clone();
                                let pin = value;
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
                                                    this.rate_limiter.reset();
                                                    cx.emit(LockScreenEvent::Unlocked);
                                                } else {
                                                    this.has_error = true;
                                                    this.rate_limiter.record_failure();
                                                    this.should_clear_otp = true;
                                                    if this.rate_limiter.is_locked() {
                                                        this.phase = AuthPhase::LockedOut;
                                                    }
                                                    cx.notify();
                                                }
                                            });
                                        }
                                    });
                                })
                                .detach();
                            }
                            ActiveAuthMethod::Totp => {
                                let mut is_valid = false;
                                if let Ok(db_guard) = this.db.lock()
                                    && let Ok(Some(secret)) =
                                        db_guard.get_setting("app_totp_secret")
                                {
                                    is_valid = bir_core::crypto::validate_totp(&secret, &value);
                                }

                                if is_valid {
                                    this.has_error = false;
                                    this.rate_limiter.reset();
                                    cx.emit(LockScreenEvent::Unlocked);
                                } else {
                                    this.has_error = true;
                                    this.rate_limiter.record_failure();
                                    this.should_clear_otp = true;
                                    if this.rate_limiter.is_locked() {
                                        this.phase = AuthPhase::LockedOut;
                                    }
                                    cx.notify();
                                }
                            }
                        }
                    }
                }
            },
        )
        .detach();

        // Auto-trigger biometrics immediately
        view.trigger_os_auth(cx);

        view
    }

    /// Start a 1-second tick for lockout countdown and TOTP time display.
    fn ensure_tick(&mut self, cx: &mut Context<Self>) {
        if self._tick_sub.is_some() {
            return;
        }
        self._tick_sub = Some(cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(std::time::Duration::from_secs(1))
                    .await;
                let should_continue = this.update(cx, |this, cx| {
                    // Transition out of lockout once cooldown expires
                    if this.phase == AuthPhase::LockedOut && !this.rate_limiter.is_locked() {
                        this.phase = AuthPhase::CodeEntry;
                        this.has_error = false;
                    }
                    cx.notify();
                    this.rate_limiter.is_locked() || this.auth_method == ActiveAuthMethod::Totp
                });
                match should_continue {
                    Ok(true) => continue,
                    _ => break,
                }
            }
            let _ = this.update(cx, |this, _cx| {
                this._tick_sub = None;
            });
        }));
    }

    #[cfg(any(target_os = "macos", target_os = "windows"))]
    fn trigger_os_auth(&mut self, cx: &mut Context<Self>) {
        self.os_auth_triggered = true;
        self.os_auth_error = None;
        cx.notify();

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
                    this.has_error = false;
                    this.rate_limiter.reset();
                    cx.emit(LockScreenEvent::Unlocked);
                } else {
                    // Biometrics cancelled/failed — fall through to code entry
                    this.os_auth_error = None;
                    if this.phase == AuthPhase::Biometric {
                        this.phase = AuthPhase::CodeEntry;
                    }
                    // Trigger focus on next render cycle
                    this.should_clear_otp = true;
                }
                cx.notify();
            });
        })
        .detach();
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    fn trigger_os_auth(&mut self, cx: &mut Context<Self>) {
        // No biometrics available — go straight to code entry
        self.os_auth_triggered = false;
        self.phase = AuthPhase::CodeEntry;
        self.should_clear_otp = true;
        cx.notify();
    }

    /// Seconds remaining in the current TOTP window (30s period).
    fn totp_seconds_remaining() -> u64 {
        let epoch_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        30 - (epoch_secs % 30)
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

        let is_locked_out = self.phase == AuthPhase::LockedOut;
        let lockout_msg = self.rate_limiter.lockout_message();

        // Start tick timer when locked out or using TOTP
        if is_locked_out || self.auth_method == ActiveAuthMethod::Totp {
            self.ensure_tick(cx);
        }

        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .bg(cx.theme().background)
            // Intercept Cmd+V / Ctrl+V for OTP paste support
            .on_key_down(cx.listener(|this, event: &KeyDownEvent, window, cx| {
                let dominated_by = &event.keystroke.modifiers;
                let is_paste = event.keystroke.key.as_str() == "v"
                    && (dominated_by.platform || dominated_by.control);
                if is_paste && this.phase == AuthPhase::CodeEntry {
                    let expected_len = match this.auth_method {
                        ActiveAuthMethod::Pin => 4,
                        ActiveAuthMethod::Totp => 6,
                    };
                    paste_otp_value(&this.otp_state, expected_len, window, cx);
                }
            }))
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
                    // ── Phase: Biometric ──
                    .when(self.phase == AuthPhase::Biometric, |this| {
                        this.child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().foreground)
                                .child("Unlock e-BIRForms"),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(if self.os_auth_triggered {
                                    "Waiting for biometric authentication..."
                                } else {
                                    "Use Touch ID, Windows Hello, or your computer password."
                                }),
                        )
                        .child(
                            gpui_component::button::Button::new("biometric_retry_btn")
                                .label(if self.os_auth_triggered {
                                    "Waiting..."
                                } else {
                                    "Retry Biometric"
                                })
                                .disabled(self.os_auth_triggered)
                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                    this.trigger_os_auth(cx);
                                })),
                        )
                        .child(
                            gpui_component::button::Button::new("skip_biometric_btn")
                                .label(match self.auth_method {
                                    ActiveAuthMethod::Totp => "Use Authenticator Code Instead",
                                    ActiveAuthMethod::Pin => "Use PIN Instead",
                                })
                                .ghost()
                                .small()
                                .disabled(self.os_auth_triggered)
                                .on_click(cx.listener(|this, _ev, window, cx| {
                                    this.phase = AuthPhase::CodeEntry;
                                    this.otp_state.update(cx, |s, cx| s.focus(window, cx));
                                    cx.notify();
                                })),
                        )
                    })
                    // ── Phase: Code Entry (TOTP or PIN) ──
                    .when(self.phase == AuthPhase::CodeEntry, |this| {
                        let title = match self.auth_method {
                            ActiveAuthMethod::Totp => "Enter Authenticator Code",
                            ActiveAuthMethod::Pin => "Enter PIN to unlock e-BIRForms",
                        };
                        this.child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().foreground)
                                .child(title),
                        )
                        // TOTP time-remaining indicator
                        .when(self.auth_method == ActiveAuthMethod::Totp, |this| {
                            let secs = Self::totp_seconds_remaining();
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(if secs <= 5 {
                                        cx.theme().danger
                                    } else {
                                        cx.theme().muted_foreground
                                    })
                                    .child(format!("Code expires in {}s", secs)),
                            )
                        })
                        .child(
                            OtpInput::new(&self.otp_state)
                                .groups(1)
                                .large()
                                .disabled(self.os_auth_triggered),
                        )
                        .when(self.has_error, |this| {
                            this.child(div().text_sm().text_color(cx.theme().danger).child(
                                match self.auth_method {
                                    ActiveAuthMethod::Totp => "Incorrect code. Please try again.",
                                    ActiveAuthMethod::Pin => "Incorrect PIN. Please try again.",
                                },
                            ))
                        })
                        .child(
                            gpui_component::button::Button::new("forgot_pin_btn")
                                .label("Use Biometric / Password Instead")
                                .ghost()
                                .small()
                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                    this.trigger_os_auth(cx);
                                })),
                        )
                    })
                    // ── Phase: Locked Out ──
                    .when(self.phase == AuthPhase::LockedOut, |this| {
                        this.child(
                            div()
                                .text_xl()
                                .font_weight(FontWeight::BOLD)
                                .text_color(cx.theme().danger)
                                .child("Account Locked"),
                        )
                        .when_some(lockout_msg, |this, msg| {
                            this.child(div().text_sm().text_color(cx.theme().danger).child(msg))
                        })
                        .child(
                            OtpInput::new(&self.otp_state)
                                .groups(1)
                                .large()
                                .disabled(true),
                        )
                        .child(
                            gpui_component::button::Button::new("os_auth_lockout_btn")
                                .label(if self.os_auth_triggered {
                                    "Waiting for OS..."
                                } else {
                                    "Unlock with Biometric / Password"
                                })
                                .disabled(self.os_auth_triggered)
                                .on_click(cx.listener(|this, _ev, _window, cx| {
                                    this.trigger_os_auth(cx);
                                })),
                        )
                        .when_some(
                            self.os_auth_error.clone(),
                            |this, err| {
                                this.child(
                                    div()
                                        .text_sm()
                                        .mt_2()
                                        .text_color(cx.theme().danger)
                                        .child(err),
                                )
                            },
                        )
                    }),
            )
            .into_any_element()
    }
}
