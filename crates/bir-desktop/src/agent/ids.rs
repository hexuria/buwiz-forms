//! Centralized stable IDs for the bir-desktop agent tree.
//!
//! These strings are the contract with CLI/MCP (`snapshot` / `click` / `assert`).
//! The same values are mirrored onto GPUI elements via `.id(...)`.
//! Tree indices are never part of the protocol.

use crate::app::ActiveView;

pub const WINDOW: &str = "bir-window";
pub const SIDEBAR: &str = "sidebar";
pub const SIDEBAR_PROFILE_LIST: &str = "sidebar-profile-list";

pub const PAGE_LOCK: &str = "page-lock";
pub const PAGE_GLOBAL_DASHBOARD: &str = "page-global-dashboard";
pub const PAGE_DASHBOARD: &str = "page-dashboard";
pub const PAGE_PROFILE_MANAGER: &str = "page-profile-manager";
pub const PAGE_CRON_TASKS: &str = "page-cron-tasks";
pub const PAGE_NOTIFICATIONS: &str = "page-notifications";
pub const PAGE_IMPORT_EXPORT: &str = "page-import-export";
pub const PAGE_SETTINGS: &str = "page-settings";
pub const PAGE_ADMIN_CALENDAR: &str = "page-admin-calendar";
pub const PAGE_FORM_2551Q: &str = "page-form-2551q";
pub const PAGE_FORM_1701Q: &str = "page-form-1701q";
pub const PAGE_FORM_1601C: &str = "page-form-1601c";
pub const PAGE_FORM_0619E: &str = "page-form-0619e";
pub const PAGE_FORM_0619F: &str = "page-form-0619f";
pub const PAGE_FORM_0605: &str = "page-form-0605";
pub const PAGE_FORM_2550Q: &str = "page-form-2550q";
pub const PAGE_FORM_1701: &str = "page-form-1701";
pub const PAGE_FORM_1702RT: &str = "page-form-1702rt";
pub const PAGE_FORM_1702MX: &str = "page-form-1702mx";

pub const NAV_GLOBAL_DASHBOARD: &str = "global_dashboard_btn";
pub const NAV_NEW_PROFILE: &str = "add_profile_mini_btn";
pub const NAV_IMPORT_EXPORT: &str = "import_export_sidebar_btn";
pub const NAV_NOTIFICATIONS: &str = "notifications_sidebar_btn";
pub const NAV_CRON_TASKS: &str = "cron_tasks_sidebar_btn";
pub const NAV_ADMIN_CALENDAR: &str = "admin_calendar_sidebar_btn";
pub const NAV_SETTINGS: &str = "settings_sidebar_btn";
pub const NAV_LOGOUT: &str = "logout_btn_full";

pub const OVERLAY_ADMIN_AUTH: &str = "overlay-admin-auth";
pub const OVERLAY_PROFILE_AUTH: &str = "overlay-profile-auth";
pub const OVERLAY_COMMAND_PALETTE: &str = "overlay-command-palette";

pub const PROFILE_TIN: &str = "profile-tin";
pub const PROFILE_NAME: &str = "profile-name";
pub const PROFILE_RDO: &str = "profile-rdo";
pub const PROFILE_LOB: &str = "profile-lob";
pub const PROFILE_ADDRESS: &str = "profile-address";
pub const PROFILE_ZIP: &str = "profile-zip";
pub const PROFILE_PHONE: &str = "profile-phone";
pub const PROFILE_EMAIL: &str = "profile-email";
pub const PROFILE_SAVE: &str = "save_profile";
pub const PROFILE_SAVE_MESSAGE: &str = "profile-save-message";
pub const PROFILE_VALIDATION: &str = "profile-validation";

pub const FORM_1601C_BACK: &str = "back_btn";
pub const FORM_1601C_SAVE: &str = "save_draft_btn";
pub const FORM_1601C_SUBMIT: &str = "submit_btn";
pub const FORM_1601C_VALIDATE: &str = "form-1601c-validate";
pub const FORM_1601C_STATUS: &str = "form-1601c-status";
pub const FORM_1601C_VALIDATION: &str = "form-1601c-validation";
pub const FORM_1601C_TAX_14: &str = "form-1601c-tax-14";
pub const FORM_1601C_TAX_25: &str = "form-1601c-tax-25";
pub const FORM_1601C_SHEETS: &str = "form-1601c-sheets";
/// Painted 1601-C “Any Taxes Withheld?” Yes/No control. Same id in the agent tree.
pub const FORM_1601C_WITHHELD: &str = "withheld_btn";
pub const FORM_1601C_SUBMIT_CONFIRM: &str = "form-1601c-submit-confirm";
pub const FORM_1601C_CANCEL_QUEUE: &str = "cancel_queue_btn";
pub const FORM_1601C_RETURN_DRAFT: &str = "form-1601c-return-draft";
pub const FORM_1601C_RELEASE_CLAIM_CONFIRM: &str = "form-1601c-release-claim-confirm";

