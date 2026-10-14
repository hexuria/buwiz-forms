// Profile manager: identity editor, archive/delete, PIN + TOTP credentials.
// The painted ProfileManager also edits compliance versions and e-mail
// tracking; those land with their own slices — this page covers the identity
// record and the unlock credentials the S2 gate needs.
import { View, div } from "gpui-kit";
import { v_flex, h_flex, Input, InputState, OtpInput } from "gpui-base";
import { Button, Checkbox } from "gpui-component";
import { pageHeading, pageSub } from "./pages.js";

export const TAXPAYER_TYPES = [
  "Individual",
  "Corporation",
  "Partnership",
  "Cooperative",
  "Estate",
  "Trust",
];

/** Digits-only TIN → the {segment1..3, branch} shape bir-core serializes. */
export function parseTin(raw) {
  const digits = (raw || "").replace(/\D/g, "");
  if (digits.length < 12 || digits.length > 14) return null;
  return {
    segment1: digits.slice(0, 3),
    segment2: digits.slice(3, 6),
    segment3: digits.slice(6, 9),
    branch: digits.slice(9),
  };
}

const FIELD_DEFS = [
  ["full_name", "Full name"],
  ["tin", "TIN (digits or dashed)"],
  ["rdo_code", "RDO code"],
  ["email", "Email"],
  ["phone", "Phone"],
  ["line_of_business", "Line of business"],
  ["registered_address", "Registered address"],
  ["zip_code", "ZIP code"],
];

/** Fresh blank profile — every non-defaulted field serde requires. */
export function blankProfile() {
  return {
    id: null,
    full_name: "",
    tin: { segment1: "", segment2: "", segment3: "", branch: "" },
    rdo_code: "",
    line_of_business: "",
    registered_address: "",
    zip_code: "",
    phone: "",
    email: "",
    default_form_type: "",
    taxpayer_type: "Individual",
    is_vat_registered: false,
    is_bmbe: false,
    is_gpp_partner: false,
    is_create_msme: false,
    is_expanded_withholding_agent: false,
    atc_codes: [],
    excise_tax_categories: [],
    tax_elections: [],
    is_archived: false,
  };
}

function inputRow(cx, label, inputState) {
  return v_flex()
    .gap(4)
    .w_full()
    .child(
      div()
        .text_size(12)
        .font_medium()
        .text_color(cx.theme().colors.muted_foreground)
        .child(label),
    )
    .child(
      Input.new(inputState)
        .w_full()
        .h(28)
        .px(8)
        .border(1)
        .border_color(cx.theme().colors.input)
        .bg(cx.theme().colors.surface)
        .text_size(12),
    );
}

function typeChip(cx, state, value) {
  const selected = state.editing.taxpayer_type === value;
  return div()
    .id(`ptype-${value.toLowerCase()}`)
    .px(12)
    .py(6)
    .border(1)
    .rounded_full()
    .border_color(selected ? cx.theme().colors.primary : cx.theme().colors.border)
    .bg(selected ? cx.theme().colors.accent : cx.theme().colors.surface)
    .text_size(12)
    .font_medium()
    .text_color(
      selected ? cx.theme().colors.primary : cx.theme().colors.muted_foreground,
    )
    .hover((s) => !selected && s.bg(cx.theme().colors.muted))
    .on_click((_ev, cx) => {
      state.editing.taxpayer_type = value;
      cx.notify();
    })
    .child(value);
}

