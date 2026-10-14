// Background Tasks: the filing-queue feed (queued → submitted → confirmed →
// paid) plus per-row actions. The painted app also runs a receipt-fetching
// worker; the shell lists the same queue rows, dry-run transport only.
import { div } from "gpui-kit";
import { v_flex, h_flex } from "gpui-base";
import { Button } from "gpui-component";
import { pageHeading, pageSub } from "./pages.js";

function queueRow(cx, state, entry) {
  const period = entry.quarter
    ? `Q${entry.quarter}`
    : entry.month
      ? `M${entry.month}`
      : "annual";
  const canCancel = entry.status === "Queued";
  return h_flex()
    .w_full()
    .px(12)
    .py(8)
    .gap(10)
    .items_center()
    .rounded_md()
    .hover((s) => s.bg(cx.theme().colors.muted))
    .child(
      div()
        .w(70)
        .flex_none()
        .text_size(13)
        .font_semibold()
        .text_color(cx.theme().colors.primary)
        .child(entry.form_code),
    )
    .child(
      div()
        .flex_1()
        .min_w_0()
        .text_size(13)
        .text_color(cx.theme().colors.foreground)
        .child(`${entry.tin} · ${entry.taxable_year} ${period} · updated ${entry.updated_at}`),
    )
    .child(
      div()
        .px(8)
        .py(2)
        .rounded_md()
        .text_size(11)
        .font_semibold()
        .bg(
          entry.status === "Queued"
            ? cx.theme().colors.accent
            : cx.theme().colors.muted,
        )
        .text_color(
          entry.status === "Queued"
            ? cx.theme().colors.accent_foreground
            : cx.theme().colors.muted_foreground,
        )
        .child(entry.status),
    )
    .when(canCancel, (el) =>
      el.child(
        Button.new(`cancel_${entry.id}`)
          .label("Cancel")
          .outline()
          .compact()
          .on_click((_ev, clickCx) => state.cancelQueuedEntry(entry, clickCx)),
      ),
    );
}

export function renderQueuePage(cx, state) {
  let entries = [];
  let error = null;
  try {
    entries = state.queuedSubmissions();
  } catch (e) {
    error = String(e);
  }
  return div()
    .size_full()
    .overflow_y_scroll()
    .p(24)
    .child(
      v_flex()
        .gap(12)
        .max_w_full()
        .child(
          h_flex()
            .items_center()
            .justify_between()
            .child(pageHeading("Background Tasks", cx))
            .child(
              pageSub(`${entries.length} queued or in-flight submissions`, cx),
            ),
        )
        .when(Boolean(error || state.queueError), (el) =>
          el.child(
            div()
              .text_size(13)
              .text_color("#EF4444")
              .child(String(error || state.queueError)),
          ),
        )
        .when(!error && entries.length === 0, (el) =>
          el.child(
            div()
              .text_size(13)
              .text_color(cx.theme().colors.muted_foreground)
              .child(
                "Nothing queued. Save a form draft and use Queue for submission.",
              ),
          ),
        )
        .children(entries.map((entry) => queueRow(cx, state, entry))),
    );
}
