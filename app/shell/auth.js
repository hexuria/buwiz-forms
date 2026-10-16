// First launch and sign-in. Both take the whole window: nothing about any
// taxpayer (sidebar, panel, title-bar pill, search) is drawn behind them.
//
// First launch asks who uses this computer. "Just me or my business" makes
// the owner's own taxpayer the admin account; "I help other taxpayers" (an
// RDO desk, an accountant) makes the reserved TIN 000-000-000 the admin.
//
// Sign-in has one door: a TIN and that account's PIN or authenticator code.
// The admin's TIN signs in as the admin. A taxpayer who forgot their PIN is
// let in with the admin's code; an admin who forgot theirs uses the one-time
// recovery key or, last, the Mac login. A PIN locks one account, never the
// app, so nobody can lock everyone out.

import { div } from "gpui-kit";
import { tokens, SPACE, RADIUS, Text, Icon, AppButton, PinCells, DigitCells, TextField, Code } from "./ui.js";
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

function heading(cx, title, line, icon = "lock") {
  return div()
    .flex()
    .flex_col()
    .items_center()
    .gap(SPACE[2])
    .child(div().flex().items_center().justify_center().size(44).rounded(RADIUS.full).bg(tokens(cx).hover).child(Icon(cx, icon, { color: "fg2" })))
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
function adminCells(cx, app) {
  const auth = app.authState();
  const rows = [];
  if (auth.has_admin_pin) rows.push(labelled(cx, "Admin PIN", PinCells(cx, app.adminPinOtp, "auth-admin-pin")));
  if (auth.has_admin_totp) rows.push(labelled(cx, "Admin authenticator code", PinCells(cx, app.adminTotpOtp, "auth-admin-totp")));
  return rows;
}

/** The sign-in screen: at launch when the admin has a code, and after Log out or Lock app. */
export function renderSignIn(cx, app) {
  const auth = app.authState();
  const s = app.signIn;
  const typed = s.tin.value();
  const forAdmin = typed.length === 9 && typed === app.adminRoot();
  const children = [];
  let line = "Enter your TIN and your PIN.";
  if (s.forgot) line = forAdmin ? "Use your recovery key, or your Mac login." : "The admin can open your account with their PIN.";
  else if (app.isHelpdesk()) line = "Enter your TIN and your PIN. The help desk admin signs in with TIN 000-000-000.";
  children.push(heading(cx, "Sign in", line));
  children.push(labelled(cx, "TIN", DigitCells(cx, s.tin, "auth-tin")));

  if (!s.forgot) {
    if (s.method === "totp") children.push(labelled(cx, "Authenticator code", PinCells(cx, app.tpTotpOtp, "auth-tp-totp")));
    else children.push(labelled(cx, "PIN", PinCells(cx, app.tpPinOtp, "auth-tp-pin")));
  } else if (forAdmin) {
    children.push(labelled(cx, "Recovery key", TextField(cx, app.recoveryInput)));
  } else {
    children.push(...adminCells(cx, app));
  }
  children.push(errorLine(cx, app));

  const links = div().flex().flex_col().gap(SPACE[1]);
  if (!s.forgot) {
    links.child(link(cx, "auth-method", s.method === "totp" ? "Use the PIN instead" : "Use an authenticator code", (cx) => app.toggleSignInMethod(cx)));
  }
  links.child(link(cx, "auth-forgot", s.forgot ? "I remember my PIN" : "Forgot PIN?", (cx) => app.toggleForgot(cx)));

  let submit;
  if (!s.forgot) submit = AppButton(cx, "auth-submit", "Sign in", { variant: "primary", onClick: (_e, cx) => app.submitSignIn(cx) });
  else if (forAdmin) submit = AppButton(cx, "auth-recovery-submit", "Use key", { variant: "primary", onClick: (_e, cx) => app.submitRecoveryKey(cx) });
  else submit = AppButton(cx, "auth-override-submit", "Open account", { variant: "primary", onClick: (_e, cx) => app.submitOverride(cx) });
  children.push(actions(cx, links, submit));

  if (s.forgot && forAdmin && auth.mac_login_available) {
    children.push(
      div()
        .flex()
        .justify_center()
        .child(
          AppButton(cx, "auth-mac-login", s.busy ? "Waiting for your Mac login…" : "Use my Mac login instead", {
            variant: "ghost",
            size: "sm",
            icon: "laptop",
            onClick: (_e, cx) => app.submitMacLogin(cx),
          }),
        ),
    );
  }
  if (!app.adminHasCode()) {
    // The admin has no code: the owner is at the keyboard, nothing to protect.
    children.push(
      div()
        .flex()
        .justify_center()
        .child(AppButton(cx, "auth-open", "Open without signing in", { variant: "ghost", size: "sm", onClick: (_e, cx) => app.openWithoutSignIn(cx) })),
    );
  }
  return screen(cx, app, children);
}

function choice(cx, id, icon, title, line, onClick) {
  const t = tokens(cx);
  return div()
    .id(id)
    .flex()
    .flex_row()
    .items_start()
    .gap(SPACE[3])
    .p(SPACE[4])
    .rounded(RADIUS.card)
    .border(1)
    .border_color(t.border)
    .cursor_pointer()
    .hover((st) => st.bg(t.hover))
    .accessibility_label(title)
    .on_click((_e, cx) => onClick(cx))
    .child(Icon(cx, icon, { size: 20, color: "accent" }))
    .child(
      div()
        .flex()
        .flex_col()
        .flex_1()
        .min_w(0)
        .gap(SPACE[1])
        .child(Text(cx, "control", title))
        .child(Text(cx, "meta", line, "fg2").whitespace_normal()),
    );
}

/** First launch: who uses this computer decides who the admin is. */
export function renderSetup(cx, app) {
  const st = app.setup;
  if (st.step === "helpdesk-pin") {
    return screen(cx, app, [
      heading(cx, "Choose the admin PIN", "The help desk admin signs in with TIN 000-000-000 and this PIN. It also opens any taxpayer who forgot theirs.", "key-round"),
      labelled(cx, "Admin PIN", PinCells(cx, st.pinOtp, "setup-admin-pin")),
      errorLine(cx, app),
      actions(
        cx,
        AppButton(cx, "setup-back", "Back", { variant: "ghost", onClick: (_e, cx) => app.cancelSetup(cx) }),
        AppButton(cx, "setup-pin-submit", "Continue", { variant: "primary", onClick: (_e, cx) => app.submitHelpdeskPin(cx) }),
      ),
    ]);
  }
  if (st.step === "helpdesk-key") {
    return screen(cx, app, [
      heading(cx, "Write down your recovery key", "If you forget the admin PIN, this key signs you in once. It is shown only now. Keep it away from this computer.", "key-round"),
      div().flex().justify_center().py(SPACE[2]).child(Code(cx, st.key || "")),
      Text(cx, "meta", "Lost both? Whoever can log in to this Mac can still reset the admin PIN.", "fg2").text_center().whitespace_normal(),
      actions(cx, null, AppButton(cx, "setup-key-done", "I wrote it down", { variant: "primary", onClick: (_e, cx) => app.finishHelpdeskSetup(cx) })),
    ]);
  }
  return screen(cx, app, [
    heading(cx, "Who will use eBIRForms on this computer?", "This decides who the admin is. Nobody can lock the others out.", "users"),
    choice(cx, "setup-solo", "user", "Just me or my business", "You are the admin. Your own tax profile is the admin account, with all its branches.", (cx) => app.chooseSolo(cx)),
    choice(
      cx,
      "setup-helpdesk",
      "users",
      "I help other taxpayers",
      "An RDO help desk, an accountant or a bookkeeper. You get a separate admin account, and each taxpayer signs in as themselves.",
      (cx) => app.chooseHelpdesk(cx),
    ),
  ]);
}