pub const FORM_2551Q_VALIDATE: &str = "form-2551q-validate";
pub const FORM_2551Q_STATUS: &str = "form-2551q-status";
pub const FORM_2551Q_VALIDATION: &str = "form-2551q-validation";
pub const FORM_2551Q_CREDITABLE: &str = "form-2551q-creditable";
pub const FORM_2551Q_OTHER_CREDIT: &str = "form-2551q-other-credit";
pub const FORM_2551Q_TAXABLE_0: &str = "form-2551q-taxable-0";

/// Chrome IDs already used by the other form views (not invented here).
pub const FORM_2551Q_BACK: &str = "back_btn";
pub const FORM_2551Q_SAVE: &str = "save_btn";
pub const FORM_2551Q_SUBMIT: &str = "submit_btn";
pub const FORM_1701Q_BACK: &str = "1701q_back";
pub const FORM_1701Q_SAVE: &str = "1701q_save";
pub const FORM_1701Q_SUBMIT: &str = "1701q_manual";
pub const FORM_0619E_BACK: &str = "0619e_back";
pub const FORM_0619E_SAVE: &str = "0619e_save";
pub const FORM_0619E_SUBMIT: &str = "0619e_manual";
pub const FORM_0619F_BACK: &str = "0619f_back";
pub const FORM_0619F_SAVE: &str = "0619f_save";
pub const FORM_0619F_SUBMIT: &str = "0619f_manual";
pub const FORM_0605_BACK: &str = "0605_back";
pub const FORM_0605_SAVE: &str = "0605_save";
pub const FORM_0605_SUBMIT: &str = "0605_manual";
pub const FORM_2550Q_BACK: &str = "2550q_back";
pub const FORM_2550Q_SAVE: &str = "2550q_save";
pub const FORM_2550Q_SUBMIT: &str = "2550q_manual";
pub const FORM_1701_BACK: &str = "1701_back";
pub const FORM_1701_SAVE: &str = "1701_save";
pub const FORM_1701_SUBMIT: &str = "1701_manual";
pub const FORM_1702RT_BACK: &str = "1702rt_back";
pub const FORM_1702RT_SAVE: &str = "1702rt_save";
pub const FORM_1702RT_SUBMIT: &str = "1702rt_submit";
pub const FORM_1702MX_BACK: &str = "1702mx_back";
pub const FORM_1702MX_SAVE: &str = "1702mx_save";
pub const FORM_1702MX_SUBMIT: &str = "1702mx_submit";

pub const DUES_LIST: &str = "dues-list";
pub const JOBS_LIST: &str = "jobs-list";
pub const SUBMISSIONS_LIST: &str = "submissions-list";
pub const CONTEXT_SELECTED_TIN: &str = "context.selected_tin";

pub const PROFILE_TAB_TAX: &str = "profile-tab-tax";
pub const PROFILE_TAB_COR: &str = "profile-tab-cor";
pub const PROFILE_TAB_EMAIL: &str = "profile-tab-email";
pub const PROFILE_TAB_SECURITY: &str = "profile-tab-security";
pub const PROFILE_TAB_EXPORT: &str = "profile-tab-export";
pub const PROFILE_TAB_CALENDAR: &str = "profile-tab-calendar";
pub const PROFILE_SECTION_TAX: &str = "profile-section-tax";
pub const PROFILE_SECTION_COR: &str = "profile-section-cor";
pub const PROFILE_SECTION_EMAIL: &str = "profile-section-email";
pub const PROFILE_SECTION_SECURITY: &str = "profile-section-security";
pub const PROFILE_SECTION_EXPORT: &str = "profile-section-export";
pub const PROFILE_SECTION_CALENDAR: &str = "profile-section-calendar";

