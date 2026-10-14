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
import { EXTRACTED_SPECS } from "./specs.js";

// kind: "text" | "number" | "money" | "bool" | "date"
// Hand-tuned specs override the ones generated from the painted views.
const HAND_SPECS = {
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
  // Typed monthly withholding return — dedicated save_1601c_draft path on the
  // host. Painted view renders schedule grids; the kit covers the flat fields.
  "1601C": {
    code: "1601C",
    title: "Monthly Remittance Return of Income Taxes Withheld on Compensation",
    periods: "monthly",
    fields: [
      { key: "is_amended", label: "Amended return", kind: "bool" },
      {
        key: "number_of_sheets",
        label: "No. of sheets attached",
        kind: "number",
      },
      { key: "tax_relief", label: "Tax relief claimed", kind: "bool" },
      {
        key: "tax_relief_specification",
        label: "Tax relief specification",
        kind: "text",
      },
      { key: "atc", label: "ATC", kind: "text" },
      {
        key: "category_of_agent",
        label: "Category of withholding agent (P/G)",
        kind: "text",
      },
      {
        key: "tax_14_total_compensation",
        label: "14 — Total compensation",
        kind: "money",
      },
      {
        key: "tax_15_statutory_minimum_wage",
        label: "15 — Less: statutory minimum wage",
        kind: "money",
      },
      {
        key: "tax_16_holiday_pay",
        label: "16 — Holiday pay / OT / night diff / hazard",
        kind: "money",
      },
      {
        key: "tax_17_13th_month_pay",
        label: "17 — 13th month pay & other benefits",
        kind: "money",
      },
      {
        key: "tax_18_de_minimis",
        label: "18 — De minimis benefits",
        kind: "money",
      },
      {
        key: "tax_19_sss_gsis",
        label: "19 — SSS/GSIS & other contributions",
        kind: "money",
      },
      {
        key: "tax_20_other_name",
        label: "20 — Other exemption (name)",
        kind: "text",
      },
      {
        key: "tax_20_other_amount",
        label: "20 — Other exemption (amount)",
        kind: "money",
      },
      {
        key: "tax_23_not_subject",
        label: "23 — Compensation not subject to withholding",
        kind: "money",
      },
      {
        key: "tax_25_total_taxes_withheld",
        label: "25 — Total taxes withheld",
        kind: "money",
      },
      {
        key: "tax_28_tax_remitted_previously",
        label: "28 — Tax remitted in previous months",
        kind: "money",
      },
      {
        key: "tax_29_other_remittances_name",
        label: "29 — Other remittances (name)",
        kind: "text",
      },
      {
        key: "tax_29_other_remittances_amount",
        label: "29 — Other remittances (amount)",
        kind: "money",
      },
    ],
    profileDefaults: {
      rdo_code: "rdo_code",
      taxpayer_name: "full_name",
      registered_address: "registered_address",
      zip_code: "zip_code",
      contact_number: "phone",
      email: "email",
    },
  },
  // Typed quarterly VAT return — dedicated typed draft path. Painted view has
  // schedule grids; the kit covers the reviewed raw-state bindings.
  "2550Q": {
    code: "2550Q",
    title: "Quarterly Value-Added Tax Return",
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
        key: "is_short_period_return",
        label: "Short period return",
        kind: "bool",
      },
      {
        key: "is_availing_tax_relief",
        label: "Availing tax relief",
        kind: "bool",
      },
      {
        key: "tax_relief_details",
        label: "Tax relief details",
        kind: "text",
      },
      {
        key: "number_of_sheets",
        label: "No. of sheets attached",
        kind: "number",
      },
      {
        key: "item_31a_vatable_sales",
        label: "31A — Vatable sales/receipts",
        kind: "money",
      },
      {
        key: "item_32a_zero_rated_sales",
        label: "32A — Zero-rated sales",
        kind: "money",
      },
      {
        key: "item_33a_exempt_sales",
        label: "33A — Exempt sales",
        kind: "money",
      },
      {
        key: "item_35b_less_output_vat_uncollected",
        label: "35B — Less: output VAT uncollected",
        kind: "money",
      },
      {
        key: "item_36b_add_output_vat_recovered",
        label: "36B — Add: output VAT recovered",
        kind: "money",
      },
      {
        key: "item_38b_input_tax_carried",
        label: "38B — Input tax carried over",
        kind: "money",
      },
      {
        key: "item_40b_transitional_input_tax",
        label: "40B — Transitional input tax",
        kind: "money",
      },
      {
        key: "item_41b_presumptive_input_tax",
        label: "41B — Presumptive input tax",
        kind: "money",
      },
      {
        key: "item_42b_other_input_tax",
        label: "42B — Other input tax",
        kind: "money",
      },
      {
        key: "item_44b_domestic_input_tax",
        label: "44B — Domestic purchases input tax",
        kind: "money",
      },
      {
        key: "item_45b_nonresident_service_input_tax",
        label: "45B — Non-resident services input tax",
        kind: "money",
      },
      {
        key: "item_46b_import_input_tax",
        label: "46B — Importations input tax",
        kind: "money",
      },
      {
        key: "item_47b_other_input_tax",
        label: "47B — Other purchases input tax",
        kind: "money",
      },
      {
        key: "item_54b_vat_refund_or_tcc_claimed",
        label: "54B — VAT refund/TCC claimed",
        kind: "money",
      },
      {
        key: "item_18_paid_on_previous_return",
        label: "18 — Paid on previous return",
        kind: "money",
      },
      {
        key: "item_19_other_credit_or_payment",
        label: "19 — Other credit/payment",
        kind: "money",
      },
      { key: "item_22_surcharge", label: "22 — Surcharge", kind: "money" },
      { key: "item_23_interest", label: "23 — Interest", kind: "money" },
      { key: "item_24_compromise", label: "24 — Compromise", kind: "money" },
    ],
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

export const FORM_SPECS = { ...EXTRACTED_SPECS, ...HAND_SPECS };

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
          .checked(Boolean(state.formFieldGet(state.formDraft, field.key)))
          .on_change((checked, cx) => {
            state.formFieldSet(state.formDraft, field.key, checked);
            cx.notify();
          }),
      );
  }
  // Base Input ships no height/border — an empty field renders as zero
  // pixels, so the frame has to come from us (same recipe as js_todolist).
  return Field.new()
    .label(field.label)
    .child(
      Input.new(state.formInputs[field.key])
        .w_full()
        .h(28)
        .px(8)
        .border(1)
        .border_color(cx.theme().colors.input)
        .bg(cx.theme().colors.surface)
        .text_size(12),
    );
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
                `${state.activeProfileTin} · ${state.formYear}${state.formQuarter ? ` · period ${state.formQuarter}` : ""} · ${spec.periods}`,
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
