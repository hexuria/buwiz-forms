// Import Data — the `import_export` view. The painted page is a single
// "Smart Import" card: one button opens a native file dialog, sniffs the zip,
// and imports it as a database backup or a profile export. The dialog + zip
// handling live host-side in `bir.import_archive`.
import { div } from "gpui-kit";
import { v_flex, h_flex } from "gpui-base";
import { Button } from "gpui-component";
import { pageHeading, pageSub } from "./pages.js";

function muted(cx) {
  return cx.theme().colors.muted_foreground;
}

export function renderImportPage(cx, state) {
  return v_flex()
    .size_full()
    .px(24)
    .py(16)
    .gap(16)
    .children([
      v_flex().gap(4).children([
        pageHeading("Import Data", cx),
        pageSub(
          "Import a profile or database backup from a .zip file.",
          cx,
        ),
      ]),
      v_flex()
        .w_full()
        .p(20)
        .gap(12)
        .rounded_xl()
        .border(1)
        .border_color(cx.theme().colors.border)
        .bg(cx.theme().colors.background)
        .children([
          v_flex().gap(4).children([
            div()
              .text_size(14)
              .font_semibold()
              .text_color(cx.theme().colors.foreground)
              .child("Smart Import"),
            div()
              .text_size(12)
              .text_color(muted(cx))
              .child(
                "Select a .zip export archive. The app will automatically detect if it is a full database backup or a profile export and import it accordingly.",
              ),
          ]),
          h_flex().children([
            Button.new("smart_import_btn")
              .label("Smart Import")
              .primary()
              .on_click((_ev, clickCx) => state.runImport(clickCx)),
          ]),
        ]),
    ])
    .when(state.importNotice !== null && state.importNotice !== undefined, (el) =>
      el.child(
        div()
          .text_size(12)
          .text_color(
            state.importNoticeIsError
              ? cx.theme().colors.danger
              : cx.theme().colors.success,
          )
          .child(state.importNotice),
      ),
    );
}