pub const DASHBOARD_FORM_FILTER: &str = "dashboard-form-filter";
pub const DASHBOARD_FILTER_QUERY: &str = "dashboard-filter-query";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileManagerTab {
    Tax,
    Cor,
    Email,
    Security,
    Export,
    Calendar,
}

impl ProfileManagerTab {
    pub fn from_slug(slug: &str) -> Option<Self> {
        match slug.trim().to_ascii_lowercase().as_str() {
            "tax" | "tax-profile" => Some(Self::Tax),
            "cor" => Some(Self::Cor),
            "email" | "email-settings" => Some(Self::Email),
            "security" => Some(Self::Security),
            "export" => Some(Self::Export),
            "calendar" => Some(Self::Calendar),
            _ => None,
        }
    }

    pub fn slug(self) -> &'static str {
        match self {
            Self::Tax => "tax",
            Self::Cor => "cor",
            Self::Email => "email",
            Self::Security => "security",
            Self::Export => "export",
            Self::Calendar => "calendar",
        }
    }

    pub fn id(self) -> &'static str {
        match self {
            Self::Tax => PROFILE_TAB_TAX,
            Self::Cor => PROFILE_TAB_COR,
            Self::Email => PROFILE_TAB_EMAIL,
            Self::Security => PROFILE_TAB_SECURITY,
            Self::Export => PROFILE_TAB_EXPORT,
            Self::Calendar => PROFILE_TAB_CALENDAR,
        }
    }

    pub fn section_id(self) -> &'static str {
        match self {
            Self::Tax => PROFILE_SECTION_TAX,
            Self::Cor => PROFILE_SECTION_COR,
            Self::Email => PROFILE_SECTION_EMAIL,
            Self::Security => PROFILE_SECTION_SECURITY,
            Self::Export => PROFILE_SECTION_EXPORT,
            Self::Calendar => PROFILE_SECTION_CALENDAR,
        }
    }

    pub fn index(self) -> usize {
        match self {
            Self::Tax => 0,
            Self::Cor => 1,
            Self::Email => 2,
            Self::Security => 3,
            Self::Export => 4,
            Self::Calendar => 6,
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        match index {
            0 => Some(Self::Tax),
            1 => Some(Self::Cor),
            2 => Some(Self::Email),
            3 => Some(Self::Security),
            4 => Some(Self::Export),
            6 => Some(Self::Calendar),
            _ => None,
        }
    }

    pub fn from_id(id: &str) -> Option<Self> {
        match id {
            PROFILE_TAB_TAX | PROFILE_SECTION_TAX => Some(Self::Tax),
            PROFILE_TAB_COR | PROFILE_SECTION_COR => Some(Self::Cor),
            PROFILE_TAB_EMAIL | PROFILE_SECTION_EMAIL => Some(Self::Email),
            PROFILE_TAB_SECURITY | PROFILE_SECTION_SECURITY => Some(Self::Security),
            PROFILE_TAB_EXPORT | PROFILE_SECTION_EXPORT => Some(Self::Export),
            PROFILE_TAB_CALENDAR | PROFILE_SECTION_CALENDAR => Some(Self::Calendar),
            _ => None,
        }
    }
}

pub fn profile_row(tin: &str) -> String {
    format!("profile-{tin}")
}

pub fn profile_row_tin(id: &str) -> Option<&str> {
    id.strip_prefix("profile-")
        .filter(|rest| rest.chars().all(|c| c.is_ascii_digit()))
}

pub fn due_row(form_code: &str, year: u16, period: u8) -> String {
    format!("due-{form_code}-{year}-{period}")
}

pub fn job_row(id: i64) -> String {
    format!("job-{id}")
}

pub fn submission_row(id: i64) -> String {
    format!("submission-{id}")
}

pub fn dashboard_form_chip(code: &str) -> String {
    format!("dashboard-form-chip-{code}")
}

