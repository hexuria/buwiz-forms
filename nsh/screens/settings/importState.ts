// Settings > Import / Export: the import (old runImport). The host asks for the file itself (a dialog), so the page passes no path.
import { signal } from "nsh";
import { errorText } from "../../lib/errors";
import { store, toast } from "../../shell";
import { legacyDatabaseReplaced } from "./legacyState";

const busy = signal(false);

/** Whether an import is waiting on the file dialog or the restore. */
export const importing = (): boolean => busy();

/**
 * Restores a taxpayer's export or a whole-database backup from a zip the person picks. The host stages and checks a
 * database backup before it replaces anything, and keeps a safety copy; every cached read is dropped afterwards.
 */
/** One plain sentence for what an import did, every zero part left out. */
export function importSentence(restored: number, keptNewer: number, unmatched: number): string {
  const items = (n: number) => (n === 1 ? "item" : "items");
  const parts: string[] = [];
  if (restored > 0) parts.push(`Restored ${restored} ${items(restored)} from the backup.`);
  if (keptNewer > 0)
    parts.push(
      `${keptNewer} ${keptNewer === 1 ? "was" : "were"} left as they are, because this device has newer versions.`,
    );
  if (unmatched > 0)
    parts.push(
      `${unmatched} couldn't be matched or read and ${unmatched === 1 ? "was" : "were"} skipped.`,
    );
  if (restored > 0) return parts.join(" ");
  if (keptNewer > 0 && unmatched === 0)
    return "Nothing was restored: this device already has the same or newer versions of everything in the backup.";
  if (parts.length === 0)
    return "Nothing was restored: this device already has the same or newer versions of everything in the backup.";
  return `Nothing was restored. ${parts.join(" ")}`;
}

export async function runImport(): Promise<void> {
  if (busy()) return;
  busy.set(true);
  try {
    const result = await store.writeAsync("import_archive");
    if (result.kind === "database") {
      legacyDatabaseReplaced();
      toast("Database imported.", "upload");
    }
    else if (result.kind === "profile")
      toast(
        importSentence(Number(result.restored ?? 0), Number(result.kept_newer ?? 0), Number(result.unmatched ?? 0)),
        "upload",
      );
    else if (result.kind === "invalid") toast("That zip has no taxpayer or database export in it.", "triangle-alert");
  } catch (error) {
    toast(`Import failed: ${errorText(error)}`, "triangle-alert");
  } finally {
    busy.set(false);
  }
}
