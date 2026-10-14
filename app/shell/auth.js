// App lock + profile unlock gates (bir-desktop lock_screen/auth flow).
// Layout mirrors the painted lock screen: centered card over the dimmed shell.
import { View, div } from "gpui-kit";
import { v_flex, h_flex, OtpInput } from "gpui-base";
import { Button } from "gpui-component";
import { pageHeading } from "./pages.js";

const CARD_W = 420;

function gateCard(cx, children) {
  return div()
    .size_full()
    .flex()
    .items_center()
    .justify_center()
    .bg(cx.theme().colors.background)
    .child(
      v_flex()
        .w(CARD_W)
        .gap(16)
        .p(32)
        .bg(cx.theme().colors.surface)
        .border(1)
        .border_color(cx.theme().colors.border)
        .rounded_md()
        .shadow_lg()
        .children(children.filter(Boolean)),
    );
}

function otpRow(cx, label, otpState) {
  return v_flex()
    .gap(8)
    .child(
      div()
        .text_size(13)
        .font_medium()
        .text_color(cx.theme().colors.muted_foreground)
        .child(label),
    )
    .child(
      OtpInput.new(otpState)
        .flex()
        .gap(8)
        .cell_style((cell) =>
          cell
            .size(44)
            .flex()
            .items_center()
            .justify_center()
            .border(1)
            .border_color(cx.theme().colors.input)
            .rounded_md()
            .bg(cx.theme().colors.background),
        )
        .cell_active_style((cell) => cell.border_color(cx.theme().colors.primary))
        .caret_style((caret) => caret.w(2).h(20).bg(cx.theme().colors.foreground)),
    );
}

function errorLine(cx, state) {
  if (!state.authError) return null;
  return div()
    .text_size(13)
    .text_color("#EF4444")
    .child(state.authError);
}

/** Full-app lock: shown when admin PIN or TOTP is configured. */
export function renderLockScreen(cx, state) {
  const children = [
    pageHeading("e-BIRForms is locked", cx),
    div()
      .text_color(cx.theme().colors.muted_foreground)
      .child("Enter the app PIN or authenticator code to continue."),
  ];
  if (state.auth.has_admin_pin) {
    children.push(otpRow(cx, "App PIN", state.adminPinOtp));
  }
  if (state.auth.has_admin_totp) {
    children.push(otpRow(cx, "Authenticator code", state.adminTotpOtp));
  }
  children.push(errorLine(cx, state));
  children.push(
    h_flex()
      .justify_end()
      .child(
        Button.new("unlock_app_btn")
          .label("Unlock")
          .primary()
          .on_click((_ev, cx) => state.tryUnlockApp(cx)),
      ),
  );
  return gateCard(cx, children);
}

/** Per-profile unlock gate (enable_profile_pins + profile has pin/totp). */
export function renderUnlockGate(cx, state, profile) {
  const children = [
    pageHeading(`Unlock ${profile.full_name || "profile"}`, cx),
    div()
      .text_color(cx.theme().colors.muted_foreground)
      .child(`TIN ${profile.tin} requires its credentials before filing.`),
  ];
  const auth = state.profileAuthCache || { has_pin: false, has_totp: false };
  if (auth.has_pin) {
    children.push(otpRow(cx, "Profile PIN", state.profilePinOtp));
  }
  if (auth.has_totp) {
    children.push(otpRow(cx, "Authenticator code", state.profileTotpOtp));
  }
  children.push(errorLine(cx, state));
  children.push(
    h_flex()
      .justify_between()
      .child(
        Button.new("cancel_unlock_btn")
          .label("Back")
          .outline()
          .on_click((_ev, cx) => state.cancelUnlock(cx)),
      )
      .child(
        Button.new("unlock_profile_btn")
          .label("Unlock")
          .primary()
          .on_click((_ev, cx) => state.tryUnlockProfile(cx)),
      ),
  );
  return gateCard(cx, children);
}
