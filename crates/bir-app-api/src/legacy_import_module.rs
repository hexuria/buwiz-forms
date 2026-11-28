//! The import from BIR's official eBIRForms (the old Windows program): a
//! person points the app at the old program's folder, reviews what was found,
//! and brings its taxpayers and filed returns in.
//!
//! The page never hands the host a path, as with `import_archive`: the host's
//! own folder dialog picks the folder (`legacy_scan_folder`), or, on Windows,
//! the page names one of the standard places by its index in
//! `legacy_standard_folders` and the host looks them up again
//! (`legacy_scan_standard`). A scan reads the folder (never changes it),
//! keeps what it read on this thread, and answers a summary: counts and the
//! files it skipped, no file bytes and no field values. `legacy_import`
//! imports exactly that held scan, by its number, and only while the folder
//! and the open database are the ones it was read against; the scan is let go
//! after the import, on `legacy_scan_cancel`, and when another scan starts.
//! [`scan_folder`] is the dialog-free inner part the tests call; it is no
//! verb at all.
//!
//! What an import brought in is read back by `list_legacy_filed_returns`: for
//! one branch's TIN, one record per return (the form, its period, whether it
//! is an amendment or the e-mailed copy, the file's name, the import date).
//! The query never reads the stored file or its fields, so no bytes and no
//! values can be in the answer.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bir_core::db;
use bir_core::legacy_migration::{self, LegacyMigrationError, LegacyScan, LegacyScanIssue};
use gpui_shell::{HostArguments, HostError, HostModule, HostObject, HostResult, HostValue};

use crate::bir_module::{after_path_prompt, with_database, with_reader};

/// TypeScript appended to the `bir` module's declarations.
pub const DECLARATIONS: &str = r#"
export interface LegacySkippedFile {
    /** The file's name, without its folder. */
    file: string;
    /** Why it was not read, in words ("not an XML file", "could not decrypt return" ...). */
    reason: string;
}
export interface LegacyScanSummary {
    /** "scanned": there is something to import and the host holds the scan for legacy_import; "empty": an eBIRForms folder with no
     *  taxpayer and no filed return in it; "not_ebirforms": the folder has no profile or IAF_RDO_Copy folder. Nothing is held unless "scanned". */
    kind: "scanned" | "empty" | "not_ebirforms";
    /** The number to hand legacy_import; 0 unless kind is "scanned". */
    scan: number;
    /** The folder that was read. */
    folder: string;
    /** Taxpayers (TIN and branch) the profiles and filed returns name. */
    taxpayers: number;
    /** Filed returns (Final Copies) that can be imported. */
    filed_returns: number;
    /** Saved drafts (savefile): counted, never imported. */
    drafts: number;
    /** Files that were not read, with the reason. */
    skipped: LegacySkippedFile[];
}
export type LegacyScanAnswer = LegacyScanSummary | { kind: "cancelled" };
export interface LegacyStandardFolder {
    index: number;
    path: string;
}
/** The standard places the old program installs to (C:\eBIRForms, also on the system drive) that exist on this computer: always empty off Windows. */
export function legacy_standard_folders(): LegacyStandardFolder[];
/** Opens the host's folder dialog and reads the eBIRForms folder the person picks (the installation, its profile or its IAF_RDO_Copy folder). Takes no path. */
export function legacy_scan_folder(): Promise<LegacyScanAnswer>;
/** Reads the standard folder at `index` of legacy_standard_folders, looked up again now. */
export function legacy_scan_standard(index: number): LegacyScanSummary;
export interface LegacyImportReport {
    taxpayers_created: number;
    taxpayers_updated: number;
    taxpayers_unchanged: number;
    /** Empty details of a taxpayer filled from the old program (what was already there is kept). */
    fields_filled: number;
    returns_imported: number;
    returns_already_present: number;
    /** Files the scan skipped. */
    skipped: number;
}
/** Imports the scan the person reviewed (`scan` from its summary). Refused when there is none, it is another one, or the folder or the open database changed since. */
export function legacy_import(scan: number): LegacyImportReport;
/** Lets go of the held scan (the review was cancelled). */
export function legacy_scan_cancel(): null;
/** One return that was filed in BIR's official eBIRForms and imported. Records only: no file bytes, no field values, nothing to send. */
export interface LegacyFiledReturn {
    /** The row's number, to key a list by. */
    id: number;
    /** The form as the old program named it: the id it matched to a form this app knows ("1601Cv2018"), else the name in the file's name. */
    form: string;
    /** This app's code for that form ("1601C"), or null when it has none. */
    form_code: string | null;
    /** The period as the file's name wrote it: "062025" (month), "2026Q1" (quarter), "122026Q1" (fiscal quarter), "2025" (year), "03152026" (a date). */
    period: string;
    /** The amendment's number ("V2" is 2), or null for an original. */
    amendment_version: number | null;
    /** True for the copy that was e-mailed to BIR. */
    email_copy: boolean;
    /** The file's name in the old program's folder, without the e-mail address an e-mailed copy's name carries. */
    source_file: string;
    /** When it was imported, as the database writes it ("2026-11-17 08:30:00", UTC). */
    imported_at: string;
}
/** The returns imported from the old program for one branch's TIN (12 to 14 digits, dashes allowed), oldest import first; none for a TIN the import never named. */
export function list_legacy_filed_returns(tin: string): LegacyFiledReturn[];
"#;

