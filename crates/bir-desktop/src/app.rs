use crate::views::dashboard::{DashboardEvent, DashboardView};
use crate::views::form_2551q_view::{Form2551QEvent, Form2551QView};
use crate::views::profile_manager::ProfileManagerView;
use crate::views::global_dashboard::GlobalDashboardView;
use gpui::prelude::FluentBuilder;
use gpui::*;
use gpui_component::input::{Input, InputEvent, InputState};
use gpui_component::*;

use bir_core::db::Database;
use bir_core::forms::form_2551q::Form2551QDraft;
use bir_core::profile::TaxpayerProfile;
use std::sync::{Arc, Mutex};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ActiveView {
    GlobalDashboard,
    Dashboard,
    Form2551Q,
    SavedForms,
    SubmissionHistory,
    ProfileManager,
    Settings,
}

pub struct AppState {
    active_view: ActiveView,
    profile_manager: Entity<ProfileManagerView>,
    dashboard_view: Entity<DashboardView>,
    global_dashboard_view: Entity<GlobalDashboardView>,
    form_2551q_view: Option<Entity<Form2551QView>>,
    pending_form_draft: Option<Form2551QDraft>,
    db: Arc<Mutex<Database>>,
    profiles: Vec<TaxpayerProfile>,
    active_profile_tin: Option<String>,
    expanded_profile_tin: Option<String>,
    profile_filter: Entity<InputState>,
    sidebar_scroll: ScrollHandle,
    _subscriptions: Vec<Subscription>,
}

impl AppState {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let db_path = app_database_path();
        if let Some(parent) = db_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        let legacy_db_path = std::env::current_dir()
            .unwrap_or_default()
            .join("bir_data.db");
        if !db_path.exists() && legacy_db_path.exists() && legacy_db_path != db_path {
            let _ = std::fs::copy(&legacy_db_path, &db_path);
        }
        let (db, recovered_backup) =
            Database::open_or_recreate(&db_path).expect("Failed to open database");
        if let Some(backup_path) = recovered_backup {
            eprintln!(
                "Recovered unreadable database at {} by moving it to {}",
                db_path.display(),
                backup_path.display()
            );
        }
        let profiles = db.list_profiles().unwrap_or_default();
        let db = Arc::new(Mutex::new(db));

        let active_view = if profiles.is_empty() {
            ActiveView::ProfileManager
        } else {
            ActiveView::GlobalDashboard
        };

        let db_clone = Arc::clone(&db);
        let profile_manager = cx.new(|cx| ProfileManagerView::new(db_clone, window, cx));
        
        let db_clone_global = Arc::clone(&db);
        let global_dashboard_view = cx.new(|cx| GlobalDashboardView::new(db_clone_global, window, cx));
        
        let profile_filter =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search TIN or name"));
        let filter_sub = cx.subscribe_in(
            &profile_filter,
            window,
            |_: &mut Self, _, event: &InputEvent, _, cx| {
                if matches!(event, InputEvent::Change) {
                    cx.notify();
                }
            },
        );

        let profile_sub = cx.subscribe(
            &profile_manager,
            |this: &mut Self, _entity, _event: &crate::views::profile_manager::ProfileEvent, cx| {
                if let Ok(db_lock) = this.db.lock() {
                    this.profiles = db_lock.list_profiles().unwrap_or_default();
                    let p_clone = this.profiles.clone();
                    this.global_dashboard_view.update(cx, |view, cx| {
                        view.set_profiles(p_clone, cx);
                    });
                    if let Some(tin) = &this.active_profile_tin
                        && let Some(profile) = this.profiles.iter().find(|p| p.tin.full() == *tin) {
                            let p = profile.clone();
                            this.dashboard_view.update(cx, |view, cx| {
                                view.set_profile(p, cx);
                            });
                        }
                    cx.notify();
                }
            },
        );

