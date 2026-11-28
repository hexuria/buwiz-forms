// The permanent failure's note (§A.3.4): on Home once per account, then under Settings › Account, in plain
// words, with the support address. "Once per account" is kept in NSH Storage under the room's database path,
// so it survives restarts but never crosses rooms.
import { signal, Storage, View } from "nsh";
import { bir } from "../../bir";
import { store } from "../../shell";
import { storageKey } from "../../shell/app";
import { Card, deviceCopy, Line, Text } from "../../ui";
import { migrationPermanent } from "./migration";

const NOTE_KEY = "migration.permanent-note-shown";
const shown = signal(false);

/** Read whether Home already showed the note for the open room. */
export function loadMigrationNoteShown(): void {
  shown.set(false);
  const stored = storageKey(NOTE_KEY, store.systemInfo().database_path ?? "");
  if (stored === null) return;
  Storage.getItem(stored)
    .then((raw) => {
      if (raw === "1") shown.set(true);
    })
    .catch(() => {});
}

/** The note's sentence, in plain words, shared by Home and Settings › Account. */
export function migrationNoteSentence(detail: string): string {
  const suffix = detail.length > 0 ? ` (${detail})` : "";
  return `Tax profiles from an earlier version of eBIRForms could not be moved${suffix}. They are still on this device, unchanged. Write to support@buwiz.com.`;
}

/** Home's once-per-account note: shows while the failure stands and Home has not shown it for this room. */
export function HomeMigrationNote() {
  if (migrationPermanent() === null || shown()) return null;
  // Shown now: do not show again on Home for this room (the Settings › Account row keeps it findable).
  shown.set(true);
  const stored = storageKey(NOTE_KEY, store.systemInfo().database_path ?? "");
  if (stored !== null) Storage.setItem(stored, "1").catch(() => {});
  const detail = migrationPermanent() ?? "";
  return (
    <View testID="home-migration-note" class="rounded-(--radius-card) border border-warning-soft bg-warning-soft/40 p-3">
      <Text variant="meta" testID="home-migration-note-text">{deviceCopy(migrationNoteSentence(detail))}</Text>
    </View>
  );
}

/** The Settings › Account row (the note's permanent home). Null while there is no permanent failure. */
export function SettingsMigrationRow() {
  if (migrationPermanent() === null) return null;
  const detail = migrationPermanent() ?? "";
  return (
    <View testID="settings-migration-note" class="gap-1 py-2">
      <Text variant="emph" testID="settings-migration-note-title">Tax profiles from an earlier version</Text>
      <Line testID="settings-migration-note-text" variant="meta" tone="fg2">{deviceCopy(migrationNoteSentence(detail))}</Line>
    </View>
  );
}

/** The safety copy's status for the Settings › Import / Export row (§A.3.4). */
export interface SafetyCopyRow {
  keptUntil: string | null;
  reason: string;
}
export function safetyCopyStatus(): SafetyCopyRow | null {
  try {
    const answer = bir.safety_copy_status() as { kind: "none" } | { kind: "until"; stamp: number } | { kind: "kept"; reason: string };
    if (answer.kind === "until") {
      const day = new Date(answer.stamp * 1000);
      const keptUntil = `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, "0")}-${String(day.getDate()).padStart(2, "0")}`;
      return { keptUntil, reason: "" };
    }
    if (answer.kind === "kept") return { keptUntil: null, reason: answer.reason };
  } catch {
    /* no claim, no copy, no row */
  }
  return null;
}

/** The row's line: "Safety copy from the move to this account: kept until <date>" or "kept" with the reason. */
export function safetyCopyLine(row: SafetyCopyRow): string {
  return row.keptUntil !== null && row.keptUntil.length > 0
    ? `Safety copy from the move to this account: kept until ${row.keptUntil}`
    : `Safety copy from the move to this account: kept${row.reason.length > 0 ? ` (${row.reason})` : ""}`;
}
