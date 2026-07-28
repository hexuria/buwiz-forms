//! Sidebar rendering for AppState — extracted from app.rs to reduce file size.
//!
//! Contains the `render_sidebar` method which renders the profile list,
//! search filter, archived toggle, navigation buttons, and theme/settings controls.

use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::WindowExt;
use gpui_component::input::Input;
use gpui_component::notification::{Notification, NotificationType};
use gpui_component::*;
use gpui_rsx::rsx;

use crate::app::{ActiveView, AppState, AppThemeMode, ProfileTargetAction};
use bir_core::profile::TaxpayerProfile;

impl AppState {
    fn persist_profile_archived(
        &mut self,
        mut profile: TaxpayerProfile,
        archived: bool,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        profile.is_archived = archived;
        let save_result = match self.db.lock() {
            Ok(db) => db
                .save_profile_with_post_commit_status(profile)
                .map(bir_core::db::PostCommitWrite::into_parts)
                .map_err(|error| error.to_string()),
            Err(_) => Err("The profile database is temporarily unavailable".to_string()),
        };

        match save_result {
            Ok((saved, refresh_status)) => {
                let tin = saved.tin.full();
                if let Some(existing) = self
                    .profiles
                    .iter_mut()
                    .find(|candidate| candidate.tin.full() == tin)
                {
                    *existing = saved;
                }
                if let Some(warning) = refresh_status.warning() {
                    window.push_notification(
                        Notification::new()
                            .message(format!(
                                "Profile {}. {warning}",
                                if archived { "archived" } else { "restored" }
                            ))
                            .with_type(NotificationType::Warning)
                            .autohide(false),
                        cx,
                    );
                }
                cx.notify();
            }
            Err(error) => {
                window.push_notification(
                    Notification::new()
                        .message(format!(
                            "Profile was not {}: {error}",
                            if archived { "archived" } else { "restored" }
                        ))
                        .with_type(NotificationType::Error)
                        .autohide(false),
                    cx,
                );
            }
        }
    }

