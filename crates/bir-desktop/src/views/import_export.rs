use bir_core::db::Database;
use gpui::*;
use gpui_component::*;
use gpui_rsx::rsx;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub enum ImportExportEvent {
    ReloadApp, // Fired when we replace the db file completely and need a cold reload
    ProfilesChanged,
}

impl EventEmitter<ImportExportEvent> for ImportExportView {}

pub struct ImportExportView {
    db: Arc<Mutex<Database>>,
    legacy_detected_root: Option<PathBuf>,
    legacy_selected_root: Option<PathBuf>,
    legacy_scan: Option<bir_core::legacy_migration::LegacyScan>,
    legacy_status: String,
}

impl ImportExportView {
    pub fn new(
        db: Arc<Mutex<Database>>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Self {
        Self {
            db,
            legacy_detected_root: bir_core::legacy_migration::discover_legacy_roots()
                .into_iter()
                .next(),
            legacy_selected_root: None,
            legacy_scan: None,
            legacy_status: String::new(),
        }
    }

    fn import_database(&mut self, new_db_path: PathBuf) -> Result<(), String> {
        let db_path = bir_core::db::default_database_path();

        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        if let Some(parent) = db_path.parent() {
            let safety_backup = parent.join(format!("bir_data.safety-{}.db", timestamp));
            let _ = std::fs::copy(&db_path, safety_backup);
        }

        match bir_core::import::extract_database_zip(&new_db_path, &db_path) {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }

    fn import_profile(&mut self, folder: PathBuf) -> Result<(), String> {
        if let Ok(db) = self.db.lock() {
            match bir_core::import::import_profile_data(&db, &folder) {
                Ok(_) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        } else {
            Err("Failed to acquire database lock".to_string())
        }
    }

    fn apply_legacy_migration(&mut self) -> Result<(String, bool), String> {
        let scan = self
            .legacy_scan
            .as_ref()
            .ok_or_else(|| "Scan a legacy eBIRForms folder first.".to_string())?;
        let db = self
            .db
            .lock()
            .map_err(|_| "Failed to acquire database lock".to_string())?;
        let report = bir_core::legacy_migration::apply_legacy_migration(&db, scan)
            .map_err(|error| error.to_string())?;
        Ok((
            format!(
                "Migration complete: {} created, {} updated, {} unchanged; {} imported, {} already present; {}.",
                pluralized_count(report.profiles_created, "profile"),
                pluralized_count(report.profiles_updated, "profile"),
                pluralized_count(report.profiles_unchanged, "profile"),
                pluralized_count(report.returns_imported, "return"),
                pluralized_count(report.returns_already_present, "return"),
                pluralized_count(report.issues.len(), "issue")
            ),
            report.profiles_created + report.profiles_updated > 0,
        ))
    }
}

impl Render for ImportExportView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let bg = cx.theme().background;
        let border = cx.theme().border;
        let legacy_root_label = self
            .legacy_selected_root
            .as_ref()
            .map(|path| format!("Selected: {}", path.display()))
            .or_else(|| {
                self.legacy_detected_root
                    .as_ref()
                    .map(|path| format!("Detected: {}", path.display()))
            })
            .unwrap_or_else(|| {
                if cfg!(target_os = "windows") {
                    "No standard eBIRForms install was detected.".to_string()
                } else {
                    "No folder selected.".to_string()
                }
            });
        let legacy_summary = self
            .legacy_scan
            .as_ref()
            .map(legacy_scan_summary)
            .unwrap_or_default();
        let legacy_issue_lines = self
            .legacy_scan
            .as_ref()
            .map(|scan| {
                let mut lines = scan
                    .issues
                    .iter()
                    .take(5)
                    .map(|issue| {
                        let file_name = issue
                            .path
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_else(|| issue.path.display().to_string());
                        format!("{file_name}: {}", issue.reason)
                    })
                    .collect::<Vec<_>>();
                let remaining = scan.issues.len().saturating_sub(5);
                if remaining > 0 {
                    lines.push(format!("…and {remaining} more"));
                }
                lines
            })
            .unwrap_or_default();
        let legacy_can_apply = self
            .legacy_scan
            .as_ref()
            .is_some_and(|scan| !scan.profiles.is_empty() || !scan.returns.is_empty());
        let legacy_status = self.legacy_status.clone();
        let mut legacy_actions = div().flex().flex_row().gap_2();
        if self.legacy_detected_root.is_some() {
            legacy_actions = legacy_actions.child(
                gpui_component::button::Button::new("legacy_migration_scan_btn")
                    .label("Scan")
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        let Some(path) = this.legacy_detected_root.clone() else {
                            return;
                        };
                        this.legacy_selected_root = None;
                        this.legacy_scan = None;
                        this.legacy_status.clear();
                        cx.notify();
                        cx.spawn(async move |this, cx| {
                            let scan_path = path.clone();
                            let result = smol::unblock(move || {
                                bir_core::legacy_migration::scan_legacy_folder(&scan_path)
                            })
                            .await;
                            let _ = this.update(cx, |this, cx| {
                                // A "Choose folder" pick supersedes this scan.
                                if this.legacy_selected_root.is_some() {
                                    return;
                                }
                                match result {
                                    Ok(scan) => {
                                        this.legacy_scan = Some(scan);
                                        this.legacy_status.clear();
                                    }
                                    Err(error) => {
                                        this.legacy_scan = None;
                                        this.legacy_status = error.to_string();
                                    }
                                }
                                cx.notify();
                            });
                        })
                        .detach();
                    })),
            );
        }
        legacy_actions = legacy_actions.child(
            gpui_component::button::Button::new("legacy_migration_choose_btn")
                .label("Choose folder…")
                .on_click(cx.listener(|_this, _ev, _window, cx| {
                    cx.spawn(async move |this, cx| {
                        let Some(folder) = rfd::AsyncFileDialog::new()
                            .set_title("Choose an old eBIRForms folder")
                            .pick_folder()
                            .await
                        else {
                            return;
                        };
                        let path = folder.path().to_path_buf();
                        let _ = this.update(cx, |this, cx| {
                            this.legacy_selected_root = Some(path.clone());
                            this.legacy_scan = None;
                            this.legacy_status.clear();
                            cx.notify();
                        });
                        let scan_path = path;
                        let applied_path = scan_path.clone();
                        let result = smol::unblock(move || {
                            bir_core::legacy_migration::scan_legacy_folder(&scan_path)
                        })
                        .await;
                        let _ = this.update(cx, |this, cx| {
                            // Only apply the scan of the folder still
                            // selected; a newer pick supersedes this result.
                            if this.legacy_selected_root.as_ref() != Some(&applied_path) {
                                return;
                            }
                            match result {
                                Ok(scan) => {
                                    this.legacy_scan = Some(scan);
                                    this.legacy_status.clear();
                                }
                                Err(error) => {
                                    this.legacy_scan = None;
                                    this.legacy_status = error.to_string();
                                }
                            }
                            cx.notify();
                        });
                    })
                    .detach();
                })),
        );
        if legacy_can_apply {
            legacy_actions = legacy_actions.child(
                gpui_component::button::Button::new("legacy_migration_apply_btn")
                    .label("Migrate")
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        match this.apply_legacy_migration() {
                            Ok((result, profiles_changed)) => {
                                this.legacy_status = result;
                                if profiles_changed {
                                    cx.emit(ImportExportEvent::ProfilesChanged);
                                }
                            }
                            Err(error) => this.legacy_status = error,
                        }
                        cx.notify();
                    })),
            );
        }
        let mut legacy_details = div().flex().flex_col().gap_1();
        if !legacy_summary.is_empty() {
            legacy_details = legacy_details.child(rsx! {
                <div text_sm>{legacy_summary}</div>
            });
        }
        for line in legacy_issue_lines {
            legacy_details = legacy_details.child(rsx! {
                <div text_sm text_color={cx.theme().muted_foreground}>{line}</div>
            });
        }
        if !legacy_status.is_empty() {
            legacy_details = legacy_details.child(rsx! {
                <div text_sm text_color={cx.theme().muted_foreground}>{legacy_status}</div>
            });
        }

        let root = rsx! {
            <div id={"import_export_scroll"} flex flex_col p_8 gap_6 overflow_y_scroll>
                <div flex flex_col gap_1>
                    <div text_2xl font_weight={FontWeight::BOLD}>{"Import Data"}</div>
                    <div text_color={cx.theme().muted_foreground}>
                        {"Import a profile or database backup from a .zip file."}
                    </div>
                </div>
                <div flex flex_col flex_shrink_0 bg={bg} border_1 border_color={border} rounded_xl overflow_hidden>
                    <div flex flex_col items_start p_6 gap_4 border_b_1 border_color={border}>
                        <div flex flex_col gap_1>
                            <div font_weight={FontWeight::SEMIBOLD}>{"Smart Import"}</div>
                            <div text_sm text_color={cx.theme().muted_foreground}>
                                {"Select a .zip export archive. The app will automatically detect if it is a full database backup or a profile export and import it accordingly."}
                            </div>
                        </div>
                        {
                                gpui_component::button::Button::new("smart_import_btn")
                                    .label("Smart Import")
                                    .on_click(cx.listener(|_this, _ev, _window, cx| {
                                        cx.spawn(async move |this, cx| {
                                            let Some(path_handle) = rfd::AsyncFileDialog::new()
                                                .set_title("Select an exported .zip archive")
                                                .add_filter("Export Archive", &["zip"])
                                                .pick_file()
                                                .await
                                            else { return; };

                                            let path = path_handle.path().to_path_buf();

                                            let is_db = if let Ok(file) = std::fs::File::open(&path) {
                                                if let Ok(mut archive) = zip::ZipArchive::new(file) {
                                                    archive.by_name("bir_data.db").is_ok()
                                                } else { false }
                                            } else { false };

                                            let is_profile = if let Ok(file) = std::fs::File::open(&path) {
                                                if let Ok(mut archive) = zip::ZipArchive::new(file) {
                                                    archive.by_name("profile.json").is_ok()
                                                } else { false }
                                            } else { false };

                                            if !is_db && !is_profile {
                                                rfd::AsyncMessageDialog::new()
                                                    .set_title("Invalid Selection")
                                                    .set_description("The selected zip file does not contain a recognizable profile or database export.")
                                                    .show()
                                                    .await;
                                                return;
                                            }

                                            let res = this.update(cx, |this, cx| {
                                                if is_db {
                                                    let res = this.import_database(path);
                                                    if res.is_ok() { cx.emit(ImportExportEvent::ReloadApp); }
                                                    res
                                                } else {
                                                    let res = this.import_profile(path);
                                                    if res.is_ok() { cx.emit(ImportExportEvent::ReloadApp); }
                                                    res
                                                }
                                            });

                                            match res {
                                                Ok(Ok(())) => {
                                                    rfd::AsyncMessageDialog::new()
                                                        .set_title("Import Successful")
                                                        .set_description(if is_db { "Database imported. Application state has been reloaded." } else { "Profile data successfully restored." })
                                                        .show()
                                                        .await;
                                                }
                                                Ok(Err(e)) => {
                                                    rfd::AsyncMessageDialog::new()
                                                        .set_title("Import Failed")
                                                        .set_description(&e)
                                                        .show()
                                                        .await;
                                                }
                                                _ => {}
                                            }
                                        }).detach();
                                    }))
                        }
                    </div>
                </div>
                <div flex flex_col flex_shrink_0 bg={bg} border_1 border_color={border} rounded_xl overflow_hidden>
                    <div flex flex_col items_start p_6 gap_3>
                        <div flex flex_col gap_1>
                            <div font_weight={FontWeight::SEMIBOLD}>{"Migrate from old eBIRForms"}</div>
                            <div text_sm text_color={cx.theme().muted_foreground}>
                                {"Import taxpayer profiles and filed Final Copies. Plain savefiles are counted but not migrated; source files are never changed."}
                            </div>
                        </div>
                        <div text_sm text_color={cx.theme().muted_foreground}>{legacy_root_label}</div>
                        {legacy_actions}
                        {legacy_details}
                    </div>
                </div>
            </div>
        };
        root.into_any_element()
    }
}

fn pluralized_count(count: usize, item: &str) -> String {
    format!("{count} {item}{}", if count == 1 { "" } else { "s" })
}

fn legacy_scan_summary(scan: &bir_core::legacy_migration::LegacyScan) -> String {
    let mut returns_by_tin = BTreeMap::<String, usize>::new();
    for legacy_return in &scan.returns {
        let branch = legacy_return.branch.parse::<u32>().unwrap_or_default();
        *returns_by_tin
            .entry(format!("{}-{:05}", legacy_return.tin, branch))
            .or_default() += 1;
    }
    let by_tin = returns_by_tin
        .iter()
        .map(|(tin, count)| format!("{tin}: {count}"))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "{}; {} by TIN{}; {}; {} not migrated.",
        pluralized_count(scan.profiles.len(), "profile"),
        pluralized_count(scan.returns.len(), "return"),
        if by_tin.is_empty() {
            String::new()
        } else {
            format!(" ({by_tin})")
        },
        pluralized_count(scan.issues.len(), "issue"),
        pluralized_count(scan.savefile_count, "savefile")
    )
}
