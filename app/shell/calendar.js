// Tax Calendars — the `admin_calendar` view. Renders the stored
// `tax_deadlines` table grouped by due month, nearest first (same source the
// painted app's calendar reads).
import { div } from "gpui-kit";
import { v_flex, h_flex } from "gpui-base";

const MONTH_NAMES = [
  "January", "February", "March", "April", "May", "June",
  "July", "August", "September", "October", "November", "December",
];

function muted(cx) {
  return cx.theme().colors.muted_foreground;
}

function monthKey(dueDate) {
  // Stored as YYYY-MM-DD (or free text) — group by YYYY-MM when parseable.
  const m = String(dueDate).match(/^(\d{4})-(\d{2})/);
  if (!m) return null;
  return { key: `${m[1]}-${m[2]}`, label: `${MONTH_NAMES[parseInt(m[2], 10) - 1]} ${m[1]}` };
}

export function renderCalendarPage(cx, state) {
  let deadlines = [];
  let error = null;
  try {
    deadlines = state.taxDeadlines();
  } catch (e) {
    error = String(e);
    deadlines = [];
  }

  const groups = new Map();
  const other = [];
  for (const d of deadlines) {
    const mk = monthKey(d.due_date);
    if (!mk) {
      other.push(d);
      continue;
    }
    if (!groups.has(mk.key)) groups.set(mk.key, { label: mk.label, items: [] });
    groups.get(mk.key).items.push(d);
  }
  const sortedKeys = [...groups.keys()].sort();

  return v_flex()
    .size_full()
    .px(24)
    .py(16)
    .gap(12)
    .child(
      h_flex().justify_between().items_center().w_full().children([
        v_flex().children([
          div().text_size(20).font_semibold().text_color(cx.theme().colors.foreground).child("Tax Calendars"),
          div().text_size(12).text_color(muted(cx)).child(`${deadlines.length} deadlines on record`),
        ]),
      ]),
    )
    .when(error !== null, (el) =>
      el.child(
        div()
          .text_size(12)
          .text_color(cx.theme().colors.danger)
          .child(`Error: ${error}`),
      ),
    )
    .when(deadlines.length === 0 && error === null, (el) =>
      el.child(
        v_flex()
          .py(32)
          .w_full()
          .items_center()
          .child(
            div()
              .text_size(13)
              .text_color(muted(cx))
              .child("No deadlines recorded. Deadlines appear after the deadline sync runs."),
          ),
      ),
    )
    .children(
      sortedKeys.map((key) => {
        const group = groups.get(key);
        return v_flex()
          .w_full()
          .gap(4)
          .child(
            div()
              .text_size(14)
              .font_semibold()
              .text_color(cx.theme().colors.foreground)
              .child(group.label),
          )
          .children(
            group.items.map((d) =>
              h_flex()
                .w_full()
                .items_center()
                .gap(8)
                .py(6)
                .px(8)
                .rounded_md()
                .hover((s) => s.bg(cx.theme().colors.muted))
                .children([
                  div()
                    .text_size(11)
                    .font_semibold()
                    .text_color(cx.theme().colors.primary_foreground)
                    .bg(cx.theme().colors.primary)
                    .px(6)
                    .py(2)
                    .rounded_md()
                    .child(d.form_type),
                  div().text_size(12).text_color(muted(cx)).child(d.due_date),
                  div().text_size(12).text_color(cx.theme().colors.foreground).child(d.description),
                ]),
            ),
          );
      }),
    )
    .children(
      other.map((d) =>
        h_flex()
          .w_full()
          .items_center()
          .gap(8)
          .py(6)
          .px(8)
          .rounded_md()
          .children([
            div()
              .text_size(11)
              .font_semibold()
              .text_color(cx.theme().colors.primary_foreground)
              .bg(cx.theme().colors.primary)
              .px(6)
              .py(2)
              .rounded_md()
              .child(d.form_type),
            div().text_size(12).text_color(muted(cx)).child(d.due_date),
            div().text_size(12).text_color(cx.theme().colors.foreground).child(d.description),
          ]),
      ),
    );
}
