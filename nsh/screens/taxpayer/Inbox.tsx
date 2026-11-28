// Confirmation inbox (old inboxBody, P2-5): the mailbox this branch's BIR receipts are tracked through. The method, the
// app password's credentials (masked; the host answers only whether one is stored), Google connect and disconnect,
// Verify, Check now and the tracking switch, all through bir-core's fetch and confirm path.
// Google connect needs a desktop and a build with a Google client id (the host's google_available). On a phone or
// tablet its sign-in waits for Google on a local web address (bir-core email/auth_oauth.rs), which iOS cannot answer once
// the browser is in front; a build without a client id could only fail. Either way the inbox is an app password, with one
// line saying why. An inbox already on Google can still be switched to an app password or disconnected.
import { View } from "nsh";
import type { InboxAuthMethod } from "../../bir";
import { shortDate } from "../../lib/dates";
import { app, store } from "../../shell";
import { AppButton, Field, FieldCell, FieldGrid, Segmented, Switch, Text, TextField, TOUCH } from "../../ui";
import { checkInboxNow, connectInbox, disconnectInbox, setTracking, shared, verifyInbox } from "./actions";
import { GOOGLE_UNAVAILABLE, GOOGLE_UNAVAILABLE_CONNECTED, trackingLine } from "./logic";
import type { PageState } from "./page";