pub fn page_root(view: ActiveView) -> &'static str {
    match view {
        ActiveView::GlobalDashboard => PAGE_GLOBAL_DASHBOARD,
        ActiveView::Dashboard => PAGE_DASHBOARD,
        ActiveView::ProfileManager => PAGE_PROFILE_MANAGER,
        ActiveView::CronTasks => PAGE_CRON_TASKS,
        ActiveView::Notifications => PAGE_NOTIFICATIONS,
        ActiveView::ImportExport => PAGE_IMPORT_EXPORT,
        ActiveView::Settings => PAGE_SETTINGS,
        ActiveView::AdminCalendarDashboard => PAGE_ADMIN_CALENDAR,
        ActiveView::Form2551Q => PAGE_FORM_2551Q,
        ActiveView::Form1701Q => PAGE_FORM_1701Q,
        ActiveView::Form1601C => PAGE_FORM_1601C,
        ActiveView::Form0619E => PAGE_FORM_0619E,
        ActiveView::Form0619F => PAGE_FORM_0619F,
        ActiveView::Form0605 => PAGE_FORM_0605,
        ActiveView::Form2550Q => PAGE_FORM_2550Q,
        ActiveView::Form1701 => PAGE_FORM_1701,
        ActiveView::Form1702RT => PAGE_FORM_1702RT,
        ActiveView::Form1702MX => PAGE_FORM_1702MX,
    }
}

pub fn view_slug(view: ActiveView) -> &'static str {
    match view {
        ActiveView::GlobalDashboard => "global-dashboard",
        ActiveView::Dashboard => "dashboard",
        ActiveView::ProfileManager => "profile-manager",
        ActiveView::CronTasks => "cron-tasks",
        ActiveView::Notifications => "notifications",
        ActiveView::ImportExport => "import-export",
        ActiveView::Settings => "settings",
        ActiveView::AdminCalendarDashboard => "admin-calendar",
        ActiveView::Form2551Q => "form-2551q",
        ActiveView::Form1701Q => "form-1701q",
        ActiveView::Form1601C => "form-1601c",
        ActiveView::Form0619E => "form-0619e",
        ActiveView::Form0619F => "form-0619f",
        ActiveView::Form0605 => "form-0605",
        ActiveView::Form2550Q => "form-2550q",
        ActiveView::Form1701 => "form-1701",
        ActiveView::Form1702RT => "form-1702rt",
        ActiveView::Form1702MX => "form-1702mx",
    }
}

pub fn view_from_slug(slug: &str) -> Option<ActiveView> {
    match slug.trim().to_ascii_lowercase().as_str() {
        "global-dashboard" | "global_dashboard" => Some(ActiveView::GlobalDashboard),
        "dashboard" => Some(ActiveView::Dashboard),
        "profile-manager" | "profile" | "profile_manager" => Some(ActiveView::ProfileManager),
        "cron-tasks" | "cron" | "background-tasks" => Some(ActiveView::CronTasks),
        "notifications" => Some(ActiveView::Notifications),
        "import-export" | "import" => Some(ActiveView::ImportExport),
        "settings" => Some(ActiveView::Settings),
        "admin-calendar" | "tax-calendars" => Some(ActiveView::AdminCalendarDashboard),
        "form-2551q" | "2551q" => Some(ActiveView::Form2551Q),
        "form-1701q" | "1701q" => Some(ActiveView::Form1701Q),
        "form-1601c" | "1601c" => Some(ActiveView::Form1601C),
        "form-0619e" | "0619e" => Some(ActiveView::Form0619E),
        "form-0619f" | "0619f" => Some(ActiveView::Form0619F),
        "form-0605" | "0605" => Some(ActiveView::Form0605),
        "form-2550q" | "2550q" => Some(ActiveView::Form2550Q),
        "form-1701" | "1701" => Some(ActiveView::Form1701),
        "form-1702rt" | "1702rt" => Some(ActiveView::Form1702RT),
        "form-1702mx" | "1702mx" => Some(ActiveView::Form1702MX),
        _ => None,
    }
}

/// Reachable `ActiveView` values used by navigation tests (not lock screen).
pub const ALL_VIEWS: &[ActiveView] = &[
    ActiveView::GlobalDashboard,
    ActiveView::Dashboard,
    ActiveView::ProfileManager,
    ActiveView::CronTasks,
    ActiveView::Notifications,
    ActiveView::ImportExport,
    ActiveView::Settings,
    ActiveView::AdminCalendarDashboard,
    ActiveView::Form2551Q,
    ActiveView::Form1701Q,
    ActiveView::Form1601C,
    ActiveView::Form0619E,
    ActiveView::Form0619F,
    ActiveView::Form0605,
    ActiveView::Form2550Q,
    ActiveView::Form1701,
    ActiveView::Form1702RT,
    ActiveView::Form1702MX,
];

pub struct FormChrome {
    pub view: ActiveView,
    pub code: &'static str,
    pub back: &'static str,
    pub save: &'static str,
    pub submit: &'static str,
}

