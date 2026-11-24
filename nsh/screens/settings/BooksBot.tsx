// Settings > Account > Buwiz Books: one-click local pairing with Buwiz Bot's read-only /v1/ext/read door,
// with a manual loopback URL / grant token fallback.
import type { JSX } from "nsh";
import { signal, View } from "nsh";
import { bir, type BooksBrokerStatus } from "../../bir";
import { AppButton, Card, Line, SettingRow, Text, TextField } from "../../ui";

const EMPTY_STATUS: BooksBrokerStatus = {
  connected: false,
  broker_url: "http://127.0.0.1:8787",
  grant_id: null,
  host_detected: false,
};

function readStatus(): BooksBrokerStatus {
  try {
    return bir.books_broker_status();
  } catch {
    return EMPTY_STATUS;
  }
}

export function BooksBotCard(): JSX.Element {
  const status = signal<BooksBrokerStatus>(readStatus());
  const busy = signal(false);
  const detail = signal("");
  const failure = signal("");
  const showManual = signal(false);
  const manualUrl = signal(status().broker_url || "http://127.0.0.1:8787");
  const manualToken = signal("");

  const refresh = () => {
    const next = readStatus();
    status.set(next);
    if (next.broker_url) manualUrl.set(next.broker_url);
  };

  const connectOneClick = () => {
    busy.set(true);
    failure.set("");
    detail.set("");
    try {
      const url = manualUrl().trim() || undefined;
      const res = bir.books_broker_connect(url);
      refresh();
      if (res.book_name) {
        detail.set(`Linked to ${res.book_name} (${res.account_count} accounts, read-only).`);
      } else if (res.note) {
        detail.set(`Paired with Buwiz Bot (${res.note})`);
      } else {
        detail.set("Connected to Buwiz Bot (read-only).");
      }
      showManual.set(false);
    } catch (e) {
      failure.set(e instanceof Error ? e.message : String(e));
    } finally {
      busy.set(false);
    }
  };

  const disconnect = () => {
    busy.set(true);
    failure.set("");
    detail.set("");
    try {
      bir.books_broker_disconnect();
      refresh();
      detail.set("Disconnected from Buwiz Bot.");
    } catch (e) {
      failure.set(e instanceof Error ? e.message : String(e));
    } finally {
      busy.set(false);
    }
  };

  const saveManual = () => {
    const url = manualUrl().trim();
    const tok = manualToken().trim();
    if (!tok) {
      failure.set("Enter a read-only grant token from Buwiz Bot.");
      return;
    }
    busy.set(true);
    failure.set("");
    detail.set("");
    try {
      if (url) bir.set_setting("books_broker_url", url);
      bir.set_setting("books_ext_read_token", tok);
      manualToken.set("");
      const coa = bir.books_coa_list();
      refresh();
      detail.set(`Linked to ${coa.book_name} (${coa.accounts.length} accounts, read-only).`);
      showManual.set(false);
    } catch (e) {
      failure.set(e instanceof Error ? e.message : String(e));
      refresh();
    } finally {
      busy.set(false);
    }
  };

  const statusTitle = () => (status().connected ? "Connected to Buwiz Bot" : "Not connected");
  const statusHint = () => {
    if (failure()) return failure();
    if (detail()) return detail();
    if (status().connected) return `${status().broker_url} · Read-only Books COA access`;
    return status().host_detected
      ? "Buwiz Bot detected on this computer. Pair to read your Books chart of accounts (read-only, local only)."
      : "Open Buwiz Bot on this computer to pair in one click, or enter a manual grant token.";
  };

  return (
    <Card testID="settings-books">
      <SettingRow testID="settings-books-status" title={statusTitle()} hint={statusHint()}>
        <If
          when={status().connected}
          else={
            <AppButton
              testID="settings-books-connect"
              label={busy() ? "Connecting…" : "Connect Buwiz Bot"}
              variant="primary"
              size="sm"
              disabled={busy()}
              onPress={connectOneClick}
            />
          }
        >
          <AppButton
            testID="settings-books-disconnect"
            label={busy() ? "Disconnecting…" : "Disconnect"}
            variant="ghost"
            size="sm"
            disabled={busy()}
            onPress={disconnect}
          />
        </If>
      </SettingRow>
      <SettingRow testID="settings-books-manual-row" title="Local broker" hint="Loopback only (default http://127.0.0.1:8787)">
        <View class="flex-row items-center gap-2">
          <Line testID="settings-books-url-value" variant="meta" tone="fg2" class="max-w-44">
            {status().broker_url}
          </Line>
          <AppButton
            testID="settings-books-manual-toggle"
            label={showManual() ? "Hide" : "Manual"}
            variant="ghost"
            size="sm"
            onPress={() => showManual.set(!showManual())}
          />
        </View>
      </SettingRow>
      <If when={showManual()}>
        <View testID="settings-books-manual" class="gap-2.5 border-t border-border px-4 py-3">
          <Text variant="meta" tone="fg2">
            Use a custom loopback address or paste a read-only grant token for app.buwiz.books.
          </Text>
          <TextField
            testID="settings-books-url-input"
            label="Broker URL"
            value={manualUrl()}
            onChange={(v) => manualUrl.set(v)}
          />
          <TextField
            testID="settings-books-token-input"
            label="Read-only grant token"
            value={manualToken()}
            onChange={(v) => manualToken.set(v)}
          />
          <View class="flex-row justify-end gap-2">
            <AppButton
              testID="settings-books-manual-save"
              label="Save token"
              variant="secondary"
              size="sm"
              disabled={busy()}
              onPress={saveManual}
            />
          </View>
        </View>
      </If>
    </Card>
  );
}
