// Settings › Security: the signed-in account's own PIN and authenticator,
// plus the admin's recovery key. One place for every lock; the Tax profile
// page has none.
//
// Whose codes these are follows the session: a taxpayer edits its own (one
// PIN and one authenticator per taxpayer, shared by every branch); the admin
// edits the admin codes, which in "Just me" mode are the owner taxpayer's own
// and on a help desk belong to the reserved admin account. The host refuses a
// change that would leave a help desk's admin with no way in.

import { div } from "gpui-kit";
import { OtpState } from "gpui-base";
import { SPACE, AppButton, Card, SettingRow, PinCells, Code } from "./ui.js";
import { errorText as message } from "./store.js";

export class SecurityActions {
  constructor(app) {
    this.app = app;
    this.pinOtp = OtpState.new(4, { masked: true });
    this.totpOtp = OtpState.new(6);
    this.totpSecret = null;
    /** A recovery key just made: shown until Settings is left, never again. */
    this.recoveryKey = null;
  }

  /** The account whose codes Settings edits: the admin, or the signed-in taxpayer. */
  target() {
    const app = this.app;
    if (app.isAdmin()) {
      const a = app.authState();
      return { admin: true, has_pin: Boolean(a.has_admin_pin), has_totp: Boolean(a.has_admin_totp) };
    }
    const root = app.session.root;
    return { admin: false, root, ...app.taxpayerCreds(root) };
  }

  /** Forget anything half-typed and the recovery key on screen. */
  leave() {
    this.pinOtp.set_value("");
    this.totpOtp.set_value("");
    this.totpSecret = null;
    this.recoveryKey = null;
  }

  /**
   * The admin's first code was just set. Nobody signed in until now (no code
   * existed), so the person at the keyboard becomes the signed-in admin, and
   * gets a recovery key if there is none yet.
   */
  afterAdminCode() {
    const app = this.app;
    if (app.session.kind === "open") app.session = { kind: "admin" };
    if (!app.authState().has_recovery_key) this.makeRecoveryKey();
  }

  makeRecoveryKey() {
    this.recoveryKey = this.app.store.write("create_recovery_key").recovery_key;
  }

