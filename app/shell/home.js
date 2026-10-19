// Home: the greeting, returns due in the next 30 days, the as-needed forms
// the taxpayers file (each with Start a return), recently filed — or,
// on a fresh install, the welcome with three numbered steps
// (DESIGN_SYSTEM.md "Empty state"). Shared row renderers for Calendar and
// Filings live here too.

import { div } from "gpui-kit";
import {
  tokens,
  SPACE,
  Text,
  Code,
  Icon,
  AppButton,
  StatusChip,
  ListGroup,
  ListRow,
  NameCell,
  EmptyRow,
  PageHeader,
  SectionTitle,
  StepCircle,
} from "./ui.js";
import { isoDate, addDays, daysUntil, MONTHS, shortDate } from "./store.js";

const WEEKDAYS = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MONTHS_LONG = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

function greeting() {
  const h = new Date().getHours();
  return h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening";
}

function longToday() {
  const d = new Date();
  return `${WEEKDAYS[d.getDay()]}, ${MONTHS_LONG[d.getMonth()]} ${d.getDate()}, ${d.getFullYear()}`;
}

function statusKind(status) {
  if (status === "Draft") return "draft";
  if (status === "Queued") return "queued";
  if (status === "Submitted" || status === "Confirmed" || status === "Paid") return "filed";
  return "not_started";
}

/** Who a row belongs to: the taxpayer, plus the branch when it has several. */
function whoLine(row) {
  return row.branch_count > 1 ? `${row.taxpayer_name} · ${row.branch_name}` : row.taxpayer_name;
}

/** A deadline row: date box, code, "<code> · <period>" over the taxpayer, when, status. */
export function dueRow(cx, app, d, width) {
  const n = daysUntil(d.due_date);
  const [, m, day] = d.due_date.split("-").map(Number);
  const open = d.status === "Not started" || d.status === "Draft";
  const when =
    open && n < 0
      ? StatusChip(cx, "overdue", n)
      : open && n <= 14
        ? StatusChip(cx, "due_soon", n)
        : Text(cx, "meta", n === 0 ? "Today" : n < 0 ? `${-n} days ago` : `In ${n} days`, "fg2");
  const compact = width < 560;
  // An as-needed row is keyed by its saved return as well. For the 2200
  // family a month holds one return (a stored return is matched by month
  // and year, an earlier maintainer decision), so two removals in one month
  // are one return; other as-needed forms can hold several returns that
  // share a year and period column, and the return keeps their rows apart.
  const key = `due-${d.tin}-${d.form_code}-${d.taxable_year}-${d.period}${d.frequency === "As needed" ? `-${d.draft_id}` : ""}`;
  const row = ListRow(cx, key, {
    onClick: (_e, cx) => {
      app.selRoot = d.tin_root;
      app.selCode = d.branch_code;
      // A saved return opens exactly (two in one month stay two).
      app.openForm({ tin: d.tin, code: d.form_code, year: d.taxable_year, period: d.period, ...(d.return_key != null ? { returnKey: d.return_key } : {}) }, cx);
    },
  })
    .child(
      div()
        .flex()
        .flex_col()
        .items_center()
        .w(44)
        .flex_none()
        .child(Text(cx, "cap", MONTHS[m - 1].toUpperCase(), "fg3").font_weight(600))
        .child(Text(cx, "title", String(day))),
    )
    .child(Code(cx, d.form_code).w(68).flex_none())
    .child(NameCell(cx, `${d.form_code} · ${d.period_label}`, whoLine(d)));
  if (!compact) row.child(div().w(96).flex_none().flex().flex_row().child(when));
  row.child(div().w(88).flex_none().flex().justify_end().child(StatusChip(cx, statusKind(d.status))));
  return row;
}

/** A filed (or saved) return row with its date and "Use as template". */
export function returnRow(cx, app, r, width) {
  const compact = width < 560;
  const filed = statusKind(r.status) === "filed";
  const hasTemplate = app.store.templates(r.tin).some((t) => t.form_code === r.form_code);
  const row = ListRow(cx, `return-${r.id}`)
    .child(Code(cx, r.form_code).w(68).flex_none())
    .child(NameCell(cx, `${r.form_code} · ${r.period_label}`, whoLine(r)));
  if (!compact) row.child(Text(cx, "meta", shortDate(r.updated_at), "fg2").w(104).flex_none());
  const actions = div().flex().flex_row().justify_end().gap(SPACE[1]).flex_none();
  if (r.status === "Queued") {
    actions.child(
      AppButton(cx, `return-${r.id}-cancel`, "Cancel queue", {
        variant: "ghost",
        size: "sm",
        onClick: (_e, cx) =>
          app.cancelQueuedEntry(
            { tin: r.tin, form_code: r.form_code, taxable_year: r.taxable_year, quarter: r.period || null },
            cx,
          ),
      }),
    );
  }
  if (hasTemplate || !filed) {
    actions.child(StatusChip(cx, statusKind(r.status)));
  } else {
    actions.child(
      AppButton(cx, `return-${r.id}-use-template`, "Use as template", {
        size: "sm",
        icon: "layout-template",
        onClick: (_e, cx) => app.taxpayerUi.useAsTemplate(r, cx),
      }),
    );
  }
  row.child(actions);
  row.child(
    AppButton(cx, `return-${r.id}-open`, "Open", {
      variant: "ghost",
      size: "sm",
      onClick: (_e, cx) => {
        app.selRoot = r.tin_root;
        app.selCode = r.branch_code;
        app.openForm({ tin: r.tin, code: r.form_code, year: r.taxable_year, period: r.period, ...(r.return_key != null ? { returnKey: r.return_key } : {}) }, cx);
      },
    }),
  );
  return row;
}

