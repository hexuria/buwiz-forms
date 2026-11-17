// Settings > Import / Export, under "Import data": "Import from BIR eBIRForms". Choose the old program's folder (the host's folder
// dialog) or, on Windows, Scan the standard folder it found; review the counts and the skipped files; Import or Cancel; then what came in.
import { View } from "nsh";
import { app } from "../../shell";
import { AppButton, Code, SettingRow, Text } from "../../ui";
import { StackRow } from "./parts";
import {
  cancelLegacyReview, chooseLegacyFolder, dismissLegacy, importLegacyScan, legacy, scanStandardFolder,
} from "./legacyState";

export const EBIR_HINT =
  "Brings in taxpayer details and filed returns from the old eBIRForms program's folder. It only reads: the folder is left as it is, and nothing is sent to BIR.";

/** The rows go straight into the Import card (a fragment), so the kit's hairline sits between them and "Import data". */
/** How many skipped files the review lists before "and N more". */
const SKIPPED_SHOWN = 12;

function ChooseButton() {
  return (
    <AppButton
      testID="settings-ebir-choose"
      label={legacy.phase() === "scanning" ? "Reading…" : "Choose folder…"}
      size="sm"
      icon="hard-drive"
      disabled={legacy.busy() || legacy.phase() === "review"}
      onPress={() => void chooseLegacyFolder()}
    />
  );
}

export function EbirFormsImport() {
  return (
    <>
      {/* Under 600 the hint is too long for the space beside a button: the button goes under the words. */}
      <If
        when={app.layout.narrow()}
        else={
          <SettingRow testID="settings-ebir-row" title="Import from BIR eBIRForms" hint={EBIR_HINT}>
            <ChooseButton />
          </SettingRow>
        }
      >
        <StackRow testID="settings-ebir-row" title="Import from BIR eBIRForms" hint={EBIR_HINT}>
          <ChooseButton />
        </StackRow>
      </If>
      <Each of={legacy.standardFolders()} key={(f) => `${f.index}:${f.path}`}>
        {(folder) => (
          <SettingRow testID={`settings-ebir-standard-${folder().index}`} title="Found on this computer" hint={folder().path}>
            <AppButton
              testID={`settings-ebir-scan-${folder().index}`}
              label="Scan"
              size="sm"
              disabled={legacy.busy() || legacy.phase() === "review"}
              onPress={() => void scanStandardFolder(folder().index)}
            />
          </SettingRow>
        )}
      </Each>
      <If when={legacy.phase() === "review" || legacy.phase() === "importing"}>
        <Review />
      </If>
      <If when={legacy.phase() === "done"}>
        <Result />
      </If>
      <If when={legacy.phase() === "problem"}>
        <Problem />
      </If>
    </>
  );
}

/** One count: words at the left, the number at the right in tabular figures. */
function Count(props: { label: string; value: number; testID: string; muted?: boolean }) {
  return (
    <View testID={props.testID} class="w-full flex-row items-center gap-3 py-1">
      <Text class="min-w-0 flex-1" tone={props.muted ? "fg3" : "fg2"}>{props.label}</Text>
      <Text testID={`${props.testID}-n`} variant="emph" class="tabular-nums text-right" tone={props.muted ? "fg3" : "fg"}>
        {String(props.value)}
      </Text>
    </View>
  );
}

function Skipped() {
  const all = () => legacy.summary()?.skipped ?? [];
  const shown = () => all().slice(0, SKIPPED_SHOWN).map((s, i) => ({ ...s, i }));
  const more = () => all().length - shown().length;
  return (
    <If when={all().length > 0}>
      <View testID="settings-ebir-skipped" class="w-full gap-1 rounded-(--radius-row) bg-hover px-3 py-2">
        <Text variant="metaStrong" tone="fg2">Not read</Text>
        <Each of={shown()} key={(s) => `${s.i}:${s.file}`}>
          {(s) => (
            <View class="w-full flex-row flex-wrap gap-x-2">
              <Code class="min-w-0 shrink">{s().file}</Code>
              <Text variant="meta" tone="fg3">{s().reason}</Text>
            </View>
          )}
        </Each>
        <If when={more() > 0}>
          <Text variant="meta" tone="fg3">{`and ${more()} more`}</Text>
        </If>
      </View>
    </If>
  );
}

