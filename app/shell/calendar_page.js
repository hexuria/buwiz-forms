// Calendar: every deadline for the forms the taxpayers file, and every
// as-needed return that holds its trigger date (a removal, a withholding),
// for one tax year. A filter bar narrows it: the taxpayer combo box, how
// often (All, Yearly, Quarterly, Monthly), then under Monthly a month chip
// and under Quarterly a quarter chip, and a year stepper. A month chip picks
// the month a return is due ("January" shows December's 1601C, due
// January); a quarter chip picks the quarter a return covers. List groups
// the rows by due month (by quarter under Quarterly); Month is a six-week
// grid with the chosen day's deadlines beside it. Holidays move deadlines
// only in the years the app ships a holiday list for (bir-core
// official_holidays: 2026 so far); a later year's deadlines move off
// weekends only, so one can show on a holiday until an update adds it.

import { div } from "gpui-kit";
import {
  tokens,
  SPACE,
  SIZE,
  RADIUS,
  Text,
  Line,
  Code,
  Avatar,
  IconButton,
  AppButton,
  Segmented,
  StatusChip,
  ListGroup,
  ListRow,
  NameCell,
  EmptyRow,
  PageHeader,
  initials,
  createMultiCombo,
} from "./ui.js";
import { isoDate, addDays, daysUntil, MONTHS, MONTHS_LONG, formatTin } from "./store.js";
import { dueRow } from "./home.js";

const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];

/** How often, and the frequency each choice keeps. */
const FREQ_OPTIONS = [
  { value: "all", label: "All" },
  { value: "yearly", label: "Yearly" },
  { value: "quarterly", label: "Quarterly" },
  { value: "monthly", label: "Monthly" },
];
const FREQ_OF = { yearly: "Annual", quarterly: "Quarterly", monthly: "Monthly" };
const QUARTER_MONTHS = ["January to March", "April to June", "July to September", "October to December"];

const plural = (n, word) => `${n} ${word}${n === 1 ? "" : "s"}`;

/** Redraw the page after a change made from inside a combo box. */
function redrawPage(app, cx) {
  if (app.invalidate) app.invalidate();
  else cx.notify();
}

/** Every taxpayer as a combo-box choice: avatar, name, TIN. */
export function taxpayerOptions(app) {
  return app.store.taxpayers().map((tp) => ({
    value: tp.tin_root,
    label: tp.name,
    sub: tp.tin_display || formatTin(tp.tin_root),
    lead: (cx) => Avatar(cx, tp.name, 20, tp.tin_root),
  }));
}

/** The taxpayer combo box's props; `get` and `set` hold the page's selection. */
export function taxpayerComboProps(app, id, get, set) {
  return {
    id,
    all: "All taxpayers",
    nouns: "taxpayers",
    icon: "users",
    placeholder: "Find a taxpayer",
    source: () => ({ options: taxpayerOptions(app), selected: get() }),
    onChange: (selected, cx) => {
      set(selected);
      redrawPage(app, cx);
    },
  };
}

/** The Calendar's filters and its one combo box. Made at boot. */
export class CalendarFilters {
  constructor(app, cx) {
    this.app = app;
    this.reset();
    /** "list" | "month" */
    this.view = "list";
    const today = new Date();
    this.gridYear = today.getFullYear();
    this.gridMonth = today.getMonth();
    /**
     * The day the Month view lists, as an ISO date, or null when none is
     * chosen. Moving to another month moves it to today when that month holds
     * today and clears it otherwise (`showMonth`), so the panel never lists a
     * day the grid no longer shows.
     */
    this.day = isoDate(today);
    this.tpCombo = createMultiCombo(
      cx,
      taxpayerComboProps(app, "calendar-taxpayers", () => this.tps, (v) => (this.tps = v)),
    );
  }

  reset() {
    /** @type {Set<string> | null} TIN roots; null is every taxpayer. */
    this.tps = null;
    /** "all" | "yearly" | "quarterly" | "monthly" */
    this.freq = "all";
    /** 1-12: the month a return is due; 0 for every month. */
    this.month = 0;
    /** 1-4: the quarter a return covers; 0 for every quarter. */
    this.quarter = 0;
    /** The tax year the returns cover (the due year under a month chip). */
    this.year = new Date().getFullYear();
  }

  active() {
    return this.tps !== null || this.freq !== "all" || this.month !== 0 || this.quarter !== 0 || this.year !== new Date().getFullYear();
  }

  clear(cx) {
    this.reset();
    this.tpCombo.refresh(cx);
    cx.notify();
  }