        let db_clone2 = Arc::clone(&db);
        let dashboard_view = cx.new(|cx| DashboardView::new(db_clone2, window, cx));

        cx.subscribe(
            &dashboard_view,
            |this: &mut Self, _entity, event: &DashboardEvent, cx| {
                let DashboardEvent::FileForm {
                    form_code,
                    year,
                    quarter,
                } = event;

                if form_code == "2551Q"
                    && let Some(tin) = &this.active_profile_tin
                        && let Some(profile) = this.profiles.iter().find(|p| p.tin.full() == *tin) {
                            let draft = if let Ok(db) = this.db.lock() {
                                let existing =
                                    db.get_2551q_draft(tin, *year, *quarter).ok().flatten();
                                if let Some(d) = existing {
                                    d
                                } else {
                                    let prev = if *quarter > 1 {
                                        db.get_2551q_draft(tin, *year, *quarter - 1).ok().flatten()
                                    } else {
                                        None
                                    };
                                    let new_draft =
                                        Form2551QDraft::new_from_profile(profile, *year, *quarter);
                                    if let Some(prev_draft) = prev {
                                        new_draft.with_carried_forward(&prev_draft)
                                    } else {
                                        new_draft
                                    }
                                }
                            } else {
                                Form2551QDraft::new_from_profile(profile, *year, *quarter)
                            };
                            // Store the draft; the view will be created on next render with Window access
                            this.pending_form_draft = Some(draft);
                            this.active_view = ActiveView::Form2551Q;
                            cx.notify();
                        }
            },
        )
        .detach();

