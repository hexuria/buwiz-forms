// Settings › Computer setup (admin only): change who this computer is set up
// for after first launch.
//
// - "Just me or my business": hand the owner (the admin account) to another
//   taxpayer, or switch to a help desk with a new admin PIN.
// - A help desk: make one of the taxpayers the owner.
//
// The host re-checks the current admin's PIN or authenticator code and
// refuses a change that would leave nobody able to sign in. Every change
// makes a new recovery key, shown once, and then signs out so the new admin
// signs in with their own code.

import { div } from "gpui-kit";
import { OtpState } from "gpui-base";
import { SPACE, AppButton, Card, SettingRow, PinCells, Code } from "./ui.js";

function message(e) {
  return String(e && e.message ? e.message : e)
    .replace(/^Error:\s*/, "")
    .replace(/^`[^`]+`:\s*/, "");
}

function controls(...children) {
  return div().flex().flex_row().flex_wrap().items_center().gap(SPACE[2]).children(children.filter(Boolean));
}

export class SetupChange {
  constructor(app) {
    this.app = app;
    this.newPinOtp = OtpState.new(4, { masked: true });
    this.confirmPinOtp = OtpState.new(4, { masked: true });
    this.confirmCodeOtp = OtpState.new(6);
    this.reset();
  }

  reset() {
    /** null (closed) | "choose" | "owner" | "helpdesk" | "confirm" | "key" */
    this.step = null;
    /** { kind: "owner", root, name, tin_display } | { kind: "helpdesk" } */
    this.target = null;
    this.useCode = false;
    this.recoveryKey = null;
    for (const otp of [this.newPinOtp, this.confirmPinOtp, this.confirmCodeOtp]) otp.set_value("");
  }

  /**
   * Leaving Settings. A finished change has already moved the admin, so the
   * old session must not carry on: sign out even if "I wrote it down" was
   * never pressed.
   */
  leave(cx) {
    const changed = this.step === "key";
    this.reset();
    if (changed && cx) this.app.signOutAfterSetupChange(cx);
  }

  go(step, cx) {
    this.step = step;
    cx.notify();
  }

  /** The current owner's name and TIN, or null on a help desk. */
  owner() {
    const app = this.app;
    if (app.isHelpdesk()) return null;
    const root = app.adminRoot();
    return app.taxpayers().find((t) => t.tin_root === root) || { tin_root: root, name: "", tin_display: root };
  }

  /** Taxpayers that could become the owner, with whether they can sign in yet. */
  candidates() {
    const app = this.app;
    const current = app.isHelpdesk() ? null : app.adminRoot();
    return app
      .taxpayers()
      .filter((t) => t.tin_root !== current && !t.archived)
      .map((t) => {
        const c = app.taxpayerCreds(t.tin_root);
        return { ...t, ready: Boolean(c.has_pin || c.has_totp) };
      });
  }

  pickOwner(t, cx) {
    this.target = { kind: "owner", root: t.tin_root, name: t.name, tin_display: t.tin_display };
    this.go("confirm", cx);
  }

  pickHelpdeskPin(cx) {
    if (this.newPinOtp.value().length !== 4) return this.app.toast("Type all 4 digits of the new admin PIN.", cx, "lock");
    this.target = { kind: "helpdesk" };
    this.go("confirm", cx);
  }

  toggleCode(cx) {
    this.useCode = !this.useCode;
    this.confirmPinOtp.set_value("");
    this.confirmCodeOtp.set_value("");
    cx.notify();
  }

  confirm(cx) {
    const app = this.app;
    const pin = this.useCode ? null : this.confirmPinOtp.value();
    const code = this.useCode ? this.confirmCodeOtp.value() : null;
    if (!this.useCode && pin.length !== 4) return app.toast("Type all 4 digits of your current admin PIN.", cx, "lock");
    if (this.useCode && code.length !== 6) return app.toast("Type all 6 digits of the code.", cx, "lock");
    try {
      const result =
        this.target.kind === "owner"
          ? app.store.write("change_owner", this.target.root, pin, code)
          : app.store.write("change_to_helpdesk", pin, code, this.newPinOtp.value());
      this.recoveryKey = result.recovery_key;
      this.newPinOtp.set_value("");
      this.confirmPinOtp.set_value("");
      this.confirmCodeOtp.set_value("");
      this.go("key", cx);
    } catch (e) {
      this.confirmPinOtp.set_value("");
      this.confirmCodeOtp.set_value("");
      app.toast(message(e), cx, "triangle-alert");
    }
  }

  done(cx) {
    this.reset();
    this.app.signOutAfterSetupChange(cx);
  }
}

function summary(app, s) {
  if (app.isHelpdesk()) return "A help desk. The admin signs in with TIN 000-000-000";
  const o = s.owner();
  return `Just me or my business. Owner: ${o.name ? `${o.name}, ` : ""}TIN ${o.tin_display}`;
}

/** The Computer setup card. */
export function SetupCard(cx, app) {
  const s = app.setupChange;
  const cancel = AppButton(cx, "setup-change-cancel", "Cancel", { variant: "ghost", size: "sm", onClick: (_e, cx) => { s.reset(); cx.notify(); } });
  const rows = [];

  if (!s.step) {
    rows.push(
      SettingRow(
        cx,
        "setup-change",
        "Set up for",
        summary(app, s),
        AppButton(cx, "setup-change-btn", "Change", { size: "sm", onClick: (_e, cx) => s.go("choose", cx) }),
      ),
    );
  } else if (s.step === "choose") {
    const helpdesk = app.isHelpdesk();
    rows.push(
      SettingRow(
        cx,
        "setup-change-choose",
        "Change to",
        helpdesk
          ? "Make one of the taxpayers the owner. They become the admin and sign in with their own PIN."
          : "Hand the computer to another taxpayer, or use it to help other taxpayers.",
        controls(
          AppButton(cx, "setup-change-owner", helpdesk ? "An owner" : "Another owner", { size: "sm", onClick: (_e, cx) => s.go("owner", cx) }),
          helpdesk ? null : AppButton(cx, "setup-change-helpdesk", "A help desk", { size: "sm", onClick: (_e, cx) => s.go("helpdesk", cx) }),
          cancel,
        ),
      ),
    );
  } else if (s.step === "owner") {
    const list = s.candidates();
    rows.push(SettingRow(cx, "setup-change-owner-head", "New owner", "The owner is the admin of this computer. Only a taxpayer with a PIN or authenticator can be chosen.", cancel));
    if (!list.length) rows.push(SettingRow(cx, "setup-change-none", "No other taxpayers", "Add the taxpayer first, then come back here.", null));
    list.forEach((t, i) =>
      rows.push(
        SettingRow(
          cx,
          `setup-change-cand-${i}`,
          t.name || t.tin_display,
          t.ready ? `TIN ${t.tin_display}` : `TIN ${t.tin_display}. No PIN yet: they set one in Settings after signing in`,
          t.ready ? AppButton(cx, `setup-change-pick-${i}`, "Choose", { size: "sm", onClick: (_e, cx) => s.pickOwner(t, cx) }) : null,
        ),
      ),
    );
  } else if (s.step === "helpdesk") {
    rows.push(
      SettingRow(
        cx,
        "setup-change-helpdesk-head",
        "Switch to a help desk",
        "The admin signs in with TIN 000-000-000. The current owner stays a taxpayer with their own PIN.",
        cancel,
      ),
    );
    rows.push(
      SettingRow(
        cx,
        "setup-change-newpin",
        "New admin PIN",
        "4 digits, for TIN 000-000-000",
        controls(
          PinCells(cx, s.newPinOtp, "setup-change-newpin-cells"),
          AppButton(cx, "setup-change-newpin-next", "Continue", { variant: "primary", size: "sm", onClick: (_e, cx) => s.pickHelpdeskPin(cx) }),
        ),
      ),
    );
  } else if (s.step === "confirm") {
    const what =
      s.target.kind === "owner"
        ? `Make ${s.target.name || "this taxpayer"} (TIN ${s.target.tin_display}) the owner and admin`
        : "Switch to a help desk with the new admin PIN";
    const hasTotp = Boolean(app.authState().has_admin_totp);
    rows.push(SettingRow(cx, "setup-change-what", "Confirm the change", what, cancel));
    rows.push(
      SettingRow(
        cx,
        "setup-change-confirm",
        s.useCode ? "Your admin authenticator code" : "Your current admin PIN",
        "Then a new recovery key, and you are signed out",
        controls(
          s.useCode ? PinCells(cx, s.confirmCodeOtp, "setup-change-code-cells") : PinCells(cx, s.confirmPinOtp, "setup-change-pin-cells"),
          AppButton(cx, "setup-change-confirm-btn", "Confirm change", { variant: "primary", size: "sm", onClick: (_e, cx) => s.confirm(cx) }),
          hasTotp ? AppButton(cx, "setup-change-toggle", s.useCode ? "Use PIN" : "Use code", { variant: "ghost", size: "sm", onClick: (_e, cx) => s.toggleCode(cx) }) : null,
        ),
      ),
    );
  } else if (s.step === "key") {
    rows.push(
      SettingRow(cx, "setup-change-key", "New recovery key", "Write it down now. It is shown once and works once; the old key no longer works.", Code(cx, s.recoveryKey)),
    );
    rows.push(
      SettingRow(
        cx,
        "setup-change-done-row",
        "Setup changed",
        "You are signed out next, so the admin signs in with their own PIN",
        AppButton(cx, "setup-change-done", "I wrote it down", { variant: "primary", size: "sm", onClick: (_e, cx) => s.done(cx) }),
      ),
    );
  }
  return Card(cx, "settings-setup", rows);
}