export function renderProfileManager(cx, state) {
  const editing = state.editing;
  const isNew = editing.id == null;
  const auth = state.editingAuth || { has_pin: false, has_totp: false };

  const rows = [
    h_flex()
      .items_center()
      .justify_between()
      .child(
        pageHeading(
          isNew ? "New taxpayer profile" : "Edit taxpayer profile",
          cx,
        ),
      )
      .child(
        pageSub(
          isNew
            ? "Core identity — compliance versions and e-mail tracking arrive in their slices."
            : `TIN ${state.editingTinDisplay}`,
          cx,
        ),
      ),
    h_flex()
      .gap(16)
      .children(
        FIELD_DEFS.slice(0, 4).map(([key, label]) =>
          inputRow(cx, label, state.editorInputs[key]),
        ),
      ),
    h_flex()
      .gap(16)
      .children(
        FIELD_DEFS.slice(4).map(([key, label]) =>
          inputRow(cx, label, state.editorInputs[key]),
        ),
      ),
    v_flex()
      .gap(8)
      .child(
        div()
          .text_size(12)
          .font_medium()
          .text_color(cx.theme().colors.muted_foreground)
          .child("Taxpayer type"),
      )
      .child(
        h_flex()
          .gap(8)
          .flex_wrap()
          .children(TAXPAYER_TYPES.map((t) => typeChip(cx, state, t))),
      ),
    h_flex()
      .gap(24)
      .child(
        Checkbox.new("pedit_vat")
          .label("VAT registered")
          .checked(Boolean(editing.is_vat_registered))
          .on_change((checked, cx) => {
            editing.is_vat_registered = checked;
            cx.notify();
          }),
      )
      .child(
        Checkbox.new("pedit_employees")
          .label("Has employees")
          .checked(Boolean(editing.has_employees))
          .on_change((checked, cx) => {
            editing.has_employees = checked;
            cx.notify();
          }),
      ),
    state.editorError
      ? div().text_size(13).text_color("#EF4444").child(state.editorError)
      : null,
    state.editorNotice
      ? div()
          .text_size(13)
          .text_color(cx.theme().colors.muted_foreground)
          .child(state.editorNotice)
      : null,
  ];

  // Credential + destructive actions on saved profiles only.
  if (!isNew) {
    rows.push(
      v_flex()
        .gap(8)
        .pt(16)
        .border_t(1)
        .border_color(cx.theme().colors.border)
        .child(
          div()
            .text_size(12)
            .font_medium()
            .text_color(cx.theme().colors.muted_foreground)
            .child(
              `Unlock credentials — PIN ${auth.has_pin ? "set" : "not set"}, authenticator ${auth.has_totp ? "enrolled" : "not enrolled"}`,
            ),
        )
        .child(
          h_flex()
            .gap(8)
            .items_center()
            .child(
              OtpInput.new(state.profilePinOtp)
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
              Button.new("pedit_set_pin")
                .label(auth.has_pin ? "Change PIN" : "Set PIN")
                .outline()
                .on_click((_ev, cx) => state.setProfilePin(cx)),
            )
            .when(auth.has_pin, (el) =>
              el.child(
                Button.new("pedit_clear_pin")
                  .label("Clear PIN")
                  .ghost()
                  .on_click((_ev, cx) => state.clearProfilePin(cx)),
              ),
            ),
        ),
    );
    rows.push(
      h_flex()
        .gap(8)
        .pt(8)
        .child(
          Button.new("pedit_archive")
            .label(editing.is_archived ? "Unarchive" : "Archive")
            .outline()
            .on_click((_ev, cx) => state.toggleArchiveProfile(cx)),
        )
        .child(
          Button.new("pedit_delete")
            .label(
              state.deleteArmed
                ? "Confirm delete — this cannot be undone"
                : "Delete profile",
            )
            .danger()
            .on_click((_ev, cx) => state.deleteProfile(cx)),
        ),
    );
  }

  rows.push(
    h_flex()
      .gap(8)
      .justify_end()
      .pt(16)
      .child(
        Button.new("pedit_cancel")
          .label("Cancel")
          .ghost()
          .on_click((_ev, cx) =>
            state.navigate(
              state.activeProfileTin ? "dashboard" : "global_dashboard",
              cx,
            ),
          ),
      )
      .child(
        Button.new("pedit_save")
          .label(isNew ? "Create profile" : "Save profile")
          .primary()
          .on_click((_ev, cx) => state.saveEditingProfile(cx)),
      ),
  );

  return div()
    .size_full()
    .overflow_y_scroll()
    .p(24)
    .child(v_flex().gap(16).max_w_full().children(rows.filter(Boolean)));
}
