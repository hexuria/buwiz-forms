// Settings > Import / Export: the import from BIR's official eBIRForms (the old Windows program). The host picks the folder itself
// (its folder dialog) or, on Windows, scans a standard place the page names by index; the page never hands it a path. A scan is held
// on the host until it is imported or cancelled; the page shows its counts for review first. Module signals: one owner, the card reads.
import { signal } from "nsh";
import { bir, type LegacyImportReport, type LegacyScanAnswer, type LegacyScanSummary, type LegacyStandardFolder } from "../../bir";
import { errorText } from "../../lib/errors";
import { app, store, toast } from "../../shell";

/** Where the card is: nothing under way, reading a folder, the review, importing, the result, or a folder that cannot be imported. */
export type LegacyPhase = "idle" | "scanning" | "review" | "importing" | "done" | "problem";

const phase = signal<LegacyPhase>("idle");
const summary = signal<LegacyScanSummary | null>(null);
const report = signal<LegacyImportReport | null>(null);
const problem = signal<string>("");
/** Counts the sessions (a lock, sign-out or sign-in starts the next): an answer that began in an earlier one is dropped. */
let session = 0;

export const legacy = {
  phase: (): LegacyPhase => phase(),
  /** The scan under review (and, for an empty folder, what it found). */
  summary: (): LegacyScanSummary | null => summary(),
  report: (): LegacyImportReport | null => report(),
  /** The words for a folder that cannot be imported, or a host error. */
  problem: (): string => problem(),
  busy: (): boolean => phase() === "scanning" || phase() === "importing",
  /** The standard eBIRForms folders on this computer (Windows only; elsewhere none). */
  standardFolders: (): LegacyStandardFolder[] =>
    store.read("legacy_standard_folders", () => bir.legacy_standard_folders(), [] as LegacyStandardFolder[]),
};

export const NOT_EBIRFORMS =
  "This is not an eBIRForms folder. Choose the folder the old program is installed in (C:\\eBIRForms), or the profile or IAF_RDO_Copy folder inside it.";
export const NOTHING_FOUND = "Nothing to import here: this eBIRForms folder has no taxpayer profiles and no filed returns.";

/** Lets a frame paint the busy state before a sync host call blocks the engine. */
const paint = () => new Promise<void>((done) => setTimeout(done, 0));

/** The card as it was when a scan started, for a dismissed folder dialog to put back. */
let before: { phase: LegacyPhase; summary: LegacyScanSummary | null; report: LegacyImportReport | null; problem: string } | null = null;

function start(): boolean {
  if (legacy.busy()) return false;
  before = { phase: phase(), summary: summary(), report: report(), problem: problem() };
  report.set(null);
  problem.set("");
  phase.set("scanning");
  return true;
}

function show(answer: LegacyScanAnswer): void {
  if (answer.kind === "cancelled") {
    // Nothing was read and the host holds what it held: the card goes back to what it showed.
    summary.set(before?.summary ?? null);
    report.set(before?.report ?? null);
    problem.set(before?.problem ?? "");
    phase.set(before?.phase ?? "idle");
    return;
  }
  summary.set(answer);
  if (answer.kind === "scanned") phase.set("review");
  else {
    problem.set(answer.kind === "empty" ? NOTHING_FOUND : NOT_EBIRFORMS);
    phase.set("problem");
  }
}

function failed(error: unknown, what: string): void {
  summary.set(null);
  problem.set(`${what}: ${errorText(error)}`);
  phase.set("problem");
}

/** "Choose folder…": the host's folder dialog, then its scan. A dismissed dialog leaves the card as it was before. */
export async function chooseLegacyFolder(): Promise<void> {
  if (!start()) return;
  const began = session;
  try {
    const answer = await bir.legacy_scan_folder();
    if (began !== session) {
      // The folder dialog outlived the session it was opened in: the scan is not this session's to review.
      if (answer.kind === "scanned") letGo();
      return;
    }
    show(answer);
  } catch (error) {
    if (began === session) failed(error, "The folder could not be read");
  }
}

/** "Scan" beside a standard folder (by its index in legacy.standardFolders()). */
export async function scanStandardFolder(index: number): Promise<void> {
  if (!start()) return;
  await paint();
  try {
    show(bir.legacy_scan_standard(index));
  } catch (error) {
    failed(error, "The folder could not be read");
  }
}

/** "Import": exactly the scan under review. Every cached read is dropped, so the new taxpayers show at once. */
export async function importLegacyScan(): Promise<void> {
  const scan = summary();
  if (phase() !== "review" || !scan) return;
  const began = session;
  phase.set("importing");
  await paint();
  // A lock or sign-out in that one frame ended the session the scan was reviewed in (and let the scan go).
  if (began !== session) return;
  try {
    const done = store.write("legacy_import", scan.scan);
    report.set(done);
    phase.set("done");
    toast(importedSentence(done), "upload");
  } catch (error) {
    failed(error, "Nothing was imported");
    toast(`Import failed: ${errorText(error)}`, "triangle-alert");
  }
}

/** Asks the host to let its held scan go. */
function letGo(): void {
  try {
    bir.legacy_scan_cancel();
  } catch {
    // Nothing held is nothing to let go; a locked gate drops it at the next scan.
  }
}

/** "Cancel" in the review: the host lets the scan go. */
export function cancelLegacyReview(): void {
  if (phase() === "importing") return;
  letGo();
  reset();
}

/** The open database was replaced (a backup was restored): the host let the scan go, so the review goes too. */
export function legacyDatabaseReplaced(): void {
  if (phase() === "review") reset();
}

/** "Done" or "OK" under a result or a problem. */
export function dismissLegacy(): void {
  if (legacy.busy()) return;
  reset();
}

function reset(): void {
  summary.set(null);
  report.set(null);
  problem.set("");
  phase.set("idle");
}

const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** The toast after an import. */
export function importedSentence(r: LegacyImportReport): string {
  const added: string[] = [];
  if (r.taxpayers_created > 0) added.push(plural(r.taxpayers_created, "taxpayer", "taxpayers"));
  if (r.returns_imported > 0) added.push(plural(r.returns_imported, "filed return", "filed returns"));
  if (added.length > 0) return `Imported ${added.join(" and ")} from eBIRForms.`;
  if (r.taxpayers_updated > 0) return `Filled in details for ${plural(r.taxpayers_updated, "taxpayer", "taxpayers")} from eBIRForms.`;
  return "Nothing new: everything in that folder is already here.";
}

// A lock, sign-out or sign-in starts the card over and lets a held scan go.
app.onSessionChange(() => {
  session += 1;
  if (phase() === "review" || phase() === "importing") letGo();
  reset();
});