        Self {
            active_view,
            profile_manager,
            dashboard_view,
            global_dashboard_view,
            form_2551q_view: None,
            pending_form_draft: None,
            db,
            profiles,
            active_profile_tin: None,
            expanded_profile_tin: None,
            profile_filter,
            sidebar_scroll: ScrollHandle::new(),
            _subscriptions: vec![filter_sub, profile_sub],
        }
    }

    fn render_sidebar(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let is_dark = cx.theme().is_dark();
        let filter = self.profile_filter.read(cx).value().to_lowercase();
        let filtered_profiles: Vec<TaxpayerProfile> = self
            .profiles
            .iter()
            .filter(|profile| {
                filter.trim().is_empty()
                    || profile.full_name.to_lowercase().contains(filter.trim())
                    || profile.tin.full().contains(filter.trim())
                    || profile.tin.formatted().contains(filter.trim())
            })
            .cloned()
            .collect();
        div()
            .w(px(280.))
            .h_full()
            .bg(cx.theme().background)
            .border_r_1()
            .border_color(cx.theme().border)
            .p_6()
            .flex()
            .flex_col()
            .justify_between()
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .id("global_dashboard_btn")
                            .flex()
                            .items_center()
                            .gap_3()
                            .mb_6()
                            .child(
                                div()
                                    .w_10()
                                    .h_10()
                                    .bg(cx.theme().primary)
                                    .rounded_lg()
                                    .flex()
                                    .justify_center()
                                    .items_center()
                                    .child(
                                        div()
                                            .text_xl()
                                            .font_weight(FontWeight::BOLD)
                                            .text_color(cx.theme().primary_foreground)
                                            .child("e"),
                                    ),
                            )
                            .child(
                                div()
                                    .flex()
                                    .flex_col()
                                    .child(
                                        div()
                                            .text_lg()
                                            .font_weight(FontWeight::BLACK)
                                            .text_color(cx.theme().foreground)
                                            .child("BIR Vault"),
                                    )
                                    .child(
                                        div()
                                            .text_xs()
                                            .font_weight(FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().primary)
                                            .child("OFFLINE SECURE"),
                                    ),
                            )
                            .cursor_pointer()
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.active_view = ActiveView::GlobalDashboard;
                                this.active_profile_tin = None;
                                cx.notify();
                            })),
                    )
                    .child(
                        div()
                            .text_xs()
                            .font_weight(FontWeight::BOLD)
                            .text_color(cx.theme().muted_foreground)
                            .child("TAXPAYER PROFILES"),
                    )
                    .child(
                        gpui_component::button::Button::new("add_profile")
                            .label("+ Add Profile")
                            .w_full()
                            .on_click(cx.listener(|this, _ev, _window, cx| {
                                this.active_view = ActiveView::ProfileManager;
                                this.active_profile_tin = None;
                                this.profile_manager.update(cx, |view, cx| {
                                    view.reset_for_new(_window, cx);
                                });
                                cx.notify();
                            })),
                    )
                    .child(Input::new(&self.profile_filter))
                    .when(
                        cfg!(debug_assertions) || std::env::var("PRODUCTION").unwrap_or_default() == "false",
                        |this| {
                            this.child(
                                gpui_component::button::Button::new("generate_mock_profile")
                                    .label("Generate Profile")
                                    .small()
                                    .on_click(cx.listener(|this, _ev, _window, cx| {
                                        let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as u64;
                                        let _tin_str = format!(
                                            "{:03}-{:03}-{:03}-{:03}",
                                            (now % 900) + 100,
                                            ((now / 1000) % 900) + 100,
                                            ((now / 1000000) % 900) + 100,
                                            now % 1000
                                        );
                                        use bir_core::naming::Tin;
                                        use bir_core::profile::TaxpayerType;
                                        
                                        let tin = Tin {
                                            segment1: format!("{:03}", (now % 900) + 100),
                                            segment2: format!("{:03}", ((now / 1000) % 900) + 100),
                                            segment3: format!("{:03}", ((now / 1000000) % 900) + 100),
                                            branch: format!("{:03}", now % 1000),
                                        };
                                        let profile = TaxpayerProfile {
                                            id: None,
                                            tin,
                                            rdo_code: "018".to_string(),
                                            taxpayer_type: TaxpayerType::Individual,
                                            line_of_business: "Software Development".to_string(),
                                            full_name: format!("Generated User {}", now % 10000),
                                            registered_address: "123 Mock Street, Dev City".to_string(),
                                            zip_code: "1234".to_string(),
                                            phone: "09123456789".to_string(),
                                            email: "mock@example.com".to_string(),
                                            default_form_type: "2551Qv2018".into(),
                                            business_start_date: chrono::NaiveDate::from_ymd_opt(2020, 1, 1),
                                            is_vat_registered: true,
                                        };
                                        if let Ok(db) = this.db.lock() {
                                            let _ = db.save_profile(profile);
                                            this.profiles = db.list_profiles().unwrap_or_default();
                                        }
                                        cx.notify();
                                    })),
                            )
                        },
                    )
                    .child(div().h_2())
                    .child(
                        v_flex()
                            .id("sidebar-profile-list")
                            .max_h(px(320.))
                            .overflow_y_scroll()
                            .track_scroll(&self.sidebar_scroll)
                            .pb_12()
                            .gap_2()
                            .children(filtered_profiles.iter().enumerate().map(|(idx, profile)| {
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
                                    .p_3()
                                    .rounded_lg()
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
                                    .child(
                                        div()
                                            .flex()
                                            .items_center()
                                            .gap_3()
                                            .when(is_expanded, |d| d.mb_4())
                                            .child(
                                                div()
                                                    .size(px(32.))
                                                    .rounded_full()
                                                    .bg(cx.theme().secondary)
                                                    .flex()
                                                    .items_center()
                                                    .justify_center()
                                                    .child(
                                                        div()
                                                            .text_lg()
                                                            .text_color(cx.theme().primary)
                                                            .font_weight(FontWeight::BOLD)
                                                            .child("👤"),
                                                    ),
                                            )
                                            .child(
                                                div()
                                                    .flex()
                                                    .flex_col()
                                                    .child(
                                                        div()
                                                            .font_weight(FontWeight::BOLD)
                                                            .text_color(cx.theme().foreground)
                                                            .child(profile.full_name.clone()),
                                                    )
                                                    .child(
                                                        div()
                                                            .text_sm()
                                                            .text_color(cx.theme().muted_foreground)
                                                            .child(format!("TIN: {}", profile.tin.full())),
                                                    ),
                                            ),
                                    )
                                    .when(is_expanded, |this| {
                                        this.child(
                                            div()
                                                .flex()
                                                .flex_col()
                                                .gap_2()
                                                .child(
                                                    div()
                                                        .flex()
                                                        .justify_between()
                                                        .child(div().text_sm().text_color(cx.theme().muted_foreground).child("RDO:"))
                                                        .child(
                                                            div()
                                                                .text_sm()
                                                                .font_weight(FontWeight::BOLD)
                                                                .text_color(cx.theme().foreground)
                                                                .child(format!("{} - {}", profile.rdo_code, rdo_desc)),
                                                        ),
                                                )
                                                .child(
                                                    div()
                                                        .flex()
                                                        .justify_between()
                                                        .gap_2()
                                                        .mt_2()
                                                        .child(
                                                            gpui_component::button::Button::new(format!("view_{}", profile.tin.full()))
                                                                .small()
                                                                .label("View")
                                                                .on_click(cx.listener({
                                                                    let tin = profile.tin.full();
                                                                    let profile_clone = profile.clone();
                                                                    move |this, _ev, _window, cx| {
                                                                        cx.stop_propagation();
                                                                        this.active_profile_tin = Some(tin.clone());
                                                                        this.active_view = ActiveView::Dashboard;
                                                                        this.dashboard_view.update(cx, |view, cx| {
                                                                            view.set_profile(profile_clone.clone(), cx);
                                                                        });
                                                                        cx.notify();
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
                                                                        this.active_view = ActiveView::ProfileManager;
                                                                        this.active_profile_tin = Some(profile_clone.tin.full());
                                                                        this.profile_manager.update(cx, |view, cx| {
                                                                            view.edit_profile(profile_clone.clone(), window, cx);
                                                                        });
                                                                        cx.notify();
                                                                    }
                                                                })),
                                                        )
                                                        .child(
                                                            gpui_component::button::Button::new(format!("delete_{}", profile.tin.full()))
                                                                .small()
                                                                .label("Delete")
                                                                .on_click(cx.listener({
                                                                    let tin = profile.tin.full();
                                                                    move |this, _ev, _window, cx| {
                                                                        cx.stop_propagation();
                                                                        if let Ok(db) = this.db.lock() {
                                                                            let _ = db.delete_profile(&tin);
                                                                        }
                                                                        this.profiles.retain(|p| p.tin.full() != tin);
                                                                        if this.active_profile_tin.as_ref() == Some(&tin) {
                                                                            this.active_profile_tin = None;
                                                                            this.active_view = ActiveView::ProfileManager;
                                                                        }
                                                                        if this.expanded_profile_tin.as_ref() == Some(&tin) {
                                                                            this.expanded_profile_tin = None;
                                                                        }
                                                                        cx.notify();
                                                                    }
                                                                })),
                                                        ),
                                                )
                                        )
                                        .on_mouse_down_out(cx.listener(|this, _ev, _window, cx| {
                                            this.expanded_profile_tin = None;
                                            cx.notify();
                                        }))
                                    })
                                    .on_click(cx.listener({
                                        let tin = profile.tin.full();
                                        let scroll = self.sidebar_scroll.clone();
                                        let is_last = idx == filtered_profiles.len() - 1;
                                        move |this, _ev, _window, cx| {
                                            if this.expanded_profile_tin.as_ref() == Some(&tin) {
                                                this.expanded_profile_tin = None;
                                            } else {
                                                this.expanded_profile_tin = Some(tin.clone());
                                                if is_last {
                                                    scroll.set_offset(gpui::Point { x: gpui::px(0.), y: gpui::px(9999.0) });
                                                }
                                            }
                                            cx.notify();
                                        }
                                    }))
                            }))
                            .child(
                                div()
                                    .w_full()
                                    .h_px()
                                    .bg(cx.theme().border)
                                    .mt_4()
                                    .mb_2()
                            )
                            .child(
                                div()
                                    .w_full()
                                    .flex()
                                    .justify_center()
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("End of profiles")
                                    )
                            ),
                    ),
            )
            .child(
                gpui_component::button::Button::new("theme_toggle")
                    .label(if is_dark {
                        "Switch to Light Mode"
                    } else {
                        "Switch to Dark Mode"
                    })
                    .on_click(cx.listener(|_this, _ev, window, cx| {
                        let is_dark = cx.theme().is_dark();
                        let new_mode = if is_dark {
                            ThemeMode::Light
                        } else {
                            ThemeMode::Dark
                        };
                        Theme::change(new_mode, Some(window), cx);
                    })),
            )
    }

    fn render_active_view(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        match self.active_view {
            ActiveView::GlobalDashboard => self.global_dashboard_view.clone().into_any_element(),
            ActiveView::ProfileManager => self.profile_manager.clone().into_any_element(),
            ActiveView::Dashboard => self.dashboard_view.clone().into_any_element(),
            ActiveView::Form2551Q => {
                if let Some(view) = &self.form_2551q_view {
                    view.clone().into_any_element()
                } else {
                    div().child("No form loaded").into_any_element()
                }
            }
            _ => div().child("Not implemented").into_any_element(),
        }
    }
}