    pub(crate) fn render_sidebar(
        &self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let is_mini = self.is_mini_sidebar || window.viewport_size().width < px(768.);
        let filter = self.profile_filter.read(cx).value().to_lowercase();
        let mut archived_count = 0;

        for p in &self.profiles {
            if p.is_archived {
                archived_count += 1;
            }
        }

        let filtered_profiles: Vec<TaxpayerProfile> = self
            .profiles
            .iter()
            .filter(|profile| {
                if self.show_archived != profile.is_archived {
                    return false;
                }
                if self.hide_tax_profiles {
                    let is_active_session =
                        self.active_session_tin.as_ref() == Some(&profile.tin.full());
                    // When filter is empty, show only the active session profile
                    if filter.trim().is_empty() {
                        return is_active_session;
                    }
                    // When typing, only exact full TIN match (digits or formatted) reveals a profile
                    let is_exact_tin_match = filter.trim() == profile.tin.full().to_lowercase()
                        || filter.trim() == profile.tin.formatted().to_lowercase();
                    is_active_session || is_exact_tin_match
                } else {
                    filter.trim().is_empty()
                        || profile
                            .full_name
                            .to_lowercase()
                            .contains(&filter.trim().to_lowercase())
                        || profile.tin.full().contains(filter.trim())
                        || profile.tin.formatted().contains(filter.trim())
                }
            })
            .cloned()
            .collect();
        rsx! {
            <div
                w={if is_mini { px(72.) } else { px(280.) }}
                h_full
                bg={cx.theme().background}
                border_r_1
                border_color={cx.theme().border}
                when={(!is_mini, |this| this.py_6())}
                when={(is_mini, |this| this.py_3())}
                flex
                flex_col
                justify_between
            >
                {div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .flex_1()
                    .overflow_hidden()
                    .child(rsx! {
                        <div
                            flex
                            flex_col
                            w_full
                            when={(!is_mini, |this| this.px_6())}
                            when={(is_mini, |this| this.px_3())}
                        >
                            <div
                                flex
                                w_full
                                when={(!is_mini, |this| this.justify_end())}
                                when={(is_mini, |this| this.justify_center())}
                                mb={px(10.)}
                            >
                                <div
                                    id="sidebar_toggle_btn"
                                    flex
                                    items_center
                                    justify_center
                                    size={px(40.)}
                                    flex_shrink_0
                                    cursor_pointer
                                    hover={|s| s.bg(cx.theme().muted).rounded_md()}
                                    on_click={cx.listener(|this, _ev, _window, cx| {
                                        this.is_mini_sidebar = !this.is_mini_sidebar;
                                        this.dashboard_view.update(cx, |view, cx| {
                                            view.set_mini_sidebar(this.is_mini_sidebar, cx);
                                        });
                                        cx.notify();
                                    })}
                                >
                                    {Icon::new(if is_mini { IconName::ChevronRight } else { IconName::ChevronLeft })
                                        .size(px(28.))
                                        .text_color(cx.theme().foreground)}
                                </div>
                            </div>
                            <div
                                id="global_dashboard_btn"
                                w_full
                                cursor_pointer
                                on_click={cx.listener(|this, _ev, window, cx| {
                                    if this.block_unsaved_compliance_navigation(window, cx) {
                                        return;
                                    }
                                    this.active_view = ActiveView::GlobalDashboard;
                                    this.active_profile_tin = None;
                                    cx.notify();
                                })}
                            >
                                {if is_mini {
                                    gpui::img("svg/e_logo.svg")
                                        .w_full()
                                        .h_8()
                                        .object_fit(gpui::ObjectFit::Contain)
                                } else {
                                    gpui::img("svg/ebirforms.png")
                                        .w_full()
                                        .h_10()
                                        .object_fit(gpui::ObjectFit::Contain)
                                }}
                            </div>
                        </div>
                    })
                    .child(rsx! {
                        <div
                            flex
                            items_center
                            justify_between
                            when={(!is_mini, |this| this.px_6())}
                            when={(is_mini, |this| this.px_3())}
                            when={(!is_mini, |this| {
                                this.child(rsx! {
                                    <div
                                        text_xs
                                        font_weight={FontWeight::BOLD}
                                        text_color={cx.theme().muted_foreground}
                                    >
                                        {"TAXPAYER PROFILES"}
                                    </div>
                                })
                            })}
                            when={(is_mini, |this| this.justify_center())}
                        >
                            <div
                                id="add_profile_mini_btn"
                                flex
                                items_center
                                justify_center
                                size={px(40.)}
                                flex_shrink_0
                                cursor_pointer
                                hover={|s| s.bg(cx.theme().muted).rounded_md()}
                                on_click={cx.listener(|this, _ev, window, cx| {
                                    if this.block_unsaved_compliance_navigation(window, cx) {
                                        return;
                                    }
                                    // End any active session when creating a new profile
                                    this.active_session_tin = None;
                                    this.active_view = ActiveView::ProfileManager;
                                    this.active_profile_tin = None;
                                    this.profile_manager.update(cx, |view, cx| {
                                        view.reset_for_new(window, cx);
                                    });
                                    cx.notify();
                                })}
                            >
                                <div
                                    text_color={cx.theme().foreground}
                                    font_weight={FontWeight::BOLD}
                                    text_xl
                                >
                                    {"+"}
                                </div>
                            </div>
                        </div>
                    })
                    .child(
                        if is_mini {
                            let root = rsx! {
                                <div px_3 flex justify_center w_full>
                                    <div
                                        id="sidebar_search_mini_btn"
                                        flex
                                        items_center
                                        justify_center
                                        size={px(48.)}
                                        rounded_full
                                        bg={cx.theme().secondary}
                                        flex_shrink_0
                                        cursor_pointer
                                        hover={|s| s.bg(cx.theme().muted)}
                                        on_click={cx.listener(|this, _ev, window, cx| {
                                            this.is_mini_sidebar = false;
                                            this.dashboard_view.update(cx, |view, cx| {
                                                view.set_mini_sidebar(this.is_mini_sidebar, cx);
                                            });
                                            cx.focus_view(&this.profile_filter, window);
                                            cx.notify();
                                        })}
                                    >
                                        {Icon::new(IconName::Search)
                                            .size(px(20.))
                                            .text_color(cx.theme().foreground)}
                                    </div>
                                </div>
                            };
                            root.into_any_element()
                        } else {
                            let root = rsx! {
                                <div px_6 w_full>{Input::new(&self.profile_filter)}</div>
                            };
                            root.into_any_element()
                        }
                    )
                    .child(rsx! { <div h_2/> })
                    .child(rsx! {
                        <div
                            w_full
                            h_px
                            bg={cx.theme().border}
                        />
                    })
                    .child(
                        v_flex()
                            .id("sidebar-profile-list")
                            .flex_1()
                            .overflow_y_scroll()
                            .track_scroll(&self.sidebar_scroll)
                            .pt_2()
                            .gap_2()
                            .children(filtered_profiles.iter().map(|profile| {
                                let is_active =
                                    self.active_profile_tin.as_ref() == Some(&profile.tin.full());
                                let is_expanded =
                                    self.expanded_profile_tin.as_ref() == Some(&profile.tin.full());
                                let bg_color = if is_active {
                                    cx.theme().accent
                                } else {
                                    gpui::rgba(0x00000000).into()
                                };
                                let border_color = if is_active {
                                    cx.theme().border
                                } else {
                                    gpui::rgba(0x00000000).into()
                                };

                                let rdo_desc = bir_core::reference::get_rdo(&profile.rdo_code)
                                    .map(|r| r.description.clone())
                                    .unwrap_or_else(|| "Unknown".to_string());

                                div()
                                    .id(profile.tin.full())
                                    .w_full()
                                    .when(!is_mini, |this| this.py_2().px_6())
                                    .when(is_mini, |this| this.py_2().px_3())
                                    .bg(bg_color)
                                    .border_1()
                                    .border_color(border_color)
                                    .flex()
                                    .flex_col()
                                    .cursor_pointer()
                                    .hover(|s| {
                                        if !is_active {
                                            s.bg(cx.theme().muted)
                                        } else {
                                            s
                                        }
                                    })
                                    .child(rsx! {
                                        <div
                                            flex
                                            items_center
                                            gap_3
                                            when={(is_expanded && !is_mini, |d| d.mb_4())}
                                            when={(is_mini, |this| this.justify_center())}
                                            when={(is_mini, |this| {
                                                let initials = profile.full_name
                                                    .split_whitespace()
                                                    .filter_map(|w| w.chars().next())
                                                    .take(2)
                                                    .collect::<String>()
                                                    .to_uppercase();

                                                this.child(rsx! {
                                                    <div
                                                        size={px(48.)}
                                                        flex_shrink_0
                                                        rounded_full
                                                        bg={cx.theme().secondary}
                                                        flex
                                                        items_center
                                                        justify_center
                                                    >
                                                        <div
                                                            text_sm
                                                            text_color={cx.theme().primary}
                                                            font_weight={FontWeight::BOLD}
                                                        >
                                                            {initials}
                                                        </div>
                                                    </div>
                                                })
                                            })}
                                            when={(!is_mini, |this| {
                                                this.child(rsx! {
                                                    <div flex flex_col>
                                                        <div
                                                            font_weight={FontWeight::BOLD}
                                                            text_color={if profile.is_archived { cx.theme().muted_foreground } else { cx.theme().foreground }}
                                                        >
                                                            {if profile.is_archived { format!("{} (Archived)", profile.full_name) } else { profile.full_name.clone() }}
                                                        </div>
                                                        <div
                                                            text_sm
                                                            text_color={cx.theme().muted_foreground}
                                                        >
                                                            {format!("TIN: {}", profile.tin.full())}
                                                        </div>
                                                    </div>
                                                })
                                            })}
                                        />
                                    })
                                    .when(is_expanded && !is_mini, |this| {
                                        this.child(rsx! {
                                            <div flex flex_col gap_2>
                                                <div flex justify_between>
                                                    <div text_sm text_color={cx.theme().muted_foreground}>{"RDO:"}</div>
                                                    <div
                                                        text_sm
                                                        font_weight={FontWeight::BOLD}
                                                        text_color={cx.theme().foreground}
                                                    >
                                                        {format!("{} - {}", profile.rdo_code, rdo_desc)}
                                                    </div>
                                                </div>
                                                <div
                                                    flex
                                                    justify_between
                                                    gap_2
                                                    mt_2
                                                    when={(!profile.is_archived, |this| {
                                                            let requires_pin = self.enable_profile_pins && self.active_session_tin.as_ref() != Some(&profile.tin.full());
                                                            this.when(requires_pin, |this| {
                                                                this.child(rsx! {
                                                                    <div
                                                                        id={format!("login_{}", profile.tin.full())}
                                                                        flex
                                                                        items_center
                                                                        justify_center
                                                                        gap_2
                                                                        w_full
                                                                        py_1
                                                                        rounded_md
                                                                        bg={cx.theme().primary}
                                                                        text_color={cx.theme().primary_foreground}
                                                                        cursor_pointer
                                                                        hover={|s| s.opacity(0.8)}
                                                                        on_click={cx.listener({
                                                                            let profile_clone = profile.clone();
                                                                            move |this, _ev, window, cx| {
                                                                                cx.stop_propagation();
                                                                                this.select_profile(profile_clone.clone(), ProfileTargetAction::ViewDashboard, window, cx);
                                                                            }
                                                                        })}
                                                                    >
                                                                        <svg src="svg/lock.svg" size={px(14.)} text_color={cx.theme().primary_foreground}/>
                                                                        <div text_sm font_weight={FontWeight::BOLD}>{"Unlock Profile"}</div>
                                                                    </div>
                                                                })
                                                            })
                                                            .when(!requires_pin, |this| {
                                                                this.child(
                                                                    gpui_component::button::Button::new(format!("view_{}", profile.tin.full()))
                                                                        .small()
                                                                        .label("View")
                                                                        .on_click(cx.listener({
                                                                            let profile_clone = profile.clone();
                                                                            move |this, _ev, window, cx| {
                                                                                cx.stop_propagation();
                                                                                this.select_profile(profile_clone.clone(), ProfileTargetAction::ViewDashboard, window, cx);
                                                                            }
                                                                        })),
                                                                )
                                                                .child(
                                                                    gpui_component::button::Button::new(format!("edit_{}", profile.tin.full()))
                                                                        .small()
                                                                        .label("Edit")
                                                                        .on_click(cx.listener({
                                                                            let profile_clone = profile.clone();
                                                                            move |this, _ev, window, cx| {
                                                                                cx.stop_propagation();
                                                                                this.select_profile(profile_clone.clone(), ProfileTargetAction::EditProfile, window, cx);
                                                                            }
                                                                        })),
                                                                )
                                                                .child(
                                                                    gpui_component::button::Button::new(format!("archive_{}", profile.tin.full()))
                                                                        .small()
                                                                        .label("Archive")
                                                                        .on_click(cx.listener({
                                                                            let profile_clone = profile.clone();
                                                                            move |this, _ev, window, cx| {
                                                                                cx.stop_propagation();
                                                                                if this.block_unsaved_compliance_navigation(window, cx) {
                                                                                    return;
                                                                                }
                                                                                this.persist_profile_archived(profile_clone.clone(), true, window, cx);
                                                                            }
                                                                        })),
                                                                )
                                                            })
                                                        })}
                                                        when={(profile.is_archived, |this| {
                                                            this.child(
                                                                gpui_component::button::Button::new(format!("restore_{}", profile.tin.full()))
                                                                    .small()
                                                                    .label("Restore")
                                                                    .on_click(cx.listener({
                                                                        let profile_clone = profile.clone();
                                                                        move |this, _ev, window, cx| {
                                                                            cx.stop_propagation();
                                                                            if this.block_unsaved_compliance_navigation(window, cx) {
                                                                                return;
                                                                            }
                                                                            this.persist_profile_archived(profile_clone.clone(), false, window, cx);
                                                                        }
                                                                    })),
                                                            )
                                                            .child(
                                                                gpui_component::button::Button::new(format!("delete_{}", profile.tin.full()))
                                                                    .small()
                                                                    .label("Delete")
                                                                    .on_click(cx.listener({
                                                                        let profile_clone = profile.clone();
                                                                        let tin = profile_clone.tin.full();
                                                                        move |this, _ev, window, cx| {
                                                                            cx.stop_propagation();
                                                                            if this.block_unsaved_compliance_navigation(window, cx) {
                                                                                return;
                                                                            }
                                                                            cx.spawn({
                                                                                let tin = tin.clone();
                                                                                async move |this, cx| {
                                                                                    let timestamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs();
                                                                                    let Some(export_handle) = rfd::AsyncFileDialog::new()
                                                                                        .set_title("Save Profile Archive")
                                                                                        .set_file_name(format!("BIR_Archive_{}_{}.zip", tin, timestamp))
                                                                                        .add_filter("Zip Archive", &["zip"])
                                                                                        .save_file()
                                                                                        .await
                                                                                    else {
                                                                                        return;
                                                                                    };
                                                                                    let export_dir = export_handle.path().to_path_buf();

                                                                                    let deletion_result = this.update(cx, |this, cx| {
                                                                                        let db = this.db.lock().map_err(|_| "Database lock poisoned".to_string())?;
                                                                                        bir_core::export_profile_data(&db, &tin, &export_dir)
                                                                                            .map_err(|error| format!("Profile export failed: {error}"))?;
                                                                                        db.delete_profile(&tin)
                                                                                            .map_err(|error| format!("Profile deletion failed: {error}"))?;
                                                                                        drop(db);

                                                                                        this.profiles.retain(|p| p.tin.full() != tin);
                                                                                        if !this.profiles.iter().any(|p| p.is_archived) {
                                                                                            this.show_archived = false;
                                                                                        }
                                                                                        if this.active_profile_tin.as_ref() == Some(&tin) {
                                                                                            this.active_profile_tin = None;
                                                                                            this.active_view = ActiveView::ProfileManager;
                                                                                        }
                                                                                        if this.expanded_profile_tin.as_ref() == Some(&tin) {
                                                                                            this.expanded_profile_tin = None;
                                                                                        }
                                                                                        cx.notify();
                                                                                        Ok::<(), String>(())
                                                                                    });

                                                                                    match deletion_result {
                                                                                        Ok(Ok(())) => {
                                                                                            rfd::AsyncMessageDialog::new()
                                                                                                .set_title("Profile Exported & Deleted")
                                                                                                .set_description(format!("Saved to {}", export_dir.display()))
                                                                                                .show()
                                                                                                .await;
                                                                                        }
                                                                                        Ok(Err(error)) => {
                                                                                            rfd::AsyncMessageDialog::new()
                                                                                                .set_title("Profile Was Not Deleted")
                                                                                                .set_description(error)
                                                                                                .show()
                                                                                                .await;
                                                                                        }
                                                                                        Err(error) => {
                                                                                            rfd::AsyncMessageDialog::new()
                                                                                                .set_title("Profile Was Not Deleted")
                                                                                                .set_description(error.to_string())
                                                                                                .show()
                                                                                                .await;
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }).detach();
                                                                        }
                                                                    })),
                                                            )
                                                        })}
                                                />
                                            </div>
                                        })
                                    })
                                    .on_click(cx.listener({
                                        let profile_clone = profile.clone();
                                        let tin = profile_clone.tin.full();
                                        move |this, _ev, window, cx| {
                                            if this.is_mini_sidebar || window.viewport_size().width < px(768.) {
                                                this.select_profile(profile_clone.clone(), ProfileTargetAction::ViewDashboard, window, cx);
                                            } else {
                                                if this.expanded_profile_tin.as_ref() == Some(&tin) {
                                                    this.expanded_profile_tin = None;
                                                } else {
                                                    this.expanded_profile_tin = Some(tin.clone());
                                                }
                                            }
                                            cx.notify();
                                        }
                                    }))
                            }))
                            .when(archived_count > 0 || self.show_archived, |this| {
                                this.child(rsx! {
                                    <div
                                        w_full
                                        py_2
                                        when={(!is_mini, |this| this.px_6())}
                                        when={(is_mini, |this| this.px_3())}
                                        flex
                                        justify_center
                                    >
                                        {
                                            if is_mini {
                                                let root = rsx! {
                                                    <div
                                                        id="toggle_archived_mini_btn"
                                                        flex
                                                        items_center
                                                        justify_center
                                                        size={px(48.)}
                                                        flex_shrink_0
                                                        rounded_full
                                                        bg={cx.theme().secondary}
                                                        cursor_pointer
                                                        hover={|s| s.bg(cx.theme().muted)}
                                                        on_click={cx.listener(|this, _, _, cx| {
                                                            this.show_archived = !this.show_archived;
                                                            cx.notify();
                                                        })}
                                                    >
                                                        {Icon::new(if self.show_archived { IconName::EyeOff } else { IconName::Eye }).size(px(20.)).text_color(cx.theme().foreground)}
                                                    </div>
                                                };
                                                root.into_any_element()
                                            } else {
                                                gpui_component::button::Button::new("toggle_archived")
                                                    .small()
                                                    .label(if self.show_archived { "Hide Archived Profiles".to_string() } else { format!("Show {} Archived", archived_count) })
                                                    .on_click(cx.listener(|this, _, _, cx| {
                                                        this.show_archived = !this.show_archived;
                                                        cx.notify();
                                                    }))
                                                    .into_any_element()
                                            }
                                        }
                                    </div>
                                })
                            })
                    )
                    .when(self.active_session_tin.is_some(), |this| {
                        this.child(
                            if is_mini {
                                let root = rsx! {
                                    <div
                                        id="exit_session_mini_btn"
                                        w_full
                                        py_2
                                        px_3
                                        flex
                                        justify_center
                                    >
                                        <div
                                            id="exit_session_mini_circle"
                                            flex
                                            items_center
                                            justify_center
                                            size={px(48.)}
                                            flex_shrink_0
                                            rounded_full
                                            bg={gpui::rgba(0xef444420)}
                                            cursor_pointer
                                            hover={|s| s.bg(gpui::rgba(0xef444440))}
                                            on_click={cx.listener(|this, _, window, cx| {
                                                this.logout(window, cx);
                                            })}
                                        >
                                            <svg src="svg/power.svg" size={px(20.)} text_color={Hsla::from(gpui::rgba(0xef4444ff))}/>
                                        </div>
                                    </div>
                                };
                                root.into_any_element()
                            } else {
                                let root = rsx! {
                                    <div
                                        id="logout_btn_full"
                                        flex
                                        flex_col
                                        items_center
                                        justify_center
                                        gap_2
                                        w_full
                                        py_4
                                        cursor_pointer
                                        hover={|s| s.bg(gpui::rgba(0xef444410))}
                                        on_click={cx.listener(|this, _, window, cx| {
                                            this.logout(window, cx);
                                        })}
                                    >
                                        <div
                                            flex
                                            items_center
                                            justify_center
                                            size={px(48.)}
                                            rounded_full
                                            bg={gpui::rgba(0xef444420)}
                                        >
                                            <svg src="svg/power.svg" size={px(24.)} text_color={Hsla::from(gpui::rgba(0xef4444ff))}/>
                                        </div>
                                        <div
                                            text_sm
                                            font_weight={FontWeight::BOLD}
                                            text_color={Hsla::from(gpui::rgba(0xef4444ff))}
                                        >
                                            {"Logout"}
                                        </div>
                                    </div>
                                };
                                root.into_any_element()
                            }
                        )
                    })
                    .child(rsx! {
                        <div
                            w_full
                            h_px
                            bg={cx.theme().border}
                        />
                    })}
                <div
                    base={v_flex()}
                    gap_2
                    w_full
                    when={(!is_mini, |this| this.px_6())}
                    when={(is_mini, |this| this.px_3())}
                >
                    {div()
                            .id("import_export_sidebar_btn")
                            .flex()
                            .items_center()
                            .w_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(48.)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h_10().px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                if this.block_unsaved_compliance_navigation(window, cx) {
                                    return;
                                }
                                this.active_view = ActiveView::ImportExport;
                                cx.notify();
                            }))
                            .child(Icon::new(IconName::HardDrive).size(px(20.)).text_color(cx.theme().foreground))
                            .when(!is_mini, |this| {
                                this.child(rsx! { <div text_sm text_color={cx.theme().foreground}>{"Import Data"}</div> })
                            })
                    }
                    {div()
                            .id("theme_toggle_sidebar_btn")
                            .flex()
                            .items_center()
                            .w_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(48.)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h_10().px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                crate::theme::cycle_theme(&mut this.theme_preference, &this.db, window, cx);
                                cx.notify();
                            }))
                            .child(match self.theme_preference {
                                AppThemeMode::Dark => Icon::new(IconName::Moon).size(px(20.)).text_color(cx.theme().foreground),
                                AppThemeMode::Light => Icon::new(IconName::Sun).size(px(20.)).text_color(cx.theme().foreground),
                                AppThemeMode::System => Icon::new(IconName::Settings).size(px(20.)).text_color(cx.theme().foreground),
                            })
                            .when(!is_mini, |this| {
                                this.child(rsx! {
                                    <div text_sm text_color={cx.theme().foreground}>
                                        {match self.theme_preference {
                                            AppThemeMode::Dark => "Dark Mode",
                                            AppThemeMode::Light => "Light Mode",
                                            AppThemeMode::System => "System Theme",
                                        }}
                                    </div>
                                })
                            })
                    }
                    {div()
                            .id("cron_tasks_sidebar_btn")
                            .flex()
                            .items_center()
                            .w_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(48.)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h_10().px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.request_admin_access(ActiveView::CronTasks, window, cx);
                            }))
                            .child(Icon::new(IconName::SquareTerminal).size(px(20.)).text_color(cx.theme().foreground))
                            .when(!is_mini, |this| {
                                this.child(rsx! { <div text_sm text_color={cx.theme().foreground}>{"Background Tasks"}</div> })
                            })
                    }
                    {div()
                            .id("admin_calendar_sidebar_btn")
                            .flex()
                            .items_center()
                            .w_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(48.)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h_10().px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.request_admin_access(ActiveView::AdminCalendarDashboard, window, cx);
                            }))
                            .child(Icon::new(IconName::Calendar).size(px(20.)).text_color(cx.theme().foreground))
                            .when(!is_mini, |this| {
                                this.child(rsx! { <div text_sm text_color={cx.theme().foreground}>{"Tax Calendars"}</div> })
                            })
                    }
                    {div()
                            .id("settings_sidebar_btn")
                            .flex()
                            .items_center()
                            .w_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(48.)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h_10().px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.request_admin_access(ActiveView::Settings, window, cx);
                            }))
                            .child(Icon::new(IconName::Settings).size(px(20.)).text_color(cx.theme().foreground))
                            .when(!is_mini, |this| {
                                this.child(rsx! { <div text_sm text_color={cx.theme().foreground}>{"Settings"}</div> })
                            })
                    }
                </div>
            </div>
        }
    }
}
