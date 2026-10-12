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

use crate::app::{ActiveView, AppState, ProfileTargetAction};
use bir_core::profile::TaxpayerProfile;

/// Height of one sidebar menu row (full width).
const MENU_ROW_HEIGHT: f32 = 32.;
/// Size of one round sidebar menu button (mini sidebar).
const MENU_MINI_SIZE: f32 = 36.;
/// The profile list keeps room for this many profiles (4 since the
/// Notifications and theme rows moved to the top strip).
const MIN_VISIBLE_PROFILES: usize = 4;
/// One full-width profile row: `py_2` plus a name and a TIN line.
const PROFILE_ROW_HEIGHT: f32 = 8. + 24. + 20. + 8.;
/// Gap between profile rows (`gap_2`) and the list's top padding (`pt_2`).
const PROFILE_ROW_GAP: f32 = 8.;
const PROFILE_LIST_TOP_PAD: f32 = 8.;

/// Minimum height of the profile list that shows `rows` profiles whole.
pub(crate) fn profile_list_min_height(rows: usize) -> f32 {
    PROFILE_LIST_TOP_PAD
        + rows as f32 * PROFILE_ROW_HEIGHT
        + rows.saturating_sub(1) as f32 * PROFILE_ROW_GAP
}

/// Number of fixed rows in the bottom menu (Import, Background Tasks, Tax
/// Calendars, Settings + Dev chip). Notifications and the theme toggle live
/// in the top strip (`top_bar`).
const MENU_ROWS: usize = 4;

/// Height the full-width sidebar spends on everything except the profile
/// list, with a session active (Logout row shown). Mirrors `render_sidebar`.
pub(crate) fn sidebar_fixed_height() -> f32 {
    let padding = 16. * 2.; // py_4
    let gap = 8.; // gap_2 between the upper sections
    let toggle = 40. + 4.; // toggle button + mb_1
    let logo = 40.; // h_10
    let header = 40.; // "TAXPAYER PROFILES" + add button
    let search = 32.;
    let divider = 1.;
    let logout = MENU_ROW_HEIGHT;
    let menu = MENU_ROWS as f32 * MENU_ROW_HEIGHT + (MENU_ROWS - 1) as f32 * 4.; // gap_1
    // Sections in the upper column: (toggle+logo), header, search, divider,
    // list, logout, divider -> 6 gaps.
    padding + toggle + logo + header + search + divider + logout + divider + 6. * gap + menu
}

