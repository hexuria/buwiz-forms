// Settings > Import / Export (old transferCards): Import data (a zip the host's file dialog picks), Import from BIR eBIRForms (the old
// program's folder, EbirFormsImport.tsx) and Export this profile (the Tax profile's card, reused). Without a taxpayer there is nothing to export.
import { app } from "../../shell";
import { AppButton, Card, SettingRow, Text } from "../../ui";
import { ProfileExportCard } from "../taxpayer";
import { Labeled } from "./parts";
import { importResultLine, importing, runImport } from "./importState";
import { EbirFormsImport } from "./EbirFormsImport";
import { safetyCopyLine, safetyCopyStatus } from "../auth/migrationNote";

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
            <Text variant="meta" tone="fg3" testID="settings-import-help">
              Importing a backup replaces this account's tax profiles with the backup's. Returns sent after this backup was made will not be in it.
            </Text>
            <If when={importResultLine() !== null}>
              <Text variant="meta" tone="fg2" testID="settings-import-result">{importResultLine() ?? ""}</Text>
            </If>
            <EbirFormsImport />
          {/* The safety copy of the move to this account, while it is kept (§A.3.4). */}
          <If when={safetyCopyStatus() !== null}>
            {(() => {
              const row = safetyCopyStatus();
              return row === null ? null : (
                <SettingRow testID="settings-safety-copy" title="Safety copy" hint="">
                  <Text variant="meta" tone="fg2" testID="settings-safety-copy-line">{safetyCopyLine(row)}</Text>
                </SettingRow>
              );
            })()}
          </If>
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
