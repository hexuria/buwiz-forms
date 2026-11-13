// Filings: every return, saved or filed, on a board with one column per
// status group: Draft, Queued, Submitted and Confirmed (a Paid return sits
// in Confirmed, marked Paid). Above it a search box, four combo boxes
// (taxpayers, forms, periods, statuses), Clear filters and a count. Each card
// opens its return, and keeps the actions the list had: Cancel queue for a
// Queued return, "Use as template" (D17) for a filed one.
//
// The rows are the two cached reads Home also makes, `list_returns("filed")`
// and `list_returns("saved")`, both served from the read-only connection, so
// this screen never waits for the submission worker.

import { div } from "gpui-kit";
import { InputState } from "gpui-base";
import {
  tokens,
  SPACE,
  RADIUS,
  Text,
  Line,
  Code,
  Icon,
  Chip,
  AppButton,
  TextField,
  ListGroup,
  EmptyRow,
  PageHeader,
  createMultiCombo,
} from "./ui.js";
import { MONTHS, shortDate } from "./store.js";
import { frequencyOf } from "./periods.js";
import { taxpayerComboProps } from "./calendar_page.js";

/** The real statuses, in board order, with their dot tone. */
export const STATUSES = ["Draft", "Queued", "Submitted", "Confirmed", "Paid"];
const STATUS_TONE = { Draft: "accent", Queued: "accent", Submitted: "fg3", Confirmed: "success", Paid: "success" };

/** Board columns: [id, title, statuses]. Paid joins Confirmed. */
export const COLUMNS = [
  ["draft", "Draft", ["Draft"]],
  ["queued", "Queued", ["Queued"]],
  ["submitted", "Submitted", ["Submitted"]],
  ["confirmed", "Confirmed", ["Confirmed", "Paid"]],
];

const plural = (n, word) => `${n} ${word}${n === 1 ? "" : "s"}`;

function redrawPage(app, cx) {
  if (app.invalidate) app.invalidate();
  else cx.notify();
}

/** Every saved and filed return, newest change first. */
export function allReturns(app) {
  if (app.isFresh()) return [];
  const rows = [...app.store.returns("filed"), ...app.store.returns("saved")];
  return rows.sort((a, b) => (a.updated_at < b.updated_at ? 1 : a.updated_at > b.updated_at ? -1 : 0));
}

/** "Q3 2026" for a monthly or quarterly return, "TY 2026" for the rest. */
export function periodKey(index, r) {
  const freq = frequencyOf(index, r.form_code);
  const q = freq === "Monthly" ? Math.ceil(r.period / 3) : freq === "Quarterly" ? r.period : 0;
  return q >= 1 && q <= 4 ? `Q${q} ${r.taxable_year}` : `TY ${r.taxable_year}`;
}
const periodRank = (k) => {
  const m = /^(?:Q(\d)|TY) (\d{4})$/.exec(k);
  return m ? Number(m[2]) * 10 + (m[1] ? Number(m[1]) : 5) : 0;
};

