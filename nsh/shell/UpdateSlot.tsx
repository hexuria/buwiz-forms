// The update banner (old app/shell/update.js): a card over the bottom of the page when a downloaded release waits (Install now, Later),
// when an update failed, or when a check you asked for found nothing (Later). The frame mounts it over the page; the polls keep
// app.updateStatus() current, and what the buttons do goes through the host functions (the desktop host's updater answers them, see
// shell/desktop). With no update source configured the status is always "idle": nothing shows.
import { effect, signal, untracked, View } from "nsh";
import { bir } from "../bir";
import { AppButton, Text } from "../ui";
import { app } from "./app";
import { promptFor, promptKey } from "./desktop/banner";
import { store } from "./store";

/** The prompt "Later" put away: a different one (another release, another phase) shows again. */
const putAway = signal("");

function later(): void {
  putAway.set(promptKey(app.updateStatus()));
  try {
    bir.dismiss_update();
  } catch (error) {
    store.logScreen("warn", "dismiss update: " + String(error));
  }
}

function install(): void {
  try {
    bir.install_update();
  } catch (error) {
    store.logScreen("warn", "install update: " + String(error));
  }
}

export function UpdateSlot() {
  // "Later" hides that prompt only: when the status moves on (the host went idle, a new check answered) the next one shows again.
  effect(() => {
    const key = promptKey(app.updateStatus());
    untracked(() => {
      if (putAway() !== "" && key !== putAway()) putAway.set("");
    });
  });
  const prompt = () => (promptKey(app.updateStatus()) === putAway() ? null : promptFor(app.updateStatus()));
  return (
    <If when={prompt()}>
      <View
        testID="update-banner"
        class="absolute right-4 left-4 flex-row items-center gap-3 rounded-(--radius-card) border border-border bg-card px-4 py-3 shadow-lg"
        // Above the narrow layout's tab bar (56) and the form page's floating pill (the toaster's own lifts).
        style={{ bottom: (app.view() === "form" ? 88 : 16) + (app.layout.narrow() ? 56 : 0) }}
      >
        <View class="min-w-0 flex-1">
          <Text testID="update-message">{prompt()?.message ?? ""}</Text>
        </View>
        <If when={prompt()?.install === true}>
          <AppButton testID="update-install" label="Install now" variant="primary" size="sm" onPress={install} />
        </If>
        <AppButton testID="update-later" label="Later" size="sm" onPress={later} />
      </View>
    </If>
  );
}
