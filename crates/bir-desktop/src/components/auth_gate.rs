//! Reusable PIN/Biometric authentication gate overlay.
//!
//! Used for both profile-level PIN gates and admin-level app lock gates.
//! Renders a full-screen overlay with OTP input, optional OS Auth override, and cancel button.
//!
// TODO: Wire AuthGateView into auth_overlays.rs to replace the remaining inline PIN modals.
#![allow(dead_code)]

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::{InputEvent, OtpInput, OtpState};
use gpui_component::*;

/// Events emitted by the AuthGate.
pub enum AuthGateEvent {
    /// PIN or OS Auth succeeded — caller should apply the gated action.
    Authenticated,
    /// User cancelled the gate.
    Cancelled,
}

impl EventEmitter<AuthGateEvent> for AuthGateView {}

pub struct AuthGateView {
    title: SharedString,
    description: SharedString,
    pin_hash: String,
    otp_state: Entity<OtpState>,
    error_msg: Option<String>,
    os_auth_triggered: bool,
    logo_path: SharedString,
}

impl AuthGateView {
    pub fn new(
        title: impl Into<SharedString>,
        description: impl Into<SharedString>,
        pin_hash: String,
        logo_path: impl Into<SharedString>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let otp_state = cx.new(|cx| OtpState::new(4, window, cx).masked(true));

        cx.subscribe_in(
            &otp_state,
            window,
            move |this: &mut Self, _entity, event: &InputEvent, window, cx| {
                if let InputEvent::Change = event {
                    let entered_pin = this.otp_state.read(cx).value().to_string();
                    if entered_pin.len() == 4 {
                        let hashed = bir_core::crypto::hash_pin(&entered_pin);
                        if hashed == this.pin_hash {
                            cx.emit(AuthGateEvent::Authenticated);
                            this.otp_state
                                .update(cx, |input, cx| input.set_value("", window, cx));
                        } else {
                            this.error_msg = Some("Incorrect PIN. Please try again.".to_string());
                            this.otp_state.update(cx, |input, cx| {
                                input.set_value("", window, cx);
                                input.focus(window, cx);
                            });
                        }
                    }
                    cx.notify();
                }
            },
        )
        .detach();

        Self {
            title: title.into(),
            description: description.into(),
            pin_hash,
            otp_state,
            error_msg: None,
            os_auth_triggered: false,
            logo_path: logo_path.into(),
        }
    }

    pub fn focus_input(&self, window: &mut Window, cx: &mut Context<Self>) {
        self.otp_state.update(cx, |input, cx| {
            input.focus(window, cx);
        });
    }
}

impl Render for AuthGateView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let error_msg = self.error_msg.clone();
        let os_triggered = self.os_auth_triggered;

        div()
            .absolute()
            .inset_0()
            .bg(cx.theme().background)
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap_6()
                    .child(
                        gpui::img(self.logo_path.clone())
                            .w(px(200.))
                            .h(px(60.))
                            .object_fit(gpui::ObjectFit::Contain),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_1()
                            .child(
                                div()
                                    .text_xl()
                                    .font_weight(FontWeight::BOLD)
                                    .text_color(cx.theme().foreground)
                                    .child(self.title.clone()),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(self.description.clone()),
                            ),
                    )
                    .child(
                        OtpInput::new(&self.otp_state)
                            .groups(1)
                            .large()
                            .disabled(os_triggered),
                    )
                    .when_some(error_msg, |this, msg| {
                        this.child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().danger)
                                .child(msg),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_center()
                            .gap_4()
                            .child(
                                gpui_component::button::Button::new("auth_gate_os_override")
                                    .label(if os_triggered {
                                        "Waiting for OS..."
                                    } else {
                                        "OS Auth Override"
                                    })
                                    .ghost()
                                    .disabled(os_triggered)
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        if this.os_auth_triggered {
                                            return;
                                        }
                                        this.os_auth_triggered = true;
                                        this.error_msg = None;

                                        #[cfg(any(
                                            target_os = "macos",
                                            target_os = "windows"
                                        ))]
                                        cx.spawn(async move |this, cx| {
                                            use robius_authentication::{
                                                BiometricStrength, Context, PolicyBuilder, Text,
                                                AndroidText, WindowsText,
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
                                                    title: "Authentication Required",
                                                    subtitle: None,
                                                    description: None,
                                                },
                                                apple: "Authenticate to continue",
                                                windows: WindowsText::new_truncated(
                                                    "Authentication Required",
                                                    "Authenticate to override PIN.",
                                                ),
                                            };

                                            let success = Context::new(())
                                                .authenticate(text, &policy)
                                                .await
                                                .is_ok();

                                            let _ = this.update(cx, |this, cx| {
                                                this.os_auth_triggered = false;
                                                if success {
                                                    cx.emit(AuthGateEvent::Authenticated);
                                                } else {
                                                    this.error_msg = Some(
                                                        "OS Authentication failed or canceled."
                                                            .to_string(),
                                                    );
                                                }
                                                cx.notify();
                                            });
                                        })
                                        .detach();

                                        #[cfg(not(any(
                                            target_os = "macos",
                                            target_os = "windows"
                                        )))]
                                        {
                                            this.os_auth_triggered = false;
                                            this.error_msg = Some(
                                                "OS Authentication is not supported on this platform."
                                                    .to_string(),
                                            );
                                            cx.notify();
                                        }
                                    })),
                            )
                            .child(
                                gpui_component::button::Button::new("auth_gate_cancel")
                                    .label("Cancel")
                                    .ghost()
                                    .small()
                                    .disabled(os_triggered)
                                    .on_click(cx.listener(|this, _ev, window, cx| {
                                        this.otp_state.update(cx, |input, cx| {
                                            input.set_value("", window, cx);
                                        });
                                        cx.emit(AuthGateEvent::Cancelled);
                                    })),
                            ),
                    ),
            )
    }
}
