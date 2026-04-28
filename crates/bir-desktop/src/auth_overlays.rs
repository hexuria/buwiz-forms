//! Authentication overlay rendering for AppState — extracted from app.rs.
//!
//! Contains the profile PIN gate and admin PIN gate overlay methods,
//! including OS biometric override support.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::button::ButtonVariants;
use gpui_component::input::OtpInput;
use gpui_component::*;

use crate::app::{ActiveView, AppState};

impl AppState {
    /// Render the profile PIN authentication overlay (if a profile is pending authentication).
    pub(crate) fn render_profile_auth_overlay(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (profile, _) = self.pending_profile.clone()?;
        let error_msg = self.profile_auth_error.clone();
        let os_triggered = self.os_auth_triggered;

        Some(
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
                            gpui::img("svg/ebirforms.png")
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
                                        .child("Enter PIN to unlock Tax Profile"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(format!("Profile: {}", profile.tin.full())),
                                ),
                        )
                        .child(
                            OtpInput::new(&self.profile_otp_state)
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
                                    gpui_component::button::Button::new("admin_override")
                                        .label(if os_triggered {
                                            "Waiting for OS..."
                                        } else {
                                            "Admin Override"
                                        })
                                        .ghost()
                                        .disabled(os_triggered)
                                        .on_click(cx.listener(|this, _ev, _window, cx| {
                                            if this.os_auth_triggered {
                                                return;
                                            }
                                            this.os_auth_triggered = true;
                                            this.profile_auth_error = None;

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
                                                        title: "Admin Override",
                                                        subtitle: None,
                                                        description: None,
                                                    },
                                                    apple: "Admin Override to Unlock Profile",
                                                    windows: WindowsText::new_truncated(
                                                        "Admin Override",
                                                        "Authenticate to override profile PIN.",
                                                    ),
                                                };

                                                let success = Context::new(())
                                                    .authenticate(text, &policy)
                                                    .await
                                                    .is_ok();

                                                let _ = this.update(cx, |this, cx| {
                                                    this.os_auth_triggered = false;
                                                    if success {
                                                        if let Some((p, a)) =
                                                            this.pending_profile.take()
                                                        {
                                                            this.unlocked_profile = Some((p, a));
                                                        }
                                                    } else {
                                                        this.profile_auth_error = Some(
                                                            "Admin Authentication failed or canceled."
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
                                                this.profile_auth_error = Some(
                                                    "Admin Override is not supported on this platform."
                                                        .to_string(),
                                                );
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    gpui_component::button::Button::new("cancel_profile_pin")
                                        .label("Cancel")
                                        .ghost()
                                        .small()
                                        .disabled(os_triggered)
                                        .on_click(cx.listener(|this, _ev, window, cx| {
                                            this.pending_profile = None;
                                            this.profile_otp_state.update(cx, |input, cx| {
                                                input.set_value("", window, cx)
                                            });
                                            this.focus_handle.focus(window, cx);
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }

    /// Render the admin PIN authentication overlay (if an admin-gated view is pending).
    pub(crate) fn render_admin_auth_overlay(
        &self,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let target = self.pending_admin_view?;
        let error_msg = self.admin_auth_error.clone();
        let os_triggered = self.admin_os_auth_triggered;

        Some(
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
                            gpui::img("images/ebirforms.png")
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
                                        .child("Admin Access Required"),
                                )
                                .child(
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .child(match target {
                                            ActiveView::Settings => {
                                                "Enter App Lock PIN to access Settings"
                                            }
                                            ActiveView::CronTasks => {
                                                "Enter App Lock PIN to access Background Tasks"
                                            }
                                            _ => "Enter App Lock PIN to continue",
                                        }),
                                ),
                        )
                        .child(
                            OtpInput::new(&self.admin_otp_state)
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
                                    gpui_component::button::Button::new("admin_os_override")
                                        .label(if os_triggered {
                                            "Waiting for OS..."
                                        } else {
                                            "OS Auth Override"
                                        })
                                        .ghost()
                                        .disabled(os_triggered)
                                        .on_click(cx.listener(move |this, _ev, _window, cx| {
                                            if this.admin_os_auth_triggered {
                                                return;
                                            }
                                            this.admin_os_auth_triggered = true;
                                            this.admin_auth_error = None;

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
                                                        title: "Admin Override",
                                                        subtitle: None,
                                                        description: None,
                                                    },
                                                    apple:
                                                        "OS Authentication to Override App Lock",
                                                    windows: WindowsText::new_truncated(
                                                        "Admin Override",
                                                        "Authenticate to override App Lock PIN.",
                                                    ),
                                                };

                                                let success = Context::new(())
                                                    .authenticate(text, &policy)
                                                    .await
                                                    .is_ok();

                                                let _ = this.update(cx, |this, cx| {
                                                    this.admin_os_auth_triggered = false;
                                                    if success {
                                                        if let Some(target) =
                                                            this.pending_admin_view.take()
                                                        {
                                                            this.active_view = target;
                                                            if target == ActiveView::CronTasks {
                                                                this.cron_tasks_view.update(
                                                                    cx,
                                                                    |view, cx| {
                                                                        view.load_settings(cx)
                                                                    },
                                                                );
                                                            }
                                                        }
                                                    } else {
                                                        this.admin_auth_error = Some(
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
                                                this.admin_os_auth_triggered = false;
                                                this.admin_auth_error = Some(
                                                    "OS Authentication is not supported on this platform."
                                                        .to_string(),
                                                );
                                                cx.notify();
                                            }
                                        })),
                                )
                                .child(
                                    gpui_component::button::Button::new("cancel_admin_pin")
                                        .label("Cancel")
                                        .ghost()
                                        .small()
                                        .disabled(os_triggered)
                                        .on_click(cx.listener(|this, _ev, window, cx| {
                                            this.pending_admin_view = None;
                                            this.admin_otp_state.update(cx, |input, cx| {
                                                input.set_value("", window, cx)
                                            });
                                            this.focus_handle.focus(window, cx);
                                            cx.notify();
                                        })),
                                ),
                        ),
                )
                .into_any_element(),
        )
    }
}