  /** Esc: close the open combo box; false when none was open. */
  dismiss(cx) {
    return this.tpCombo.close(cx);
  }

  /** Show a month (0-11) of a year in the grid; the chosen day follows (see `day`). */
  showMonth(year, month) {
    const today = new Date();
    this.gridYear = year;
    this.gridMonth = month;
    this.day = today.getFullYear() === year && today.getMonth() === month ? isoDate(today) : null;
  }
}

/** Taxpayer and how-often filters, which both views apply. */
function byWhoAndHowOften(f, rows) {
  let r = rows;
  if (f.tps) r = r.filter((d) => f.tps.has(d.tin_root));
  if (f.freq !== "all") r = r.filter((d) => d.frequency === FREQ_OF[f.freq]);
  return r;
}

/**
 * The List view's rows: a month chip keeps the returns due in that month of
 * the year; otherwise the year is the tax year the returns cover, and a
 * quarter chip keeps one covered quarter.
 */
export function calendarRows(f, rows) {
  const r = byWhoAndHowOften(f, rows);
  if (f.freq === "monthly" && f.month) {
    const prefix = `${f.year}-${String(f.month).padStart(2, "0")}-`;
    return r.filter((d) => d.due_date.startsWith(prefix));
  }
  let out = r.filter((d) => d.taxable_year === f.year);
  if (f.freq === "quarterly" && f.quarter) out = out.filter((d) => d.period === f.quarter);
  return out;
}

/**
 * Every deadline the List view can show for a year, in one read: due from
 * January 1 of the year (a month chip) through the end of the next year
 * (the year's December, fourth-quarter and annual returns fall due then).
 */
function yearDeadlines(app, year) {
  return app.store.deadlines(`${year}-01-01`, `${year + 1}-12-31`);
}

const byDue = (a, b) => (a.due_date < b.due_date ? -1 : a.due_date > b.due_date ? 1 : 0);

function countLine(cx, f, rows) {
  const who = new Set(rows.map((d) => d.tin_root)).size;
  let scope = `${f.year} returns`;
  if (f.freq === "monthly" && f.month) scope = `due in ${MONTHS_LONG[f.month - 1]} ${f.year}`;
  else if (f.freq === "quarterly" && f.quarter) scope = `returns for Q${f.quarter} ${f.year}`;
  return div()
    .id("calendar-count")
    .mt(SPACE[4])
    .child(Text(cx, "meta", `${plural(rows.length, "deadline")} · ${plural(who, "taxpayer")} · ${scope}`, "fg2"));
}

function yearStepper(cx, app, f) {
  const t = tokens(cx);
  const { min, max } = app.store.yearBounds();
  const step = (by) => (_e, cx) => {
    f.year = Math.min(max, Math.max(min, f.year + by));
    cx.notify();
  };
  return div()
    .id("calendar-year")
    .flex()
    .flex_row()
    .flex_none()
    .items_center()
    .h(SIZE.control)
    .px(2)
    .rounded(RADIUS.row)
    .border(1)
    .border_color(t.borderStrong)
    .bg(t.surface)
    .accessibility_label(`Tax year ${f.year}`)
    .child(IconButton(cx, "calendar-year-prev", "chevron-left", { tooltip: "Previous year", disabled: f.year <= min, onClick: step(-1) }))
    .child(Text(cx, "control", String(f.year)).font_weight(600).min_w(42).px(6).text_center())
    .child(IconButton(cx, "calendar-year-next", "chevron-right", { tooltip: "Next year", disabled: f.year >= max, onClick: step(1) }));
}

/** Month chips under Monthly, quarter chips under Quarterly. */
function chipRow(cx, f) {
  const row = div().flex().flex_row().flex_wrap().items_center().gap(SPACE[2]).mt(10);
  if (f.freq === "monthly") {
    row.child(
      Segmented(
        cx,
        "calendar-month",
        [{ value: 0, label: "All months" }, ...MONTHS.map((m, i) => ({ value: i + 1, label: m }))],
        f.month,
        (v, cx) => {
          f.month = v;
          cx.notify();
        },
        { label: "Month due", wrap: true },
      ),
    );
    if (f.month) row.child(Text(cx, "meta", `Forms due in ${MONTHS_LONG[f.month - 1]}`, "fg3"));
    return row;
  }
  if (f.freq === "quarterly") {
    row.child(
      Segmented(
        cx,
        "calendar-quarter",
        [{ value: 0, label: "All quarters" }, ...[1, 2, 3, 4].map((q) => ({ value: q, label: `Q${q}` }))],
        f.quarter,
        (v, cx) => {
          f.quarter = v;
          cx.notify();
        },
        { label: "Quarter covered" },
      ),
    );
    if (f.quarter) row.child(Text(cx, "meta", `Returns for ${QUARTER_MONTHS[f.quarter - 1]}`, "fg3"));
    return row;
  }
  return null;
}

