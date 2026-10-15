// Sign-in and the taxpayer lock. Both take the whole window: nothing about
// any taxpayer (sidebar, panel, title-bar pill, search) is drawn behind them.
//
// Sign-in has two doors. A taxpayer signs in with their TIN and PIN and then
// sees only that taxpayer and its branches; the admin signs in with the admin
// PIN or authenticator and sees everyone. A taxpayer who forgot their PIN can
// be let in by the admin ("Forgot PIN?"), and can then set a new one under
// Tax profile › Security.

import { div } from "gpui-kit";
import { tokens, SPACE, RADIUS, Text, Icon, AppButton, Segmented, PinCells, DigitCells } from "./ui.js";
import { renderBareTitleBar } from "./chrome.js";

const CARD_W = 400;

function screen(cx, app, children) {
  const t = tokens(cx);
  return div()
    .id("auth-screen")
    .flex()
    .flex_col()
    .size_full()
    .bg(t.bg)
    .text_color(t.fg)
    .track_focus(app.focusRoot)
    .on_action("toggle-theme", (_e, cx) => app.cycleTheme(cx))
    .child(renderBareTitleBar(cx, app))
    .child(
      div()
        .flex()
        .flex_1()
        .min_h(0)
        .items_center()
        .justify_center()
        .child(
          div()
            .id("auth-card")
            .flex()
            .flex_col()
            .gap(SPACE[4])
            .w(CARD_W)
            .p(SPACE[8])
            .rounded(RADIUS.dialog)
            .border(1)
            .border_color(t.border)
            .bg(t.surface)
            .shadow_lg()
            .children(children.filter(Boolean)),
        ),
    );
}

function heading(cx, title, line) {
  return div()
    .flex()
    .flex_col()
    .items_center()
    .gap(SPACE[2])
    .child(div().flex().items_center().justify_center().size(44).rounded(RADIUS.full).bg(tokens(cx).hover).child(Icon(cx, "lock", { color: "fg2" })))
    .child(Text(cx, "page", title).text_center())
    .child(Text(cx, "meta", line, "fg2").text_center().whitespace_normal());
}

function labelled(cx, label, el) {
  return div().flex().flex_col().gap(SPACE[2]).child(Text(cx, "meta", label, "fg2")).child(el);
}

function errorLine(cx, app) {
  return app.authError ? Text(cx, "meta", app.authError, "danger").whitespace_normal() : null;
}

function link(cx, id, label, onClick) {
  return div()
    .id(id)
    .cursor_pointer()
    .accessibility_label(label)
    .on_click((_e, cx) => onClick(cx))
    .child(Text(cx, "metaStrong", label, "accent"));
}

function actions(cx, left, right) {
  return div().flex().flex_row().items_center().justify_between().gap(SPACE[2]).child(left || div()).child(right);
}

/** Admin code cells: PIN and/or authenticator, whichever the admin set up. */
function adminCells(cx, app, auth) {
  const rows = [];
  if (auth.has_admin_pin) rows.push(labelled(cx, "Admin PIN", PinCells(cx, app.adminPinOtp, "auth-admin-pin")));
  if (auth.has_admin_totp) rows.push(labelled(cx, "Admin authenticator code", PinCells(cx, app.adminTotpOtp, "auth-admin-totp")));
  return rows;
}

