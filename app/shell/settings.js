// Settings page: theme preference + app-lock credentials.
// Keys are shared verbatim with the painted app (theme_preference,
// enable_profile_pins, hide_tax_profiles, app_lock_pin_hash, app_totp_secret).
import { View, div } from "gpui-kit";
import { v_flex, h_flex, OtpInput } from "gpui-base";
import { Button, Checkbox } from "gpui-component";
import { pageHeading, pageSub } from "./pages.js";

function section(cx, title, children) {
  return v_flex()
    .gap(12)
    .p(16)
    .w_full()
    .bg(cx.theme().colors.surface)
    .border(1)
    .border_color(cx.theme().colors.border)
    .rounded_md()
    .child(
      div()
        .text_size(14)
        .font_semibold()
        .text_color(cx.theme().colors.foreground)
        .child(title),
    )
    .children(children.filter(Boolean));
}

function switchRow(cx, id, label, checked, onChange) {
  return h_flex()
    .items_center()
    .w_full()
    .child(
      Checkbox.new(id)
        .label(label)
        .checked(checked)
        .on_change(onChange),
    );
}

function themeChip(cx, state, label, value) {
  const selected = state.themePref === value;
  return div()
    .id(`theme-${value}`)
    .px(16)
    .py(8)
    .border(1)
    .rounded_md()
    .border_color(selected ? cx.theme().colors.primary : cx.theme().colors.border)
    .bg(selected ? cx.theme().colors.accent : cx.theme().colors.surface)
    .text_size(13)
    .font_medium()
    .text_color(
      selected ? cx.theme().colors.primary : cx.theme().colors.muted_foreground,
    )
    .hover((s) => !selected && s.bg(cx.theme().colors.muted))
    .on_click((_ev, cx) => state.setThemePreference(value, cx))
    .child(label);
}

export function renderSettings(cx, state) {
  return div()
    .size_full()
    .overflow_y_scroll()
    .p(24)
    .child(
      v_flex()
        .gap(16)
        .max_w_full()
        .child(pageHeading("Settings", cx))
        .child(
          pageSub(
            "Preferences share the painted app's setting keys, so both apps agree.",
            cx,
          ),
        )
        .child(
          section(cx, "Appearance", [
            h_flex()
              .gap(8)
              .children([
                themeChip(cx, state, "System", "system"),
                themeChip(cx, state, "Dark", "dark"),
                themeChip(cx, state, "Light", "light"),
              ]),
          ]),
        )
        .child(
          section(cx, "Privacy", [
            switchRow(
              cx,
              "set_enable_pins",
              "Require profile PIN / authenticator to unlock",
              state.auth.enable_profile_pins,
              (checked, cx) => state.setAuthFlag("enable_profile_pins", checked, cx),
            ),
            switchRow(
              cx,
              "set_hide_profiles",
              "Hide taxpayer profiles in the sidebar",
              state.auth.hide_tax_profiles,
              (checked, cx) => state.setAuthFlag("hide_tax_profiles", checked, cx),
            ),
          ]),
        )
        .child(
          section(cx, "App lock", [
            div()
              .text_size(13)
              .text_color(cx.theme().colors.muted_foreground)
              .child(
                `Admin PIN ${state.auth.has_admin_pin ? "set" : "not set"} · authenticator ${state.auth.has_admin_totp ? "enrolled" : "not enrolled"}`,
              ),
            h_flex()
              .gap(8)
              .items_center()
              .child(
                OtpInput.new(state.adminPinOtp)
                  .flex()
                  .gap(6)
                  .cell_style((cell) =>
                    cell
                      .size(36)
                      .flex()
                      .items_center()
                      .justify_center()
                      .border(1)
                      .border_color(cx.theme().colors.input)
                      .rounded_md(),
                  )
                  .cell_active_style((cell) =>
                    cell.border_color(cx.theme().colors.primary),
                  )
                  .caret_style((caret) =>
                    caret.w(2).h(16).bg(cx.theme().colors.foreground),
                  ),
              )
              .child(
                Button.new("set_admin_pin")
                  .label(state.auth.has_admin_pin ? "Change admin PIN" : "Set admin PIN")
                  .outline()
                  .on_click((_ev, cx) => state.setAdminPin(cx)),
              )
              .when(state.auth.has_admin_pin, (el) =>
                el.child(
                  Button.new("clear_admin_pin")
                    .label("Clear")
                    .ghost()
                    .on_click((_ev, cx) => state.clearAdminPin(cx)),
                ),
              ),
            h_flex()
              .gap(8)
              .items_center()
              .child(
                Button.new("set_totp")
                  .label(
                    state.auth.has_admin_totp
                      ? "Remove authenticator"
                      : "Enroll authenticator",
                  )
                  .outline()
                  .on_click((_ev, cx) => state.toggleAdminTotp(cx)),
              )
              .when(state.newTotpSecret, (el) =>
                el.child(
                  div()
                    .text_size(13)
                    .text_color(cx.theme().colors.muted_foreground)
                    .child(`Secret: ${state.newTotpSecret}`),
                ),
              ),
            state.settingsNotice
              ? div()
                  .text_size(13)
                  .text_color(cx.theme().colors.muted_foreground)
                  .child(state.settingsNotice)
              : null,
          ]),
        ),
    );
}
