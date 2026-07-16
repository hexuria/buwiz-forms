//! Per-profile Google Calendar creation and synchronization.

use super::*;

impl ProfileManagerView {
    pub(super) fn render_calendar_tab(&self, cx: &Context<Self>) -> gpui::AnyElement {
        if self.active_tab != 6 || self.editing_id.is_none() {
            return div().into_any_element();
        }

        let tin = self.tin_input.read(cx).value(cx).to_string();
        let (link, forms_years, connection) = self
            .db
            .lock()
            .ok()
            .map(|db| {
                (
                    db.get_profile_calendar_link(&tin).ok().flatten(),
                    db.list_forms_set_years(&tin).unwrap_or_default(),
                    bir_core::google_calendar::google_calendar_connection_from_db(&db),
                )
            })
            .unwrap_or_default();
        let linked = link.is_some();

        div()
            .p_4()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .flex()
            .flex_col()
            .gap_5()
            .w_full()
            .min_w_0()
            .child(
                div()
                    .flex()
                    .justify_between()
                    .items_start()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child("Google Filing Calendar"),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(
                                        "Create a separately toggleable Google calendar from this profile's Forms Sets.",
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .text_sm()
                            .text_color(if linked {
                                cx.theme().success
                            } else {
                                cx.theme().muted_foreground
                            })
                            .child(if linked { "Linked" } else { "Not linked" }),
                    ),
            )
            .when(!connection.configured, |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(
                            "Google OAuth is not configured in this build. Configure it in Settings and rebuild.",
                        ),
                )
            })
            .when(connection.configured && connection.connected_email.is_none(), |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child("Connect a Google account in Settings first."),
                )
            })
            .when(forms_years.is_empty(), |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(
                            "No Forms Set is configured. Add forms in the Forms Set tab before creating a calendar.",
                        ),
                )
            })
            .when_some(connection.connected_email.clone(), |this, email| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().muted_foreground)
                        .child(format!("Google account: {email}")),
                )
            })
            .when_some(link.clone(), |this, link| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_1()
                        .child(
                            div()
                                .font_weight(FontWeight::MEDIUM)
                                .child(link.calendar_name),
                        )
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!(
                                    "Years: {} · Last sync: {}",
                                    forms_years
                                        .iter()
                                        .map(u16::to_string)
                                        .collect::<Vec<_>>()
                                        .join(", "),
                                    link.last_synced_at
                                        .unwrap_or_else(|| "Not synced yet".to_string())
                                )),
                        )
                        .when_some(link.last_error, |this, error| {
                            this.child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().danger)
                                    .child(format!("Last error: {error}")),
                            )
                        }),
                )
            })
            .when(!linked, |this| {
                this.child(
                    div()
                        .flex()
                        .flex_col()
                        .gap_2()
                        .child(Self::field_label("Calendar Name", cx))
                        .child(Input::new(&self.calendar_name_input)),
                )
            })
            .when_some(self.calendar_action_message.clone(), |this, message| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(if message.0 {
                            cx.theme().success
                        } else {
                            cx.theme().danger
                        })
                        .child(message.1),
                )
            })
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .when(!linked, |this| {
                        this.child(
                            gpui_component::button::Button::new("create_profile_google_calendar")
                                .label("Create Google Calendar")
                                .disabled(
                                    !connection.configured
                                        || connection.connected_email.is_none()
                                        || forms_years.is_empty(),
                                )
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let tin = this.tin_input.read(cx).value(cx).to_string();
                                    let name =
                                        this.calendar_name_input.read(cx).value().to_string();
                                    let db = this.db.clone();
                                    this.calendar_action_message =
                                        Some((true, "Creating and syncing calendar...".into()));
                                    cx.notify();
                                    Self::run_calendar_action(cx, move || {
                                        bir_core::google_calendar::create_profile_calendar(
                                            db,
                                            &tin,
                                            Some(&name),
                                        )
                                        .map(|report| {
                                            format!(
                                                "Calendar created: {} events added, {} undated forms excluded.",
                                                report.inserted, report.excluded_undated
                                            )
                                        })
                                    });
                                })),
                        )
                    })
                    .when(linked, |this| {
                        this.child(
                            gpui_component::button::Button::new("sync_profile_google_calendar")
                                .label("Sync Now")
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let tin = this.tin_input.read(cx).value(cx).to_string();
                                    let db = this.db.clone();
                                    this.calendar_action_message =
                                        Some((true, "Synchronizing...".into()));
                                    cx.notify();
                                    Self::run_calendar_action(cx, move || {
                                        bir_core::google_calendar::sync_profile_calendar(db, &tin)
                                            .map(|report| {
                                                format!(
                                                    "Sync complete: {} added, {} updated, {} removed, {} unchanged.",
                                                    report.inserted,
                                                    report.updated,
                                                    report.deleted,
                                                    report.unchanged
                                                )
                                            })
                                    });
                                })),
                        )
                        .child(
                            gpui_component::button::Button::new("open_google_calendar")
                                .label("Open Google Calendar")
                                .ghost()
                                .on_click(|_, _, _| {
                                    let _ =
                                        open::that("https://calendar.google.com/calendar/u/0/r");
                                }),
                        )
                        .child(
                            gpui_component::button::Button::new("unlink_profile_google_calendar")
                                .label("Unlink")
                                .ghost()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let tin = this.tin_input.read(cx).value(cx).to_string();
                                    let db = this.db.clone();
                                    Self::run_calendar_action(cx, move || {
                                        bir_core::google_calendar::unlink_profile_calendar(db, &tin)
                                            .map(|_| {
                                                "Calendar unlinked; the Google calendar was preserved."
                                                    .to_string()
                                            })
                                    });
                                })),
                        )
                        .child(
                            gpui_component::button::Button::new("delete_profile_google_calendar")
                                .label("Delete Calendar")
                                .danger()
                                .on_click(cx.listener(|this, _, _, cx| {
                                    let tin = this.tin_input.read(cx).value(cx).to_string();
                                    let db = this.db.clone();
                                    cx.spawn(async move |this, cx| {
                                        let confirmed = rfd::AsyncMessageDialog::new()
                                            .set_title("Delete Google Calendar")
                                            .set_description(
                                                "Permanently delete this profile calendar and all its managed deadline events?",
                                            )
                                            .set_buttons(rfd::MessageButtons::YesNo)
                                            .show()
                                            .await;
                                        if !matches!(
                                            confirmed,
                                            rfd::MessageDialogResult::Yes
                                        ) {
                                            return;
                                        }
                                        let (tx, rx) = tokio::sync::oneshot::channel();
                                        std::thread::spawn(move || {
                                            let result =
                                                bir_core::google_calendar::delete_profile_calendar(
                                                    db, &tin,
                                                )
                                                .map(|_| "Google calendar deleted.".to_string());
                                            let _ = tx.send(result);
                                        });
                                        let result = rx.await.unwrap_or_else(|_| {
                                            Err(anyhow::anyhow!("Calendar worker stopped"))
                                        });
                                        let _ = this.update(cx, |this, cx| {
                                            this.calendar_action_message = Some(match result {
                                                Ok(message) => (true, message),
                                                Err(error) => (false, error.to_string()),
                                            });
                                            cx.notify();
                                        });
                                    })
                                    .detach();
                                })),
                        )
                    }),
            )
            .into_any_element()
    }

    fn run_calendar_action<F>(cx: &mut Context<Self>, operation: F)
    where
        F: FnOnce() -> Result<String, anyhow::Error> + Send + 'static,
    {
        cx.spawn(async move |this, cx| {
            let (tx, rx) = tokio::sync::oneshot::channel();
            std::thread::spawn(move || {
                let _ = tx.send(operation());
            });
            let result = rx
                .await
                .unwrap_or_else(|_| Err(anyhow::anyhow!("Calendar worker stopped")));
            let _ = this.update(cx, |this, cx| {
                this.calendar_action_message = Some(match result {
                    Ok(message) => (true, message),
                    Err(error) => (false, error.to_string()),
                });
                cx.notify();
            });
        })
        .detach();
    }
}
