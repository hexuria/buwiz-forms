//! Email Settings tab — IMAP/OAuth authentication method selection and connection testing.

use super::*;
use gpui_rsx::rsx;

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

        let root = rsx! {
            <div
                p_4
                rounded_lg
                border_1
                border_color={cx.theme().border}
                bg={cx.theme().background}
                flex
                flex_col
                gap_4
                w_full
                min_w_0
            >
                <div flex flex_col gap_4>
                    {Self::field_label("Authentication Method", cx)}
                    <div flex gap_6>
                        <div
                            id={"app_password_select"}
                            flex
                            items_center
                            gap_2
                            cursor_pointer
                            on_click={cx.listener(|this, _, _, cx| {
                                this.email_auth_method = EmailAuthMethod::AppPassword;
                                this.connection_test_message = None;
                                cx.notify();
                            })}
                        >
                            <div
                                w_4
                                h_4
                                rounded_full
                                border_1
                                border_color={cx.theme().primary}
                                flex
                                items_center
                                justify_center
                            >
                                {if matches!(self.email_auth_method, EmailAuthMethod::AppPassword) {
                                    div().w_2().h_2().rounded_full().bg(cx.theme().primary)
                                } else {
                                    div()
                                }}
                            </div>
                            <div text_sm>{"App Password (Gmail/Outlook/Yahoo)"}</div>
                        </div>
                        <div
                            id={"oauth_select"}
                            flex
                            items_center
                            gap_2
                            cursor_pointer
                            on_click={cx.listener(|this, _, _, cx| {
                                this.email_auth_method = EmailAuthMethod::GoogleOAuth;
                                this.connection_test_message = None;
                                cx.notify();
                            })}
                        >
                            <div
                                w_4
                                h_4
                                rounded_full
                                border_1
                                border_color={cx.theme().primary}
                                flex
                                items_center
                                justify_center
                            >
                                {if matches!(self.email_auth_method, EmailAuthMethod::GoogleOAuth) {
                                    div().w_2().h_2().rounded_full().bg(cx.theme().primary)
                                } else {
                                    div()
                                }}
                            </div>
                            <div text_sm>{"Google Account (OAuth2)"}</div>
                        </div>
                    </div>
                    {if matches!(self.email_auth_method, EmailAuthMethod::AppPassword) {
                        rsx! {
                            <div flex flex_col gap_3 w_full overflow_x_hidden>
                                <div w_full>
                                    {Self::field_label("IMAP Host", cx)}
                                    {Input::new(&self.imap_host_input)}
                                </div>
                                <div w_full>
                                    {Self::field_label("IMAP Email", cx)}
                                    {Input::new(&self.imap_email_input)}
                                </div>
                                <div w_full>
                                    {Self::field_label("App Password", cx)}
                                    <div flex gap_2 items_center>
                                        <div flex_1>
                                            {Input::new(&self.imap_password_input)
                                                .mask_toggle()
                                                .disabled(!self.is_editing_password)}
                                        </div>
                                        {gpui_component::button::Button::new("edit_app_pw")
                                            .ghost()
                                            .label(
                                                if self.is_editing_password
                                                    && self.stored_imap_app_password.is_some()
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
                                            }))}
                                    </div>
                                    {self.field_error("imap_app_password", cx)}
                                    <div flex flex_col gap_2>
                                        <div
                                            flex
                                            items_center
                                            gap_1
                                            text_xs
                                            text_color={cx.theme().muted_foreground}
                                        >
                                            {"Get App Password:"}
                                            {div()
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
                                                })}
                                            <div text_xs text_color={cx.theme().muted_foreground}>
                                                {"·"}
                                            </div>
                                            {div()
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
                                                })}
                                            <div text_xs text_color={cx.theme().muted_foreground}>
                                                {"·"}
                                            </div>
                                            {div()
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
                                                })}
                                        </div>
                                        <div text_xs text_color={cx.theme().muted_foreground}>
                                            {"Note: You must enable 2-Step Verification (2FA) in your account settings before you can generate an App Password."}
                                        </div>
                                    </div>
                                </div>
                            </div>
                        }
                    } else {
                        rsx! {
                            <div flex flex_col gap_3 w_full overflow_x_hidden>
                                {if self.oauth_connected {
                                    let email = self
                                        .stored_oauth_inbox_email
                                        .clone()
                                        .filter(|email| !email.trim().is_empty())
                                        .unwrap_or_else(|| {
                                            self.imap_email_input.read(cx).value().to_string()
                                        });
                                    div()
                                        .text_sm()
                                        .text_color(cx.theme().muted_foreground)
                                        .w_full()
                                        .overflow_x_hidden()
                                        .child(format!("Connected as {}", email))
                                } else {
                                    div()
                                }}
                                {div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .w_full()
                                    .child("Profiles that share this inbox share one Google OAuth credential set. Re-authorize or Disconnect updates every profile using this email.")}
                                {
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
                                                    let source_tin = this.persisted_profile_tin.clone();
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
                                                            this.apply_google_oauth_result(
                                                                result, source_tin,
                                                            );
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
                                            .child(
                                                gpui_component::button::Button::new(
                                                    "disconnect_google",
                                                )
                                                .ghost()
                                                .label("Disconnect")
                                                .on_click(cx.listener(|this, _, _, cx| {
                                                    this.disconnect_shared_google_oauth(cx);
                                                })),
                                            )
                                        })
                                }
                            </div>
                        }
                    }}
                    <div mt_2 flex flex_col items_start gap_4>
                        {gpui_component::button::Button::new("test_connection")
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
                                                            let inbox = this
                                                                .stored_oauth_inbox_email
                                                                .clone()
                                                                .unwrap_or_else(|| {
                                                                    this.current_profile(cx)
                                                                        .inbox_email()
                                                                        .to_string()
                                                                });
                                                            if let Ok(db) = this.db.lock() {
                                                                let _ = db
                                                                    .update_inbox_oauth_access_token(
                                                                        &inbox, &token,
                                                                    );
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
                                    }))}
                        {if let Some((success, msg)) = self.connection_test_message.clone() {
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
                        }}
                    </div>
                    <div
                        mt_4
                        pt_4
                        border_t_1
                        border_color={cx.theme().border}
                        flex
                        items_center
                        gap_2
                    >
                        {if self.email_tracking_enabled {
                            div()
                                .text_sm()
                                .text_color(crate::theme::success_on_tint(cx.theme()))
                                .child("● Automated BIR Receipt Tracking is active")
                        } else {
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child("○ Verify your connection to activate email tracking")
                        }}
                    </div>
                </div>
            </div>
        };
        root.into_any_element()
    }

    fn apply_google_oauth_result(
        &mut self,
        result: Result<(String, String, String), anyhow::Error>,
        source_tin: Option<String>,
    ) {
        match result {
            Ok((email, access_token, refresh_token)) => {
                if refresh_token.trim().is_empty() {
                    self.oauth_connected = false;
                    self.connection_test_message = Some((
                        false,
                        "Google did not return a refresh token. Re-authorize with consent so a new refresh token is issued.".to_string(),
                    ));
                    return;
                }

                self.oauth_connected = true;
                self.email_tracking_enabled = true;
                self.email_auth_method = EmailAuthMethod::GoogleOAuth;
                self.stored_oauth_access_token = Some(access_token.clone());
                self.stored_oauth_refresh_token = Some(refresh_token.clone());
                self.stored_oauth_inbox_email = Some(email.clone());
                self.connection_test_message = Some((
                    true,
                    format!("Google account connected successfully for {}.", email),
                ));

                match self.db.lock() {
                    Ok(db) => match db.persist_google_oauth_for_inbox(
                        &email,
                        &access_token,
                        &refresh_token,
                        source_tin.as_deref(),
                    ) {
                        Ok(_) => {}
                        Err(error) => {
                            self.oauth_connected = false;
                            self.connection_test_message = Some((
                                false,
                                format!(
                                    "Google authorized {}, but the tokens could not be saved: {}",
                                    email, error
                                ),
                            ));
                        }
                    },
                    Err(_) => {
                        self.oauth_connected = false;
                        self.connection_test_message = Some((
                            false,
                            "Google authorized the account, but the database lock was busy. Try Re-authorize.".to_string(),
                        ));
                    }
                }
            }
            Err(error) => {
                self.connection_test_message = Some((false, format!("OAuth failed: {}", error)));
            }
        }
    }

    fn disconnect_shared_google_oauth(&mut self, cx: &mut Context<Self>) {
        let inbox = self
            .stored_oauth_inbox_email
            .clone()
            .filter(|email| !email.trim().is_empty())
            .unwrap_or_else(|| self.imap_email_input.read(cx).value().trim().to_string());

        if inbox.trim().is_empty() {
            self.connection_test_message = Some((
                false,
                "No inbox email is set, so there is nothing to disconnect.".to_string(),
            ));
            cx.notify();
            return;
        }

        match self.db.lock() {
            Ok(db) => {
                if let Err(error) = db.disconnect_google_oauth_for_inbox(&inbox) {
                    self.connection_test_message =
                        Some((false, format!("Disconnect failed: {}", error)));
                    cx.notify();
                    return;
                }
            }
            Err(_) => {
                self.connection_test_message = Some((
                    false,
                    "Could not disconnect because the database lock was busy.".to_string(),
                ));
                cx.notify();
                return;
            }
        }

        self.oauth_connected = false;
        self.email_tracking_enabled = false;
        self.stored_oauth_access_token = None;
        self.stored_oauth_refresh_token = None;
        self.stored_oauth_inbox_email = None;
        self.connection_test_message = Some((
            true,
            format!(
                "Disconnected Google OAuth for {} across every profile sharing this inbox.",
                inbox
            ),
        ));
        cx.notify();
    }
}