/// A scan held for `legacy_import`, with what it was read against.
struct HeldScan {
    id: u64,
    folder: PathBuf,
    database: PathBuf,
    sources: Vec<SourceFile>,
    scan: LegacyScan,
}

/// One file a scan reads: where, how long, last changed.
#[derive(Debug, PartialEq, Eq)]
struct SourceFile {
    path: PathBuf,
    len: u64,
    modified: Option<SystemTime>,
}

/// The scan under review: process-wide, never per-thread (X3). A sign-out runs on the account
/// thread, not the engine's, and must end the scan for everyone; the scan already records the
/// database path it was made against and refuses an import when the room changed, so one scan
/// for the process cannot cross rooms either.
static HELD: std::sync::Mutex<Option<HeldScan>> = std::sync::Mutex::new(None);
static NEXT_SCAN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

fn held_lock() -> std::sync::MutexGuard<'static, Option<HeldScan>> {
    HELD.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn register(module: HostModule) -> HostModule {
    module
        .function("legacy_standard_folders", legacy_standard_folders)
        .async_function("legacy_scan_folder", legacy_scan_folder)
        .function("legacy_scan_standard", legacy_scan_standard)
        .function("legacy_import", legacy_import)
        .function("legacy_scan_cancel", |_| legacy_scan_cancel())
        .function("list_legacy_filed_returns", list_legacy_filed_returns)
}

fn legacy_standard_folders(arguments: &HostArguments) -> HostResult {
    if !arguments.is_empty() {
        return Err(HostError::new("legacy_standard_folders takes no arguments"));
    }
    crate::account_gate::ensure_unlocked()?;
    Ok(HostValue::Array(
        legacy_migration::discover_legacy_roots()
            .into_iter()
            .enumerate()
            .map(|(index, path)| {
                HostObject::new()
                    .field("index", index)
                    .field("path", path.to_string_lossy().into_owned())
                    .into()
            })
            .collect(),
    ))
}

/// The page's scan of a folder it does not name: the host opens its folder
/// dialog. A call with any argument is refused before anything opens.
fn legacy_scan_folder(
    arguments: &HostArguments,
) -> Result<gpui_shell::host_modules::HostFuture, HostError> {
    if !arguments.is_empty() {
        return Err(HostError::new(
            "legacy_scan_folder takes no path; the host opens the folder dialog",
        ));
    }
    crate::account_gate::ensure_unlocked()?;
    after_path_prompt(
        |answer| crate::hooks::hooks().prompt_for_folder("Scan", answer),
        |path| scan_folder(&path),
    )
}

/// A standard folder by its index, looked up again: a path is refused.
fn legacy_scan_standard(arguments: &HostArguments) -> HostResult {
    let index = match arguments.get(0) {
        Some(HostValue::Number(_)) if arguments.len() == 1 => arguments.integer(0)?,
        _ => {
            return Err(HostError::new(
                "legacy_scan_standard takes the folder's index from legacy_standard_folders, not a path",
            ));
        }
    };
    crate::account_gate::ensure_unlocked()?;
    let roots = legacy_migration::discover_legacy_roots();
    let Some(folder) = usize::try_from(index)
        .ok()
        .and_then(|index| roots.get(index))
    else {
        return Err(HostError::new(
            "That eBIRForms folder is no longer on this computer. Choose the folder instead.",
        ));
    };
    scan_folder(folder)
}