fn app_database_path() -> std::path::PathBuf {
    if cfg!(target_os = "macos")
        && let Some(home) = std::env::var_os("HOME") {
            return std::path::PathBuf::from(home)
                .join("Library")
                .join("Application Support")
                .join("Taxman")
                .join("eBIRForms")
                .join("bir_data.db");
        }

    if let Some(home) = std::env::var_os("HOME") {
        return std::path::PathBuf::from(home)
            .join(".taxman-ebir")
            .join("bir_data.db");
    }

    std::env::current_dir()
        .unwrap_or_default()
        .join("bir_data.db")
}

impl Render for AppState {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Materialize any pending form view now that we have `window`
        if let Some(draft) = self.pending_form_draft.take() {
            let db_for_view = Arc::clone(&self.db);
            let form_view = cx.new(|cx| Form2551QView::new(draft, db_for_view, window, cx));

            cx.subscribe(
                &form_view,
                |this: &mut Self, _entity, event: &Form2551QEvent, cx| match event {
                    Form2551QEvent::BackToDashboard => {
                        this.active_view = ActiveView::Dashboard;
                        if let Some(tin) = &this.active_profile_tin
                            && let Some(profile) =
                                this.profiles.iter().find(|p| p.tin.full() == *tin)
                            {
                                let p = profile.clone();
                                this.dashboard_view.update(cx, |view, cx| {
                                    view.set_profile(p, cx);
                                });
                            }
                        cx.notify();
                    }
                    Form2551QEvent::Saved
                    | Form2551QEvent::Submitted
                    | Form2551QEvent::Confirmed => {
                        cx.notify();
                    }
                },
            )
            .detach();

            self.form_2551q_view = Some(form_view);
        }

        div()
            .flex()
            .flex_row()
            .size_full()
            .bg(cx.theme().background)
            .child(self.render_sidebar(cx))
            .child(
                div()
                    .flex_1()
                    .flex()
                    .flex_col()
                    .h_full()
                    .overflow_hidden()
                    .child(self.render_active_view(cx)),
            )
    }
}

impl Drop for AppState {
    fn drop(&mut self) {
        // Flush any pending WAL data to the main database file before shutdown
        if let Ok(db) = self.db.lock()
            && let Err(e) = db.checkpoint() {
                eprintln!("Warning: WAL checkpoint on shutdown failed: {e}");
            }
    }
}
