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

        if !connection.profile_calendar_available() {
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
            .when(forms_years.is_empty(), |this| {
                this.child(
                    div()
                        .text_sm()
                        .text_color(cx.theme().danger)
                        .child(
                            "No Forms Set is configured. Add forms in COR > Forms & Elections before creating a calendar.",
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
            .when(!forms_years.is_empty(), |this| {
                this.child(self.render_calendar_form_selection(cx))
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

    fn persist_calendar_form_selection(&mut self, cx: &mut Context<Self>) {
        // The selection was loaded under the persisted TIN; save it back
        // under the same key even if the TIN inputs were edited since.
        let tin = self
            .persisted_profile_tin
            .clone()
            .unwrap_or_else(|| self.tin_input.read(cx).value(cx).to_string());
        let save_result = match self.db.lock() {
            Ok(db) => bir_core::google_calendar::save_calendar_form_selection(
                &db,
                &tin,
                &self.calendar_form_selection,
            ),
            Err(_) => Err(anyhow::anyhow!("Database lock poisoned")),
        };
        if let Err(error) = save_result {
            self.calendar_action_message =
                Some((false, format!("Could not save the form selection: {error}")));
            return;
        }
        let linked = self
            .db
            .lock()
            .ok()
            .and_then(|db| db.get_profile_calendar_link(&tin).ok().flatten())
            .is_some();
        self.calendar_action_message = Some((
            true,
            if linked {
                "Form selection saved. Press Sync Now to apply it to Google Calendar.".to_string()
            } else {
                "Form selection saved. It will apply when the calendar is created.".to_string()
            },
        ));
    }

    fn render_calendar_form_selection(&self, cx: &Context<Self>) -> gpui::AnyElement {
        use bir_core::google_calendar::CalendarFormPreset;

        // Union of active form codes across every configured Forms Set year.
        let mut codes: Vec<String> = self
            .stored_per_year_forms
            .values()
            .flat_map(|set| set.entries.iter())
            .filter(|entry| entry.is_filing_active())
            .map(|entry| entry.form_code.clone())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        codes.sort();

        let selection = &self.calendar_form_selection;
        let preset = selection.preset;
        let any_overrides =
            !selection.manual_include.is_empty() || !selection.manual_exclude.is_empty();

        let preset_button = |id: &'static str,
                             label: &'static str,
                             value: CalendarFormPreset,
                             cx: &Context<Self>| {
            let active = preset == value;
            div()
                .id(id)
                .px_3()
                .py_1()
                .rounded_md()
                .text_xs()
                .cursor_pointer()
                .when(active, |s| {
                    s.bg(cx.theme().primary)
                        .text_color(cx.theme().primary_foreground)
                        .font_weight(FontWeight::SEMIBOLD)
                })
                .when(!active, |s| {
                    s.bg(cx.theme().secondary)
                        .text_color(cx.theme().foreground)
                        .hover(|s| s.bg(cx.theme().muted))
                })
                .on_click(cx.listener(move |this, _, _, cx| {
                    this.calendar_form_selection.set_preset(value);
                    this.persist_calendar_form_selection(cx);
                    cx.notify();
                }))
                .child(label)
        };

        let mut rows = div().flex().flex_col().gap_1();
        for code in &codes {
            let allowed = selection.allows(code);
            let overridden = selection.is_overridden(code);
            let definition = bir_core::forms::registry::find_form(code);
            let title = definition.map(|form| form.title).unwrap_or("Custom form");
            let frequency = definition
                .map(|form| match form.frequency {
                    bir_core::forms::FilingFrequency::Monthly => "Monthly",
                    bir_core::forms::FilingFrequency::Quarterly => "Quarterly",
                    bir_core::forms::FilingFrequency::Annual => "Annual",
                    bir_core::forms::FilingFrequency::OpenEnded => "Event-based",
                })
                .unwrap_or("Unknown frequency");
            let code_for_toggle = code.clone();
            rows = rows.child(
                div()
                    .id(SharedString::from(format!("calendar-form-{code}")))
                    .flex()
                    .items_center()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .rounded_md()
                    .cursor_pointer()
                    .hover(|s| s.bg(cx.theme().muted))
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.calendar_form_selection.toggle(&code_for_toggle);
                        this.persist_calendar_form_selection(cx);
                        cx.notify();
                    }))
                    .child(
                        div()
                            .w_4()
                            .h_4()
                            .rounded_sm()
                            .border_1()
                            .border_color(cx.theme().border)
                            .bg(if allowed {
                                cx.theme().primary
                            } else {
                                cx.theme().background
                            }),
                    )
                    .child(
                        div()
                            .text_sm()
                            .font_weight(FontWeight::MEDIUM)
                            .text_color(cx.theme().foreground)
                            .child(code.clone()),
                    )
                    .child(
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .flex_1()
                            .min_w_0()
                            .child(format!("{title} · {frequency}")),
                    )
                    .when(overridden, |this| {
                        this.child(
                            div()
                                .text_xs()
                                .px_1p5()
                                .py_0p5()
                                .rounded_sm()
                                .bg(cx.theme().secondary)
                                .text_color(cx.theme().muted_foreground)
                                .child("override"),
                        )
                    }),
            );
        }

        div()
            .flex()
            .flex_col()
            .gap_3()
            .p_3()
            .rounded_lg()
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().secondary.opacity(0.4))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(
                        div()
                            .font_weight(FontWeight::SEMIBOLD)
                            .text_color(cx.theme().foreground)
                            .child("Notified Forms"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(preset_button(
                                "calendar_preset_all",
                                "All forms",
                                CalendarFormPreset::AllForms,
                                cx,
                            ))
                            .child(preset_button(
                                "calendar_preset_recurring",
                                "Recurring only",
                                CalendarFormPreset::RecurringOnly,
                                cx,
                            )),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Checked forms create deadline events and reminders on the Google calendar. The preset sets the default; clicking a form records a manual override. Changes apply on the next sync.",
                    ),
            )
            .child(rows)
            .when(any_overrides, |this| {
                this.child(
                    gpui_component::button::Button::new("clear_calendar_form_overrides")
                        .label("Clear overrides")
                        .ghost()
                        .small()
                        .on_click(cx.listener(|this, _, _, cx| {
                            this.calendar_form_selection.clear_overrides();
                            this.persist_calendar_form_selection(cx);
                            cx.notify();
                        })),
                )
            })
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
