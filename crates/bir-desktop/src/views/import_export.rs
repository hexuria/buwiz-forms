use bir_core::db::Database;
use gpui::*;
use gpui_component::*;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub enum ImportExportEvent {
    ReloadApp, // Fired when we replace the db file completely and need a cold reload
}

impl EventEmitter<ImportExportEvent> for ImportExportView {}

pub struct ImportExportView {
    db: Arc<Mutex<Database>>,
}

impl ImportExportView {
    pub fn new(
        db: Arc<Mutex<Database>>,
        _window: &mut Window,
        _cx: &mut Context<'_, Self>,
    ) -> Self {
        Self { db }
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
}

impl Render for ImportExportView {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<'_, Self>) -> impl IntoElement {
        let bg = cx.theme().background;
        let border = cx.theme().border;

        div()
            .id("import_export_scroll")
            .flex()
            .flex_col()
            .p_8()
            .gap_6()
            .overflow_y_scroll()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(div().text_2xl().font_weight(FontWeight::BOLD).child("Import Data"))
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child("Import a profile or database backup from a .zip file."),
                    ),
            )
            .child(
                div()
                    .flex()
                    .flex_col()
                    .flex_shrink_0()
                    .bg(bg)
                    .border_1()
                    .border_color(border)
                    .rounded_xl()
                    .overflow_hidden()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .items_start()
                            .p_6()
                            .gap_4()
                            .border_b_1()
                            .border_color(border)
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .gap_1()
                                    .child(div().font_weight(FontWeight::SEMIBOLD).child("Smart Import"))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Select a .zip export archive. The app will automatically detect if it is a full database backup or a profile export and import it accordingly."),
                                    ),
                            )
                            .child(
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
                                    })),
                            )
                    )
            )
            .into_any_element()
    }
}