/// Read the eBIRForms folder at `path` and hold the scan; not a verb.
/// Replaces any scan held before.
pub(crate) fn scan_folder(path: &Path) -> HostResult {
    crate::account_gate::ensure_unlocked()?;
    forget_scan();
    let folder = path.to_string_lossy().into_owned();
    let not_ebirforms = || {
        HostObject::new()
            .field("kind", "not_ebirforms")
            .field("scan", 0)
            .field("folder", folder.clone())
            .field("taxpayers", 0)
            .field("filed_returns", 0)
            .field("drafts", 0)
            .field("skipped", HostValue::Array(Vec::new()))
            .into()
    };
    // The file list is taken before the read and checked after it, so what
    // is held is the folder as that list describes it.
    let sources = source_files(path);
    let scan = match legacy_migration::scan_legacy_folder(path) {
        Ok(scan) => scan,
        Err(LegacyMigrationError::NotALegacyFolder(_)) => return Ok(not_ebirforms()),
        Err(error) => {
            return Err(HostError::new(format!(
                "Could not read that folder ({error}). Nothing was imported."
            )));
        }
    };
    if source_files(path) != sources {
        return Err(HostError::new(
            "The folder changed while it was being read. Close the old program and scan it again.",
        ));
    }
    let taxpayers = taxpayer_count(&scan);
    let has_something = !scan.profiles.is_empty() || !scan.returns.is_empty();
    let id = if has_something {
        NEXT_SCAN.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
    } else {
        0
    };
    let summary = HostObject::new()
        .field("kind", if has_something { "scanned" } else { "empty" })
        .field("scan", id)
        .field("folder", folder)
        .field("taxpayers", taxpayers)
        .field("filed_returns", scan.returns.len())
        .field("drafts", scan.savefile_count)
        .field("skipped", skipped(&scan.issues))
        .into();
    if has_something {
        let held = HeldScan {
            id,
            folder: path.to_path_buf(),
            database: db::app_database_path(),
            sources,
            scan,
        };
        *held_lock() = Some(held);
    }
    Ok(summary)
}

/// Import the held scan numbered `scan`, and let it go whatever happens.
fn legacy_import(arguments: &HostArguments) -> HostResult {
    let id = match arguments.get(0) {
        Some(HostValue::Number(_)) if arguments.len() == 1 => arguments.integer(0)?,
        _ => {
            return Err(HostError::new(
                "legacy_import takes the number of the scan that was reviewed, not a path",
            ));
        }
    };
    crate::account_gate::ensure_unlocked()?;
    let Some(held) = held_lock().take() else {
        return Err(HostError::new(
            "There is no scanned folder to import. Choose the folder again.",
        ));
    };
    if u64::try_from(id).ok() != Some(held.id) {
        // A newer scan is the one under review: keep it.
        *held_lock() = Some(held);
        return Err(HostError::new(
            "That is not the folder under review. Scan the folder again.",
        ));
    }
    if held.database != db::app_database_path() {
        return Err(HostError::new(
            "The open database changed since the folder was read. Scan the folder again.",
        ));
    }
    if source_files(&held.folder) != held.sources {
        return Err(HostError::new(
            "The folder changed since it was read. Scan it again before importing.",
        ));
    }
    let report = with_database(|db| {
        legacy_migration::apply_legacy_migration(db, &held.scan)
            .map_err(|error| HostError::new(format!("The import stopped: {error}")))
    })?;
    Ok(HostObject::new()
        .field("taxpayers_created", report.profiles_created)
        .field("taxpayers_updated", report.profiles_updated)
        .field("taxpayers_unchanged", report.profiles_unchanged)
        .field("fields_filled", report.profile_fields_filled)
        .field("returns_imported", report.returns_imported)
        .field("returns_already_present", report.returns_already_present)
        .field("skipped", report.issues.len())
        .into())
}

fn legacy_scan_cancel() -> HostResult {
    forget_scan();
    Ok(HostValue::Null)
}

/// The returns imported from the old program for one branch's TIN: what each
/// is, never its bytes or field values (the query does not read them). A read
/// like `list_taxpayers`: behind the account gate, on the reader connection,
/// so it does not wait for the submission worker.
fn list_legacy_filed_returns(arguments: &HostArguments) -> HostResult {
    let tin = arguments.string(0)?.to_owned();
    with_reader(|db| {
        let rows = db
            .list_legacy_filed_return_summaries_for_tin(&tin)
            .map_err(|error| HostError::new(error.to_string()))?;
        Ok(HostValue::Array(
            rows.into_iter()
                .map(|row| {
                    HostObject::new()
                        .field("id", row.id)
                        .field("form", row.form)
                        .field("form_code", row.form_code)
                        .field("period", row.period)
                        .field("amendment_version", row.amendment_version)
                        .field("email_copy", row.email_copy)
                        .field("source_file", shown_file_name(&row.source_filename))
                        .field("imported_at", row.imported_at)
                        .into()
                })
                .collect(),
        ))
    })
}