/** The sign-in screen: after Log out or Lock app, and at launch when the app is locked. */
export function renderSignIn(cx, app) {
  const auth = app.store.authState();
  const doors = app.signInDoors();
  const s = app.signIn;
  const mode = doors.taxpayer && doors.admin ? s.mode : doors.taxpayer ? "taxpayer" : "admin";
  const children = [];
  if (mode === "taxpayer") {
    children.push(heading(cx, "Sign in", s.forgot ? "The admin can open this taxpayer for you." : "Enter your TIN and your taxpayer PIN."));
  } else {
    children.push(heading(cx, "Sign in as admin", "Enter the admin PIN or authenticator code."));
  }
  if (doors.taxpayer && doors.admin) {
    children.push(
      div()
        .flex()
        .justify_center()
        .child(
          Segmented(
            cx,
            "auth-door",
            [
              { value: "taxpayer", label: "Taxpayer" },
              { value: "admin", label: "Admin" },
            ],
            mode,
            (v, cx) => app.setSignInMode(v, cx),
            { label: "Sign in as" },
          ),
        ),
    );
  }
  if (mode === "taxpayer") {
    children.push(labelled(cx, "TIN", DigitCells(cx, s.tin, "auth-tin")));
    if (s.forgot) {
      children.push(...adminCells(cx, app, auth));
    } else {
      if (s.method === "totp") children.push(labelled(cx, "Authenticator code", PinCells(cx, app.tpTotpOtp, "auth-tp-totp")));
      else children.push(labelled(cx, "PIN", PinCells(cx, app.tpPinOtp, "auth-tp-pin")));
    }
  } else {
    children.push(...adminCells(cx, app, auth));
  }
  children.push(errorLine(cx, app));

  let left = null;
  if (mode === "taxpayer") {
    const links = div().flex().flex_col().gap(SPACE[1]);
    if (!s.forgot) {
      links.child(
        link(cx, "auth-method", s.method === "totp" ? "Use the PIN instead" : "Use an authenticator code", (cx) => app.toggleSignInMethod(cx)),
      );
    }
    if (doors.admin) {
      links.child(link(cx, "auth-forgot", s.forgot ? "I remember my PIN" : "Forgot PIN? Ask the admin", (cx) => app.toggleForgot(cx)));
    }
    left = links;
  }
  const submit =
    mode === "admin"
      ? AppButton(cx, "auth-admin-submit", "Sign in", { variant: "primary", onClick: (_e, cx) => app.submitAdminSignIn(cx) })
      : s.forgot
        ? AppButton(cx, "auth-override-submit", "Open taxpayer", { variant: "primary", onClick: (_e, cx) => app.submitOverride(cx) })
        : AppButton(cx, "auth-tp-submit", "Sign in", { variant: "primary", onClick: (_e, cx) => app.submitTaxpayerSignIn(cx) });
  children.push(actions(cx, left, submit));
  if (!doors.admin) {
    // No admin PIN exists, so there is no admin view to protect.
    children.push(
      div()
        .flex()
        .justify_center()
        .child(AppButton(cx, "auth-open", "Open without signing in", { variant: "ghost", size: "sm", onClick: (_e, cx) => app.openWithoutSignIn(cx) })),
    );
  }
  return screen(cx, app, children);
}

/** The lock on one taxpayer: only the code cells, never the taxpayer's name or TIN. */
export function renderTaxpayerLock(cx, app, root) {
  const auth = app.store.authState();
  const creds = app.taxpayerCreds(root);
  const g = app.gate;
  const children = [heading(cx, "Taxpayer locked", g.forgot ? "The admin can open this taxpayer for you." : "Enter this taxpayer's PIN to continue.")];
  if (g.forgot) {
    children.push(...adminCells(cx, app, auth));
  } else {
    if (creds.has_pin) children.push(labelled(cx, "PIN", PinCells(cx, app.tpPinOtp, "gate-pin")));
    if (creds.has_totp) children.push(labelled(cx, "Authenticator code", PinCells(cx, app.tpTotpOtp, "gate-totp")));
  }
  children.push(errorLine(cx, app));
  if (auth.has_admin_pin || auth.has_admin_totp) {
    children.push(link(cx, "gate-forgot", g.forgot ? "I remember the PIN" : "Forgot PIN? Ask the admin", (cx) => app.toggleGateForgot(cx)));
  }
  children.push(
    actions(
      cx,
      AppButton(cx, "gate-back", "Back", { variant: "ghost", onClick: (_e, cx) => app.cancelUnlock(cx) }),
      AppButton(cx, "gate-unlock", "Unlock", { variant: "primary", onClick: (_e, cx) => (g.forgot ? app.submitOverride(cx) : app.tryUnlockProfile(cx)) }),
    ),
  );
  return screen(cx, app, children);
}
