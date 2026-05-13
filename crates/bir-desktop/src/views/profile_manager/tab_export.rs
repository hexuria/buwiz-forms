//! Export tab — Profile data export to ZIP and profile deletion.

use super::*;

impl ProfileManagerView {
    /// Render the "Export" tab (tab index 3).
    ///
    /// Only shown when editing an existing profile (`editing_id.is_some()`).
    /// Contains: profile ZIP export workflow with folder picker dialog.
    pub(super) fn render_export_tab(&self, cx: &Context<Self>) -> gpui::AnyElement {
        if self.active_tab != 3 || self.editing_id.is_none() {
            return div().into_any_element();
        }

        div()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .flex()
            .flex_col()
            .gap_4()
            .w_full()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_1()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("Export Profile"),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child("Create a complete backup of this profile (JSON)."),
                    ),
            )
            .child(
                gpui_component::button::Button::new("export_profile_btn")
                    .label("Export Profile Data")
                    .on_click(cx.listener(|this, _ev, _window, cx| {
                        let tin = this.tin_input.read(cx).value(cx).to_string();
                        cx.spawn(async move |this, cx| {
                            let Some(target_dir_handle) = rfd::AsyncFileDialog::new()
                                .set_title("Select Folder to Save Profile Backup")
                                .pick_folder()
                                .await
                            else {
                                return;
                            };

                            let target_dir = target_dir_handle.path().to_path_buf();

                            let res = this.update(cx, |this, _cx| {
                                // Safe: SystemTime arithmetic is infallible after UNIX_EPOCH
                                let timestamp = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                                let backup_path = target_dir
                                    .join(format!("BIR_Profile_{}_{}.zip", tin, timestamp));

                                if let Ok(db) = this.db.lock() {
                                    match bir_core::export::export_profile_data(
                                        &db,
                                        &tin,
                                        &backup_path,
                                    ) {
                                        Ok(_) => Ok(backup_path),
                                        Err(e) => Err(e.to_string()),
                                    }
                                } else {
                                    Err("Failed to acquire database lock".to_string())
                                }
                            });

                            match res {
                                Ok(Ok(path)) => {
                                    rfd::AsyncMessageDialog::new()
                                        .set_title("Profile Exported")
                                        .set_description(format!("Saved to {}", path.display()))
                                        .show()
                                        .await;
                                }
                                Ok(Err(e)) => {
                                    rfd::AsyncMessageDialog::new()
                                        .set_title("Export Failed")
                                        .set_description(&e)
                                        .show()
                                        .await;
                                }
                                _ => {}
                            }
                        })
                        .detach();
                    })),
            )
            .into_any_element()
    }
}