/** "2026-11" of a SQLite UTC timestamp, in local time. */
export function localMonth(text) {
  const ts = /^(\d{4})-(\d{2})-(\d{2})[ T](\d{2}):(\d{2})/.exec(String(text || ""));
  if (!ts) return String(text || "").slice(0, 7);
  const d = new Date(Date.UTC(+ts[1], +ts[2] - 1, +ts[3], +ts[4], +ts[5]));
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}`;
}
const monthLabel = (ym) => `${MONTHS[Number(ym.slice(5, 7)) - 1]} ${ym.slice(0, 4)}`;

function dot(cx, tone) {
  const t = tokens(cx);
  return div().size(8).flex_none().rounded(RADIUS.full).bg(t[tone] || t.fg3);
}

/** The Filings filters and their four combo boxes. Made at boot. */
export class FilingsFilters {
  constructor(app, cx) {
    this.app = app;
    this.query = InputState.new({ placeholder: "Search returns" });
    this.query.on("change", (_e, cx) => cx.notify());
    this.reset();
    const set = (key) => (selected, cx) => {
      this[key] = selected;
      redrawPage(app, cx);
    };
    const count = (pred) => plural(allReturns(app).filter(pred).length, "return");
    this.tpCombo = createMultiCombo(
      cx,
      taxpayerComboProps(app, "filings-taxpayers", () => this.tps, (v) => (this.tps = v)),
    );
    this.formCombo = createMultiCombo(cx, {
      id: "filings-forms",
      all: "All forms",
      nouns: "forms",
      icon: "file-text",
      placeholder: "Find a form",
      source: () => {
        const index = app.store.screenIndex();
        const codes = [...new Set(allReturns(app).map((r) => r.form_code))].sort();
        return {
          options: codes.map((c) => ({ value: c, label: c, sub: (index.find((f) => f.c === c) || {}).t || "" })),
          selected: this.forms,
        };
      },
      onChange: set("forms"),
    });
    this.periodCombo = createMultiCombo(cx, {
      id: "filings-periods",
      all: "All periods",
      nouns: "periods",
      icon: "calendar",
      placeholder: "Find a period",
      width: 280,
      source: () => {
        const index = app.store.screenIndex();
        const rows = allReturns(app);
        const keys = [...new Set(rows.map((r) => periodKey(index, r)))].sort((a, b) => periodRank(b) - periodRank(a));
        return {
          options: keys.map((k) => ({ value: k, label: k, sub: plural(rows.filter((r) => periodKey(index, r) === k).length, "return") })),
          selected: this.periods,
        };
      },
      onChange: set("periods"),
    });
    this.statusCombo = createMultiCombo(cx, {
      id: "filings-statuses",
      all: "All statuses",
      nouns: "statuses",
      icon: "circle-check",
      placeholder: "Find a status",
      width: 280,
      source: () => ({
        options: STATUSES.map((s) => ({ value: s, label: s, sub: count((r) => r.status === s), lead: (cx) => dot(cx, STATUS_TONE[s]) })),
        selected: this.statuses,
      }),
      onChange: set("statuses"),
    });
    this.combos = [this.tpCombo, this.formCombo, this.periodCombo, this.statusCombo];
  }

  reset() {
    /** @type {Set<string> | null} each null is "every value" */
    this.tps = null;
    this.forms = null;
    this.periods = null;
    this.statuses = null;
    /** "YYYY-MM": only returns last changed in that month (Home's "Filed this month"). */
    this.updatedMonth = null;
  }

  active() {
    return Boolean(this.query.value().trim()) || this.updatedMonth !== null || [this.tps, this.forms, this.periods, this.statuses].some((s) => s !== null);
  }

  clear(cx) {
    this.reset();
    if (this.query.value()) this.query.set_value("");
    for (const c of this.combos) c.refresh(cx);
    cx.notify();
  }

  /**
   * A Home tile's preset (`app.filingsPreset`, dashboard.js FilingsPreset):
   * its statuses and month replace every filter, once.
   */
  applyPreset(app, cx) {
    const preset = app.filingsPreset;
    if (!preset) return;
    app.filingsPreset = null;
    this.reset();
    if (this.query.value()) this.query.set_value("");
    if (Array.isArray(preset.statuses)) {
      const on = new Set(preset.statuses.filter((s) => STATUSES.includes(s)));
      this.statuses = on.size === STATUSES.length ? null : on;
    }
    if (typeof preset.updatedMonth === "string" && /^\d{4}-\d{2}$/.test(preset.updatedMonth)) this.updatedMonth = preset.updatedMonth;
    for (const c of this.combos) c.refresh(cx);
  }

  /** Esc: close the open combo box; false when none was open. */
  dismiss(cx) {
    return this.combos.some((c) => c.close(cx));
  }

  /** The returns every filter and the search keep. */
  rows() {
    const index = this.app.store.screenIndex();
    const words = this.query.value().trim().toLowerCase().split(/\s+/).filter(Boolean);
    return allReturns(this.app).filter((r) => {
      if (this.tps && !this.tps.has(r.tin_root)) return false;
      if (this.forms && !this.forms.has(r.form_code)) return false;
      if (this.statuses && !this.statuses.has(r.status)) return false;
      if (this.periods && !this.periods.has(periodKey(index, r))) return false;
      if (this.updatedMonth && localMonth(r.updated_at) !== this.updatedMonth) return false;
      if (!words.length) return true;
      const hay = `${r.form_code} ${r.period_label} ${r.taxpayer_name} ${r.branch_name} ${r.status} ${r.tin}`.toLowerCase();
      return words.every((w) => hay.includes(w));
    });
  }
}

const whoLine = (r) => (r.branch_count > 1 ? `${r.taxpayer_name} · ${r.branch_name}` : r.taxpayer_name);

function statusChip(cx, status) {
  if (status === "Paid") return Chip(cx, "Paid", "success", "check");
  if (status === "Confirmed") return Chip(cx, "Confirmed", "success", "check");
  if (status === "Submitted") return Chip(cx, "Submitted", "neutral", "send");
  if (status === "Queued") return Chip(cx, "Queued", "accent");
  return Chip(cx, "Draft", "accent");
}

function openReturn(app, r, cx) {
  app.selRoot = r.tin_root;
  app.selCode = r.branch_code;
  app.openForm({ tin: r.tin, code: r.form_code, year: r.taxable_year, period: r.period, ...(r.return_key != null ? { returnKey: r.return_key } : {}) }, cx);
}

/** A button inside a card: its click stays off the card's own Open. */
const own = (run) => (e, cx) => {
  cx.stop_propagation();
  run(e, cx);
};

/** One return on the board: code and period, taxpayer, status and date, its actions. */
function boardCard(cx, app, r) {
  const t = tokens(cx);
  const filed = r.status === "Submitted" || r.status === "Confirmed" || r.status === "Paid";
  const actions = div().flex().flex_row().flex_wrap().gap(SPACE[1]).mt(2);
  if (r.status === "Queued") {
    actions.child(
      AppButton(cx, `return-${r.id}-cancel`, "Cancel queue", {
        variant: "ghost",
        size: "sm",
        onClick: own((_e, cx) =>
          app.cancelQueuedEntry({ tin: r.tin, form_code: r.form_code, taxable_year: r.taxable_year, quarter: r.period || null }, cx),
        ),
      }),
    );
  }
  if (filed && !app.store.templates(r.tin).some((tpl) => tpl.form_code === r.form_code)) {
    actions.child(
      AppButton(cx, `return-${r.id}-use-template`, "Use as template", {
        size: "sm",
        icon: "layout-template",
        onClick: own((_e, cx) => app.taxpayerUi.useAsTemplate(r, cx)),
      }),
    );
  }
  actions.child(AppButton(cx, `return-${r.id}-open`, "Open", { variant: "ghost", size: "sm", onClick: own((_e, cx) => openReturn(app, r, cx)) }));
  return div()
    .id(`return-${r.id}`)
    .flex()
    .flex_col()
    .gap(SPACE[1])
    .w_full()
    .py(10)
    .px(SPACE[3])
    .rounded(RADIUS.row)
    .border(1)
    .border_color(t.border)
    .bg(t.surface)
    .cursor_pointer()
    .hover((s) => s.border_color(t.borderStrong))
    .accessibility_label(`${r.form_code} ${r.period_label}, ${whoLine(r)}, ${r.status}`)
    .on_click((_e, cx) => openReturn(app, r, cx))
    .child(div().flex().flex_row().items_center().gap(6).min_w(0).child(Code(cx, r.form_code)).child(Line(cx, "meta", r.period_label, "fg2")))
    .child(Line(cx, "meta", whoLine(r), "fg3"))
    // Status and date. A board column leaves a card about 147 px inside at
    // 1200 px and wider (816 px over four columns, less their 10 px padding
    // and the card's 12 px): "Submitted" with its icon and "Nov 13, 2026"
    // need more, so the row wraps and the date takes its own line, right
    // aligned, rather than running past the card's edge.
    .child(
      div()
        .id(`return-${r.id}-meta`)
        .flex()
        .flex_row()
        .flex_wrap()
        .items_center()
        .gap_x(6)
        .gap_y(2)
        .mt(2)
        .min_w(0)
        .child(div().flex_none().child(statusChip(cx, r.status)))
        .child(Text(cx, "cap", shortDate(r.updated_at), "fg3").whitespace_nowrap().flex_none().ml_auto()),
    )
    .child(actions);
}

function board(cx, app, rows, width) {
  const t = tokens(cx);
  const wide = width >= 720;
  const cols = div().id("filings-board").flex().gap(SPACE[3]).mt(SPACE[4]).w_full();
  if (wide) cols.flex_row().items_start();
  else cols.flex_col();
  for (const [id, title, statuses] of COLUMNS) {
    const list = rows.filter((r) => statuses.includes(r.status));
    const col = div()
      .id(`filings-col-${id}`)
      .flex()
      .flex_col()
      .gap(SPACE[2])
      .p(10)
      .min_w(0)
      .rounded(RADIUS.card)
      .border(1)
      .border_color(t.border)
      .bg(t.bg)
      .child(
        div()
          .flex()
          .flex_row()
          .items_center()
          .gap(6)
          .px(2)
          .child(Text(cx, "control", title).font_weight(600))
          .child(Chip(cx, String(list.length))),
      );
    if (wide) col.flex_1().flex_basis(0);
    if (list.length) for (const r of list) col.child(boardCard(cx, app, r));
    else col.child(Text(cx, "meta", "Nothing here.", "fg3").px(2).py(SPACE[2]));
    cols.child(col);
  }
  return cols;
}

export function renderFilings(cx, app, width) {
  const f = app.filingsUi;
  const all = allReturns(app);
  const rows = f.rows();
  const bar = div()
    .id("filings-filters")
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap(SPACE[2])
    .mt(SPACE[6])
    .child(TextField(cx, f.query, { id: "filings-search", label: "Search returns", width: 200 }));
  for (const c of f.combos) bar.child(c.entity);
  if (f.updatedMonth) {
    bar.child(
      AppButton(cx, "filings-month-chip", `Changed in ${monthLabel(f.updatedMonth)}`, {
        size: "sm",
        trailing: Icon(cx, "x", { size: 14, color: "fg3" }),
        ariaLabel: `Changed in ${monthLabel(f.updatedMonth)}. Remove`,
        onClick: (_e, cx) => {
          f.updatedMonth = null;
          cx.notify();
        },
      }),
    );
  }
  if (f.active()) {
    bar.child(AppButton(cx, "filings-clear", "Clear filters", { variant: "ghost", size: "sm", icon: "x", onClick: (_e, cx) => f.clear(cx) }));
  }
  bar.child(div().id("filings-count").ml_auto().child(Text(cx, "meta", plural(rows.length, "return"), "fg2")));
  const page = div()
    .flex()
    .flex_col()
    .child(
      PageHeader(cx, {
        title: "Filings",
        lede: "Every return, from draft to confirmed. Filed returns stay exactly as submitted. Use one as a template to prefill the next period.",
      }),
    )
    .child(bar);
  if (!all.length) {
    return page.child(div().mt(SPACE[4]).child(ListGroup(cx, "filings-list", [EmptyRow(cx, "filings-empty", "Saved and filed returns show here.")])));
  }
  if (!rows.length) {
    return page.child(
      div()
        .mt(SPACE[4])
        .child(
          ListGroup(cx, "filings-list", [
            EmptyRow(
              cx,
              "filings-empty",
              "No returns match these filters.",
              AppButton(cx, "filings-clear-empty", "Clear filters", { size: "sm", onClick: (_e, cx) => f.clear(cx) }),
            ),
          ]),
        ),
    );
  }
  return page.child(board(cx, app, rows, width));
}
