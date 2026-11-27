// Settings > Import / Export: the import (old runImport). The host asks for the file itself (a dialog), so the page passes no path.
import { signal } from "nsh";
import { errorText } from "../../lib/errors";
import { store, toast } from "../../shell";
import { legacyDatabaseReplaced } from "./legacyState";

const busy = signal(false);
const resultLine = signal<string | null>(null);

/** The outcome of the last database-archive restore, shown under the Import control (9c). */
export const importResultLine = (): string | null => resultLine();

/** Whether an import is waiting on the file dialog or the restore. */
export const importing = (): boolean => busy();

/**
 * Restores a taxpayer's export or a whole-database backup from a zip the person picks. The host stages and checks a
 * database backup before it replaces anything, and keeps a safety copy; every cached read is dropped afterwards.
 */
export async function runImport(): Promise<void> {
  if (busy()) return;
  busy.set(true);
  try {
    const result = await store.writeAsync("import_archive");
    if (result.kind === "database") {
      legacyDatabaseReplaced();
      // 9c: no confirm step, so the person reads it here — what an import means for returns already
      // sent, and the way back. There is no in-app row that imports a safety copy back, so the line
      // names the truthful route.
      resultLine.set(
        "Database imported. Returns sent after this backup was made will not be in it. " +
          "A copy of what was here before is kept on this device; write to support@buwiz.com to get it back."
      );
      toast("Database imported.", "upload");
    }
    else if (result.kind === "profile") toast("Taxpayer data restored.", "upload");
    else if (result.kind === "invalid") toast("That zip has no taxpayer or database export in it.", "triangle-alert");
  } catch (error) {
    toast(`Import failed: ${errorText(error)}`, "triangle-alert");
  } finally {
    busy.set(false);
  }
}
