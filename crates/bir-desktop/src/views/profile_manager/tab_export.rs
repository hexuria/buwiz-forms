//! Export tab — Profile data export to ZIP, and the profile lifecycle.

use super::*;
use crate::app::ProfileLifecycleAction;
use gpui_rsx::rsx;

impl ProfileManagerView {
    /// Render the "Export" tab (tab index 4).
    ///
    /// Only shown when editing an existing profile (`editing_id.is_some()`).
    /// Contains: profile ZIP export workflow with folder picker dialog, and
    /// the administrator-only archive / restore / delete controls.
    pub(super) fn render_export_tab(&self, cx: &Context<Self>) -> gpui::AnyElement {
        if self.active_tab != 4 || self.editing_id.is_none() {
            return div().into_any_element();
        }

        let root = rsx! {
            <div
                p_4
                rounded_lg
                border_1
                border_color={cx.theme().border}
                bg={cx.theme().background}
                flex
                flex_col
                gap_4
                w_full
                min_w_0
            >
                <div flex flex_col gap_1>
                    <div font_weight={FontWeight::SEMIBOLD}>{"Export Profile"}</div>
                    <div text_sm text_color={cx.theme().muted_foreground}>
                        {"Create a complete backup of this profile (JSON)."}
                    </div>
                </div>
                {gpui_component::button::Button::new("export_profile_btn")
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
                    }))}
            </div>
        };

        let lifecycle = self.render_profile_lifecycle_section(cx);

        div()
            .flex()
            .flex_col()
            .gap_4()
            .w_full()
            .min_w_0()
            .child(root)
            .child(lifecycle)
            .into_any_element()
    }

    /// Archive / restore / delete for the profile being edited.
    ///
    /// These used to sit on the sidebar row, where any click reached them.
    /// They live here because this page is the profile, and every one of them
    /// is administrator-only: the button states the intent and `AppState`
    /// decides whether to prompt for the app-lock credential first.
    fn render_profile_lifecycle_section(&self, cx: &Context<Self>) -> gpui::AnyElement {
        let is_archived = self.stored_is_archived;
        let tin = self.tin_input.read(cx).value(cx).to_string();

        let root = rsx! {
            <div
                p_4
                rounded_lg
                border_1
                border_color={cx.theme().border}
                bg={cx.theme().background}
                flex
                flex_col
                gap_4
                w_full
                min_w_0
            >
                <div flex flex_col gap_1>
                    <div font_weight={FontWeight::SEMIBOLD}>{"Profile Status"}</div>
                    <div text_sm text_color={cx.theme().muted_foreground}>
                        {if is_archived {
                            "This profile is archived and hidden from the sidebar. Restoring returns it to the list. Deleting is permanent."
                        } else {
                            "Archiving hides this profile from the sidebar and keeps every filed return. Administrator access is required."
                        }}
                    </div>
                </div>
                <div flex gap_2>
                    {
                        if is_archived {
                            div()
                                .flex()
                                .gap_2()
                                .child(
                                    gpui_component::button::Button::new("restore_profile_btn")
                                        .label("Restore Profile")
                                        .on_click(cx.listener({
                                            let tin = tin.clone();
                                            move |_this, _ev, _window, cx| {
                                                cx.emit(ProfileEvent::LifecycleRequested {
                                                    tin: tin.clone(),
                                                    action: ProfileLifecycleAction::Restore,
                                                });
                                            }
                                        })),
                                )
                                .child(
                                    gpui_component::button::Button::new("delete_profile_btn")
                                        .danger()
                                        .label("Delete Profile")
                                        .on_click(cx.listener({
                                            let tin = tin.clone();
                                            move |_this, _ev, _window, cx| {
                                                cx.emit(ProfileEvent::LifecycleRequested {
                                                    tin: tin.clone(),
                                                    action: ProfileLifecycleAction::Delete,
                                                });
                                            }
                                        })),
                                )
                                .into_any_element()
                        } else {
                            gpui_component::button::Button::new("archive_profile_btn")
                                .label("Archive Profile")
                                .on_click(cx.listener({
                                    let tin = tin.clone();
                                    move |_this, _ev, _window, cx| {
                                        cx.emit(ProfileEvent::LifecycleRequested {
                                            tin: tin.clone(),
                                            action: ProfileLifecycleAction::Archive,
                                        });
                                    }
                                }))
                                .into_any_element()
                        }
                    }
                </div>
            </div>
        };
        root.into_any_element()
    }
}
