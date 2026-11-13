// Home's dashboard: five status tiles, a 30-day deadline strip with the
// list it filters, and each taxpayer at a glance (round-two plan, claim 5).
//
// Every number comes from one named host read through the Store:
//   Overdue, Due in 7 days   deadlines_between(today − 366, today + 7)
//   Drafts                   list_returns("saved"), as `savedReturns()`:
//                            outside the poll's sweep, read once per visit
//   Waiting to send          list_returns("filed"), status Queued
//   Filed this month         list_returns("filed"), Submitted · Confirmed · Paid
//                            updated this calendar month
//   30-day strip             deadlines_between(today, today + 29)
//   Taxpayer progress        deadlines_between(Jan 1, today): filed of due
//   Taxpayer next deadline   deadlines_between(today, today + 60): first open
//   Taxpayer rows            list_taxpayers
//
// Tiles: Overdue and Due in 7 days narrow the list under the strip; Drafts,
// Waiting to send and Filed this month open Filings. Filings' own filters
// belong to that screen, so a tile leaves `app.filingsPreset` (below) for
// Filings to apply and clear, and navigates.

import { div } from "gpui-kit";
import { tokens, SPACE, RADIUS, SIZE, Text, Line, Icon, Avatar, AppButton, StatusChip, ListRow, NameCell, EmptyRow } from "./ui.js";
import { isoDate, addDays, daysUntil, MONTHS, MONTHS_LONG } from "./store.js";

const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const FILED = new Set(["Submitted", "Confirmed", "Paid"]);
const STRIP_DAYS = 30;

/** Still to do: nobody has queued or filed it. */
function isOpen(d) {
  return d.status === "Not started" || d.status === "Draft";
}

/** [year, month 1-12] of a return's update, in local time (SQLite timestamps are UTC). */
function localMonth(text) {
  const ts = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})/.exec(String(text || ""));
  if (ts) {
    const local = new Date(Date.UTC(+ts[1], +ts[2] - 1, +ts[3], +ts[4], +ts[5]));
    return [local.getFullYear(), local.getMonth() + 1];
  }
  const [y, m] = String(text || "").split("-").map(Number);
  return [y, m];
}

/**
 * Hook for the Filings screen: what a Home tile asks it to show.
 * `statuses` are return statuses ("Draft", "Queued", "Submitted",
 * "Confirmed", "Paid"); `updatedMonth` ("YYYY-MM") keeps returns updated in
 * that month. Filings applies it to its filters once and sets it to null.
 * @typedef {{ statuses: string[], updatedMonth?: string }} FilingsPreset
 */
function openFilings(app, preset, cx) {
  /** @type {FilingsPreset} */
  app.filingsPreset = preset;
  app.go("filings", cx);
}

/** Open a deadline's return, as a deadline row does. */
function openDeadline(app, d, cx) {
  app.selRoot = d.tin_root;
  app.selCode = d.branch_code;
  app.openForm({ tin: d.tin, code: d.form_code, year: d.taxable_year, period: d.period, ...(d.return_key != null ? { returnKey: d.return_key } : {}) }, cx);
}

/** Everything the dashboard shows, from the cached reads listed at the top. */
export function loadDashboard(app) {
  const store = app.store;
  const now = new Date();
  now.setHours(0, 0, 0, 0);
  const today = isoDate(now);
  const in7 = isoDate(addDays(now, 7));

  const near = store.deadlines(isoDate(addDays(now, -366)), in7).filter(isOpen);
  const overdue = near.filter((d) => d.due_date < today);
  const week = near.filter((d) => d.due_date >= today && d.due_date <= in7);

  const drafts = store.savedReturns().length;
  const filed = store.returns("filed");
  const waiting = filed.filter((r) => r.status === "Queued").length;
  const month = [now.getFullYear(), now.getMonth() + 1];
  const filedMonth = filed.filter((r) => {
    if (!FILED.has(r.status)) return false;
    const [y, m] = localMonth(r.updated_at);
    return y === month[0] && m === month[1];
  }).length;

  const stripAll = store.deadlines(today, isoDate(addDays(now, STRIP_DAYS - 1)));
  const upcoming = stripAll.filter(isOpen);
  const strip = [];
  for (let i = 0; i < STRIP_DAYS; i++) {
    const date = addDays(now, i);
    const iso = isoDate(date);
    strip.push({ iso, date, rows: upcoming.filter((d) => d.due_date === iso) });
  }

  const sinceJan = store.deadlines(`${now.getFullYear()}-01-01`, today).filter((d) => d.due_date < today);
  const ahead = store.deadlines(today, isoDate(addDays(now, 60))).filter(isOpen);
  const taxpayers = store
    .taxpayers()
    .filter((t) => !t.archived)
    .map((t) => {
      const past = sinceJan.filter((d) => d.tin_root === t.tin_root);
      return {
        taxpayer: t,
        due: past.length,
        done: past.filter((d) => FILED.has(d.status)).length,
        next: ahead.find((d) => d.tin_root === t.tin_root) || null,
      };
    });

  return {
    today,
    in7,
    overdue,
    week,
    drafts,
    waiting,
    filedMonth,
    strip,
    upcoming,
    anyDeadlines: stripAll.length > 0 || near.length > 0,
    taxpayers,
  };
}