function emptyState(cx, f, hasAny) {
  if (!hasAny && !f.active()) {
    return ListGroup(cx, "calendar-empty", [EmptyRow(cx, "calendar-empty-row", "Deadlines show here once a taxpayer has forms for a tax year.")]);
  }
  return ListGroup(cx, "calendar-empty", [
    EmptyRow(
      cx,
      "calendar-empty-row",
      "No deadlines match these filters.",
      AppButton(cx, "calendar-clear", "Clear filters", { size: "sm", onClick: (_e, cx) => f.clear(cx) }),
    ),
  ]);
}

function listView(cx, app, f, width, page) {
  const all = yearDeadlines(app, f.year);
  const rows = calendarRows(f, all).slice().sort(byDue);
  const chips = chipRow(cx, f);
  if (chips) page.child(chips);
  page.child(countLine(cx, f, rows));
  if (!rows.length) return page.child(div().mt(SPACE[4]).child(emptyState(cx, f, all.length > 0)));
  const groups = new Map();
  if (f.freq === "quarterly") {
    rows.sort((a, b) => a.taxable_year - b.taxable_year || a.period - b.period || byDue(a, b));
    for (const d of rows) {
      const key = `Q${d.period} ${d.taxable_year}`;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(d);
    }
  } else {
    for (const d of rows) {
      const [y, m] = d.due_date.split("-").map(Number);
      const key = `${MONTHS_LONG[m - 1]} ${y}`;
      if (!groups.has(key)) groups.set(key, []);
      groups.get(key).push(d);
    }
  }
  let first = true;
  for (const [label, list] of groups) {
    const head = div()
      .flex()
      .flex_row()
      .flex_wrap()
      .items_center()
      .gap(SPACE[2])
      .mt(first ? SPACE[5] : SPACE[8])
      .mb(SPACE[3]);
    if (f.freq === "quarterly") {
      const due = [...new Set(list.map((d) => d.due_date))].map((iso) => {
        const [, m, day] = iso.split("-").map(Number);
        return `${MONTHS[m - 1]} ${day}`;
      });
      head.child(Text(cx, "title", `${label} returns`)).child(Text(cx, "meta", `due ${due.join(", ")}`, "fg3"));
    } else {
      head.child(Text(cx, "title", label));
    }
    page.child(head);
    page.child(ListGroup(cx, `calendar-${label.replace(" ", "-")}`, list.map((d) => dueRow(cx, app, d, width))));
    first = false;
  }
  return page;
}

// ---------------------------------------------------------------- month grid

function kindOf(status) {
  if (status === "Draft") return "draft";
  if (status === "Queued") return "queued";
  if (status === "Submitted" || status === "Confirmed" || status === "Paid") return "filed";
  return "not_started";
}

const isOpenStatus = (d) => d.status === "Not started" || d.status === "Draft";

/** A grid chip's tone: overdue, due within 14 days, filed, in progress. */
function toneOf(d) {
  const n = daysUntil(d.due_date);
  if (isOpenStatus(d) && n < 0) return "danger";
  if (isOpenStatus(d) && n <= 14) return "warning";
  const kind = kindOf(d.status);
  if (kind === "filed") return "success";
  if (kind === "draft" || kind === "queued") return "accent";
  return "neutral";
}

function openDeadline(app, d, cx) {
  app.selRoot = d.tin_root;
  app.selCode = d.branch_code;
  app.openForm({ tin: d.tin, code: d.form_code, year: d.taxable_year, period: d.period, ...(d.return_key != null ? { returnKey: d.return_key } : {}) }, cx);
}

const whoLine = (d) => (d.branch_count > 1 ? `${d.taxpayer_name} · ${d.branch_name}` : d.taxpayer_name);
const rowKey = (d) => `${d.tin}-${d.form_code}-${d.taxable_year}-${d.period}${d.frequency === "As needed" ? `-${d.draft_id}` : ""}`;