function Review() {
  const s = () => legacy.summary();
  const importing = () => legacy.phase() === "importing";
  return (
    <View testID="settings-ebir-review" class="w-full gap-3 border-t border-border py-3.5">
      <View class="gap-0.5">
        <Text variant="control">Ready to import</Text>
        <Text testID="settings-ebir-folder" variant="meta" tone="fg3" lines={2}>{s()?.folder ?? ""}</Text>
      </View>
      <View class="w-full">
        <Count testID="settings-ebir-taxpayers" label="Taxpayers" value={s()?.taxpayers ?? 0} />
        <Count testID="settings-ebir-returns" label="Filed returns" value={s()?.filed_returns ?? 0} />
        <Count testID="settings-ebir-drafts" label="Saved drafts (not imported)" value={s()?.drafts ?? 0} muted />
        <Count testID="settings-ebir-skipped-n" label="Files not read" value={s()?.skipped.length ?? 0} muted />
      </View>
      <Skipped />
      <Text variant="meta" tone="fg3">
        Details already entered here are kept; only empty ones are filled. Filed returns are kept as records and are never sent to BIR.
      </Text>
      <View class="w-full flex-row flex-wrap justify-end gap-2">
        <AppButton testID="settings-ebir-cancel" label="Cancel" variant="ghost" size="sm" disabled={importing()} onPress={cancelLegacyReview} />
        <AppButton
          testID="settings-ebir-import"
          label={importing() ? "Importing…" : "Import"}
          variant="primary"
          size="sm"
          icon="upload"
          disabled={importing()}
          onPress={() => void importLegacyScan()}
        />
      </View>
    </View>
  );
}

function Result() {
  const r = () => legacy.report();
  return (
    <View testID="settings-ebir-result" class="w-full gap-3 border-t border-border py-3.5">
      <Text variant="control" tone="success">Import finished</Text>
      <View class="w-full">
        <Count testID="settings-ebir-created" label="Taxpayers added" value={r()?.taxpayers_created ?? 0} />
        <Count testID="settings-ebir-updated" label="Taxpayers with details filled in" value={r()?.taxpayers_updated ?? 0} />
        <Count testID="settings-ebir-unchanged" label="Taxpayers already here" value={r()?.taxpayers_unchanged ?? 0} muted />
        <Count testID="settings-ebir-imported" label="Filed returns added" value={r()?.returns_imported ?? 0} />
        <Count testID="settings-ebir-present" label="Filed returns already here" value={r()?.returns_already_present ?? 0} muted />
        <Count testID="settings-ebir-notread" label="Files not read" value={r()?.skipped ?? 0} muted />
      </View>
      <Text variant="meta" tone="fg3">
        New taxpayers are in the taxpayer list. Filed returns are listed in Filings, under Filed in BIR eBIRForms, for the taxpayer you have open.
      </Text>
      <View class="w-full flex-row justify-end">
        <AppButton testID="settings-ebir-done" label="Done" size="sm" onPress={dismissLegacy} />
      </View>
    </View>
  );
}

function Problem() {
  const s = () => legacy.summary();
  return (
    <View testID="settings-ebir-problem" class="w-full gap-3 border-t border-border py-3.5">
      <Text testID="settings-ebir-problem-text" tone="danger">{legacy.problem()}</Text>
      <If when={s()?.kind === "empty" && (s()?.drafts ?? 0) > 0}>
        <Text variant="meta" tone="fg3">{`It holds ${s()?.drafts ?? 0} saved drafts; drafts are not imported.`}</Text>
      </If>
      <Skipped />
      <View class="w-full flex-row justify-end">
        <AppButton testID="settings-ebir-ok" label="OK" size="sm" onPress={dismissLegacy} />
      </View>
    </View>
  );
}
