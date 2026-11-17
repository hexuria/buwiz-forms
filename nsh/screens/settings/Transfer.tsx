// Settings > Import / Export (old transferCards): Import data (a zip the host's file dialog picks), Import from BIR eBIRForms (the old
// program's folder, EbirFormsImport.tsx) and Export this profile (the Tax profile's card, reused). Without a taxpayer there is nothing to export.
import { app } from "../../shell";
import { AppButton, Card, SettingRow, Text } from "../../ui";
import { ProfileExportCard } from "../taxpayer";
import { Labeled } from "./parts";
import { importing, runImport } from "./importState";
import { EbirFormsImport } from "./EbirFormsImport";

export function TransferSection(props: { rows: string[] }) {
  const has = (id: string) => props.rows.includes(id);
  const showExport = () => has("export") && app.taxpayer() !== null;
  return (
    <>
      <If when={has("import")}>
        <Labeled label="Import">
          <Card testID="settings-data">
            <SettingRow testID="settings-import" title="Import data" hint="Restore a taxpayer or database export (.zip)">
              <AppButton
                testID="settings-import-btn"
                label={importing() ? "Importing…" : "Import"}
                size="sm"
                icon="upload"
                disabled={importing()}
                onPress={() => void runImport()}
              />
            </SettingRow>
            <EbirFormsImport />
          </Card>
        </Labeled>
      </If>
      <If when={showExport()}>
        <Labeled label="Export">
          <ProfileExportCard />
        </Labeled>
      </If>
      <If when={!has("import") && !showExport()}>
        <Text testID="settings-transfer-empty" tone="fg3">Add a taxpayer before exporting a profile.</Text>
      </If>
    </>
  );
}