pub const FORM_CHROME: &[FormChrome] = &[
    FormChrome {
        view: ActiveView::Form2551Q,
        code: "2551Q",
        back: FORM_2551Q_BACK,
        save: FORM_2551Q_SAVE,
        submit: FORM_2551Q_SUBMIT,
    },
    FormChrome {
        view: ActiveView::Form1701Q,
        code: "1701Q",
        back: FORM_1701Q_BACK,
        save: FORM_1701Q_SAVE,
        submit: FORM_1701Q_SUBMIT,
    },
    FormChrome {
        view: ActiveView::Form1601C,
        code: "1601C",
        back: FORM_1601C_BACK,
        save: FORM_1601C_SAVE,
        submit: FORM_1601C_SUBMIT,
    },
    FormChrome {
        view: ActiveView::Form0619E,
        code: "0619E",
        back: FORM_0619E_BACK,
        save: FORM_0619E_SAVE,
        submit: FORM_0619E_SUBMIT,
    },
    FormChrome {
        view: ActiveView::Form0619F,
        code: "0619F",
        back: FORM_0619F_BACK,
        save: FORM_0619F_SAVE,
        submit: FORM_0619F_SUBMIT,
    },
    FormChrome {
        view: ActiveView::Form0605,
        code: "0605",
        back: FORM_0605_BACK,
        save: FORM_0605_SAVE,
        submit: FORM_0605_SUBMIT,
    },
    FormChrome {
        view: ActiveView::Form2550Q,
        code: "2550Q",
        back: FORM_2550Q_BACK,
        save: FORM_2550Q_SAVE,
        submit: FORM_2550Q_SUBMIT,
    },
    FormChrome {
        view: ActiveView::Form1701,
        code: "1701",
        back: FORM_1701_BACK,
        save: FORM_1701_SAVE,
        submit: FORM_1701_SUBMIT,
    },
    FormChrome {
        view: ActiveView::Form1702RT,
        code: "1702RT",
        back: FORM_1702RT_BACK,
        save: FORM_1702RT_SAVE,
        submit: FORM_1702RT_SUBMIT,
    },
    FormChrome {
        view: ActiveView::Form1702MX,
        code: "1702MX",
        back: FORM_1702MX_BACK,
        save: FORM_1702MX_SAVE,
        submit: FORM_1702MX_SUBMIT,
    },
];

pub fn form_chrome(view: ActiveView) -> Option<&'static FormChrome> {
    FORM_CHROME.iter().find(|chrome| chrome.view == view)
}

/// Submit / confirm controls that would queue or file if the real widget ran.
/// Semantic dispatch exposes confirmation only; virtual clicks must not hit these.
pub fn is_filing_submit_control(id: &str) -> bool {
    id == FORM_1601C_SUBMIT_CONFIRM || FORM_CHROME.iter().any(|chrome| chrome.submit == id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn page_roots_are_unique_and_stable() {
        let mut seen = HashSet::new();
        for view in ALL_VIEWS {
            let id = page_root(*view);
            assert!(id.starts_with("page-"), "{id}");
            assert!(seen.insert(id), "duplicate page root {id}");
            assert_eq!(view_from_slug(view_slug(*view)), Some(*view));
        }
        assert_eq!(ALL_VIEWS.len(), 18);
    }

    #[test]
    fn due_and_profile_ids_are_not_indices() {
        assert_eq!(due_row("1601C", 2026, 1), "due-1601C-2026-1");
        assert_eq!(profile_row("12345678900000"), "profile-12345678900000");
        assert!(
            !due_row("1601C", 2026, 1)
                .chars()
                .all(|c| c.is_ascii_digit())
        );
        assert!(is_filing_submit_control(FORM_1601C_SUBMIT));
        assert!(is_filing_submit_control(FORM_1601C_SUBMIT_CONFIRM));
        assert!(!is_filing_submit_control(FORM_1601C_SAVE));
        assert!(!is_filing_submit_control(FORM_1601C_WITHHELD));
        assert_eq!(
            profile_row_tin("profile-12345678900000"),
            Some("12345678900000")
        );
        assert_eq!(profile_row_tin(PROFILE_TAB_TAX), None);
        assert_eq!(profile_row_tin(PROFILE_TIN), None);
        assert_eq!(
            ProfileManagerTab::from_id(PROFILE_TAB_CALENDAR).map(|tab| tab.index()),
            Some(6)
        );
    }
}
