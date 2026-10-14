// Filings: every return that left Draft, newest first, with "Use as
// template" (D17) and Cancel queue for a return still Queued.

import { div } from "gpui-kit";
import { ListGroup, EmptyRow, PageHeader } from "./ui.js";
import { returnRow } from "./home.js";

export function renderFilings(cx, app, width) {
  const rows = app.isFresh() ? [] : app.store.returns("filed");
  return div()
    .flex()
    .flex_col()
    .child(
      PageHeader(cx, {
        title: "Filings",
        lede: "Filed returns stay exactly as submitted. Use one as a template to prefill the next period.",
      }),
    )
    .child(
      div()
        .mt(24)
        .child(
          ListGroup(
            cx,
            "filings-list",
            rows.length ? rows.map((r) => returnRow(cx, app, r, width)) : [EmptyRow(cx, "filings-empty", "Filed returns show here.")],
          ),
        ),
    );
}
