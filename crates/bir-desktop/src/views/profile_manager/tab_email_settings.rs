//! Email Settings tab — IMAP/OAuth authentication method selection and connection testing.

use super::*;

impl ProfileManagerView {
    /// Render the "Email Settings" tab (tab index 2).
    ///
    /// Contains: authentication method radio (App Password vs Google OAuth),
    /// IMAP host/email/password fields with edit toggle, OAuth connect button,
    /// connection test button and status indicator, and email tracking status.
    pub(super) fn render_email_settings_tab(&self, cx: &Context<Self>) -> gpui::AnyElement {
        if self.active_tab != 2 {
            return div().into_any_element();
        }

        div()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .flex()
            .flex_col()
            .gap_4()
            .w_full()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(Self::field_label("Authentication Method", cx))
                    .child(
                        div()
                            .flex()
                            .gap_6()
                            .child(
                                div()
                                    .id("app_password_select")
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.email_auth_method = EmailAuthMethod::AppPassword;
                                        this.connection_test_message = None;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .w_4()
                                            .h_4()
                                            .rounded_full()
                                            .border_1()
                                            .border_color(cx.theme().primary)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                if matches!(
                                                    self.email_auth_method,
                                                    EmailAuthMethod::AppPassword
                                                ) {
                                                    div()
                                                        .w_2()
                                                        .h_2()
                                                        .rounded_full()
                                                        .bg(cx.theme().primary)
                                                } else {
                                                    div()
                                                },
                                            ),
                                    )
                                    .child(div().text_sm().child("App Password (Gmail/Outlook/Yahoo)")),
                            )
                            .child(
                                div()
                                    .id("oauth_select")
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .cursor_pointer()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.email_auth_method = EmailAuthMethod::GoogleOAuth;
                                        this.connection_test_message = None;
                                        cx.notify();
                                    }))
                                    .child(
                                        div()
                                            .w_4()
                                            .h_4()
                                            .rounded_full()
                                            .border_1()
                                            .border_color(cx.theme().primary)
                                            .flex()
                                            .items_center()
                                            .justify_center()
                                            .child(
                                                if matches!(
                                                    self.email_auth_method,
                                                    EmailAuthMethod::GoogleOAuth
                                                ) {
                                                    div()
                                                        .w_2()
                                                        .h_2()
                                                        .rounded_full()
                                                        .bg(cx.theme().primary)
                                                } else {
                                                    div()
                                                },
                                            ),
                                    )
                                    .child(div().text_sm().child("Google Account (OAuth2)")),
                            ),
                    )
                    .child(
                        if matches!(self.email_auth_method, EmailAuthMethod::AppPassword) {
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .w_full()
                                .overflow_x_hidden()
                                .child(
                                    div()
                                        .w_full()
                                        .child(Self::field_label("IMAP Host", cx))
                                        .child(Input::new(&self.imap_host_input)),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .child(Self::field_label("IMAP Email", cx))
                                        .child(Input::new(&self.imap_email_input)),
                                )
                                .child(
                                    div()
                                        .w_full()
                                        .child(Self::field_label("App Password", cx))
                                        .child(
                                            div()
                                                .flex()
                                                .gap_2()
                                                .items_center()
                                                .child(
                                                    div().flex_1().child(
                                                        Input::new(&self.imap_password_input)
                                                            .mask_toggle()
                                                            .disabled(!self.is_editing_password),
                                                    ),
                                                )
                                                .child(
                                                    gpui_component::button::Button::new(
                                                        "edit_app_pw",
                                                    )
                                                    .ghost()
                                                    .label(
                                                        if self.is_editing_password
                                                            && self
                                                                .stored_imap_app_password
                                                                .is_some()
                                                        {
                                                            "Cancel"
                                                        } else {
                                                            "Edit"
                                                        },
                                                    )
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.is_editing_password =
                                                            !this.is_editing_password;
                                                        cx.notify();
                                                    })),
                                                ),
                                        )
                                        .child(self.field_error("imap_app_password", cx))
                                        .child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .gap_1()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child("Get App Password:")
                                                        .child(
                                                            div()
                                                                .id("link_google_app_pw")
                                                                .text_xs()
                                                                .text_color(gpui::Hsla::from(
                                                                    gpui::rgba(0x3b82f6ff),
                                                                ))
                                                                .cursor_pointer()
                                                                .hover(|s| s.underline())
                                                                .child("Google")
                                                                .on_click(|_, _, _| {
                                                                    let _ = open::that(
                                                                        "https://myaccount.google.com/apppasswords",
                                                                    );
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(
                                                                    cx.theme().muted_foreground,
                                                                )
                                                                .child("·"),
                                                        )
                                                        .child(
                                                            div()
                                                                .id("link_outlook_app_pw")
                                                                .text_xs()
                                                                .text_color(gpui::Hsla::from(
                                                                    gpui::rgba(0x3b82f6ff),
                                                                ))
                                                                .cursor_pointer()
                                                                .hover(|s| s.underline())
                                                                .child("Outlook")
                                                                .on_click(|_, _, _| {
                                                                    let _ = open::that(
                                                                        "https://account.live.com/proofs/AppPassword",
                                                                    );
                                                                }),
                                                        )
                                                        .child(
                                                            div()
                                                                .text_xs()
                                                                .text_color(
                                                                    cx.theme().muted_foreground,
                                                                )
                                                                .child("·"),
                                                        )
                                                        .child(
                                                            div()
                                                                .id("link_yahoo_app_pw")
                                                                .text_xs()
                                                                .text_color(gpui::Hsla::from(
                                                                    gpui::rgba(0x3b82f6ff),
                                                                ))
                                                                .cursor_pointer()
                                                                .hover(|s| s.underline())
                                                                .child("Yahoo")
                                                                .on_click(|_, _, _| {
                                                                    let _ = open::that(
                                                                        "https://login.yahoo.com/account/security/app-passwords",
                                                                    );
                                                                }),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .text_xs()
                                                        .text_color(cx.theme().muted_foreground)
                                                        .child("Note: You must enable 2-Step Verification (2FA) in your account settings before you can generate an App Password."),
                                                ),
                                        ),
                                )
                        } else {
                            div()
                                .flex()
                                .flex_col()
                                .gap_3()
                                .w_full()
                                .overflow_x_hidden()
                                .child(if self.oauth_connected {
                                    let email = self.imap_email_input.read(cx).value();
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .w_full()
                                        .overflow_x_hidden()
                                        .child(format!("Connected as {}", email))
                                } else {
                                    div()
                                })
                                .child(
                                    div()
                                        .flex()
                                        .items_center()
                                        .gap_2()
                                        .child(
                                            gpui_component::button::Button::new("connect_google")
                                                .label(if self.oauth_connected {
                                                    "Re-authorize"
                                                } else {
                                                    "Connect Google Account"
                                                })
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    let editing_id = this.editing_id;
                                                    cx.spawn(async move |this, cx| {
                                                        let (tx, rx) =
                                                            tokio::sync::oneshot::channel();
                                                        std::thread::spawn(move || {
                                                            let res =
                                                                bir_core::email::start_oauth_flow();
                                                            let _ = tx.send(res);
                                                        });
                                                        let result = rx.await.unwrap_or_else(
                                                            |_| {
                                                                Err(anyhow::anyhow!(
                                                                    "OAuth thread failed"
                                                                ))
                                                            },
                                                        );

                                                        let _ = this.update(cx, |this, cx| {
                                                            match result {
                                                                Ok((
                                                                    email,
                                                                    access_token,
                                                                    refresh_token,
                                                                )) => {
                                                                    this.oauth_connected = true;
                                                                    this.email_tracking_enabled =
                                                                        true;
                                                                    this.stored_oauth_access_token =
                                                                        Some(
                                                                            access_token.clone(),
                                                                        );
                                                                    this.stored_oauth_refresh_token = Some(refresh_token.clone());
                                                                    this.connection_test_message = Some((true, format!("Google account connected successfully for {}.", email)));

                                                                    if let Some(id) = editing_id
                                                                        && let Ok(db) =
                                                                            this.db.lock()
                                                                        && let Ok(Some(mut profile)) = db.get_profile(&id.to_string())
                                                                    {
                                                                        profile.email = email;
                                                                        profile.oauth_access_token =
                                                                            Some(access_token);
                                                                        profile.oauth_refresh_token =
                                                                            Some(refresh_token);
                                                                        let _ = db.save_profile(
                                                                            profile,
                                                                        );
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    this.connection_test_message = Some((false, format!("OAuth failed: {}", e)));
                                                                }
                                                            }
                                                            cx.notify();
                                                        });
                                                    })
                                                    .detach();
                                                })),
                                        )
                                        .when(self.oauth_connected, |this| {
                                            this.child(
                                                div()
                                                    .text_sm()
                                                    .text_color(gpui::Hsla::from(gpui::rgba(
                                                        0x22c55eff,
                                                    )))
                                                    .child("● Connected ✓"),
                                            )
                                        }),
                                )
                        },
                    )
                    .child(
                        div()
                            .mt_2()
                            .flex()
                            .flex_col()
                            .items_start()
                            .gap_4()
                            .child(
                                gpui_component::button::Button::new("test_connection")
                                    .label(if matches!(
                                        self.email_auth_method,
                                        EmailAuthMethod::AppPassword
                                    ) {
                                        "Verify App Password"
                                    } else {
                                        "Verify Google OAuth2"
                                    })
                                    .outline()
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        let profile = this.current_profile(cx);
                                        let typed_password = this
                                            .imap_password_input
                                            .read(cx)
                                            .value()
                                            .to_string()
                                            .replace(' ', "");
                                        let mut test_profile = profile.clone();
                                        if !typed_password.is_empty() {
                                            test_profile.imap_app_password =
                                                Some(typed_password.clone());
                                        }

                                        this.connection_test_message =
                                            Some((true, "Testing connection...".to_string()));
                                        cx.notify();

                                        cx.spawn(async move |this, cx| {
                                            let (tx, rx) = tokio::sync::oneshot::channel();
                                            std::thread::spawn(move || {
                                                let res = bir_core::email::test_connection(
                                                    &test_profile,
                                                );
                                                let _ = tx.send(res);
                                            });
                                            let result = rx.await.unwrap_or_else(|_| {
                                                Err(anyhow::anyhow!(
                                                    "Test connection thread failed"
                                                ))
                                            });

                                            let _ = this.update(cx, |this, cx| {
                                                match result {
                                                    Ok(new_access_token) => {
                                                        this.connection_test_message = Some((
                                                            true,
                                                            "Connection successful! ✓ Email tracking is now active.".to_string(),
                                                        ));
                                                        this.email_tracking_enabled = true;
                                                        if matches!(
                                                            this.email_auth_method,
                                                            EmailAuthMethod::AppPassword
                                                        ) {
                                                            this.oauth_connected = false;
                                                        }
                                                        if let Some(token) = new_access_token {
                                                            this.stored_oauth_access_token =
                                                                Some(token.clone());
                                                            if let Ok(db) = this.db.lock()
                                                                && let Some(id) = this.editing_id
                                                                && let Ok(Some(mut prof)) =
                                                                    db.get_profile(
                                                                        &id.to_string(),
                                                                    )
                                                            {
                                                                prof.oauth_access_token =
                                                                    Some(token);
                                                                let _ = db.save_profile(prof);
                                                            }
                                                        }
                                                    }
                                                    Err(e) => {
                                                        this.connection_test_message = Some((
                                                            false,
                                                            format!("Connection failed: {}", e),
                                                        ));
                                                        this.email_tracking_enabled = false;
                                                    }
                                                }
                                                cx.notify();
                                            });
                                        })
                                        .detach();
                                    })),
                            )
                            .child(
                                if let Some((success, msg)) = self.connection_test_message.clone() {
                                    div()
                                        .text_sm()
                                        .whitespace_normal()
                                        .w_full()
                                        .overflow_x_hidden()
                                        .text_color(if success {
                                            gpui::Hsla::from(gpui::rgba(0x22c55eff))
                                        } else {
                                            gpui::Hsla::from(gpui::rgba(0xef4444ff))
                                        })
                                        .child(msg)
                                } else {
                                    div()
                                },
                            ),
                    )
                    .child(
                        div()
                            .mt_4()
                            .pt_4()
                            .border_t_1()
                            .border_color(cx.theme().border)
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(if self.email_tracking_enabled {
                                div()
                                    .text_sm()
                                    .text_color(crate::theme::success_on_tint(cx.theme()))
                                    .child("● Automated BIR Receipt Tracking is active")
                            } else {
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("○ Verify your connection to activate email tracking")
                            }),
                    ),
            )
            .into_any_element()
    }
}
