//! Security tab — Profile PIN and TOTP/Authenticator App setup.

use super::*;

impl ProfileManagerView {
    /// Render the "Security" tab (tab index 2).
    ///
    /// Shown only when `global_pins_enabled` is true.
    /// Contains: Profile PIN enable toggle + 4-digit OTP input, TOTP authenticator
    /// app enable toggle (mutually exclusive with PIN), and TOTP QR/secret setup UI.
    pub(super) fn render_security_tab(
        &self,
        global_pins_enabled: bool,
        cx: &Context<Self>,
    ) -> gpui::AnyElement {
        if self.active_tab != 2 || !global_pins_enabled {
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
                    .p_4()
                    .bg(cx.theme().muted)
                    .rounded_md()
                    .flex()
                    .flex_col()
                    .gap_4()
                    // ── PIN toggle ────────────────────────────────────────
                    .child(
                        div()
                            .id("profile_pin_toggle")
                            .flex()
                            .items_center()
                            .gap_2()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _, _, cx| {
                                if this.is_totp_enabled {
                                    return;
                                }
                                this.enable_profile_pin = !this.enable_profile_pin;
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w_4()
                                    .h_4()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(cx.theme().border)
                                    .bg(if self.enable_profile_pin {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().background
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(if self.enable_profile_pin {
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().primary_foreground)
                                            .child("✓")
                                    } else {
                                        div()
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child("Secure this Profile with a PIN"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(
                                                "Require a 4-digit PIN when switching to this profile.",
                                            ),
                                    )
                                    .when(self.is_totp_enabled, |this| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().danger)
                                                .child(
                                                    "Disable Authenticator App first to use PIN.",
                                                ),
                                        )
                                    }),
                            ),
                    )
                    // ── PIN input (shown only when enabled) ───────────────
                    .when(self.enable_profile_pin, |this| {
                        this.child(
                            div()
                                .flex()
                                .flex_col()
                                .gap_2()
                                .child(Self::field_label("4-Digit PIN", cx))
                                .child(OtpInput::new(&self.profile_pin_input).large()),
                        )
                    })
                    // ── TOTP toggle ───────────────────────────────────────
                    .child(
                        div()
                            .id("totp_toggle_btn")
                            .flex()
                            .items_start()
                            .gap_3()
                            .mt_4()
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                if this.enable_profile_pin {
                                    return;
                                }
                                if this.is_totp_enabled {
                                    this.is_totp_enabled = false;
                                    this.stored_totp_secret = None;
                                    this.show_totp_setup = false;
                                } else {
                                    let (secret, qr_path) =
                                        bir_core::crypto::generate_totp_secret(&format!(
                                            "e-BIRForms ({})",
                                            this.tin_input.read(cx).value(cx)
                                        ));
                                    this.totp_secret_temp = Some(secret);
                                    this.totp_qr_path = Some(qr_path);
                                    this.show_totp_setup = true;
                                    this.setup_totp_state
                                        .update(cx, |s, cx| s.focus(window, cx));
                                }
                                cx.notify();
                            }))
                            .child(
                                div()
                                    .w(px(16.))
                                    .h(px(16.))
                                    .mt_1()
                                    .rounded_sm()
                                    .border_1()
                                    .border_color(if self.is_totp_enabled {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().border
                                    })
                                    .bg(if self.is_totp_enabled {
                                        cx.theme().primary
                                    } else {
                                        cx.theme().background
                                    })
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .child(if self.is_totp_enabled {
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().primary_foreground)
                                            .child("✓")
                                    } else {
                                        div()
                                    }),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(FontWeight::MEDIUM)
                                            .text_color(cx.theme().foreground)
                                            .child(
                                                "Secure this Profile with Authenticator App",
                                            ),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(
                                                "Require a 6-digit TOTP code when switching to this profile.",
                                            ),
                                    )
                                    .when(self.enable_profile_pin, |this| {
                                        this.child(
                                            div()
                                                .text_xs()
                                                .text_color(cx.theme().danger)
                                                .child(
                                                    "Disable 4-digit PIN first to use Authenticator App.",
                                                ),
                                        )
                                    }),
                            ),
                    ),
            )
            .into_any_element()
    }
}
