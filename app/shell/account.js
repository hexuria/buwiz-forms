// Settings › Account: the Buwiz cloud account. Separate from the PIN that
// unlocks this computer. Filing keeps working when the server is unreachable.

import { div } from "gpui-kit";
import { InputState } from "gpui-base";
import { SPACE, AppButton, Card, SettingRow, TextField } from "./ui.js";
import { errorText } from "./store.js";

export class AccountActions {
  constructor(app) {
    this.app = app;
    this.busy = false;
    this.error = null;
    this.freshStarted = false;
    try {
      this.status = app.store.call("cloud_account_status");
    } catch (error) {
      this.status = {
        server_url: "",
        signed_in: false,
        email: null,
        user_id: null,
        offline: false,
        detail: errorText(error),
      };
      this.error = errorText(error);
    }
    this.url = InputState.new({ value: this.status.server_url || "" });
  }

  /** Refresh a saved sign-in once, when Settings opens. */
  ensureFresh(cx) {
    if (this.freshStarted || !this.status.signed_in || this.busy) return;
    this.freshStarted = true;
    this.refresh(cx);
  }

  refresh(cx) {
    // The caller's cx ends when the caller returns. The host call waits on
    // the network, so the update runs in a task that outlives that call.
    cx.spawn(async (acx) => {
      try {
        this.status = await this.app.store.call("cloud_account_refresh");
        this.error = null;
      } catch (error) {
        this.error = errorText(error);
      }
      acx.notify();
    });
  }

  saveUrl(cx) {
    this.error = null;
    try {
      this.status = this.app.store.call("cloud_account_set_server_url", this.url.value());
      this.url.set_value(this.status.server_url || "");
    } catch (error) {
      this.error = errorText(error);
    }
    cx.notify();
  }

  signIn(cx) {
    if (this.busy) return;
    this.busy = true;
    this.error = null;
    this.status = { ...this.status, detail: "Opening your browser to sign in…" };
    cx.notify();
    cx.spawn(async (acx) => {
      try {
        const started = await this.app.store.call("cloud_account_begin_sign_in");
        const code = started.user_code ? ` Code ${started.user_code}.` : "";
        this.status = {
          ...this.status,
          detail: started.browser_opened
            ? `Finish signing in in the browser, then return here.${code}`
            : `Open ${started.verification_uri_complete} and approve this computer.${code}`,
        };
        acx.notify();
        this.status = await this.app.store.call("cloud_account_finish_sign_in");
        this.error = null;
        if (this.app.finishUnlock) this.app.finishUnlock(acx);
      } catch (error) {
        this.error = errorText(error);
        try {
          this.status = this.app.store.call("cloud_account_status");
        } catch (_) {
          /* keep the previous status */
        }
      }
      this.busy = false;
      acx.notify();
    });
  }

  signOut(cx) {
    if (this.busy) return;
    this.busy = true;
    this.error = null;
    cx.notify();
    cx.spawn(async (acx) => {
      try {
        this.status = await this.app.store.call("cloud_account_sign_out");
        this.error = null;
        if (this.app.lockCloud) this.app.lockCloud(acx);
      } catch (error) {
        this.error = errorText(error);
      }
      this.busy = false;
      acx.notify();
    });
  }
}

export function AccountCard(cx, app) {
  const account = app.account;
  account.ensureFresh(cx);
  const status = account.status || {};
  const who = status.signed_in ? status.email || status.user_id || "Signed in" : "Not signed in";
  const hint = account.busy ? status.detail || "Waiting…" : account.error || status.detail || "";
  const button = status.signed_in
    ? AppButton(cx, "settings-account-sign-out", account.busy ? "Signing out…" : "Sign out", {
        size: "sm",
        variant: "ghost",
        onClick: (_e, cx) => account.signOut(cx),
      })
    : AppButton(cx, "settings-account-sign-in", account.busy ? "Waiting…" : "Sign in", {
        size: "sm",
        variant: "primary",
        onClick: (_e, cx) => account.signIn(cx),
      });
  const server = div()
    .flex()
    .flex_row()
    .items_center()
    .gap(SPACE[2])
    .child(TextField(cx, account.url, { id: "settings-account-url", width: 280, label: "Buwiz server" }))
    .child(
      AppButton(cx, "settings-account-url-save", "Save", {
        size: "sm",
        onClick: (_e, cx) => account.saveUrl(cx),
      }),
    );
  return Card(cx, "settings-account", [
    SettingRow(
      cx,
      "settings-account-server",
      "Server",
      "Production API unless you change it. An environment value is used until you save one here.",
      server,
    ),
    SettingRow(cx, "settings-account-who", who, hint, button),
  ]);
}
