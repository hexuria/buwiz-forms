// "Export this profile" (old profileExportCard), drawn in Settings > Import / Export: a credential-free archive saved
// through the host's save dialog. A taxpayer with a PIN types it in the cells, which clear after every attempt.
import { signal, View } from "nsh";
import { app, store } from "../../shell";
import { AppButton, Card, PinCells, SettingRow } from "../../ui";
import { exportProfile, shared } from "./actions";

export function ProfileExportCard() {
  const pin = signal("");
  const hasPin = () => {
    const t = app.taxpayer();
    return t ? Boolean(store.taxpayerCreds(t.tin_root).has_pin) : false;
  };
  return (
    <Card testID="tp-export">
      <SettingRow
        testID="tp-export-row"
        title="Export this profile"
        hint={hasPin() ? "The archive leaves out credentials; the profile PIN gates saving it" : "Saves a credential-free archive of every branch and return"}
      >
        <View class="flex-row items-center gap-2">
          <If when={hasPin()}>
            <PinCells testID="tp-export-pin" length={4} masked label="Profile PIN" value={pin()} onChange={(v) => pin.set(v)} />
          </If>
          <AppButton
            testID="tp-export-btn"
            label="Export"
            size="sm"
            icon="save"
            disabled={shared.exportBusy()}
            onPress={() => {
              const typed = pin();
              // The PIN never stays in page state: cleared on every attempt.
              pin.set("");
              void exportProfile(hasPin() ? typed : null);
            }}
          />
        </View>
      </SettingRow>
    </Card>
  );
}