export function InboxBody(props: { s: PageState }) {
  const s = props.s;
  const tin = () => app.branch()?.tin ?? "";
  const st = () => store.inboxState(app.branch()?.tin);
  const ed = () => s.inbox();
  // Google can be connected on a desktop build that has a client id; elsewhere the method is an app password unless
  // the inbox is already on Google.
  const canConnect = () => !TOUCH && Boolean(st()?.google_available);
  const choosable = () => canConnect() || st()?.auth_method === "GoogleOAuth";
  const method = (): InboxAuthMethod => (choosable() ? (ed()?.method ?? st()?.auth_method ?? "AppPassword") : "AppPassword");
  const pw = () => method() === "AppPassword";
  return (
    <View testID="tp-inbox" class="w-full">
      <FieldGrid>
        <If when={choosable()}>
          <FieldCell full>
            <Field label="Authentication method">
              <Segmented
                testID="tp-inbox-method"
                label="Inbox authentication method"
                wrap
                options={[
                  { value: "GoogleOAuth", label: "Google account (OAuth2)" },
                  { value: "AppPassword", label: "App password" },
                ]}
                value={method()}
                onChange={(v) => s.pickInboxMethod(v as InboxAuthMethod)}
              />
            </Field>
          </FieldCell>
        </If>
        <If
          when={pw()}
          else={
            <FieldCell full>
              <Field
                label="Google account"
                value={st()?.oauth_connected ? `Connected as ${st()?.oauth_inbox_email || st()?.inbox_email}` : "Not connected"}
              />
            </FieldCell>
          }
        >
          <FieldCell>
            <If when={ed()} else={<Field label="Inbox email" value={st()?.imap_login || st()?.inbox_email || "Not set"} />}>
              <Field label="Inbox email">
                <TextField testID="tp-inbox-email" accessibilityLabel="Inbox email" value={ed()?.email ?? ""} onChange={(v) => s.setInbox("email", v)} />
              </Field>
            </If>
          </FieldCell>
          <FieldCell>
            <If when={ed()} else={<Field label="IMAP host" value={st()?.imap_host} />}>
              <Field label="IMAP host">
                <TextField testID="tp-inbox-host" accessibilityLabel="IMAP host" value={ed()?.host ?? ""} onChange={(v) => s.setInbox("host", v)} />
              </Field>
            </If>
          </FieldCell>
          <FieldCell full>
            <If when={ed()} else={<Field label="App password" value={st()?.has_app_password ? "Stored" : "Not set"} />}>
              <Field label="App password" error={ed()?.error ?? undefined}>
                <TextField
                  testID="tp-inbox-password"
                  accessibilityLabel="App password"
                  masked
                  placeholder={st()?.has_app_password ? "Leave empty to keep the stored password" : "16-letter app password"}
                  value={ed()?.password ?? ""}
                  onChange={(v) => s.setInbox("password", v)}
                  error={Boolean(ed()?.error)}
                />
              </Field>
            </If>
          </FieldCell>
        </If>
        <If when={st()?.last_check_at}>
          <FieldCell>
            <Field label="Last check" value={`${shortDate(st()?.last_check_at)}${st()?.last_check_status ? ` — ${st()?.last_check_status}` : ""}`} />
          </FieldCell>
        </If>
      </FieldGrid>
      <If when={!pw()}>
        <Text variant="meta" tone="fg3" class="mt-2">
          Profiles that share this inbox share one Google OAuth credential set. Re-authorize or Disconnect updates every profile using this email.
        </Text>
      </If>
      <If when={pw() && ed()}>
        <Text variant="meta" tone="fg3" class="mt-2">
          An app password needs 2-Step Verification on the mail account first (Google / Outlook / Yahoo security settings).
        </Text>
      </If>
      <View class="mt-3 flex-row flex-wrap items-center gap-2">
        <If when={ed()}>
          <AppButton testID="tp-inbox-cancel" label="Cancel" variant="ghost" size="sm" onPress={() => s.inbox.set(null)} />
          <If when={pw()}>
            <AppButton testID="tp-inbox-save" label="Save" variant="primary" size="sm" icon="save" onPress={s.saveInbox} />
          </If>
        </If>
        <If when={!ed() && pw()}>
          <AppButton testID="tp-inbox-edit" label="Edit credentials" variant="ghost" size="sm" icon="pencil" onPress={() => s.startInboxEdit()} />
        </If>
        <If when={!pw()}>
          <If
            when={st()?.oauth_connected}
            else={
              <If when={canConnect()}>
                <AppButton
                  testID="tp-inbox-connect"
                  label="Connect Google"
                  variant="primary"
                  size="sm"
                  icon="key-round"
                  disabled={shared.inboxBusy()}
                  onPress={() => void connectInbox(tin())}
                />
              </If>
            }
          >
            <If when={canConnect()}>
              <AppButton testID="tp-inbox-reconnect" label="Re-authorize" size="sm" icon="key-round" disabled={shared.inboxBusy()} onPress={() => void connectInbox(tin())} />
            </If>
            <AppButton testID="tp-inbox-disconnect" label="Disconnect" variant="ghost" size="sm" onPress={() => disconnectInbox(tin())} />
          </If>
        </If>
        <AppButton testID="tp-inbox-verify" label="Verify connection" size="sm" disabled={shared.inboxBusy()} onPress={() => void verifyInbox(tin())} />
        <AppButton testID="tp-inbox-check" label="Check now" variant="ghost" size="sm" icon="search" disabled={shared.inboxBusy()} onPress={() => void checkInboxNow(tin())} />
      </View>
      <View class="mt-3 flex-row items-center gap-3">
        <Switch testID="tp-inbox-tracking" checked={Boolean(st()?.tracking_enabled)} onChange={(on) => setTracking(tin(), on)} label="Receipt tracking" />
        <Text variant="control">Receipt tracking</Text>
      </View>
      <If when={shared.inboxMsg()}>
        <Text testID="tp-inbox-msg" tone={shared.inboxMsg()?.ok ? "success" : "danger"} class="mt-2">{shared.inboxMsg()?.text ?? ""}</Text>
      </If>
      <If when={TOUCH}>
        <Text testID="tp-inbox-device-note" variant="meta" tone="fg3" class="mt-2">
          On this device the inbox is checked only while eBIRForms is open, with an app password.
        </Text>
      </If>
      <If when={!TOUCH && st() && !st()?.google_available}>
        <Text testID="tp-inbox-google-unavailable" variant="meta" tone="fg3" class="mt-2">
          {st()?.oauth_connected ? GOOGLE_UNAVAILABLE_CONNECTED : GOOGLE_UNAVAILABLE}
        </Text>
      </If>
      <Text testID="tp-inbox-status" variant="meta" tone={st()?.tracking_enabled ? "success" : "fg3"} class="mt-2">
        {trackingLine({ tracking_enabled: Boolean(st()?.tracking_enabled), still_pending: Boolean(st()?.still_pending) })}
      </Text>
    </View>
  );
}