impl AppState {
    /// Small orange "Dev" chip beside Settings. Debug builds only; release
    /// builds render nothing.
    fn render_dev_mode_chip(&self, is_mini: bool, cx: &mut Context<Self>) -> Option<AnyElement> {
        #[cfg(debug_assertions)]
        {
            let orange = crate::dev_mode::dev_orange();
            let active = self.active_view == ActiveView::DevMode;
            Some(
                div()
                    .id("dev_mode_sidebar_btn")
                    .flex()
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .gap_1()
                    .h(px(if is_mini { 24. } else { MENU_ROW_HEIGHT }))
                    .px_2()
                    .rounded_full()
                    .cursor_pointer()
                    .border_1()
                    .border_color(orange)
                    .when(active, |this| this.bg(orange.opacity(0.25)))
                    .hover(move |s| s.bg(orange.opacity(0.15)))
                    .on_click(cx.listener(|this, _ev, window, cx| {
                        this.open_dev_mode(window, cx);
                    }))
                    .when(!is_mini, |this| {
                        this.child(
                            Icon::new(IconName::SquareTerminal)
                                .size(px(14.))
                                .text_color(orange),
                        )
                    })
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(orange)
                            .child("Dev"),
                    )
                    .into_any_element(),
            )
        }
        #[cfg(not(debug_assertions))]
        {
            let _ = (is_mini, cx);
            None
        }
    }

    /// Writes the archived flag for one profile. Callers must already have
    /// cleared the administrator gate - `AppState::request_profile_lifecycle`
    /// is the only supported entry point.
    pub(crate) fn persist_profile_archived(
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
                id={crate::agent::ids::SIDEBAR}
                w={if is_mini { px(72.) } else { px(280.) }}
                h_full
                bg={cx.theme().background}
                border_r_1
                border_color={cx.theme().border}
                when={(!is_mini, |this| this.py_4())}
                when={(is_mini, |this| this.py_3())}
                flex
                flex_col
                justify_between
            >
                {div()
                    .flex()
                    .flex_col()
                    .gap_2()
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
                                mb_1
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
                    .child(rsx! {
                        <div
                            w_full
                            h_px
                            bg={cx.theme().border}
                        />
                    })
                    .child(
                        v_flex()
                            .id(crate::agent::ids::SIDEBAR_PROFILE_LIST)
                            .flex_1()
                            .when(!is_mini, |this| {
                                this.flex_shrink_0()
                                    .min_h(px(profile_list_min_height(MIN_VISIBLE_PROFILES)))
                            })
                            .overflow_y_scroll()
                            .track_scroll(&self.sidebar_scroll)
                            .pt_2()
                            .gap_2()
                            .children(filtered_profiles.iter().map(|profile| {
                                let is_active =
                                    self.active_profile_tin.as_ref() == Some(&profile.tin.full());
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

                                div()
                                    .id(crate::agent::ids::profile_row(&profile.tin.full()))
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
                                    .on_click(cx.listener({
                                        let profile_clone = profile.clone();
                                        move |this, _ev, window, cx| {
                                            // A profile row is a destination, not a
                                            // disclosure. Selecting one opens its
                                            // dashboard - the calendar and tax form
                                            // library - at every width. An archived
                                            // profile has no dashboard worth showing,
                                            // so it opens its editor instead, which is
                                            // where restore and delete now live.
                                            let action = if profile_clone.is_archived {
                                                ProfileTargetAction::EditProfile
                                            } else {
                                                ProfileTargetAction::ViewDashboard
                                            };
                                            this.select_profile(
                                                profile_clone.clone(),
                                                action,
                                                window,
                                                cx,
                                            );
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
                                        items_center
                                        gap_3
                                        mx_6
                                        px_3
                                        h={px(MENU_ROW_HEIGHT)}
                                        rounded_md
                                        cursor_pointer
                                        bg={gpui::rgba(0xef444420)}
                                        hover={|s| s.bg(gpui::rgba(0xef444440))}
                                        on_click={cx.listener(|this, _, window, cx| {
                                            this.logout(window, cx);
                                        })}
                                    >
                                        <svg src="svg/power.svg" size={px(18.)} text_color={Hsla::from(gpui::rgba(0xef4444ff))}/>
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
                    gap_1
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
                                this.justify_center().size(px(MENU_MINI_SIZE)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h(px(MENU_ROW_HEIGHT)).px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .when(self.active_view == ActiveView::ImportExport, |this| this.bg(cx.theme().accent))
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
                            .id("cron_tasks_sidebar_btn")
                            .flex()
                            .items_center()
                            .w_full()
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(MENU_MINI_SIZE)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h(px(MENU_ROW_HEIGHT)).px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .when(self.active_view == ActiveView::CronTasks, |this| this.bg(cx.theme().accent))
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
                                this.justify_center().size(px(MENU_MINI_SIZE)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h(px(MENU_ROW_HEIGHT)).px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .when(self.active_view == ActiveView::AdminCalendarDashboard, |this| this.bg(cx.theme().accent))
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.request_admin_access(ActiveView::AdminCalendarDashboard, window, cx);
                            }))
                            .child(Icon::new(IconName::Calendar).size(px(20.)).text_color(cx.theme().foreground))
                            .when(!is_mini, |this| {
                                this.child(rsx! { <div text_sm text_color={cx.theme().foreground}>{"Tax Calendars"}</div> })
                            })
                    }
                    {div()
                        .flex()
                        .w_full()
                        .gap_1()
                        .when(is_mini, |this| this.flex_col().items_center())
                        .when(!is_mini, |this| this.items_center())
                        .child(div()
                            .id("settings_sidebar_btn")
                            .flex()
                            .items_center()
                            .when(!is_mini, |this| this.flex_1())
                            .cursor_pointer()
                            .hover(|s| s.bg(cx.theme().muted))
                            .when(is_mini, |this| {
                                this.justify_center().size(px(MENU_MINI_SIZE)).flex_shrink_0().rounded_full().bg(cx.theme().secondary)
                            })
                            .when(!is_mini, |this| {
                                this.justify_start().h(px(MENU_ROW_HEIGHT)).px_3().gap_3().rounded_md().bg(cx.theme().secondary)
                            })
                            .when(self.active_view == ActiveView::Settings, |this| this.bg(cx.theme().accent))
                            .on_click(cx.listener(|this, _ev, window, cx| {
                                this.request_admin_access(ActiveView::Settings, window, cx);
                            }))
                            .child(Icon::new(IconName::Settings).size(px(20.)).text_color(cx.theme().foreground))
                            .when(!is_mini, |this| {
                                this.child(rsx! { <div text_sm text_color={cx.theme().foreground}>{"Settings"}</div> })
                            })
                        )
                        .children(self.render_dev_mode_chip(is_mini, cx))
                    }
                </div>
            </div>
        }
    }
}

#[cfg(test)]
mod layout_tests {
    use super::{
        MENU_MINI_SIZE, MENU_ROW_HEIGHT, MENU_ROWS, MIN_VISIBLE_PROFILES, PROFILE_ROW_GAP,
        PROFILE_ROW_HEIGHT, profile_list_min_height, sidebar_fixed_height,
    };
    use crate::app::ActiveView;

    /// Default window height before F10A sizing (and the smallest we target).
    const DEFAULT_HEIGHT: f32 = 768.;

    #[test]
    fn min_visible_profiles_fit_at_default_height() {
        let room = DEFAULT_HEIGHT - sidebar_fixed_height();
        assert!(
            room >= profile_list_min_height(MIN_VISIBLE_PROFILES),
            "profile list gets {room}px, needs {}",
            profile_list_min_height(MIN_VISIBLE_PROFILES)
        );
    }

    #[test]
    fn min_height_counts_rows_and_gaps() {
        assert_eq!(profile_list_min_height(1), 8. + PROFILE_ROW_HEIGHT);
        assert_eq!(
            profile_list_min_height(3),
            8. + 3. * PROFILE_ROW_HEIGHT + 2. * PROFILE_ROW_GAP
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    fn dev_mode_is_an_active_view() {
        let view = ActiveView::DevMode;
        assert_ne!(view, ActiveView::Settings);
        #[cfg(feature = "agent")]
        assert_eq!(
            crate::agent::ids::view_from_slug(crate::agent::ids::view_slug(view)),
            Some(view)
        );
    }

    #[test]
    fn menu_has_four_rows_without_notifications_or_theme() {
        assert_eq!(MENU_ROWS, 4);
        assert_eq!(MIN_VISIBLE_PROFILES, 4);
        // Menu: 4 rows and 3 gaps.
        let menu = 4. * MENU_ROW_HEIGHT + 3. * 4.;
        let room = DEFAULT_HEIGHT - sidebar_fixed_height();
        assert_eq!(room, DEFAULT_HEIGHT - 270. - menu);
    }

    #[test]
    fn menu_rows_are_slimmer_than_before() {
        const { assert!(MENU_ROW_HEIGHT < 40.) };
        const { assert!(MENU_MINI_SIZE < 48.) };
    }
}