/** The chosen day's row: code, period over the taxpayer, when or status. */
function dayRow(cx, app, d) {
  const n = daysUntil(d.due_date);
  const chip =
    isOpenStatus(d) && n < 0
      ? StatusChip(cx, "overdue", n)
      : isOpenStatus(d) && n <= 14
        ? StatusChip(cx, "due_soon", n)
        : StatusChip(cx, kindOf(d.status));
  return ListRow(cx, `calendar-day-row-${rowKey(d)}`, { onClick: (_e, cx) => openDeadline(app, d, cx) })
    .child(Code(cx, d.form_code).w(52).flex_none())
    .child(NameCell(cx, d.period_label, whoLine(d)))
    .child(div().flex_none().child(chip));
}

function miniChip(cx, d) {
  const t = tokens(cx);
  const tone = toneOf(d);
  const fg = tone === "neutral" ? "fg2" : tone;
  return div()
    .flex()
    .flex_row()
    .items_center()
    .h(18)
    .px(6)
    .min_w(0)
    .overflow_hidden()
    .rounded(5)
    .bg(tone === "neutral" ? t.hover : t[`${tone}Soft`])
    .child(Line(cx, "cap", `${d.form_code} ${initials(d.taxpayer_name)}`, fg).font_weight(600));
}

/** The Sunday on or before the first of the month: the grid's first cell. */
export function monthGridStart(year, month) {
  const first = new Date(year, month, 1);
  return addDays(first, -first.getDay());
}

function monthView(cx, app, f, width, page) {
  const t = tokens(cx);
  const y = f.gridYear;
  const mo = f.gridMonth;
  const start = monthGridStart(y, mo);
  // Six weeks always, so the grid keeps its height from month to month.
  const end = addDays(start, 41);
  const rows = byWhoAndHowOften(f, app.store.deadlines(isoDate(start), isoDate(end)));
  const byDay = new Map();
  for (const d of rows.slice().sort(byDue)) {
    if (!byDay.has(d.due_date)) byDay.set(d.due_date, []);
    byDay.get(d.due_date).push(d);
  }
  const prefix = `${y}-${String(mo + 1).padStart(2, "0")}-`;
  const inMonth = rows.filter((d) => d.due_date.startsWith(prefix)).length;
  const move = (by) => (_e, cx) => {
    let m = f.gridMonth + by;
    let yy = f.gridYear;
    if (m < 0) {
      m = 11;
      yy -= 1;
    } else if (m > 11) {
      m = 0;
      yy += 1;
    }
    f.showMonth(yy, m);
    cx.notify();
  };
  page.child(
    div()
      .flex()
      .flex_row()
      .flex_wrap()
      .items_center()
      .gap(SPACE[1])
      .mt(SPACE[5])
      .child(IconButton(cx, "calendar-prev-month", "chevron-left", { tooltip: "Previous month", onClick: move(-1) }))
      .child(div().id("calendar-month-title").min_w(150).child(Text(cx, "title", `${MONTHS_LONG[mo]} ${y}`).text_center()))
      .child(IconButton(cx, "calendar-next-month", "chevron-right", { tooltip: "Next month", onClick: move(1) }))
      .child(
        AppButton(cx, "calendar-today", "Today", {
          variant: "ghost",
          size: "sm",
          onClick: (_e, cx) => {
            const today = new Date();
            f.showMonth(today.getFullYear(), today.getMonth());
            cx.notify();
          },
        }),
      )
      .child(div().id("calendar-month-count").ml_auto().child(Text(cx, "meta", `${plural(inMonth, "deadline")} this month`, "fg3"))),
  );

  const today = isoDate(new Date());
  const compact = width < 560;
  const grid = div()
    .id("calendar-grid")
    .flex()
    .flex_col()
    .flex_1()
    .min_w(0)
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .overflow_hidden()
    .accessibility_label(`${MONTHS_LONG[mo]} ${y}`);
  const head = div().flex().flex_row().bg(t.bg).border_b(1).border_color(t.border);
  for (const name of WEEKDAYS) {
    head.child(div().flex_1().flex_basis(0).min_w(0).px(SPACE[2]).py(6).child(Line(cx, "cap", name.slice(0, 3).toUpperCase(), "fg3").font_weight(600)));
  }
  grid.child(head);
  for (let w = 0; w < 6; w++) {
    const week = div().flex().flex_row();
    if (w > 0) week.border_t(1).border_color(t.border);
    for (let i = 0; i < 7; i++) {
      const date = addDays(start, w * 7 + i);
      const iso = isoDate(date);
      const items = byDay.get(iso) || [];
      const cell = div()
        .id(`calendar-day-${iso}`)
        .flex()
        .flex_col()
        .gap(3)
        .flex_1()
        .flex_basis(0)
        .min_w(0)
        .min_h(compact ? 52 : 78)
        .p(compact ? SPACE[1] : 6)
        .cursor_pointer()
        .accessibility_label(`${MONTHS_LONG[date.getMonth()]} ${date.getDate()}, ${plural(items.length, "deadline")}`)
        .on_click((_e, cx) => {
          f.day = iso;
          cx.notify();
        });
      if (i > 0) cell.border_l(1).border_color(t.border);
      if (iso === f.day) cell.bg(t.selected);
      else cell.hover((s) => s.bg(t.hover));
      if (date.getMonth() !== mo) cell.opacity(0.4);
      const isToday = iso === today;
      const num = div()
        .flex()
        .flex_none()
        .items_center()
        .justify_center()
        .size(22)
        .rounded(RADIUS.full)
        .child(Text(cx, "metaStrong", String(date.getDate()), isToday ? "primaryFg" : "fg2"));
      if (isToday) num.id("calendar-today-ring").bg(t.accent);
      cell.child(num);
      if (compact) {
        if (items.length) cell.child(Text(cx, "cap", String(items.length), "fg2"));
      } else {
        for (const d of items.slice(0, 2)) cell.child(miniChip(cx, d));
        if (items.length > 2) cell.child(Text(cx, "cap", `+${items.length - 2} more`, "fg3").pl(SPACE[1]));
      }
      week.child(cell);
    }
    grid.child(week);
  }

  const side = div().id("calendar-day").flex().flex_col();
  if (f.day) {
    const [sy, sm, sd] = f.day.split("-").map(Number);
    const picked = new Date(sy, sm - 1, sd);
    const dayItems = byDay.get(f.day) || [];
    side
      .child(Text(cx, "emph", `${WEEKDAYS[picked.getDay()]}, ${MONTHS_LONG[picked.getMonth()]} ${picked.getDate()}`).mb(SPACE[2]))
      .child(
        ListGroup(
          cx,
          "calendar-day-list",
          dayItems.length ? dayItems.map((d) => dayRow(cx, app, d)) : [EmptyRow(cx, "calendar-day-empty", "Nothing due this day.")],
        ),
      );
  } else {
    side
      .child(Text(cx, "emph", `${MONTHS_LONG[mo]} ${y}`).mb(SPACE[2]))
      .child(ListGroup(cx, "calendar-day-list", [EmptyRow(cx, "calendar-day-none", "Pick a day to see what is due.")]));
  }
  const body = div().flex().gap(SPACE[4]).mt(SPACE[3]);
  if (width >= 760) body.flex_row().items_start().child(grid).child(side.w(280).flex_none());
  else body.flex_col().child(grid).child(side.w_full());
  return page.child(body);
}