  setPin(cx) {
    const app = this.app;
    const pin = this.pinOtp.value();
    if (pin.length !== 4) return app.toast("Type all 4 digits of the new PIN first.", cx, "lock");
    const t = this.target();
    try {
      if (t.admin) app.store.write("set_admin_pin", pin);
      else app.store.write("set_taxpayer_pin", t.root, pin);
      this.pinOtp.set_value("");
      if (t.admin) this.afterAdminCode();
      app.toast(t.has_pin ? "PIN changed." : "PIN saved. Sign-in asks for it from now on.", cx, "lock");
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  clearPin(cx) {
    const app = this.app;
    const t = this.target();
    try {
      if (t.admin) app.store.write("set_admin_pin", null);
      else app.store.write("set_taxpayer_pin", t.root, null);
      app.toast("PIN removed.", cx, "lock");
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  startTotp(cx) {
    const app = this.app;
    const t = this.target();
    const who = t.admin ? (app.isHelpdesk() ? "admin" : (app.taxpayer() || {}).tin_display || "admin") : (app.taxpayer() || {}).tin_display || t.root;
    try {
      this.totpSecret = app.store.call("generate_totp_secret", `eBIRForms ${who}`).secret;
      this.totpOtp.set_value("");
      cx.notify();
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  confirmTotp(cx) {
    const app = this.app;
    const t = this.target();
    try {
      if (t.admin) app.store.write("set_admin_totp", this.totpSecret, this.totpOtp.value());
      else app.store.write("set_taxpayer_totp", t.root, this.totpSecret, this.totpOtp.value());
      this.totpSecret = null;
      this.totpOtp.set_value("");
      if (t.admin) this.afterAdminCode();
      app.toast("Authenticator added.", cx, "lock");
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  cancelTotp(cx) {
    this.totpSecret = null;
    this.totpOtp.set_value("");
    cx.notify();
  }

  clearTotp(cx) {
    const app = this.app;
    const t = this.target();
    try {
      if (t.admin) app.store.write("set_admin_totp", null);
      else app.store.write("set_taxpayer_totp", t.root, null);
      app.toast("Authenticator removed.", cx, "lock");
    } catch (e) {
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  newRecoveryKey(cx) {
    try {
      this.makeRecoveryKey();
      cx.notify();
    } catch (e) {
      this.app.toast(message(e), cx, "triangle-alert");
    }
  }
}

function controls(...children) {
  return div().flex().flex_row().flex_wrap().items_center().gap(SPACE[2]).children(children.filter(Boolean));
}

/** The Security card: PIN, authenticator and, for the admin, the recovery key. */
export function SecurityCard(cx, app) {
  const s = app.security;
  const t = s.target();
  const helpdeskAdmin = t.admin && app.isHelpdesk();
  const pinLabel = helpdeskAdmin ? "Admin PIN" : "My PIN";
  const totpLabel = helpdeskAdmin ? "Admin authenticator" : "My authenticator";
  const rows = [];

  rows.push(
    SettingRow(
      cx,
      "settings-pin",
      pinLabel,
      t.has_pin
        ? helpdeskAdmin
          ? "Signs in TIN 000-000-000; opens locked-out taxpayers"
          : t.admin
            ? "Signs you in; opens locked-out taxpayers"
            : "Asked when you sign in with your TIN"
        : "Not set. Anyone at this computer can open this account",
      controls(
        PinCells(cx, s.pinOtp, "settings-pin-cells"),
        AppButton(cx, "settings-pin-save", t.has_pin ? "Change" : "Set PIN", { size: "sm", onClick: (_e, cx) => s.setPin(cx) }),
        t.has_pin ? AppButton(cx, "settings-pin-clear", "Remove", { variant: "ghost", size: "sm", onClick: (_e, cx) => s.clearPin(cx) }) : null,
      ),
    ),
  );

  let totpControl;
  if (s.totpSecret) {
    totpControl = controls(
      PinCells(cx, s.totpOtp, "settings-totp-cells"),
      AppButton(cx, "settings-totp-confirm", "Confirm", { variant: "primary", size: "sm", onClick: (_e, cx) => s.confirmTotp(cx) }),
      AppButton(cx, "settings-totp-cancel", "Cancel", { variant: "ghost", size: "sm", onClick: (_e, cx) => s.cancelTotp(cx) }),
    );
  } else {
    totpControl = AppButton(cx, "settings-totp-btn", t.has_totp ? "Remove" : "Add", {
      variant: t.has_totp ? "ghost" : "secondary",
      size: "sm",
      onClick: (_e, cx) => (t.has_totp ? s.clearTotp(cx) : s.startTotp(cx)),
    });
  }
  rows.push(
    SettingRow(
      cx,
      "settings-totp",
      totpLabel,
      s.totpSecret
        ? `Add this key to your authenticator app, then type the code it shows: ${s.totpSecret}`
        : t.has_totp
          ? "A code from your authenticator app also signs you in"
          : "Use a 6-digit code from an authenticator app instead of the PIN",
      totpControl,
    ),
  );

  if (t.admin) {
    const a = app.authState();
    const hasCode = t.has_pin || t.has_totp;
    let hint;
    if (s.recoveryKey) hint = "Write it down now. It is shown once and works once.";
    else if (!hasCode) hint = "Made when you set a PIN or authenticator";
    else if (a.has_recovery_key) hint = "Signs you in if you forget the PIN. Making a new one cancels the old one";
    else hint = "None yet. Without one, only your Mac login can reset a forgotten PIN";
    const control = s.recoveryKey
      ? Code(cx, s.recoveryKey)
      : hasCode
        ? AppButton(cx, "settings-recovery-btn", a.has_recovery_key ? "Make a new key" : "Make a key", { variant: "secondary", size: "sm", onClick: (_e, cx) => s.newRecoveryKey(cx) })
        : null;
    rows.push(SettingRow(cx, "settings-recovery", "Recovery key", hint, control));
  }
  return Card(cx, "settings-security", rows);
}