/// Lets the held scan go: after an import, on cancel, when another scan
/// starts, and when the open database is replaced (a restore).
pub(crate) fn forget_scan() {
    held_lock().take();
}

/// Whether a scan is held for `legacy_import`.
#[cfg(test)]
pub(crate) fn holds_scan() -> bool {
    held_lock().is_some()
}

/// A file's name as the page may show it. The old program writes the address
/// an e-mailed copy went to into the name, between two `#`
/// (`...-062025#someone@example.test#.xml`): that part is left out.
fn shown_file_name(name: &str) -> String {
    match (name.find('#'), name.rfind('#')) {
        (Some(open), Some(close)) if open < close => {
            format!("{}{}", &name[..open], &name[close + 1..])
        }
        _ => name.to_owned(),
    }
}

/// Distinct taxpayers (TIN and numeric branch) the scan names, as the
/// migration keys them; a placeholder all-zero TIN is not one.
fn taxpayer_count(scan: &LegacyScan) -> usize {
    let key =
        |tin: &str, branch: &str| (tin.to_string(), branch.parse::<u32>().unwrap_or_default());
    scan.profiles
        .iter()
        .map(|profile| key(&profile.tin, &profile.branch))
        .chain(scan.returns.iter().map(|ret| key(&ret.tin, &ret.branch)))
        .filter(|(tin, _)| !tin.bytes().all(|byte| byte == b'0'))
        .collect::<BTreeSet<_>>()
        .len()
}

fn skipped(issues: &[LegacyScanIssue]) -> HostValue {
    HostValue::Array(
        issues
            .iter()
            .map(|issue| {
                let file = issue
                    .path
                    .file_name()
                    .map(|name| shown_file_name(&name.to_string_lossy()))
                    .unwrap_or_default();
                HostObject::new()
                    .field("file", file)
                    .field("reason", issue.reason.clone())
                    .into()
            })
            .collect(),
    )
}

