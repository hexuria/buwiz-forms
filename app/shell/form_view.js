// Form kit: the shared draft-backed field grid every form view uses.
//
// A form spec declares its fields; the kit owns InputState lifecycle, draft
// load/save through `get_form_draft`/`save_form_draft`, the filing-status
// tracker, and the header. S4 grows FORM_SPECS until every catalog entry has
// one — forms without a spec keep the launch notice instead of faking a view.
import { View, div } from "gpui-kit";
import { v_flex, h_flex, Input } from "gpui-base";
import { Button, Checkbox, VForm, Field } from "gpui-component";
import { pageHeading, pageSub } from "./pages.js";

// kind: "text" | "number" | "money" | "bool" | "date"
export const FORM_SPECS = {
  "2551Q": {
    code: "2551Q",
    title: "Quarterly Percentage Tax Return",
    periods: "quarterly",
    fields: [
      { key: "rdo_code", label: "RDO code", kind: "text" },
      { key: "taxpayer_name", label: "Taxpayer name", kind: "text" },
      { key: "registered_address", label: "Registered address", kind: "text" },
      { key: "zip_code", label: "ZIP code", kind: "text" },
      { key: "contact_number", label: "Contact number", kind: "text" },
      { key: "email", label: "Email", kind: "text" },
      { key: "is_amended", label: "Amended return", kind: "bool" },
      {
        key: "number_of_attached_sheets",
        label: "Attached sheets",
        kind: "number",
      },
      { key: "tax_relief", label: "Tax relief claimed", kind: "bool" },
      {
        key: "tax_relief_specification",
        label: "Tax relief specification",
        kind: "text",
      },
      {
        key: "total_tax_due",
        label: "Total tax due (item 25)",
        kind: "money",
      },
      {
        key: "creditable_tax_withheld",
        label: "Creditable tax withheld (item 26)",
        kind: "money",
      },
      {
        key: "tax_paid_previous",
        label: "Tax paid previous periods (item 27)",
        kind: "money",
      },
      {
        key: "other_tax_credit",
        label: "Other tax credit (item 28)",
        kind: "money",
      },
      {
        key: "other_tax_credit_description",
        label: "Other credit description",
        kind: "text",
      },
    ],
    // Fields the kit fills from the active profile when a draft is new.
    profileDefaults: {
      rdo_code: "rdo_code",
      taxpayer_name: "full_name",
      registered_address: "registered_address",
      zip_code: "zip_code",
      contact_number: "phone",
      email: "email",
    },
  },
};

const FILING_STAGES = ["Draft", "Queued", "Submitted", "Confirmed", "Paid"];

function statusTracker(cx, status) {
  const current = FILING_STAGES.indexOf(status);
  return h_flex()
    .items_center()
    .gap(4)
    .children(
      FILING_STAGES.flatMap((stage, i) => {
        const reached = i <= current;
        const chip = div()
          .px(10)
          .py(4)
          .rounded_md()
          .text_size(12)
          .font_medium()
          .bg(
            i === current
              ? cx.theme().colors.primary
              : cx.theme().colors.muted,
          )
          .text_color(
            i === current
              ? cx.theme().colors.primary_foreground
              : cx.theme().colors.muted_foreground,
          )
          .child(stage);
        return i === 0
          ? [chip]
          : [
              div()
                .text_size(12)
                .text_color(
                  reached
                    ? cx.theme().colors.foreground
                    : cx.theme().colors.muted_foreground,
                )
                .child("→"),
              chip,
            ];
      }),
    );
}

function specField(cx, state, field) {
  if (field.kind === "bool") {
    return Field.new()
      .label("")
      .child(
        Checkbox.new(`ff_${field.key}`)
          .label(field.label)
          .checked(Boolean(state.formDraft[field.key]))
          .on_change((checked, cx) => {
            state.formDraft[field.key] = checked;
            cx.notify();
          }),
      );
  }
  return Field.new()
    .label(field.label)
    .child(Input.new(state.formInputs[field.key]).w_full());
}

export function renderFormView(cx, state) {
  const spec = state.formSpec;
  const draft = state.formDraft;
  const status = state.formDraftStatus || "Draft";

  return div()
    .size_full()
    .overflow_y_scroll()
    .p(24)
    .child(
      v_flex()
        .gap(16)
        .max_w_full()
        .child(
          h_flex()
            .items_center()
            .justify_between()
            .child(pageHeading(`${spec.code} — ${spec.title}`, cx))
            .child(
              pageSub(
                `${state.activeProfileTin} · ${state.formYear}${state.formQuarter ? ` Q${state.formQuarter}` : ""} · ${spec.periods}`,
                cx,
              ),
            ),
        )
        .child(statusTracker(cx, status))
        .child(
          VForm.new()
            .columns(2)
            .children(spec.fields.map((f) => specField(cx, state, f))),
        )
        .when(Boolean(state.formError), (el) =>
          el.child(
            div()
              .text_size(13)
              .text_color("#EF4444")
              .child(state.formError),
          ),
        )
        .when(Boolean(state.formNotice), (el) =>
          el.child(
            div()
              .text_size(13)
              .text_color(cx.theme().colors.muted_foreground)
              .child(state.formNotice),
          ),
        )
        .child(
          h_flex()
            .gap(8)
            .justify_end()
            .pt(8)
            .border_t(1)
            .border_color(cx.theme().colors.border)
            .child(
              Button.new("form_back")
                .label("Back to dashboard")
                .ghost()
                .on_click((_ev, cx) => state.navigate("dashboard", cx)),
            )
            .child(
              Button.new("form_save_draft")
                .label("Save draft")
                .primary()
                .on_click((_ev, cx) => state.saveFormDraft(cx)),
            ),
        ),
    );
}
