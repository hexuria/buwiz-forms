// Calendar: every deadline for the forms the taxpayers file, and every
// as-needed return that holds its trigger date (a removal, a withholding),
// grouped by month, from today through the next twelve months. Holidays
// move deadlines only in the years the app ships a holiday list for
// (bir-core official_holidays: 2026 so far); a later year's deadlines move
// off weekends only, so one can show on a holiday until an update adds it.

import { div } from "gpui-kit";
import { ListGroup, EmptyRow, PageHeader, SectionTitle, Segmented } from "./ui.js";
import { isoDate, addDays, MONTHS_LONG } from "./store.js";
import { dueRow } from "./home.js";

export function renderCalendar(cx, app, width) {
  const year = new Date().getFullYear();
  const header = PageHeader(cx, {
    title: "Calendar",
    lede: `Every deadline for the forms your taxpayers file, from the ${year} BIR tax calendar.`,
  });
  const today = new Date();
  const all = app.isFresh() ? [] : app.store.deadlines(isoDate(today), isoDate(addDays(today, 366)));
  // Scoped to the selected taxpayer's branch unless "All taxpayers" is chosen.
  const tp = app.taxpayer();
  const b = app.branch();
  const scope = tp ? app.calendarScope || "mine" : "all";
  const rows = scope === "all" ? all : all.filter((d) => d.tin === b.tin);
  const page = div().flex().flex_col().child(header);
  if (tp) {
    page.child(
      div()
        .mt(24)
        .flex()
        .flex_row()
        .child(
          Segmented(
            cx,
            "calendar-scope",
            [
              { value: "mine", label: tp.branches.length > 1 ? `${tp.name} · ${b.name}` : tp.name },
              { value: "all", label: "All taxpayers" },
            ],
            scope,
            (v, cx) => {
              app.calendarScope = v;
              cx.notify();
            },
            { label: "Whose deadlines" },
          ),
        ),
    );
  }
  if (!rows.length) {
    return page.child(
      div()
        .mt(24)
        .child(ListGroup(cx, "calendar-empty", [EmptyRow(cx, "calendar-empty-row", "Deadlines show here once a taxpayer has forms for a tax year.")])),
    );
  }
  const groups = new Map();
  for (const d of rows) {
    const [y, m] = d.due_date.split("-").map(Number);
    const key = `${MONTHS_LONG[m - 1]} ${y}`;
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(d);
  }
  let first = true;
  for (const [label, list] of groups) {
    page.child(div().mt(first ? 28 : 0).child(SectionTitle(cx, label, first)));
    page.child(ListGroup(cx, `calendar-${label.replace(" ", "-")}`, list.map((d) => dueRow(cx, app, d, width))));
    first = false;
  }
  return page;
}