/// The files a scan of `root` reads, in the folders it reads them from
/// (`scan_legacy_folder`'s choice: `root` itself when it is a `profile`
/// or `IAF_RDO_Copy` folder, else its `profile`, `IAF_RDO_Copy` and
/// `savefile` folders), with size and change time, sorted.
fn source_files(root: &Path) -> Vec<SourceFile> {
    let is = |path: &Path, name: &str| {
        path.file_name()
            .and_then(|own| own.to_str())
            .is_some_and(|own| own.eq_ignore_ascii_case(name))
    };
    let folders: Vec<PathBuf> = if is(root, "profile") || is(root, "IAF_RDO_Copy") {
        vec![root.to_path_buf()]
    } else {
        std::fs::read_dir(root)
            .map(|entries| {
                entries
                    .filter_map(Result::ok)
                    .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                    .map(|entry| entry.path())
                    .filter(|path| {
                        is(path, "profile") || is(path, "IAF_RDO_Copy") || is(path, "savefile")
                    })
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut files: Vec<SourceFile> = folders
        .iter()
        .filter_map(|folder| std::fs::read_dir(folder).ok())
        .flatten()
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok()?;
            metadata.is_file().then(|| SourceFile {
                path: entry.path(),
                len: metadata.len(),
                modified: metadata.modified().ok(),
            })
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bir_module::{to_json, use_ephemeral_database};
    use serde_json::json;

    /// The committed fixture: one profile, three filed returns (an original,
    /// its amendment, an e-mail copy), a file that is not a return, a draft.
    fn fixture() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../bir-core/tests/legacy-ebirforms")
    }

    fn args(values: Vec<HostValue>) -> HostArguments {
        HostArguments::new(values)
    }

    fn scan(path: &Path) -> serde_json::Value {
        to_json(&scan_folder(path).expect("the folder scans"))
    }

    fn import(id: &serde_json::Value) -> HostResult {
        legacy_import(&args(vec![HostValue::Number(id.as_f64().unwrap())]))
    }

    /// A copy of the fixture the test may change.
    fn fixture_copy() -> tempfile::TempDir {
        let copy = tempfile::tempdir().unwrap();
        for folder in ["profile", "IAF_RDO_Copy", "savefile"] {
            let to = copy.path().join(folder);
            std::fs::create_dir_all(&to).unwrap();
            for entry in std::fs::read_dir(fixture().join(folder)).unwrap() {
                let entry = entry.unwrap();
                std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
            }
        }
        copy
    }

    /// The scan under review is process-wide (X3): parallel tests in this
    /// module share it, so they take turns.
    fn scan_turn() -> std::sync::MutexGuard<'static, ()> {
        // The session tests scan too (cloud_account's sign-out ends the
        // review), so the turn is the machine's one turn, not a local one.
        crate::session::test_turn()
    }

    #[test]
    fn a_scan_of_the_fixture_counts_what_is_there_and_shows_no_values() {
        let _turn = scan_turn();
        use_ephemeral_database();
        let summary = scan(&fixture());
        assert_eq!(summary["kind"], "scanned");
        assert!(summary["scan"].as_u64().unwrap() > 0);
        assert_eq!(summary["taxpayers"], 1);
        assert_eq!(summary["filed_returns"], 3);
        assert_eq!(summary["drafts"], 1);
        assert_eq!(
            summary["skipped"],
            json!([{ "file": "garbage.xml", "reason": "invalid return filename" }])
        );
        // Counts, a folder, file names and reasons: no name, address or amount of the files.
        let text = summary.to_string();
        for value in ["Juan Dela Cruz Test", "Test Street", "legacy@example.test"] {
            assert!(!text.contains(value), "{value} is in the summary");
        }
        legacy_scan_cancel().unwrap();
    }

    /// An e-mailed copy that cannot be read is named without the address its
    /// file name carries.
    #[test]
    fn a_skipped_e_mailed_copy_is_named_without_its_address() {
        let _turn = scan_turn();
        use_ephemeral_database();
        let copy = fixture_copy();
        std::fs::write(
            copy.path()
                .join("IAF_RDO_Copy/123456788000-1601Cv2018-072025#someone@example.test#.xml"),
            "not a final copy",
        )
        .unwrap();
        let summary = scan(copy.path());
        let skipped = summary["skipped"].as_array().unwrap();
        assert!(
            skipped
                .iter()
                .any(|row| row["file"] == "123456788000-1601Cv2018-072025.xml"),
            "{skipped:?}"
        );
        assert!(!summary["skipped"].to_string().contains('@'), "{skipped:?}");
        legacy_scan_cancel().unwrap();
    }

    /// A database restore between the review and the import lets the scan go:
    /// what was reviewed against one database is not imported into another.
    #[test]
    fn a_scan_does_not_outlive_the_database_it_was_reviewed_against() {
        let _turn = scan_turn();
        use_ephemeral_database();
        let summary = scan(&fixture());
        crate::bir_module::reset_database().unwrap();
        assert_eq!(
            import(&summary["scan"]).unwrap_err().message(),
            "There is no scanned folder to import. Choose the folder again."
        );
    }

    #[test]
    fn the_import_brings_in_the_reviewed_scan_once_and_a_second_import_adds_nothing() {
        let _turn = scan_turn();
        use_ephemeral_database();
        let first = scan(&fixture());
        let report = to_json(&import(&first["scan"]).unwrap());
        assert_eq!(
            report,
            json!({
                "taxpayers_created": 1, "taxpayers_updated": 0, "taxpayers_unchanged": 0, "fields_filled": 8,
                "returns_imported": 3, "returns_already_present": 0, "skipped": 1,
            })
        );
        with_database(|db| {
            assert_eq!(db.list_profiles().unwrap().len(), 1);
            assert_eq!(
                db.list_legacy_filed_returns_for_tin("12345678800000")
                    .unwrap()
                    .len(),
                3
            );
            Ok(())
        })
        .unwrap();
        // The scan was used: importing it again is refused.
        assert_eq!(
            import(&first["scan"]).unwrap_err().message(),
            "There is no scanned folder to import. Choose the folder again."
        );
        // The same folder again: nothing new.
        let second = scan(&fixture());
        assert_ne!(second["scan"], first["scan"]);
        let again = to_json(&import(&second["scan"]).unwrap());
        assert_eq!(again["taxpayers_created"], 0);
        assert_eq!(again["taxpayers_unchanged"], 1);
        assert_eq!(again["fields_filled"], 0);
        assert_eq!(again["returns_imported"], 0);
        assert_eq!(again["returns_already_present"], 3);
    }

    #[test]
    fn an_import_with_no_scan_another_scan_or_after_cancel_is_refused() {
        let _turn = scan_turn();
        use_ephemeral_database();
        legacy_scan_cancel().unwrap();
        assert_eq!(
            import(&json!(1)).unwrap_err().message(),
            "There is no scanned folder to import. Choose the folder again."
        );
        let summary = scan(&fixture());
        let other = summary["scan"].as_u64().unwrap() + 100;
        assert_eq!(
            import(&json!(other)).unwrap_err().message(),
            "That is not the folder under review. Scan the folder again."
        );
        // The one under review is kept through that refusal, and let go on cancel.
        legacy_scan_cancel().unwrap();
        assert!(import(&summary["scan"]).is_err());
        with_database(|db| {
            assert!(
                db.list_profiles().unwrap().is_empty(),
                "nothing was imported"
            );
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn a_folder_changed_since_the_scan_is_scanned_again_first() {
        let _turn = scan_turn();
        use_ephemeral_database();
        let copy = fixture_copy();
        let summary = scan(copy.path());
        std::fs::write(
            copy.path().join("profile/999999999000.xml"),
            "<div>fn=Afn=</div>",
        )
        .unwrap();
        assert_eq!(
            import(&summary["scan"]).unwrap_err().message(),
            "The folder changed since it was read. Scan it again before importing."
        );
        with_database(|db| {
            assert!(db.list_profiles().unwrap().is_empty());
            Ok(())
        })
        .unwrap();
    }

    #[test]
    fn an_empty_or_wrong_folder_says_so_and_holds_nothing() {
        let _turn = scan_turn();
        use_ephemeral_database();
        let wrong = tempfile::tempdir().unwrap();
        std::fs::create_dir(wrong.path().join("Documents")).unwrap();
        let summary = scan(wrong.path());
        assert_eq!(summary["kind"], "not_ebirforms");
        assert_eq!(summary["scan"], 0);

        let empty = tempfile::tempdir().unwrap();
        std::fs::create_dir(empty.path().join("profile")).unwrap();
        std::fs::create_dir(empty.path().join("savefile")).unwrap();
        std::fs::write(empty.path().join("savefile/draft.xml"), "x").unwrap();
        let summary = scan(empty.path());
        assert_eq!(summary["kind"], "empty");
        assert_eq!(summary["scan"], 0);
        assert_eq!(summary["drafts"], 1);
        assert!(import(&json!(0)).is_err(), "nothing is held");
    }

    /// The page never names a folder: a path, or any argument where none is
    /// taken, is refused before anything opens or is read.
    #[test]
    fn the_verbs_refuse_a_path() {
        let _turn = scan_turn();
        use_ephemeral_database();
        let module = crate::bir_module::module();
        for verb in [
            "legacy_standard_folders",
            "legacy_scan_folder",
            "legacy_scan_standard",
            "legacy_import",
            "legacy_scan_cancel",
        ] {
            assert!(module.has(verb), "{verb}");
        }
        assert!(!module.has("scan_folder"), "not a verb");
        let path = HostValue::Str(fixture().to_string_lossy().into_owned());
        match module.begin("legacy_scan_folder", &args(vec![path.clone()])) {
            Ok(_) => panic!("legacy_scan_folder answered a call with a path"),
            Err(refusal) => assert_eq!(
                refusal.message(),
                "legacy_scan_folder takes no path; the host opens the folder dialog"
            ),
        }
        for (verb, refusal) in [
            (
                "legacy_scan_standard",
                "legacy_scan_standard takes the folder's index from legacy_standard_folders, not a path",
            ),
            (
                "legacy_import",
                "legacy_import takes the number of the scan that was reviewed, not a path",
            ),
            (
                "legacy_standard_folders",
                "legacy_standard_folders takes no arguments",
            ),
        ] {
            let error = module.call(verb, &args(vec![path.clone()])).unwrap_err();
            assert_eq!(error.message(), refusal, "{verb}");
        }
        // Off Windows there is no standard folder, so index 0 is not one.
        if !cfg!(windows) {
            assert_eq!(
                to_json(
                    &module
                        .call("legacy_standard_folders", &args(vec![]))
                        .unwrap()
                ),
                json!([])
            );
            let error = module
                .call("legacy_scan_standard", &args(vec![HostValue::Number(0.0)]))
                .unwrap_err();
            assert_eq!(
                error.message(),
                "That eBIRForms folder is no longer on this computer. Choose the folder instead."
            );
        }
    }

    /// Past the argument check the folder panel is the host's: with no
    /// running app there is nothing to open it on, so the call refuses.
    #[test]
    fn the_folder_dialog_needs_a_running_app() {
        let _turn = scan_turn();
        use_ephemeral_database();
        let module = crate::bir_module::module();
        assert!(module.is_async("legacy_scan_folder"));
        match module.begin("legacy_scan_folder", &args(vec![])) {
            Ok(_) => panic!("legacy_scan_folder started without an app"),
            Err(refusal) => {
                assert_eq!(
                    refusal.message(),
                    "no app is running to open the file dialog"
                )
            }
        }
    }

    #[test]
    fn a_locked_gate_refuses_the_scan_and_the_import() {
        let _turn = scan_turn();
        use_ephemeral_database();
        crate::account_gate::force_locked(|| {
            let locked = crate::account_gate::LOCKED_MESSAGE;
            assert_eq!(scan_folder(&fixture()).unwrap_err().message(), locked);
            assert_eq!(import(&json!(1)).unwrap_err().message(), locked);
        });
    }

    /// The fixture's taxpayer as a page asks for it: a branch's TIN, 14 digits.
    const FIXTURE_TIN: &str = "12345678800000";

    fn listed_result(tin: &str) -> HostResult {
        list_legacy_filed_returns(&args(vec![HostValue::Str(tin.to_string())]))
    }

    fn listed(tin: &str) -> serde_json::Value {
        to_json(&listed_result(tin).expect("the list reads"))
    }

    fn import_fixture() {
        let summary = scan(&fixture());
        import(&summary["scan"]).expect("the fixture imports");
    }

    /// A return put in the table directly, for what the fixture does not hold.
    fn insert(
        tin: &str,
        identity: &str,
        form_id: Option<&str>,
        period: &str,
        version: Option<u32>,
    ) {
        with_database(|db| {
            db.insert_legacy_filed_return(&db::LegacyFiledReturn {
                id: None,
                sha256: format!("{tin}{identity}{period}{version:?}"),
                tin: tin.to_string(),
                form_identity: identity.to_string(),
                form_id: form_id.map(str::to_string),
                period: period.to_string(),
                amendment_version: version,
                email_copy: None,
                encrypted: false,
                source_filename: format!("{tin}-{identity}-{period}.xml"),
                raw_file: b"stored bytes".to_vec(),
                fields_json: r#"{"txtTaxpayerName":"A Stored Name"}"#.to_string(),
                imported_at: None,
            })
            .map(|_| HostValue::Null)
            .map_err(|error| HostError::new(error.to_string()))
        })
        .unwrap();
    }

    #[test]
    fn the_imported_returns_are_listed_for_their_taxpayer_and_no_other() {
        let _turn = scan_turn();
        use_ephemeral_database();
        assert_eq!(listed(FIXTURE_TIN), json!([]), "nothing before the import");
        import_fixture();

        let rows = listed(FIXTURE_TIN);
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 3, "the fixture's three filed returns");
        // The e-mailed copy is listed under its name without the address.
        let named = |file: &str, email_copy: bool| {
            rows.iter()
                .find(|row| row["source_file"] == file && row["email_copy"] == email_copy)
                .unwrap_or_else(|| panic!("{file} is not listed in {rows:?}"))
        };
        for (file, version, email_copy) in [
            ("123456788000-1601Cv2018-062025.xml", json!(null), false),
            ("123456788000-1601Cv2018-062025V1.xml", json!(1), false),
            ("123456788000-1601Cv2018-062025.xml", json!(null), true),
        ] {
            let row = named(file, email_copy);
            assert_eq!(row["form"], "1601Cv2018", "{file}");
            assert_eq!(row["form_code"], "1601C", "{file}");
            assert_eq!(row["period"], "062025", "{file}");
            assert_eq!(row["amendment_version"], version, "{file}");
            assert_eq!(row["email_copy"], email_copy, "{file}");
            let imported = row["imported_at"].as_str().unwrap();
            assert!(
                chrono::NaiveDateTime::parse_from_str(imported, "%Y-%m-%d %H:%M:%S").is_ok(),
                "{file}: {imported} is the database's timestamp"
            );
        }
        let mut ids: Vec<_> = rows.iter().map(|row| row["id"].as_i64().unwrap()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), 3, "each row has its own number");

        // The same TIN, written the other ways a caller may: dashes, the old 12 digits.
        assert_eq!(listed("123-456-788-00000"), listed(FIXTURE_TIN));
        assert_eq!(listed("123456788000"), listed(FIXTURE_TIN));
        // Importing again adds no row to the list.
        import_fixture();
        assert_eq!(listed(FIXTURE_TIN).as_array().unwrap().len(), 3);

        // Another taxpayer, another branch of this one, a bare root, nothing, nonsense: none.
        for tin in [
            "00012345300000",
            "12345678800001",
            "123456788",
            "",
            "not a tin",
        ] {
            assert_eq!(listed(tin), json!([]), "{tin:?}");
        }
        // Rows of another taxpayer in the table are not this one's.
        insert("00012345300000", "1601Cv2018", None, "072025", None);
        assert_eq!(listed(FIXTURE_TIN).as_array().unwrap().len(), 3);
        assert_eq!(listed("00012345300000").as_array().unwrap().len(), 1);
    }

    /// A form this app has no code for is named as the old program named it.
    #[test]
    fn a_form_the_app_does_not_know_is_listed_under_its_stored_identity() {
        let _turn = scan_turn();
        use_ephemeral_database();
        let demo = "00012345300000";
        insert(demo, "9999Xv2000", None, "2025", None);
        insert(demo, "1601C", None, "062025", Some(2));
        let rows = listed(demo);
        let rows = rows.as_array().unwrap();
        assert_eq!(rows.len(), 2);
        let unknown = &rows[0];
        assert_eq!(unknown["form"], "9999Xv2000");
        assert_eq!(unknown["form_code"], json!(null));
        // A code stored as the identity resolves, and keeps the identity it was stored under.
        assert_eq!(rows[1]["form"], "1601C");
        assert_eq!(rows[1]["form_code"], "1601C");
        assert_eq!(rows[1]["amendment_version"], 2);
    }

    /// Records only: the answer has these keys and nothing the stored file holds.
    #[test]
    fn the_list_holds_no_file_bytes_and_no_field_values() {
        let _turn = scan_turn();
        use_ephemeral_database();
        import_fixture();
        insert("00012345300000", "1601Cv2018", None, "062025", None);
        for tin in [FIXTURE_TIN, "00012345300000"] {
            let rows = listed(tin);
            for row in rows.as_array().unwrap() {
                let mut keys: Vec<&str> = row
                    .as_object()
                    .unwrap()
                    .keys()
                    .map(String::as_str)
                    .collect();
                keys.sort_unstable();
                assert_eq!(
                    keys,
                    [
                        "amendment_version",
                        "email_copy",
                        "form",
                        "form_code",
                        "id",
                        "imported_at",
                        "period",
                        "source_file"
                    ]
                );
                for key in keys {
                    assert!(
                        DECLARATIONS.contains(&format!("    {key}: ")),
                        "{key} is declared for the page"
                    );
                }
            }
            let text = rows.to_string();
            // The fixture's name, address and e-mail, the field names of its files, and the demo row's stored bytes and name.
            for value in [
                "Juan Dela Cruz Test",
                "Test Street",
                "legacy@example.test",
                "Test Consulting Services",
                "txtTaxpayerName",
                "frm1601c",
                "A Stored Name",
                "stored bytes",
                "raw_file",
                "fields",
                // The address in the e-mailed copy's file name, and any other.
                "copy@example.test",
                "@",
            ] {
                assert!(!text.contains(value), "{value} is in the list");
            }
        }
    }

    #[test]
    fn the_list_is_a_verb_that_takes_a_tin() {
        let _turn = scan_turn();
        use_ephemeral_database();
        let module = crate::bir_module::module();
        assert!(module.has("list_legacy_filed_returns"));
        assert!(!module.is_async("list_legacy_filed_returns"));
        import_fixture();
        let rows = module
            .call(
                "list_legacy_filed_returns",
                &args(vec![HostValue::Str(FIXTURE_TIN.to_string())]),
            )
            .unwrap();
        assert_eq!(to_json(&rows).as_array().unwrap().len(), 3);
        assert!(
            module
                .call("list_legacy_filed_returns", &args(vec![]))
                .is_err()
        );
        assert!(
            module
                .call(
                    "list_legacy_filed_returns",
                    &args(vec![HostValue::Number(1.0)])
                )
                .is_err()
        );
    }

    #[test]
    fn a_locked_gate_refuses_the_list() {
        let _turn = scan_turn();
        use_ephemeral_database();
        import_fixture();
        crate::account_gate::force_locked(|| {
            assert_eq!(
                listed_result(FIXTURE_TIN).unwrap_err().message(),
                crate::account_gate::LOCKED_MESSAGE
            );
        });
        assert_eq!(
            listed(FIXTURE_TIN).as_array().unwrap().len(),
            3,
            "unlocked again"
        );
    }
}
