// Shared heading helpers for the retained pre-redesign pages (lock screen,
// the legacy profile editor and Background tasks). New screens use ui.js.

import { div } from "gpui-kit";

function muted(cx) {
  return cx.theme().colors.muted_foreground;
}

export function pageHeading(text, cx) {
  return div()
    .text_size(20)
    .font_semibold()
    .text_color(cx.theme().colors.foreground)
    .child(text);
}

export function pageSub(text, cx) {
  return div()
    .text_size(13)
    .text_color(muted(cx))
    .child(text);
}
