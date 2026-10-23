// Full-window Buwiz account gate. Nothing that names a taxpayer is drawn
// until the device flow is approved. The host refuses data verbs on its own;
// this screen is only how a person gets there.

import { div } from "gpui-kit";
import { tokens, SPACE, RADIUS, Text, AppButton, TextField } from "./ui.js";
import { errorText } from "./store.js";

const CARD_W = 400;

export function renderAccountGate(cx, app) {
  const t = tokens(cx);
  const busy = Boolean(app.gateBusy);
  const detail =
    app.gateError ||
    app.gateDetail ||
    "Sign in with your Buwiz account to file from this computer.";
  return div()
    .id("gate-root")
    .flex()
    .flex_col()
    .size_full()
    .bg(t.bg)
    .text_color(t.fg)
    .track_focus(app.focusRoot)
    .on_action("cloud-sign-out", (_e, cx) => app.lockCloud(cx))
    .child(
      div()
        .flex()
        .flex_1()
        .min_h(0)
        .items_center()
        .justify_center()
        .px(SPACE[4])
        .child(
          div()
            .id("gate-card")
            .flex()
            .flex_col()
            .gap(SPACE[4])
            .w(CARD_W)
            .p(SPACE[8])
            .rounded(RADIUS.dialog)
            .border(1)
            .border_color(t.border)
            .bg(t.surface)
            .child(Text(cx, "page", "Welcome to eBIRForms").text_center())
            .child(
              Text(cx, "meta", detail, app.gateError ? "danger" : "fg2")
                .text_center()
                .whitespace_normal(),
            )
            .child(TextField(cx, app.serverUrl, { id: "gate-server", label: "Buwiz server" }))
            .child(
              AppButton(cx, "gate-sign-in", busy ? "Waiting…" : "Sign in", {
                variant: "primary",
                block: true,
                disabled: busy,
                onClick: (_e, cx) => startAccountFlow(app, "sign-in", cx),
              }),
            )
            .child(
              AppButton(cx, "gate-create-account", busy ? "Waiting…" : "Create account", {
                block: true,
                disabled: busy,
                onClick: (_e, cx) => startAccountFlow(app, "create", cx),
              }),
            ),
        ),
    );
}

export function startAccountFlow(app, kind, cx) {
  if (app.gateBusy) return;
  app.gateBusy = true;
  app.gateError = null;
  app.gateDetail = "Opening your browser…";
  cx.notify();
  const verb = kind === "create" ? "cloud_account_begin_sign_up" : "cloud_account_begin_sign_in";
  cx.spawn(async (acx) => {
    try {
      const url = app.serverUrl && app.serverUrl.value ? app.serverUrl.value().trim() : "";
      if (url) app.store.call("cloud_account_set_server_url", url);
      const started = await app.store.call(verb);
      const code = started.user_code ? ` Code ${started.user_code}.` : "";
      app.gatePhase = "waiting";
      app.gateDetail = started.browser_opened
        ? `Finish in the browser, then return here.${code}`
        : `Open ${started.verification_uri_complete} and approve this computer.${code}`;
      acx.notify();
      await app.store.call("cloud_account_finish_sign_in");
      app.finishUnlock(acx);
    } catch (error) {
      app.gateError = errorText(error);
      app.gateBusy = false;
      app.gatePhase = "welcome";
      acx.notify();
    }
  });
}
