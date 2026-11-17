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
export async function runImport(): Promise<void> {
  if (busy()) return;
  busy.set(true);
  try {
    const result = await store.writeAsync("import_archive");
    if (result.kind === "database") {
      legacyDatabaseReplaced();
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