// ---------------------------------------------------------------- tiles

function tileSpecs(data) {
  const [, m, d] = data.in7.split("-").map(Number);
  const monthName = MONTHS_LONG[Number(data.today.split("-")[1]) - 1];
  return [
    { key: "overdue", label: "Overdue", sub: "past the due date", tone: "danger", icon: "triangle-alert", n: data.overdue.length },
    { key: "week", label: "Due in 7 days", sub: `by ${MONTHS[m - 1]} ${d}`, tone: "warning", icon: "calendar", n: data.week.length },
    { key: "drafts", label: "Drafts", sub: "started, not queued", tone: "accent", icon: "pencil", n: data.drafts },
    { key: "waiting", label: "Waiting to send", sub: "queued to send", tone: "fg2", icon: "send", n: data.waiting },
    { key: "filed", label: "Filed this month", sub: `submitted in ${monthName}`, tone: "success", icon: "circle-check", n: data.filedMonth },
  ];
}

function tileClick(app, key, data, cx) {
  if (key === "overdue" || key === "week") {
    app.homePick = app.homePick === key ? null : key;
    cx.notify();
  } else if (key === "drafts") {
    openFilings(app, { statuses: ["Draft"] }, cx);
  } else if (key === "waiting") {
    openFilings(app, { statuses: ["Queued"] }, cx);
  } else {
    openFilings(app, { statuses: [...FILED], updatedMonth: data.today.slice(0, 7) }, cx);
  }
}

/** The five counts; five across, or wrapped two or three to a row on a narrow pane. */
export function statusTiles(cx, app, data, width) {
  const t = tokens(cx);
  const cols = width >= 640 ? 5 : width >= 420 ? 3 : 2;
  const gap = SPACE[2];
  const w = Math.floor((width - gap * (cols - 1)) / cols);
  const row = div().id("home-tiles").flex().flex_row().flex_wrap().gap(gap).w_full();
  for (const spec of tileSpecs(data)) {
    const on = app.homePick === spec.key;
    const head = div()
      .flex()
      .flex_row()
      .items_center()
      .gap(6)
      .w_full()
      .child(Icon(cx, spec.icon, { color: spec.tone }))
      .child(Line(cx, "meta", spec.label, "fg2").flex_1());
    if (on) head.child(Icon(cx, "x", { size: SIZE.iconSmall, color: "fg3" }));
    row.child(
      div()
        .id(`home-tile-${spec.key}`)
        .flex()
        .flex_col()
        .items_start()
        .gap(2)
        .w(w)
        .flex_none()
        .min_w(0)
        .py(14)
        .px(SPACE[4])
        .rounded(RADIUS.card)
        .border(1)
        .border_color(on ? t.borderStrong : t.border)
        .bg(on ? t.selected : t.surface)
        .cursor_pointer()
        .hover((s) => s.border_color(t.borderStrong))
        .accessibility_label(`${spec.label}: ${spec.n}`)
        .on_click((_e, cx) => tileClick(app, spec.key, data, cx))
        .child(head)
        .child(Text(cx, "page", String(spec.n)))
        .child(Line(cx, "meta", spec.sub, "fg3")),
    );
  }
  return row;
}

// ---------------------------------------------------------------- strip

/** The next 30 days: a column per day with a dot per open deadline (five at most). */
export function deadlineStrip(cx, app, data, width) {
  const t = tokens(cx);
  const everyDay = width >= 560;
  const strip = div()
    .id("home-strip")
    .flex()
    .flex_row()
    .items_end()
    .gap(3)
    .w_full()
    .h(92)
    .pt(10)
    .px(SPACE[3])
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border);
  data.strip.forEach((day, i) => {
    const sel = app.homePick === "day" && app.homeDay === day.iso;
    const n = day.rows.length;
    const label = `${i === 0 ? "Today, " : ""}${MONTHS[day.date.getMonth()]} ${day.date.getDate()}: ${n ? `${n} deadline${n === 1 ? "" : "s"}` : "nothing due"}`;
    const col = div()
      .id(`home-strip-${day.iso}`)
      .flex()
      .flex_col_reverse()
      .items_center()
      .gap(3)
      .flex_1()
      .min_w(0)
      .h_full()
      .pb(SPACE[2])
      .rounded(RADIUS.ctl)
      .cursor_pointer()
      .accessibility_label(label)
      .tooltip(label)
      .on_click((_e, cx) => {
        if (app.homePick === "day" && app.homeDay === day.iso) app.homePick = null;
        else {
          app.homePick = "day";
          app.homeDay = day.iso;
        }
        cx.notify();
      });
    if (sel) col.bg(t.selected);
    else col.hover((s) => s.bg(t.hover));
    col.child(
      everyDay || i % 7 === 0
        ? Text(cx, "cap", String(day.date.getDate()), i === 0 ? "fg" : "fg3")
        : div().h(SPACE[3]),
    );
    for (const d of day.rows.slice(0, 5)) {
      const tone = d.status === "Draft" ? "accent" : daysUntil(d.due_date) <= 7 ? "warning" : "fg3";
      col.child(div().size(8).flex_none().rounded(RADIUS.full).bg(t[tone]));
    }
    strip.child(col);
  });
  const last = data.strip[data.strip.length - 1].date;
  const first = data.strip[0].date;
  return div()
    .flex()
    .flex_col()
    .child(strip)
    .child(
      div()
        .flex()
        .flex_row()
        .justify_between()
        .px(SPACE[3])
        .pt(SPACE[1])
        .child(Text(cx, "cap", `Today, ${MONTHS[first.getMonth()]} ${first.getDate()}`, "fg3"))
        .child(Text(cx, "cap", `${MONTHS[last.getMonth()]} ${last.getDate()}`, "fg3")),
    );
}

