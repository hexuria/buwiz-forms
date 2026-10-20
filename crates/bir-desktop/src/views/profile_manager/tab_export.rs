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
        let export_needs_pin = self.export_needs_pin(cx);

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
                {if export_needs_pin {
                    rsx! {
                        <div flex flex_col gap_2>
                            {Self::field_label("Profile PIN", cx)}
                            {OtpInput::new(&self.export_pin_input).large()}
                            <div text_xs text_color={cx.theme().muted_foreground}>
                                {"This profile's PIN is required to export it."}
                            </div>
                        </div>
                    }
                    .into_any_element()
                } else {
                    div().into_any_element()
                }}
                {gpui_component::button::Button::new("export_profile_btn")
                    .label("Export Profile Data")
                    .on_click(cx.listener(move |this, _ev, _window, cx| {
                        let tin = this.tin_input.read(cx).value(cx).to_string();
                        // The PIN is asked for before the folder dialog: a
                        // profile with one exports only under it (the
                        // maintainer's 2026-10-20 decision, enforced in
                        // bir-core), while a profile without one behaves as
                        // it always did.
                        let pin = if export_needs_pin {
                            let pin = this.export_pin_input.read(cx).value().to_string();
                            if pin.len() != 4 {
                                this.pending_notification = Some((
                                    gpui_component::notification::NotificationType::Error,
                                    "Enter this profile's 4-digit PIN to export it.".to_string(),
                                ));
                                cx.notify();
                                return;
                            }
                            Some(pin)
                        } else {
                            None
                        };
                        cx.spawn(async move |this, cx| {
                            // Whatever happens — cancel, refusal or export —
                            // the entered PIN is done being held.
                            let Some(target_dir_handle) = rfd::AsyncFileDialog::new()
                                .set_title("Select Folder to Save Profile Backup")
                                .pick_folder()
                                .await
                            else {
                                let _ = this.update_in(cx, |this, window, cx| {
                                    this.export_pin_input.update(cx, |input, cx| {
                                        input.set_value("", window, cx)
                                    });
                                });
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
                                        pin.as_deref(),
                                    ) {
                                        Ok(_) => Ok(backup_path),
                                        Err(e) => Err(e.to_string()),
                                    }
                                } else {
                                    Err("Failed to acquire database lock".to_string())
                                }
                            });

                            let _ = this.update_in(cx, |this, window, cx| {
                                this.export_pin_input
                                    .update(cx, |input, cx| input.set_value("", window, cx));
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

    /// Whether exporting this profile asks for its PIN first — what
    /// `export_profile_data` will require: the taxpayer row's PIN, or a
    /// legacy `profile_pin_hash` still on this profile's row.
    fn export_needs_pin(&self, cx: &gpui::App) -> bool {
        if self.stored_profile_pin_hash.is_some() {
            return true;
        }
        let tin = self.tin_input.read(cx).value(cx).to_string();
        let root: String = tin.chars().filter(|c| c.is_ascii_digit()).take(9).collect();
        if root.len() != 9 {
            return false;
        }
        self.db
            .lock()
            .ok()
            .and_then(|db| {
                db.taxpayer_credentials(&root)
                    .ok()
                    .and_then(|credentials| credentials.pin_hash)
            })
            .is_some()
    }
}
