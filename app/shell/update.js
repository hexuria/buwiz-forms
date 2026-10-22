// The update prompt. The host downloads and verifies a release on its own
// thread; this module only shows the result and asks before installing.
// It sits over setup and sign-in as well as the signed-in app, because a
// first launch is when a scratch install is most likely to be behind.

import { div } from "gpui-kit";
import * as bir from "bir";
import { tokens, RADIUS, Text, AppButton } from "./ui.js";

export function pollUpdateStatus(app) {
  let next;
  try {
    next = bir.update_status();
  } catch (_e) {
    return false;
  }
  const prev = app.updateStatus;
  if (
    prev &&
    prev.phase === next.phase &&
    prev.version === next.version &&
    prev.message === next.message &&
    prev.auto_download === next.auto_download
  ) {
    return false;
  }
  app.updateStatus = next;
  return true;
}

export function setAutoUpdateDownload(enabled) {
  bir.set_auto_update_download(Boolean(enabled));
}

export function installReadyUpdate() {
  try {
    bir.install_update();
  } catch (_e) {}
}

function banner(cx, t, message, buttons) {
  const row = div()
    .id("update-banner")
    .absolute()
    .bottom(16)
    .left(16)
    .right(16)
    .flex()
    .flex_row()
    .items_center()
    .gap(12)
    .px(16)
    .py(12)
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .bg(t.surface)
    .shadow_lg()
    .child(
      div()
        .id("update-message")
        .flex_1()
        .min_w(0)
        .child(Text(cx, "body", message, "fg").whitespace_normal()),
    );
  for (const button of buttons) row.child(button);
  return row;
}

function promptFor(cx, app) {
  const status = app.updateStatus;
  if (!status) return null;
  const t = tokens(cx);
  if (status.phase === "failed") {
    return banner(
      cx,
      t,
      status.message || "The update could not be verified and was not installed.",
      [
        AppButton(cx, "update-later", "Later", {
          size: "sm",
          onClick: (_e, cx) => {
            try { bir.dismiss_update(); } catch (_e) {}
            app.updateStatus = { ...status, phase: "idle", message: null };
            cx.notify();
          },
        }),
      ],
    );
  }
  if (status.phase === "ready" && status.version) {
    return banner(cx, t, `Version ${status.version} is ready.`, [
      AppButton(cx, "update-install", "Install now", {
        variant: "primary",
        size: "sm",
        onClick: () => installReadyUpdate(),
      }),
      AppButton(cx, "update-later", "Later", {
        size: "sm",
        onClick: (_e, cx) => {
          try { bir.dismiss_update(); } catch (_e) {}
          app.updateStatus = { ...status, phase: "idle" };
          cx.notify();
        },
      }),
    ]);
  }
  if (status.phase === "up_to_date" && status.message) {
    return banner(cx, t, status.message, [
      AppButton(cx, "update-later", "Later", {
        size: "sm",
        onClick: (_e, cx) => {
          try { bir.dismiss_update(); } catch (_e) {}
          app.updateStatus = { ...status, phase: "idle", message: null };
          cx.notify();
        },
      }),
    ]);
  }
  return null;
}

/** Draw `page` and, when an update is waiting, a banner over its bottom edge. */
export function renderUpdatePrompt(cx, app, page) {
  const prompt = promptFor(cx, app);
  if (!prompt) return page;
  return div().id("update-layer").relative().size_full().child(page).child(prompt);
}