/**
 * The deadlines under the strip, for the tile or day picked (all open ones
 * in the next 30 days when nothing is), with its heading and empty line.
 */
export function pickedDeadlines(app, data) {
  const pick = app.homePick;
  if (pick === "overdue") return { title: "Overdue", rows: data.overdue, empty: "Nothing is overdue." };
  if (pick === "week") return { title: "Due in the next 7 days", rows: data.week, empty: "Nothing is due in the next 7 days." };
  const day = pick === "day" ? data.strip.find((d) => d.iso === app.homeDay) : null;
  if (day) {
    const date = day.date;
    return { title: `${WEEKDAYS[date.getDay()]}, ${MONTHS_LONG[date.getMonth()]} ${date.getDate()}`, rows: day.rows, empty: "Nothing due this day." };
  }
  return {
    title: "Open deadlines",
    rows: data.upcoming,
    empty: data.anyDeadlines ? "Nothing is due in the next 30 days." : "Deadlines show here once a taxpayer has forms for a tax year.",
  };
}

/** The heading over the picked list: what it shows, Show all, Open Calendar. */
export function pickedHeader(cx, app, picked) {
  const row = div()
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[2])
    .mt(SPACE[4])
    .mb(SPACE[2])
    .child(Text(cx, "emph", picked.title).flex_1());
  if (app.homePick) {
    row.child(
      AppButton(cx, "home-due-show-all", "Show all", {
        variant: "ghost",
        size: "sm",
        onClick: (_e, cx) => {
          app.homePick = null;
          cx.notify();
        },
      }),
    );
  }
  row.child(
    AppButton(cx, "home-open-calendar", "Open Calendar", {
      variant: "ghost",
      size: "sm",
      icon: "calendar",
      onClick: (_e, cx) => app.go("calendar", cx),
    }),
  );
  return row;
}

// ---------------------------------------------------------------- taxpayers

const BAR = 120;

function progressText(row) {
  return row.due ? `${row.done} of ${row.due} filed this year` : "Nothing was due yet this year";
}

function nextText(next) {
  if (!next) return "Nothing due in 60 days";
  const [, m, d] = next.due_date.split("-").map(Number);
  return `Next: ${next.form_code} · ${MONTHS[m - 1]} ${d}`;
}

/** One row per taxpayer: avatar, filed progress this year, the next open deadline. */
export function taxpayerRows(cx, app, data, width) {
  const t = tokens(cx);
  const compact = width < 560;
  if (!data.taxpayers.length) return [EmptyRow(cx, "home-taxpayers-empty", "Archived taxpayers are left out of Home.")];
  return data.taxpayers.map((row) => {
    const tp = row.taxpayer;
    const next = row.next;
    const item = ListRow(cx, `home-tp-${tp.tin_root}`).child(Avatar(cx, tp.name, 28, tp.tin_root));
    if (compact) {
      item.child(NameCell(cx, tp.name, `${progressText(row)} · ${nextText(next)}`));
    } else {
      const pct = row.due ? row.done / row.due : 0;
      item
        .child(NameCell(cx, tp.name, tp.tin_display, { width: 210 }))
        .child(
          div()
            .flex()
            .flex_col()
            .flex_1()
            .min_w(BAR)
            .gap(SPACE[1])
            .child(
              div()
                .id(`home-tp-${tp.tin_root}-bar`)
                .w(BAR)
                .h(6)
                .rounded(RADIUS.full)
                .bg(t.hover)
                .overflow_hidden()
                .child(div().h_full().w(Math.round(BAR * pct)).rounded(RADIUS.full).bg(t.success)),
            )
            .child(Line(cx, "meta", progressText(row), "fg3")),
        );
      const when = div().flex().flex_col().gap(2).w(170).flex_none().child(Line(cx, "meta", nextText(next), next ? "fg2" : "fg3"));
      if (next) {
        const n = daysUntil(next.due_date);
        when.child(div().flex().flex_row().child(n <= 14 ? StatusChip(cx, "due_soon", n) : Text(cx, "meta", `In ${n} days`, "fg3")));
      }
      item.child(when);
    }
    if (next) {
      item.child(
        AppButton(cx, `home-tp-${tp.tin_root}-next`, next.status === "Draft" ? "Open next" : "Start next", {
          size: "sm",
          onClick: (_e, cx) => openDeadline(app, next, cx),
        }),
      );
    }
    return item;
  });
}