// ---------------------------------------------------------------- page

export function renderCalendar(cx, app, width) {
  const f = app.calendarUi;
  const page = div()
    .flex()
    .flex_col()
    .child(
      PageHeader(cx, {
        title: "Calendar",
        lede: "Every deadline for the forms your taxpayers file, from the BIR tax calendar.",
      }),
    );
  const month = f.view === "month";
  const bar = div().id("calendar-filters").flex().flex_row().flex_wrap().items_center().gap(SPACE[2]).mt(SPACE[6]);
  bar.child(f.tpCombo.entity);
  bar.child(
    Segmented(
      cx,
      "calendar-freq",
      FREQ_OPTIONS,
      f.freq,
      (v, cx) => {
        f.freq = v;
        f.month = 0;
        f.quarter = 0;
        cx.notify();
      },
      { label: "How often" },
    ),
  );
  if (!month) bar.child(yearStepper(cx, app, f));
  bar.child(
    div()
      .ml_auto()
      .child(
        Segmented(
          cx,
          "calendar-view",
          [
            { value: "list", label: "List", icon: "list" },
            { value: "month", label: "Month", icon: "calendar" },
          ],
          f.view,
          (v, cx) => {
            f.view = v;
            cx.notify();
          },
          { label: "View" },
        ),
      ),
  );
  page.child(bar);
  if (app.isFresh()) {
    return page.child(
      div()
        .mt(SPACE[6])
        .child(ListGroup(cx, "calendar-empty", [EmptyRow(cx, "calendar-empty-row", "Deadlines show here once a taxpayer has forms for a tax year.")])),
    );
  }
  return month ? monthView(cx, app, f, width, page) : listView(cx, app, f, width, page);
}
