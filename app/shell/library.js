// Form library: every return the app can prepare (the 43 on-screen forms),
// as a card grid with search and a frequency filter. Cards get pixel widths
// computed from the column width (no auto-fill grid in GPUI, and no
// flex-grown text cells: see the layout lesson in AGENTS.md).

import { div } from "gpui-kit";
import { tokens, SPACE, RADIUS, Text, Icon, Chip, Segmented, TextField, PageHeader, EmptyRow } from "./ui.js";
import { periodOptions } from "./periods.js";

const FREQS = ["All", "Monthly", "Quarterly", "Annual", "As needed"];

export function searchText(f) {
  const vat = /Value-Added/i.test(f.t) ? " vat" : "";
  return `${f.c} ${f.c.replace(/-/g, "")} ${f.t}${vat}`.toLowerCase();
}

function formCard(cx, app, f, width) {
  const t = tokens(cx);
  const top = div().flex().flex_row().items_center().gap(SPACE[2]).child(Text(cx, "title", f.c)).child(div().flex_1());
  if (f.q < 0.8) top.child(Chip(cx, "Draft layout", "warning"));
  return div()
    .id(`library-${f.c}`)
    .flex()
    .flex_col()
    .gap(SPACE[2])
    .w(width)
    .py(14)
    .px(SPACE[4])
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .bg(t.surface)
    .cursor_pointer()
    .hover((s) => s.border_color(t.borderStrong).shadow_md())
    .accessibility_label(`Form ${f.c}: ${f.t}`)
    .on_click((_e, cx) => {
      if (app.isFresh()) return app.toast("Add a taxpayer first, then open a form for them.", cx);
      const b = app.branch();
      const year = app.year() || new Date().getFullYear();
      const options = periodOptions(f.c, f.freq, year);
      app.openForm({ tin: b.tin, code: f.c, year, period: options.length ? options[0].value : 0 }, cx);
    })
    .child(top)
    .child(Text(cx, "control", f.t).line_height("18px").h(36).overflow_hidden().whitespace_normal())
    .child(
      div()
        .flex()
        .flex_row()
        .flex_wrap()
        .gap(6)
        .child(Chip(cx, f.freq))
        .child(Chip(cx, `${f.pages}p`))
        .child(Chip(cx, `${f.n} fields`)),
    );
}

export function renderLibrary(cx, app, width) {
  const t = tokens(cx);
  const index = app.store.screenIndex();
  const q = app.library.query.value().trim().toLowerCase();
  const tp = app.taxpayer();
  const b = app.branch();
  const year = app.year();
  // Scoped to the selected taxpayer and branch: the forms it files in the
  // selected tax year, unless "All forms" is chosen.
  const owned = new Set(app.yearForms());
  const scope = tp ? app.library.scope || "owned" : "all";
  const list = index.filter(
    (f) =>
      (scope === "all" || owned.has(f.c)) &&
      (app.library.freq === "All" || f.freq === app.library.freq) &&
      (!q || searchText(f).includes(q)),
  );

  const tools = div()
    .flex()
    .flex_row()
    .flex_wrap()
    .items_center()
    .gap_x(SPACE[3])
    .gap_y(SPACE[2])
    .mt(36)
    .mb(SPACE[4])
    .child(
      div()
        .flex()
        .flex_row()
        .items_center()
        .gap(SPACE[2])
        .w(Math.min(320, width))
        .h(32)
        .pl(10)
        .rounded(RADIUS.row)
        .border(1)
        .border_color(t.border)
        .bg(t.surface)
        .child(Icon(cx, "search", { color: "fg3" }))
        .child(TextField(cx, app.library.query, { id: "library-search", label: "Find a form" }).border(0).px(0).h(30).flex_1()),
    )
    .when(Boolean(tp), (el) =>
      el.child(
        Segmented(
          cx,
          "library-scope",
          [
            { value: "owned", label: `Filed in ${year || "this year"} (${owned.size})` },
            { value: "all", label: `All forms (${index.length})` },
          ],
          scope,
          (v, cx) => {
            app.library.scope = v;
            cx.notify();
          },
          { label: "Which forms" },
        ),
      ),
    )
    .child(
      Segmented(
        cx,
        "library-freq",
        FREQS.map((x) => ({ value: x, label: x })),
        app.library.freq,
        (v, cx) => {
          app.library.freq = v;
          cx.notify();
        },
        { label: "Frequency" },
      ),
    )
    .child(Text(cx, "meta", `${list.length} of ${index.length}`, "fg2"));

  const page = div()
    .flex()
    .flex_col()
    .child(
      PageHeader(cx, {
        title: "Form library",
        lede: tp
          ? `Forms ${tp.name}${tp.branches.length > 1 ? ` (${b.name})` : ""} files in ${year || "this year"}. Choose All forms to see every return eBIRForms can prepare.`
          : "Every return eBIRForms can prepare. Add a taxpayer to fill one in.",
      }),
    )
    .child(tools);
  if (!list.length) {
    const text =
      scope === "owned" && !owned.size
        ? `${tp.name} has no forms for ${year || "this year"} yet. Choose them under Forms, or see All forms.`
        : "No form matches. Try a code like 1601C or a word like VAT.";
    return page.child(EmptyRow(cx, "library-empty", text));
  }
  const gap = 12;
  const cols = Math.max(1, Math.floor((width + gap) / (250 + gap)));
  const cardW = Math.floor((width - gap * (cols - 1)) / cols);
  const grid = div().id("library-grid").flex().flex_row().flex_wrap().gap(gap).w(width);
  for (const f of list) grid.child(formCard(cx, app, f, cardW));
  return page.child(grid);
}