/**
 * An as-needed form a branch files: code, "<code> · <name>" over the
 * taxpayer and when it is due, and "Start a return". A return that holds its
 * trigger date is listed with the deadlines instead; one that does not yet
 * follows on its own row, with "Open".
 */
function asNeededRows(cx, app, f, width) {
  const compact = width < 560;
  const id = `as-needed-${f.tin}-${f.form_code}`;
  const select = () => {
    app.selRoot = f.tin_root;
    app.selCode = f.branch_code;
    app.selYear = f.taxable_year;
  };
  const head = ListRow(cx, id)
    .child(Code(cx, f.form_code).w(68).flex_none())
    .child(NameCell(cx, `${f.form_code} · ${f.form_name}`, compact ? whoLine(f) : `${whoLine(f)} · ${f.due_rule}`))
    .child(
      AppButton(cx, `${id}-start`, "Start a return", {
        size: "sm",
        icon: "plus",
        onClick: (_e, cx) => {
          select();
          app.startNewReturn(cx, f.form_code);
        },
      }),
    );
  const rows = [head];
  for (const r of f.undated) {
    const row = ListRow(cx, `${id}-${r.draft_id}`)
      .child(div().w(68).flex_none())
      .child(NameCell(cx, f.trigger ? `No ${f.trigger.toLowerCase()} date yet` : "Saved return", `Saved ${shortDate(r.updated_at)}`));
    row.child(div().w(88).flex_none().flex().justify_end().child(StatusChip(cx, statusKind(r.status))));
    row.child(
      AppButton(cx, `${id}-${r.draft_id}-open`, "Open", {
        variant: "ghost",
        size: "sm",
        onClick: (_e, cx) => {
          select();
          app.openForm({ tin: f.tin, code: f.form_code, year: f.taxable_year, period: r.period, ...(r.return_key != null ? { returnKey: r.return_key } : {}) }, cx);
        },
      }),
    );
    rows.push(row);
  }
  return rows;
}

function welcome(cx, app) {
  const steps = [
    ["user", "Add a taxpayer", "TIN, registered name and head-office address"],
    ["calendar", "Add a tax year", "Any year up to next year, per branch"],
    ["file-text", "Choose its forms", "All 43 BIR forms, or start from a pack"],
  ];
  const rows = steps.map(([icon, title, line], i) =>
    ListRow(cx, `welcome-step-${i + 1}`)
      .child(StepCircle(cx, String(i + 1)))
      .child(Icon(cx, icon))
      .child(NameCell(cx, title, line)),
  );
  rows.push(
    ListRow(cx, "welcome-cta").child(
      AppButton(cx, "welcome-add-taxpayer", "Add your first taxpayer", {
        variant: "primary",
        icon: "plus",
        onClick: (_e, cx) => app.taxpayerUi.openNewTaxpayer(cx),
      }),
    ),
  );
  return div()
    .flex()
    .flex_col()
    .child(
      PageHeader(cx, {
        title: "Welcome to eBIRForms",
        lede: "Prepare, print and track BIR returns for every taxpayer you handle. Everything stays on this computer.",
      }),
    )
    .child(div().mt(28).child(ListGroup(cx, "welcome-steps", rows)));
}

export function renderHome(cx, app, width) {
  if (app.isFresh()) return welcome(cx, app);
  const today = new Date();
  const soon = app.store
    .deadlines(isoDate(addDays(today, -30)), isoDate(addDays(today, 30)))
    .filter((d) => daysUntil(d.due_date) >= 0 || d.status === "Not started" || d.status === "Draft");
  const filed = app.store.returns("filed").slice(0, 3);
  const asNeeded = app.store.asNeeded(today.getFullYear());
  const taxpayerCount = new Set(soon.map((d) => d.tin_root)).size;
  const lede = soon.length
    ? `${soon.length} return${soon.length === 1 ? " is" : "s are"} due in the next 30 days across ${taxpayerCount} taxpayer${taxpayerCount === 1 ? "" : "s"}. ${longToday()}.`
    : `Nothing is due in the next 30 days. ${longToday()}.`;
  const page = div()
    .flex()
    .flex_col()
    .child(PageHeader(cx, { title: greeting(), lede }))
    .child(div().mt(28).child(SectionTitle(cx, "Due in the next 30 days", true)))
    .child(
      ListGroup(
        cx,
        "home-due",
        soon.length
          ? soon.map((d) => dueRow(cx, app, d, width))
          : [EmptyRow(cx, "home-due-empty", "Deadlines show here once a taxpayer has forms for a tax year.")],
      ),
    );
  // Filed when something happens (a removal, a withholding, a sale): no
  // deadline until a return holds that date.
  if (asNeeded.length) {
    page
      .child(SectionTitle(cx, "As needed"))
      .child(ListGroup(cx, "home-as-needed", asNeeded.flatMap((f) => asNeededRows(cx, app, f, width))));
  }
  return page
    .child(SectionTitle(cx, "Recently filed"))
    .child(
      ListGroup(
        cx,
        "home-filed",
        filed.length ? filed.map((r) => returnRow(cx, app, r, width)) : [EmptyRow(cx, "home-filed-empty", "Filed returns show here.")],
      ),
    );
}

