//! The app state (port of `nsh/shell/app.ts`): where the app is, what it is on, who is signed in, what
//! it keeps, and the polls. Plain Rust state mutated by the frame and by screens — the TS signals
//! become fields; the computed accessors become methods that read `store` fresh each call.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::time::{Duration, Instant};

use chrono::Datelike;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::bir;
use crate::lib::chrome_gate::ChromeState;
use crate::lib::dates::iso_date;
use crate::lib::errors::error_text;
use crate::lib::events::{FreshEvent, take_fresh};
use crate::lib::recent::{self, RecentEntry};
use crate::lib::start::{allowed_start, parse_start, parse_start_form};
use crate::store;
use crate::theme::ThemePreference;
use crate::types::{AuthState, ReturnRow, ScreenFormIndexEntry, Taxpayer};
use bir_app_api::hooks::UpdateStatus;

// ---------------------------------------------------------------- routes (routes.ts)

/// How the frame places a view in the main surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewLayout {
    Page,
    Wide,
    Full,
    Self_,
}

/// What the shell knows of a view.
#[derive(Debug, Clone, Copy)]
pub struct RouteMeta {
    pub label: &'static str,
    /// Icon name (the ui kit maps it; CUI has no icon set yet — text badges for now).
    pub icon: &'static str,
    pub needs_taxpayer: bool,
    pub layout: ViewLayout,
    /// Only where the dev environment is on.
    pub dev: bool,
}

const META_HOME: RouteMeta = RouteMeta {
    label: "Home",
    icon: "house",
    needs_taxpayer: false,
    layout: ViewLayout::Page,
    dev: false,
};
const META_LIBRARY: RouteMeta = RouteMeta {
    label: "Form library",
    icon: "file-text",
    needs_taxpayer: false,
    layout: ViewLayout::Wide,
    dev: false,
};
const META_CALENDAR: RouteMeta = RouteMeta {
    label: "Calendar",
    icon: "calendar",
    needs_taxpayer: false,
    layout: ViewLayout::Page,
    dev: false,
};
const META_FILINGS: RouteMeta = RouteMeta {
    label: "Filings",
    icon: "history",
    needs_taxpayer: false,
    layout: ViewLayout::Full,
    dev: false,
};
const META_SETTINGS: RouteMeta = RouteMeta {
    label: "Settings",
    icon: "settings",
    needs_taxpayer: false,
    layout: ViewLayout::Self_,
    dev: false,
};
const META_PROFILE: RouteMeta = RouteMeta {
    label: "Tax profile",
    icon: "user",
    needs_taxpayer: true,
    layout: ViewLayout::Page,
    dev: false,
};
const META_FORMS: RouteMeta = RouteMeta {
    label: "Forms",
    icon: "file-text",
    needs_taxpayer: true,
    layout: ViewLayout::Page,
    dev: false,
};
const META_TEMPLATES: RouteMeta = RouteMeta {
    label: "Templates",
    icon: "layout-template",
    needs_taxpayer: true,
    layout: ViewLayout::Page,
    dev: false,
};
const META_NEW_TAXPAYER: RouteMeta = RouteMeta {
    label: "New taxpayer",
    icon: "user",
    needs_taxpayer: false,
    layout: ViewLayout::Page,
    dev: false,
};
const META_NEW_BRANCH: RouteMeta = RouteMeta {
    label: "New branch",
    icon: "git-branch",
    needs_taxpayer: true,
    layout: ViewLayout::Page,
    dev: false,
};
const META_FORM: RouteMeta = RouteMeta {
    label: "Return",
    icon: "file-text",
    needs_taxpayer: true,
    layout: ViewLayout::Self_,
    dev: false,
};
const META_KIT: RouteMeta = RouteMeta {
    label: "UI kit",
    icon: "layout-template",
    needs_taxpayer: false,
    layout: ViewLayout::Self_,
    dev: true,
};
const META_THEME: RouteMeta = RouteMeta {
    label: "Theme page",
    icon: "monitor",
    needs_taxpayer: false,
    layout: ViewLayout::Self_,
    dev: true,
};

/// A view's name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewName {
    Home,
    Library,
    Calendar,
    Filings,
    Settings,
    Profile,
    Forms,
    Templates,
    NewTaxpayer,
    NewBranch,
    Form,
    Kit,
    Theme,
}

impl ViewName {
    pub fn parse(name: &str) -> Option<ViewName> {
        Some(match name {
            "home" => ViewName::Home,
            "library" => ViewName::Library,
            "calendar" => ViewName::Calendar,
            "filings" => ViewName::Filings,
            "settings" => ViewName::Settings,
            "profile" => ViewName::Profile,
            "forms" => ViewName::Forms,
            "templates" => ViewName::Templates,
            "new_taxpayer" => ViewName::NewTaxpayer,
            "new_branch" => ViewName::NewBranch,
            "form" => ViewName::Form,
            "kit" => ViewName::Kit,
            "theme" => ViewName::Theme,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            ViewName::Home => "home",
            ViewName::Library => "library",
            ViewName::Calendar => "calendar",
            ViewName::Filings => "filings",
            ViewName::Settings => "settings",
            ViewName::Profile => "profile",
            ViewName::Forms => "forms",
            ViewName::Templates => "templates",
            ViewName::NewTaxpayer => "new_taxpayer",
            ViewName::NewBranch => "new_branch",
            ViewName::Form => "form",
            ViewName::Kit => "kit",
            ViewName::Theme => "theme",
        }
    }

    pub fn meta(self) -> RouteMeta {
        match self {
            ViewName::Home => META_HOME,
            ViewName::Library => META_LIBRARY,
            ViewName::Calendar => META_CALENDAR,
            ViewName::Filings => META_FILINGS,
            ViewName::Settings => META_SETTINGS,
            ViewName::Profile => META_PROFILE,
            ViewName::Forms => META_FORMS,
            ViewName::Templates => META_TEMPLATES,
            ViewName::NewTaxpayer => META_NEW_TAXPAYER,
            ViewName::NewBranch => META_NEW_BRANCH,
            ViewName::Form => META_FORM,
            ViewName::Kit => META_KIT,
            ViewName::Theme => META_THEME,
        }
    }

    /// Every view by name, for the palette's "Go to".
    pub fn all() -> [ViewName; 13] {
        [
            ViewName::Home,
            ViewName::Library,
            ViewName::Calendar,
            ViewName::Filings,
            ViewName::Settings,
            ViewName::Profile,
            ViewName::Forms,
            ViewName::Templates,
            ViewName::NewTaxpayer,
            ViewName::NewBranch,
            ViewName::Form,
            ViewName::Kit,
            ViewName::Theme,
        ]
    }
}

/// The content column of a page (routes.ts contentColumn): the cap INCLUDES the surface's padding.
pub const SIZE_CONTENT_MAX: f32 = 880.0;
pub const SIZE_WIDE_MAX: f32 = 1120.0;
pub const BREAK_NARROW_CONTENT: f32 = 640.0;
pub const BREAK_PHONE: f32 = 600.0;
pub const BREAK_OVERLAYS: f32 = 860.0;
pub const SIZE_SIDEBAR: f32 = 248.0;
pub const SIZE_PANEL: f32 = 320.0;

pub fn content_column(layout: ViewLayout, main_width: f32) -> f32 {
    let pad = if main_width < BREAK_NARROW_CONTENT {
        16.0
    } else {
        32.0
    };
    let cap = match layout {
        ViewLayout::Page => Some(SIZE_CONTENT_MAX),
        ViewLayout::Wide => Some(SIZE_WIDE_MAX),
        _ => None,
    };
    let max_width = cap.map(|c| (c - pad * 2.0).max(200.0));
    (main_width - pad * 2.0)
        .max(200.0)
        .min(max_width.unwrap_or(f32::INFINITY))
}

/// What a view is opened with.
pub type ViewParams = serde_json::Map<String, Value>;

/// Where the app is: a view and its params. `seq` changes with every move.
#[derive(Debug, Clone)]
pub struct Route {
    pub view: ViewName,
    pub params: ViewParams,
    pub seq: u64,
}

impl Default for Route {
    fn default() -> Self {
        Self {
            view: ViewName::Home,
            params: ViewParams::new(),
            seq: 0,
        }
    }
}

/// The four bottom tabs of the narrow layout.
pub const TABS: [(ViewName, &str, &str); 4] = [
    (ViewName::Home, "Home", "house"),
    (ViewName::Library, "Forms", "file-text"),
    (ViewName::Filings, "Filings", "history"),
    (ViewName::Calendar, "Calendar", "calendar"),
];

// ---------------------------------------------------------------- session and layers

/// Who is signed in (the seam wave 4 fills: solo owner, help-desk admin, or one taxpayer).
#[derive(Debug, Clone, PartialEq)]
pub enum Session {
    /// Nobody signed in; the admin has no code so nobody can be locked out.
    Open,
    Admin,
    Taxpayer {
        root: String,
    },
}

impl Default for Session {
    fn default() -> Self {
        Session::Open
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelName {
    Profile,
    Notifs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OverlayName {
    Palette,
    New,
}

/// What the form page opens: the return's identity, its mode and the stored return's key.
#[derive(Debug, Clone, PartialEq)]
pub struct FormTarget {
    pub tin: String,
    pub form_code: String,
    pub year: i64,
    pub period: i64,
    pub key: Option<i64>,
    /// "return" | "amend" | "new" | "sample".
    pub mode: String,
    pub snapshot_id: Option<i64>,
    pub new_key: Option<Value>,
    pub new_key_from: Option<Value>,
}

/// What openReturn takes.
#[derive(Debug, Clone)]
pub struct OpenReturnOptions {
    pub tin: String,
    pub code: String,
    pub year: i64,
    pub period: i64,
    pub return_key: Option<Value>,
    pub mode: Option<String>,
    pub snapshot_id: Option<i64>,
    pub new_key: Option<Value>,
    pub new_key_from: Option<Value>,
}

/// The taxpayer, branch and year the frame is on.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Selection {
    pub root: Option<String>,
    pub code: Option<String>,
    pub year: Option<i64>,
}

/// A place in the history: the route, and the selection the frame had when the person left it.
#[derive(Debug, Clone)]
struct Stop {
    route: Route,
    sel: Selection,
}

/// A toast at the bottom of the window.
#[derive(Debug, Clone)]
pub struct Toast {
    pub id: u64,
    pub message: String,
    pub icon: Option<String>,
    pub action: Option<ToastAction>,
    pub at: Instant,
}

#[derive(Debug, Clone)]
pub struct ToastAction {
    pub label: String,
    /// An index into `AppState::toast_actions` — the frame resolves the press.
    pub index: usize,
}

/// A notification: a filed return's confirmation or submission.
#[derive(Debug, Clone)]
pub struct Notification {
    pub id: String,
    pub title: String,
    pub sub: String,
    pub icon: String,
    pub tone: String,
    pub when: String,
    pub unread: bool,
}

impl crate::lib::events::HasId for Notification {
    fn id(&self) -> &str {
        &self.id
    }
}

/// Preferences the app keeps.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Prefs {
    pub show_recent: bool,
    pub compact_form_nav: bool,
}

impl Default for Prefs {
    fn default() -> Self {
        Self {
            show_recent: true,
            compact_form_nav: false,
        }
    }
}

// ---------------------------------------------------------------- persistence (NSH Storage -> a JSON file)

const KEYS_RECENT: &str = "redesign.recent";
const KEYS_SELECTION: &str = "redesign.selection";
const KEYS_READ: &str = "redesign.read_notifications";
const KEYS_PREFS: &str = "redesign.prefs";
const THEME_PREF_KEY: &str = "theme_preference";

/// Why nothing can be opened yet.
pub const FRESH_HINT: &str = "Add a taxpayer first, then open a form for them.";

// ---------------------------------------------------------------- polls

/// Milliseconds between the runs of each poll (shell/poll.ts POLL + the queued watch).
pub const POLL_EVENTS_MS: u64 = 30_000;
pub const POLL_DEVICE_FILINGS_MS: u64 = 5_000;
pub const POLL_UPDATE_MS: u64 = 1_000;
pub const QUEUED_POLL_MS: u64 = 3_000;
pub const QUEUED_FAST_MS: u64 = 30_000;
/// How long after the frame is first shown the form library is built.
pub const WARM_DELAY_MS: u64 = 600;

// ---------------------------------------------------------------- the state

/// Toast action callbacks the frame installs (form page's Undo). Index-addressed so a `Toast` stays
/// `Clone`.
pub type ToastActionFn = Rc<RefCell<dyn FnMut()>>;

/// Leave hooks: a screen registers work to finish before the app leaves it. Synchronous; the TS
/// version could await — the form page's autosave flush is the only real user and it is fast.
pub type LeaveHook = Rc<RefCell<dyn FnMut()>>;

/// Session-change hooks: a screen puts its own state back to how a first visit finds it.
pub type SessionHook = Rc<RefCell<dyn FnMut()>>;

/// Everything the frame and the screens share. One instance, owned by the frame loop.
pub struct AppState {
    // boot and the gates
    pub booted: bool,
    pub cloud_locked: bool,
    pub app_locked: bool,
    pub kept_loaded: bool,
    pub first_run: bool,
    pub setup_pending: Option<String>,
    pub setup_step: String,
    pub access_error: Option<String>,
    pub gate_error: Option<String>,
    pub update_status: Option<UpdateStatus>,
    /// The update prompt "Later" put away (banner.ts's putAway): a different prompt shows again.
    pub update_put_away: String,
    pub database_path: String,
    pub boot_start: Option<String>,
    /// The dev start route's parsed steps (key -> value), fixed once boot has the route.
    pub start_params: HashMap<String, String>,

    // where the app is
    pub route: Route,
    history: Vec<Stop>,
    forward: Vec<Stop>,
    seq: u64,

    // selection
    pub sel_root: Option<String>,
    pub sel_code: Option<String>,
    pub sel_year: Option<i64>,

    // layers
    pub sidebar_open: bool,
    pub panel: Option<PanelName>,
    pub last_panel: PanelName,
    pub overlay: Option<OverlayName>,
    pub new_return_code: Option<String>,
    pub new_return_period: Option<i64>,
    pub palette_query: String,
    pub palette_index: usize,
    /// The desktop About dialog is open (shell/desktop's aboutOpen).
    pub about_open: bool,
    pub sidebar_width: f32,
    pub window_width: f32,

    // kept values
    pub recent: Vec<RecentEntry>,
    pub read_ids: HashSet<String>,
    pub prefs: Prefs,
    pub theme_pref: ThemePreference,
    known_events: HashSet<String>,
    /// The kept values' backing store: storage key -> JSON text (the NSH Storage map).
    local: HashMap<String, String>,
    local_loaded: bool,

    // session
    pub session: Session,
    locked_from: Option<Session>,

    // toasts
    pub toasts: Vec<Toast>,
    toast_seq: u64,
    pub toast_actions: Vec<ToastActionFn>,

    // hooks
    leave_hooks: Vec<LeaveHook>,
    session_hooks: Vec<SessionHook>,

    // polls
    last_events: Instant,
    last_device: Instant,
    last_update: Instant,
    last_queued: Instant,
    last_warm: Option<Instant>,
    warm_since: Option<Instant>,
    queued_seen: String,
    queued_changed_at: Instant,
    pub quit_requested: bool,
    /// Home forgot the drafts read once this visit (home/Home.tsx's setTimeout forget).
    pub saved_returns_forgotten: bool,
}

impl Default for AppState {
    fn default() -> Self {
        let now = Instant::now();
        Self {
            booted: false,
            cloud_locked: false,
            app_locked: false,
            kept_loaded: false,
            first_run: false,
            setup_pending: None,
            setup_step: "choose".to_owned(),
            access_error: None,
            gate_error: None,
            update_status: None,
            update_put_away: String::new(),
            database_path: String::new(),
            boot_start: None,
            start_params: HashMap::new(),
            route: Route::default(),
            history: Vec::new(),
            forward: Vec::new(),
            seq: 0,
            sel_root: None,
            sel_code: None,
            sel_year: None,
            sidebar_open: true,
            panel: None,
            last_panel: PanelName::Profile,
            overlay: None,
            new_return_code: None,
            new_return_period: None,
            palette_query: String::new(),
            palette_index: 0,
            about_open: false,
            sidebar_width: SIZE_SIDEBAR,
            window_width: 1440.0,
            recent: Vec::new(),
            read_ids: HashSet::new(),
            prefs: Prefs::default(),
            theme_pref: ThemePreference::System,
            known_events: HashSet::new(),
            local: HashMap::new(),
            local_loaded: false,
            session: Session::Open,
            locked_from: None,
            toasts: Vec::new(),
            toast_seq: 0,
            toast_actions: Vec::new(),
            leave_hooks: Vec::new(),
            session_hooks: Vec::new(),
            last_events: now - Duration::from_secs(60),
            last_device: now,
            last_update: now,
            last_queued: now,
            last_warm: None,
            warm_since: None,
            queued_seen: String::new(),
            queued_changed_at: now,
            quit_requested: false,
            saved_returns_forgotten: false,
        }
    }
}

// ---------------------------------------------------------------- computed reads

impl AppState {
    /// The taxpayers the session may see (store.taxpayers()).
    pub fn taxpayers(&self) -> Vec<Taxpayer> {
        store::taxpayers()
    }

    /// The effective taxpayer: the selected root, else the first listed.
    pub fn taxpayer(&self) -> Option<Taxpayer> {
        let list = self.taxpayers();
        list.iter()
            .find(|t| Some(t.tin_root.as_str()) == self.sel_root.as_deref())
            .or_else(|| list.first())
            .cloned()
    }

    /// The effective branch: the selected code on the effective taxpayer, else its first.
    pub fn branch(&self) -> Option<crate::types::Branch> {
        let t = self.taxpayer()?;
        t.branches
            .iter()
            .find(|b| Some(b.code.as_str()) == self.sel_code.as_deref())
            .or_else(|| t.branches.first())
            .cloned()
    }

    /// Whether there is no taxpayer at all (needs-taxpayer views fall back to Home).
    pub fn is_fresh(&self) -> bool {
        self.taxpayers().is_empty()
    }

    /// The effective taxpayer's tax years.
    pub fn years(&self) -> Vec<i64> {
        match self.branch() {
            Some(b) => store::tax_years(&b.tin),
            None => Vec::new(),
        }
    }

    /// The selected year, kept valid: this year if the branch has it, else its latest.
    pub fn year(&self) -> Option<i64> {
        let ys = self.years();
        if let Some(picked) = self.sel_year
            && ys.contains(&picked)
        {
            return Some(picked);
        }
        let now = chrono::Local::now().year();
        if ys.contains(&i64::from(now)) {
            return Some(i64::from(now));
        }
        ys.last().copied()
    }

    /// The selected year's form codes for the effective branch.
    pub fn year_forms(&self) -> Vec<String> {
        match (self.branch(), self.year()) {
            (Some(b), Some(y)) => store::yearly_forms(&b.tin, y),
            _ => Vec::new(),
        }
    }

    /// The effective branch's form templates.
    pub fn templates(&self) -> Vec<crate::types::FormTemplateSummary> {
        match self.branch() {
            Some(b) => store::templates(&b.tin),
            None => Vec::new(),
        }
    }

    /// The branch a full TIN names in this database, if any.
    pub fn branch_for(&self, tin: &str) -> Option<crate::types::Branch> {
        let root = &tin[..tin.len().min(9)];
        self.taxpayers()
            .iter()
            .find(|t| t.tin_root == root)
            .and_then(|t| t.branches.iter().find(|b| b.tin == tin).cloned())
    }

    /// The view that shows: a view that needs a taxpayer is Home while none exists.
    pub fn view(&self) -> ViewName {
        let v = self.route.view;
        if v.meta().needs_taxpayer && self.is_fresh() {
            ViewName::Home
        } else {
            v
        }
    }

    /// The route's params as a typed value (screens read `target`, `section`, ...).
    pub fn params(&self) -> &ViewParams {
        &self.route.params
    }

    // ---- session predicates (app.ts auth section)

    pub fn auth_state(&self) -> AuthState {
        store::auth_state()
    }

    pub fn is_helpdesk(&self) -> bool {
        self.auth_state().mode.as_deref() == Some("helpdesk")
    }

    pub fn admin_has_code(&self) -> bool {
        let s = self.auth_state();
        s.has_admin_pin || s.has_admin_totp
    }

    /// Admin rights: signed in as the admin, or nobody while the admin has no code.
    pub fn is_admin(&self) -> bool {
        self.session == Session::Admin || (self.session == Session::Open && !self.admin_has_code())
    }

    pub fn can_add_taxpayer(&self) -> bool {
        self.taxpayers().is_empty() || (self.is_admin() && self.is_helpdesk())
    }

    /// Whether the filing chrome may show (chrome_gate over the live state).
    pub fn chrome_visible(&self) -> bool {
        let state = ChromeState {
            booted: self.booted,
            cloud_locked: self.cloud_locked,
            app_locked: self.app_locked,
            mode: if self.first_run {
                self.auth_state().mode
            } else {
                Some("solo".to_owned())
            },
            setup_pending: self.setup_pending.clone(),
            setup_step: self.setup_step.clone(),
        };
        crate::lib::chrome_gate::shell_chrome_visible(&state)
    }

    /// First launch has not picked "Just me" or "I help other taxpayers" yet.
    pub fn needs_setup(&self) -> bool {
        self.first_run && self.auth_state().mode.is_none()
    }

    /// The agent host refuses while true (the Buwiz gate, the sign-in lock, not booted).
    pub fn agent_locked(&self) -> bool {
        !self.booted || self.cloud_locked || self.app_locked
    }

    // ---- layout (from the window's width)

    pub fn narrow(&self) -> bool {
        self.window_width < BREAK_PHONE
    }

    pub fn overlay_mode(&self) -> bool {
        self.window_width < BREAK_OVERLAYS
    }

    pub fn sidebar_docked(&self) -> bool {
        self.sidebar_open && !self.overlay_mode()
    }

    pub fn panel_docked(&self) -> bool {
        self.panel.is_some() && !self.overlay_mode()
    }

    /// The main surface's width: the window less what is docked beside it and the inset.
    pub fn main_width(&self) -> f32 {
        let left = if self.sidebar_docked() {
            SIZE_SIDEBAR
        } else {
            8.0
        };
        let right = if self.panel_docked() { SIZE_PANEL } else { 8.0 };
        (self.window_width - left - right - 2.0).max(240.0)
    }

    // ---------------------------------------------------------------- persistence

    /// The storage key of one kept value: per database except prefs (app.ts storageKey).
    fn storage_key(&self, key: &str) -> Option<String> {
        if key == KEYS_PREFS || self.database_path.is_empty() {
            Some(key.to_owned())
        } else {
            Some(format!("{key}@{}", self.database_path))
        }
    }

    fn local_path(&self) -> Option<std::path::PathBuf> {
        let dir = store::system_info().data_dir;
        if dir.is_empty() {
            return None;
        }
        Some(std::path::PathBuf::from(dir).join("ui-state.json"))
    }

    /// Loads the kept values once the data dir is known (loadKept's readLocal calls).
    fn ensure_local(&mut self) {
        if self.local_loaded {
            return;
        }
        self.local_loaded = true;
        if let Some(path) = self.local_path()
            && let Ok(text) = std::fs::read_to_string(&path)
            && let Ok(map) = serde_json::from_str::<HashMap<String, String>>(&text)
        {
            self.local = map;
        }
    }

    fn read_local<T: serde::de::DeserializeOwned>(&mut self, key: &str, fallback: T) -> T {
        self.ensure_local();
        let Some(stored) = self.storage_key(key) else {
            return fallback;
        };
        self.local
            .get(&stored)
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|v| serde_json::from_value(v).ok())
            .unwrap_or(fallback)
    }

    fn write_local(&mut self, key: &str, value: &Value) {
        self.ensure_local();
        let Some(stored) = self.storage_key(key) else {
            return;
        };
        if let Ok(text) = serde_json::to_string(value) {
            self.local.insert(stored, text);
            self.flush_local();
        }
    }

    fn flush_local(&self) {
        let Some(path) = self.local_path() else {
            return;
        };
        if let Ok(text) = serde_json::to_string(&self.local)
            && let Some(parent) = path.parent()
        {
            let _ = std::fs::create_dir_all(parent);
            let _ = std::fs::write(&path, text);
        }
    }

    // ---------------------------------------------------------------- selection

    pub fn select_taxpayer(&mut self, root: &str, quiet: bool, code: Option<String>) {
        let Some(t) = self.taxpayers().into_iter().find(|x| x.tin_root == root) else {
            return;
        };
        self.sel_root = Some(root.to_owned());
        self.sel_code = code
            .filter(|c| t.branches.iter().any(|b| &b.code == c))
            .or_else(|| t.branches.first().map(|b| b.code.clone()));
        self.sel_year = None;
        self.save_selection();
        if !quiet {
            self.toast(format!("Switched to {}", t.name), None, None);
        }
    }

    pub fn select_branch(&mut self, code: &str) {
        self.sel_code = Some(code.to_owned());
        self.sel_year = None;
        self.save_selection();
    }

    /// Keeps the taxpayer and branch the frame is on (the effective ones).
    fn save_selection(&mut self) {
        let sel = json!({
            "root": self.taxpayer().map(|t| t.tin_root),
            "code": self.branch().map(|b| b.code),
        });
        self.write_local(KEYS_SELECTION, &sel);
    }

    pub fn select_year(&mut self, y: i64) {
        self.sel_year = Some(y);
    }

    fn current_selection(&self) -> Selection {
        Selection {
            root: self.taxpayer().map(|t| t.tin_root),
            code: self.branch().map(|b| b.code),
            year: self.sel_year,
        }
    }

    fn restore_selection(&mut self, sel: &Selection) {
        let Some(root) = &sel.root else {
            return;
        };
        if self.taxpayer().is_some_and(|t| t.tin_root == *root)
            && self.branch().is_some_and(|b| Some(b.code) == sel.code)
        {
            return;
        }
        if !self.taxpayers().iter().any(|t| &t.tin_root == root) {
            return;
        }
        self.select_taxpayer(root, true, sel.code.clone());
        self.sel_year = sel.year;
    }

    // ---------------------------------------------------------------- toasts

    /// A toast at the bottom; answers its id. `action` registers a callback the frame runs on press.
    pub fn toast(
        &mut self,
        message: impl Into<String>,
        icon: Option<&str>,
        action: Option<(String, ToastActionFn)>,
    ) -> u64 {
        self.toast_seq += 1;
        let id = self.toast_seq;
        let action = action.map(|(label, f)| {
            self.toast_actions.push(f);
            ToastAction {
                label,
                index: self.toast_actions.len() - 1,
            }
        });
        self.toasts.push(Toast {
            id,
            message: message.into(),
            icon: icon.map(str::to_owned),
            action,
            at: Instant::now(),
        });
        // Toasts live 6 s.
        self.toasts
            .retain(|t| t.at.elapsed() < Duration::from_secs(6));
        id
    }

    pub fn dismiss_toast(&mut self, id: u64) {
        self.toasts.retain(|t| t.id != id);
    }

    pub fn clear_toasts(&mut self) {
        self.toasts.clear();
        self.toast_actions.clear();
    }

    /// Runs a toast action's callback.
    pub fn run_toast_action(&mut self, index: usize) {
        if let Some(f) = self.toast_actions.get_mut(index) {
            (f.borrow_mut())();
        }
    }

    // ---------------------------------------------------------------- navigation

    /// Opens a view (app.go). `cron_tasks` maps to Settings > Jobs.
    pub fn go(&mut self, name: ViewName, params: ViewParams) {
        self.navigate(name, params);
    }

    /// go() for a caller that changed the selection as part of the move.
    fn navigate(&mut self, name: ViewName, params: ViewParams) {
        // Leave hooks run synchronously, capped by the caller's own flush work being fast.
        self.run_leave_hooks();
        let mut target = name;
        let mut params = params;
        if target == ViewName::Settings && params.get("section").is_none() {
            // "cron_tasks" was folded into Settings>Jobs by callers passing section: "jobs".
        }
        let meta = target.meta();
        if (meta.needs_taxpayer && self.is_fresh())
            || (meta.dev && !crate::bir::door_status().test_environment)
        {
            target = ViewName::Home;
            params = ViewParams::new();
        }
        let here = self.route.clone();
        if target == ViewName::Home {
            self.saved_returns_forgotten = false;
        }
        if target != here.view || params != here.params {
            self.history.push(Stop {
                route: here,
                sel: self.current_selection(),
            });
            if self.history.len() > 50 {
                self.history.remove(0);
            }
            self.forward.clear();
            self.seq += 1;
            self.route = Route {
                view: target,
                params,
                seq: self.seq,
            };
        }
        self.close_transient();
    }

    /// Shows another view in place of the current one: no history entry.
    pub fn replace(&mut self, name: ViewName, params: ViewParams) {
        self.run_leave_hooks();
        let meta = name.meta();
        if (meta.needs_taxpayer && self.is_fresh())
            || (meta.dev && !crate::bir::door_status().test_environment)
        {
            return;
        }
        self.seq += 1;
        self.route = Route {
            view: name,
            params,
            seq: self.seq,
        };
        self.close_transient();
    }

    /// Back one step, onto the taxpayer that screen was left on.
    pub fn back(&mut self) -> bool {
        let Some(at) = self.last_alive(&self.history) else {
            return false;
        };
        self.run_leave_hooks();
        let stop = self.history[at].clone();
        let here = Stop {
            route: self.route.clone(),
            sel: self.current_selection(),
        };
        self.restore_selection(&stop.sel);
        self.forward.push(here);
        self.history.truncate(at);
        self.seq += 1;
        self.route = Route {
            view: stop.route.view,
            params: stop.route.params,
            seq: self.seq,
        };
        self.close_transient();
        true
    }

    /// Forward one step, onto the taxpayer that screen was left on.
    pub fn forward_one(&mut self) -> bool {
        let Some(at) = self.last_alive(&self.forward) else {
            return false;
        };
        self.run_leave_hooks();
        let stop = self.forward[at].clone();
        let here = Stop {
            route: self.route.clone(),
            sel: self.current_selection(),
        };
        self.restore_selection(&stop.sel);
        self.history.push(here);
        self.forward.truncate(at);
        self.seq += 1;
        self.route = Route {
            view: stop.route.view,
            params: stop.route.params,
            seq: self.seq,
        };
        self.close_transient();
        true
    }

    pub fn can_back(&self) -> bool {
        self.last_alive(&self.history).is_some()
    }

    pub fn can_forward(&self) -> bool {
        self.last_alive(&self.forward).is_some()
    }

    /// Whether a stop can still be shown: its taxpayer is in the database.
    fn stop_alive(&self, stop: &Stop) -> bool {
        let taxpayers = self.taxpayers();
        if taxpayers.is_empty() {
            return true;
        }
        let roots: HashSet<&str> = taxpayers.iter().map(|t| t.tin_root.as_str()).collect();
        let target = if stop.route.view == ViewName::Form {
            form_target(&stop.route.params)
        } else {
            None
        };
        if let Some(root) = &stop.sel.root
            && !roots.contains(root.as_str())
        {
            return false;
        }
        if let Some(t) = &target
            && !roots.contains(t.tin.get(..9).unwrap_or(&t.tin))
        {
            return false;
        }
        true
    }

    fn last_alive(&self, stops: &[Stop]) -> Option<usize> {
        (0..stops.len()).rev().find(|&i| self.stop_alive(&stops[i]))
    }

    /// The form target a route's params name (params.target).
    pub fn form_target(&self) -> Option<FormTarget> {
        form_target(&self.route.params)
    }

    /// The form page shows another return than the one it opened: the route follows, the page stays.
    pub fn retarget(&mut self, target: FormTarget) {
        if self.route.view == ViewName::Form {
            self.route.params.insert(
                "target".to_owned(),
                serde_json::to_value(&target_json(&target)).unwrap_or(Value::Null),
            );
        }
        if target.mode == "return" {
            self.settle_amendment(
                &target.tin,
                &target.form_code,
                target.year,
                target.period,
                target.key,
            );
        }
    }

    /// An amendment that is stored now is a return: every place that would still start it again opens
    /// the saved return.
    fn settle_amendment(
        &mut self,
        tin: &str,
        code: &str,
        year: i64,
        period: i64,
        key: Option<i64>,
    ) {
        fn settle_route(
            r: &mut Route,
            tin: &str,
            code: &str,
            year: i64,
            period: i64,
            key: Option<i64>,
        ) {
            if r.view != ViewName::Form {
                return;
            }
            let Some(t) = form_target(&r.params) else {
                return;
            };
            if t.mode != "amend"
                || t.tin != tin
                || t.form_code != code
                || t.year != year
                || t.period != period
                || t.key != key
            {
                return;
            }
            let mut target = target_json(&t);
            target["mode"] = json!("return");
            target["key"] = json!(key);
            r.params.insert("target".to_owned(), target);
        }
        settle_route(&mut self.route, tin, code, year, period, key);
        for stop in &mut self.history {
            settle_route(&mut stop.route, tin, code, year, period, key);
        }
        for stop in &mut self.forward {
            settle_route(&mut stop.route, tin, code, year, period, key);
        }
    }

    pub fn open_profile(&mut self, edit_section: Option<&str>) {
        let mut params = ViewParams::new();
        if let Some(s) = edit_section {
            params.insert("edit".to_owned(), json!(s));
        }
        self.go(ViewName::Profile, params);
    }

    /// Search picked a taxpayer: it opens on its Tax profile.
    pub fn open_taxpayer(&mut self, root: &str) {
        let from = self.current_selection();
        if !self.taxpayer().is_some_and(|t| t.tin_root == root) {
            self.select_taxpayer(root, false, None);
        }
        self.navigate_with_from(ViewName::Profile, ViewParams::new(), from);
    }

    /// Search picked a branch: select it and open the Tax profile.
    pub fn open_branch(&mut self, root: &str, code: &str) {
        let from = self.current_selection();
        if self.taxpayer().is_some_and(|t| t.tin_root == root) {
            self.select_branch(code);
        } else {
            self.select_taxpayer(root, true, Some(code.to_owned()));
        }
        self.navigate_with_from(ViewName::Profile, ViewParams::new(), from);
    }

    /// go() whose history entry carries `from` (the selection the screen being left was on).
    fn navigate_with_from(&mut self, name: ViewName, params: ViewParams, from: Selection) {
        self.run_leave_hooks();
        let meta = name.meta();
        let (mut target, mut params) = (name, params);
        if (meta.needs_taxpayer && self.is_fresh())
            || (meta.dev && !crate::bir::door_status().test_environment)
        {
            target = ViewName::Home;
            params = ViewParams::new();
        }
        let here = self.route.clone();
        if target == ViewName::Home {
            self.saved_returns_forgotten = false;
        }
        if target != here.view || params != here.params {
            self.history.push(Stop {
                route: here,
                sel: from,
            });
            if self.history.len() > 50 {
                self.history.remove(0);
            }
            self.forward.clear();
            self.seq += 1;
            self.route = Route {
                view: target,
                params,
                seq: self.seq,
            };
        }
        self.close_transient();
    }

    // ---------------------------------------------------------------- recent returns

    fn push_recent(
        &mut self,
        tin: &str,
        code: &str,
        year: i64,
        period: i64,
        return_key: Option<Value>,
    ) {
        let entry = recent::recent_entry(tin, code, year, period, return_key);
        let next = recent::add_recent(&self.recent, entry);
        if next == self.recent {
            return;
        }
        self.recent = next;
        self.write_local(
            KEYS_RECENT,
            &serde_json::to_value(&self.recent).unwrap_or(Value::Null),
        );
    }

    /// A return was changed and saved: it moves to the top of Recent.
    pub fn bump_recent_return(
        &mut self,
        tin: &str,
        code: &str,
        year: i64,
        period: i64,
        return_key: Option<Value>,
    ) {
        self.settle_amendment(tin, code, year, period, target_key(return_key.as_ref()));
        let entry = recent::recent_entry(tin, code, year, period, return_key);
        self.recent = recent::bump_recent(&self.recent, entry);
        self.write_local(
            KEYS_RECENT,
            &serde_json::to_value(&self.recent).unwrap_or(Value::Null),
        );
    }

    /// Rewrite what names a TIN after a TIN or branch-code move (remapRecent).
    pub fn remap_recent(&mut self, map: &dyn Fn(&str) -> String) {
        for r in &mut self.recent {
            r.tin = map(&r.tin);
        }
        self.write_local(
            KEYS_RECENT,
            &serde_json::to_value(&self.recent).unwrap_or(Value::Null),
        );
        fn remap_route(r: &mut Route, map: &dyn Fn(&str) -> String) {
            if r.view != ViewName::Form {
                return;
            }
            if let Some(mut t) = form_target(&r.params) {
                let tin = map(&t.tin);
                if tin != t.tin {
                    t.tin = tin;
                    r.params.insert("target".to_owned(), target_json(&t));
                }
            }
        }
        remap_route(&mut self.route, map);
        for stop in self.history.iter_mut().chain(self.forward.iter_mut()) {
            remap_route(&mut stop.route, map);
        }
    }

    /// Opens a return on the form page.
    pub fn open_return(&mut self, opts: OpenReturnOptions) {
        if self.is_fresh() {
            self.toast(FRESH_HINT, None, None);
            return;
        }
        let Some(owner) = self.branch_for(&opts.tin) else {
            self.toast(
                "That return belongs to a taxpayer that is not in this database.",
                Some("triangle-alert"),
                None,
            );
            return;
        };
        let from = self.current_selection();
        if self.branch().is_none_or(|b| b.tin != owner.tin) {
            self.select_taxpayer(
                &opts.tin[..opts.tin.len().min(9)],
                true,
                Some(owner.code.clone()),
            );
        }
        self.overlay = None;
        let mode = opts.mode.unwrap_or_else(|| "return".to_owned());
        if mode == "return" || mode == "amend" {
            self.push_recent(
                &opts.tin,
                &opts.code,
                opts.year,
                opts.period,
                opts.return_key.clone(),
            );
        }
        let target = FormTarget {
            tin: opts.tin,
            form_code: opts.code,
            year: opts.year,
            period: opts.period,
            key: target_key(opts.return_key.as_ref()),
            mode,
            snapshot_id: opts.snapshot_id,
            new_key: opts.new_key,
            new_key_from: opts.new_key_from,
        };
        let mut params = ViewParams::new();
        params.insert("target".to_owned(), target_json(&target));
        self.navigate_with_from(ViewName::Form, params, from);
    }

    // ---------------------------------------------------------------- layers

    pub fn toggle_sidebar(&mut self) {
        if !self.chrome_visible() || self.narrow() {
            return;
        }
        self.sidebar_open = !self.sidebar_open;
        if self.sidebar_open && self.overlay_mode() {
            self.panel = None;
        }
    }

    /// A panel closes from its own trigger and Esc.
    pub fn toggle_panel(&mut self, which: PanelName) {
        if !self.chrome_visible() {
            return;
        }
        self.panel = if self.panel == Some(which) {
            None
        } else {
            Some(which)
        };
        if self.panel.is_some() {
            self.last_panel = which;
            if self.overlay_mode() {
                self.sidebar_open = false;
            }
        }
    }

    /// The title-bar panel button: the tax profile is its home — hide the panel, switch a
    /// notifications panel over to the profile, or reopen the profile. (Notifications stay
    /// reachable through the bell; this button never reopens them.)
    pub fn toggle_right_panel(&mut self) {
        self.toggle_panel(PanelName::Profile);
    }

    pub fn close_panel(&mut self) {
        self.panel = None;
    }

    /// The window crossed 860 points (or just opened): below it the sidebar and panel float over the
    /// page; back above it the sidebar docks open again. `follow_width` keeps the previous mode.
    pub fn follow_width(&mut self, overlays: bool, before: Option<bool>) {
        match (overlays, before) {
            (true, Some(false) | None) => {
                self.sidebar_open = false;
                if before == Some(false) {
                    self.panel = None;
                }
            }
            (false, Some(true)) => self.sidebar_open = true,
            _ => {}
        }
    }

    /// The scrim's press: the sidebar and the panel that float over the page both close.
    pub fn close_floating(&mut self) {
        self.sidebar_open = false;
        self.panel = None;
    }

    pub fn open_palette(&mut self) {
        if !self.chrome_visible() {
            return;
        }
        self.overlay = Some(OverlayName::Palette);
        self.palette_index = 0;
        self.palette_query.clear();
    }

    pub fn close_overlay(&mut self) {
        self.overlay = None;
    }

    /// Esc: the topmost layer closes. True when something closed.
    pub fn dismiss(&mut self) -> bool {
        if self.overlay.is_some() {
            self.overlay = None;
            return true;
        }
        if self.panel.is_some() {
            self.panel = None;
            return true;
        }
        if self.overlay_mode() && self.sidebar_open {
            self.sidebar_open = false;
            return true;
        }
        false
    }

    /// The title-bar and sidebar "+": the start-a-return dialog, or why it cannot open.
    pub fn start_new_return(&mut self, code: Option<&str>) {
        if !self.chrome_visible() {
            return;
        }
        if self.is_fresh() {
            self.toast(FRESH_HINT, None, None);
            return;
        }
        let (Some(b), Some(t)) = (self.branch(), self.taxpayer()) else {
            return;
        };
        if self.years().is_empty() {
            self.toast(
                format!(
                    "{} has no tax year yet. Add one to choose the forms it files.",
                    b.name
                ),
                None,
                None,
            );
            return;
        }
        let forms = self.year_forms();
        if forms.is_empty() {
            self.toast(
                format!(
                    "No forms for {} yet. Choose them under Forms first.",
                    self.year().unwrap_or(0)
                ),
                None,
                None,
            );
            return;
        }
        if let Some(c) = code
            && !forms.iter().any(|f| f == c)
        {
            self.toast(
                format!(
                    "{} does not file {c} in {}. Add it under Forms first.",
                    t.name,
                    self.year().unwrap_or(0)
                ),
                None,
                None,
            );
            return;
        }
        self.new_return_code = Some(code.map(str::to_owned).unwrap_or_else(|| forms[0].clone()));
        self.new_return_period = None;
        self.overlay = Some(OverlayName::New);
    }

    pub fn pick_new_return_form(&mut self, code: &str) {
        self.new_return_code = Some(code.to_owned());
        self.new_return_period = None;
    }

    pub fn pick_new_return_period(&mut self, period: i64) {
        self.new_return_period = Some(period);
    }

    /// "Open form" in the New return dialog: opens the picked form at the picked period (the
    /// first option when none was picked, as `period()` in NewReturn.tsx).
    pub fn confirm_new_return(&mut self) {
        let Some(branch) = self.branch() else {
            return;
        };
        let Some(code) = self.new_return_code.clone() else {
            return;
        };
        let Some(year) = self.year() else {
            return;
        };
        let freq = screen_index()
            .iter()
            .find(|f| f.c == code)
            .map(|f| crate::lib::periods::Frequency::parse(&f.freq))
            .unwrap_or(crate::lib::periods::Frequency::Annual);
        let opts =
            crate::lib::periods::period_options(&code, freq, year as i32, chrono::Local::now());
        let period = self
            .new_return_period
            .filter(|p| opts.iter().any(|o| o.value == *p))
            .unwrap_or_else(|| opts.first().map(|o| o.value).unwrap_or(0));
        self.open_return(OpenReturnOptions {
            tin: branch.tin,
            code,
            year,
            period,
            return_key: None,
            mode: None,
            snapshot_id: None,
            new_key: None,
            new_key_from: None,
        });
    }

    pub fn set_palette_query(&mut self, text: &str) {
        self.palette_query = text.to_owned();
        self.palette_index = 0;
    }

    pub fn set_palette_index(&mut self, i: usize) {
        self.palette_index = i;
    }

    /// Moving anywhere closes the layers that float over the page.
    fn close_transient(&mut self) {
        if self.overlay_mode() {
            self.sidebar_open = false;
            self.panel = None;
        }
    }

    // ---------------------------------------------------------------- theme and prefs

    pub fn set_theme_preference(&mut self, pref: ThemePreference) {
        self.theme_pref = pref;
        let _ = store::write(
            "set_setting",
            vec![json!(THEME_PREF_KEY), json!(pref.stored())],
        );
    }

    pub fn cycle_theme(&mut self) {
        let next = self.theme_pref.next();
        self.set_theme_preference(next);
    }

    pub fn set_pref(&mut self, show_recent: Option<bool>, compact_form_nav: Option<bool>) {
        if let Some(v) = show_recent {
            self.prefs.show_recent = v;
        }
        if let Some(v) = compact_form_nav {
            self.prefs.compact_form_nav = v;
        }
        self.write_local(
            KEYS_PREFS,
            &serde_json::to_value(&self.prefs).unwrap_or(Value::Null),
        );
    }

    // ---------------------------------------------------------------- notifications

    /// Submissions sent and confirmation emails received, newest first.
    pub fn notifications(&self) -> Vec<Notification> {
        let mut items = Vec::new();
        for r in returns_filed() {
            let confirmed = r.status == "Confirmed" || r.status == "Paid";
            if !confirmed && r.status != "Submitted" {
                continue;
            }
            let id = format!(
                "return:{}:{}",
                r.id,
                if confirmed { "confirmed" } else { "submitted" }
            );
            let who = if r.branch_count > 1 {
                format!("{} · {}", r.taxpayer_name, r.branch_name)
            } else {
                r.taxpayer_name.clone()
            };
            let unread = !self.read_ids.contains(&id);
            items.push(Notification {
                id,
                title: if confirmed {
                    format!("BIR confirmed {} for {}", r.form_code, r.period_label)
                } else {
                    format!("{} for {} was submitted", r.form_code, r.period_label)
                },
                sub: if confirmed {
                    format!("Confirmation email received · {who}")
                } else {
                    format!("Waiting for BIR's confirmation email · {who}")
                },
                icon: if confirmed { "circle-check" } else { "send" }.to_owned(),
                tone: if confirmed { "success" } else { "neutral" }.to_owned(),
                when: r.updated_at.clone(),
                unread,
            });
        }
        items
    }

    pub fn unread_count(&self) -> usize {
        self.notifications().iter().filter(|n| n.unread).count()
    }

    pub fn mark_all_read(&mut self) {
        for n in self.notifications() {
            self.read_ids.insert(n.id);
        }
        self.write_local(KEYS_READ, &json!(self.read_ids.iter().collect::<Vec<_>>()));
    }

    /// A notification is about a filed return: read it, and show Filings.
    pub fn open_notification(&mut self, id: &str) {
        self.read_ids.insert(id.to_owned());
        self.write_local(KEYS_READ, &json!(self.read_ids.iter().collect::<Vec<_>>()));
        self.go(ViewName::Filings, ViewParams::new());
    }

    // ---------------------------------------------------------------- hooks

    /// A screen with work to finish before the app leaves it registers it here.
    pub fn before_leave(&mut self, hook: LeaveHook) {
        self.leave_hooks.push(hook);
    }

    /// Runs every registered leave hook (synchronous; the TS capped at 3 s of awaiting).
    fn run_leave_hooks(&mut self) {
        let hooks = std::mem::take(&mut self.leave_hooks);
        // Hooks stay registered (the TS Set kept them); put them back after running.
        for h in &hooks {
            (h.borrow_mut())();
        }
        self.leave_hooks = hooks;
    }

    /// A screen that keeps state in its own signals registers the reset of that state here.
    pub fn on_session_change(&mut self, hook: SessionHook) {
        self.session_hooks.push(hook);
    }

    fn session_changed(&mut self) {
        for h in &self.session_hooks {
            (h.borrow_mut())();
        }
        // The frame's own reset: the palette's search and the New return dialog's picks.
        self.palette_query.clear();
        self.palette_index = 0;
        self.new_return_code = None;
        self.new_return_period = None;
    }

    // ---------------------------------------------------------------- session

    /// Whether the admin has a PIN or an authenticator, asked of the host now. `None` when it could
    /// not say (the app stays locked; access_error says why).
    pub fn read_access(&mut self) -> Option<bool> {
        match bir::call0("auth_state")
            .and_then(|v| serde_json::from_value::<AuthState>(v).map_err(|e| e.to_string()))
        {
            Ok(state) => {
                store::forget("auth_state");
                self.access_error = None;
                Some(state.has_admin_pin || state.has_admin_totp)
            }
            Err(e) => {
                self.access_error = Some(error_text(&e));
                None
            }
        }
    }

    /// The lock screen's Retry after the access state could not be read.
    pub fn retry_access(&mut self) {
        if self.access_error.is_none() {
            return;
        }
        if let Some(code) = self.read_access() {
            self.app_locked = code;
        }
    }

    /// Nobody can sign back in without a code: with no admin code a taxpayer-less session cannot lock.
    fn end_session(&mut self, message: Option<&str>) {
        let s = self.session.clone();
        if !matches!(s, Session::Taxpayer { .. }) && self.read_access() == Some(false) {
            self.toast("Set your PIN in Settings first. With no PIN there is nothing to sign back in with.", Some("lock"), None);
            return;
        }
        self.locked_from = Some(s);
        store::cancel_rereads();
        self.session = Session::Open;
        self.panel = None;
        self.overlay = None;
        self.app_locked = true;
        self.clear_toasts();
        self.session_changed();
        store::bump(None);
        self.apply_scope();
        if let Some(m) = message {
            self.toast(m, Some("log-out"), None);
        }
    }

    /// Sign-in succeeded: a different person starts from Home, with nothing of the last session.
    pub fn start_session(&mut self, next: Session) {
        let same = match (&self.locked_from, &next) {
            (Some(Session::Taxpayer { root: a }), Session::Taxpayer { root: b }) => a == b,
            (Some(Session::Admin), Session::Admin) | (Some(Session::Open), Session::Open) => true,
            _ => false,
        };
        self.session = next.clone();
        self.app_locked = false;
        self.locked_from = None;
        self.session_changed();
        store::bump(None);
        self.apply_scope();
        if let Session::Taxpayer { root } = &next {
            let t = self.taxpayers().into_iter().find(|t| &t.tin_root == root);
            self.sel_root = Some(root.clone());
            let keep = self.sel_code.clone();
            if !same
                || t.as_ref()
                    .is_none_or(|t| !t.branches.iter().any(|b| Some(&b.code) == keep.as_ref()))
            {
                self.sel_code = t.and_then(|t| t.branches.first().map(|b| b.code.clone()));
            }
            self.save_selection();
        }
        if !same {
            self.history.clear();
            self.forward.clear();
            self.seq += 1;
            self.route = Route {
                view: ViewName::Home,
                params: ViewParams::new(),
                seq: self.seq,
            };
            self.panel = None;
            self.overlay = None;
        }
    }

    pub fn lock_app(&mut self) {
        if !self.chrome_visible() {
            return;
        }
        self.end_session(None);
    }

    pub fn log_out(&mut self) {
        self.end_session(Some("Signed out."));
        self.locked_from = None;
    }

    /// Computer setup changed (Settings): the admin moved, so the old session ends.
    pub fn sign_out_after_setup_change(&mut self) {
        self.end_session(Some("Computer setup changed. Sign in as the admin with the new owner's PIN or the help desk PIN."));
        self.locked_from = None;
    }

    /// Tells the host so the menu bar matches the window (bir.shell_set_chrome).
    pub fn sync_shell_chrome(&mut self, open: bool) {
        let _ = store::write("shell_set_chrome", vec![json!(open)]);
    }

    fn apply_scope(&mut self) {
        let root = match &self.session {
            Session::Taxpayer { root } => Some(root.clone()),
            _ => None,
        };
        store::set_scope(root);
    }

    // ---------------------------------------------------------------- boot and the gates

    /// What the app keeps (theme, selection, Recent, read notifications, prefs), and whether
    /// sign-in comes first. Called when the account gate is open (loadKept).
    pub fn load_kept(&mut self, start: Option<String>) {
        self.kept_loaded = true;
        store::bump(None);
        self.database_path = store::system_info().database_path.clone();
        let stored_theme: Option<String> = self.read_local(THEME_PREF_KEY, None);
        self.theme_pref = start
            .as_deref()
            .and_then(|raw| crate::theme::theme_preference_from_start(Some(raw)))
            .unwrap_or_else(|| ThemePreference::parse(stored_theme.as_deref()));
        let stored: Value = self.read_local(KEYS_SELECTION, json!({}));
        self.sel_root = stored
            .get("root")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|s| !s.is_empty());
        self.sel_code = stored
            .get("code")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .filter(|s| !s.is_empty());
        let stored_recent: Value = self.read_local(KEYS_RECENT, json!([]));
        self.recent = recent::load_recent(&stored_recent);
        // First run against this database: what already happened is not news.
        let stored_read: Option<Vec<String>> = self.read_local(KEYS_READ, None);
        let had_stored = stored_read.is_some();
        let ids: HashSet<String> = match stored_read {
            Some(list) => list.into_iter().collect(),
            None => self.notifications().iter().map(|n| n.id.clone()).collect(),
        };
        self.read_ids = ids.clone();
        if !had_stored {
            self.write_local(KEYS_READ, &json!(ids.iter().collect::<Vec<_>>()));
        }
        self.known_events = self.notifications().iter().map(|n| n.id.clone()).collect();
        let stored_prefs: Value = self.read_local(KEYS_PREFS, json!({}));
        self.prefs = serde_json::from_value(stored_prefs).unwrap_or_default();
        self.apply_scope();
        // The admin has a PIN or an authenticator: the sign-in screen comes first.
        self.app_locked = self.read_access() != Some(false);
    }

    /// Asks the host whether the Buwiz account gate is open; if so loads what the app keeps, then
    /// shows the frame (or what stands in front of it). Idempotent.
    pub fn boot(&mut self, requested: Option<String>) {
        if self.booted {
            return;
        }
        let test_env = crate::bir::door_status().test_environment;
        let start = allowed_start(requested.as_deref(), test_env);
        self.first_run = true; // the shipped app turns first-run setup on
        self.boot_start = start.clone();
        self.start_params = start
            .as_deref()
            .map(|s| {
                parse_start(Some(s))
                    .into_iter()
                    .map(|st| (st.key, st.value))
                    .collect()
            })
            .unwrap_or_default();
        // The dev environment's host bypass answers unlocked.
        let mut unlocked = false;
        match bir::call0("cloud_account_gate")
            .and_then(|v| serde_json::from_value::<serde_json::Value>(v).map_err(|e| e.to_string()))
        {
            Ok(gate) => {
                unlocked = gate
                    .get("unlocked")
                    .and_then(Value::as_bool)
                    .unwrap_or(false);
            }
            Err(e) => {
                self.gate_error = Some(error_text(&e));
            }
        }
        if unlocked {
            self.load_kept(start.clone());
        } else {
            self.theme_pref = start
                .as_deref()
                .and_then(|raw| crate::theme::theme_preference_from_start(Some(raw)))
                .unwrap_or(ThemePreference::System);
            self.cloud_locked = true;
        }
        self.booted = true;
        if unlocked && start.is_some() {
            self.run_start_route(&start.unwrap());
        }
    }

    /// The dev start route's steps, applied once the account gate is open.
    fn run_start_route(&mut self, raw: &str) {
        for step in parse_start(Some(raw)) {
            match step.key.as_str() {
                "tp" => self.select_taxpayer(&step.value, true, None),
                "branch" => self.select_branch(&step.value),
                "theme" => {
                    self.set_theme_preference(ThemePreference::parse(Some(&step.value)));
                }
                "view" => {
                    if step.value == "cron_tasks" {
                        let mut p = ViewParams::new();
                        p.insert("section".to_owned(), json!("jobs"));
                        self.go(ViewName::Settings, p);
                    } else if let Some(v) = ViewName::parse(&step.value) {
                        self.go(v, ViewParams::new());
                    }
                }
                "section" => {
                    let mut p = ViewParams::new();
                    p.insert("section".to_owned(), json!(step.value));
                    self.go(ViewName::Settings, p);
                }
                "panel" => self.toggle_panel(if step.value == "notifs" {
                    PanelName::Notifs
                } else {
                    PanelName::Profile
                }),
                "palette" => {
                    self.open_palette();
                    self.set_palette_query(&step.value);
                }
                "form" => {
                    if let Some(f) = parse_start_form(&step.value)
                        && let Some(b) = self.branch()
                    {
                        self.open_return(OpenReturnOptions {
                            tin: b.tin.clone(),
                            code: f.code,
                            year: f.year,
                            period: f.period,
                            return_key: f.key.map(Value::from),
                            mode: None,
                            snapshot_id: None,
                            new_key: None,
                            new_key_from: None,
                        });
                    }
                }
                _ => {}
            }
        }
    }

    /// The Buwiz account approved this computer: the host opens the session and the filing UI shows.
    pub fn finish_unlock(&mut self) -> bool {
        if let Err(e) = bir::call0("cloud_account_session_opened") {
            self.gate_error = Some(error_text(&e));
            self.cloud_locked = true;
            return false;
        }
        self.gate_error = None;
        if !self.kept_loaded {
            self.load_kept(self.boot_start.clone());
            self.cloud_locked = false;
            if let Some(start) = self.boot_start.clone() {
                self.run_start_route(&start);
            }
        } else {
            if self.cloud_locked {
                self.session_changed();
            }
            store::bump(None);
            self.cloud_locked = false;
        }
        true
    }

    /// Signed out of the Buwiz account: the account gate comes back.
    pub fn lock_cloud(&mut self) {
        let _ = bir::call0("cloud_account_session_closed");
        self.gate_error = None;
        self.cloud_locked = true;
        store::cancel_rereads();
        self.clear_toasts();
        self.session_changed();
    }

    // ---------------------------------------------------------------- polls (the frame ticks these)

    /// One frame's worth of poll work: events, device filings, update status, the queued watch and
    /// the library warm-up. Cheap when nothing is due.
    pub fn tick(&mut self) {
        store::take_due_rereads();
        let now = Instant::now();
        // The three polls run in the shipped app; in a test environment they wait for
        // BIR_SHELL_POLL=1 (App.tsx `polling`). The queued watch and the library warm-up always run.
        if self.polling_enabled() {
            if now.duration_since(self.last_events) >= Duration::from_millis(POLL_EVENTS_MS) {
                self.last_events = now;
                self.poll_events();
            }
            if now.duration_since(self.last_device) >= Duration::from_millis(POLL_DEVICE_FILINGS_MS)
            {
                self.last_device = now;
                self.pull_device_filings();
            }
            if now.duration_since(self.last_update) >= Duration::from_millis(POLL_UPDATE_MS) {
                self.last_update = now;
                self.watch_update();
            }
        }
        if self.chrome_visible()
            && now.duration_since(self.last_queued) >= Duration::from_millis(QUEUED_POLL_MS)
        {
            self.last_queued = now;
            self.queued_look();
        }
        self.warm_library(now);
    }

    /// Re-reads what the submission worker changes and announces events that appeared.
    fn poll_events(&mut self) {
        if self.cloud_locked || self.app_locked {
            return;
        }
        store::forget_where(store::poll_key);
        let notifs = self.notifications();
        let fresh: Vec<FreshEvent> = take_fresh(&mut self.known_events, &notifs)
            .into_iter()
            .map(|n| FreshEvent {
                title: n.title,
                icon: n.icon,
            })
            .collect();
        if let Some((message, icon)) = crate::lib::events::event_toast(&fresh) {
            self.toast(message, Some(&icon), None);
        }
    }

    /// An assistant queued a filing on the account server: this computer uploads it.
    fn pull_device_filings(&mut self) {
        if self.cloud_locked {
            return;
        }
        let Ok(note) = bir::call0("pull_device_filings") else {
            return;
        };
        if let Some(message) = note.get("message").and_then(Value::as_str)
            && !message.is_empty()
            && !self.app_locked
        {
            let icon = note.get("icon").and_then(Value::as_str).unwrap_or("send");
            self.toast(message, Some(icon), None);
        }
    }

    /// The host checks for a release on its own thread; polling is how the prompt appears.
    fn watch_update(&mut self) {
        if self.cloud_locked {
            return;
        }
        let status = bir::desktop_update_status();
        let changed = match &self.update_status {
            Some(prev) => {
                prev.phase != status.phase
                    || prev.version != status.version
                    || prev.message != status.message
                    || prev.auto_download != status.auto_download
            }
            None => true,
        };
        if changed {
            self.update_status = Some(status);
        }
    }

    /// The ids of the Queued returns in the filed list, as one sorted text; "" for none.
    fn queued_key(&self) -> String {
        if self.cloud_locked || self.app_locked {
            return String::new();
        }
        let mut ids: Vec<i64> = returns_filed()
            .into_iter()
            .filter(|r| r.status == "Queued")
            .map(|r| r.id)
            .collect();
        ids.sort_unstable();
        ids.iter().map(i64::to_string).collect::<Vec<_>>().join(",")
    }

    /// One look of the Queued watch.
    fn queued_look(&mut self) {
        let queued = self.queued_key();
        if queued != self.queued_seen {
            self.queued_seen = queued.clone();
            self.queued_changed_at = Instant::now();
        }
        let last_action = store::last_queue_action().unwrap_or(self.queued_changed_at);
        let changed_at = self.queued_changed_at.max(last_action);
        if !queued.is_empty() && changed_at.elapsed() < Duration::from_millis(QUEUED_FAST_MS) {
            self.poll_events_worker();
        }
    }

    /// pollEvents with the wider WORKER_KEYS.
    fn poll_events_worker(&mut self) {
        if self.cloud_locked || self.app_locked {
            return;
        }
        store::forget_where(store::worker_key);
        let notifs = self.notifications();
        let fresh: Vec<FreshEvent> = take_fresh(&mut self.known_events, &notifs)
            .into_iter()
            .map(|n| FreshEvent {
                title: n.title,
                icon: n.icon,
            })
            .collect();
        if let Some((message, icon)) = crate::lib::events::event_toast(&fresh) {
            self.toast(message, Some(&icon), None);
        }
    }

    /// The polls' gate (`polling` in App.tsx): on outside a test environment, or with
    /// BIR_SHELL_POLL=1 inside one.
    fn polling_enabled(&self) -> bool {
        !crate::bir::door_status().test_environment
            || std::env::var("BIR_SHELL_POLL").as_deref() == Ok("1")
    }

    /// Builds the form library's index once, WARM_DELAY_MS after the frame is first shown; a
    /// lock that covers the frame again within the delay re-arms it (warmLibrary in app.ts).
    fn warm_library(&mut self, now: Instant) {
        if self.last_warm.is_some() {
            return;
        }
        if !self.chrome_visible() {
            self.warm_since = None;
            return;
        }
        let since = *self.warm_since.get_or_insert(now);
        if now.duration_since(since) < Duration::from_millis(WARM_DELAY_MS) {
            return;
        }
        self.last_warm = Some(now);
        let _ = screen_index();
    }

    /// Quits the app once the screens' leave hooks are done: the door's quit path, then the window.
    pub fn quit(&mut self) {
        self.run_leave_hooks();
        self.quit_requested = true;
    }
}

// ---------------------------------------------------------------- form target (serde)

/// Reads `params.target` into a FormTarget.
pub fn form_target(params: &ViewParams) -> Option<FormTarget> {
    let v = params.get("target")?;
    let get = |k: &str| v.get(k).cloned().unwrap_or(Value::Null);
    let form_code = get("formCode");
    let form_code = if form_code.is_null() {
        get("form_code")
    } else {
        form_code
    };
    Some(FormTarget {
        tin: get("tin").as_str().unwrap_or("").to_owned(),
        form_code: form_code.as_str().unwrap_or("").to_owned(),
        year: get("year").as_i64().unwrap_or(0),
        period: get("period").as_i64().unwrap_or(0),
        key: get("key").as_i64(),
        mode: get("mode").as_str().unwrap_or("return").to_owned(),
        snapshot_id: {
            let s = get("snapshotId");
            if s.is_null() {
                get("snapshot_id").as_i64()
            } else {
                s.as_i64()
            }
        },
        new_key: opt_value(&get("newKey")),
        new_key_from: opt_value(&get("newKeyFrom")),
    })
}

fn opt_value(v: &Value) -> Option<Value> {
    if v.is_null() { None } else { Some(v.clone()) }
}

fn target_json(t: &FormTarget) -> Value {
    json!({
        "tin": t.tin,
        "formCode": t.form_code,
        "year": t.year,
        "period": t.period,
        "key": t.key,
        "mode": t.mode,
        "snapshotId": t.snapshot_id,
        "newKey": t.new_key,
        "newKeyFrom": t.new_key_from,
    })
}

/// The key a FormTarget holds for a stored return's key.
pub fn target_key(key: Option<&Value>) -> Option<i64> {
    key.and_then(|k| {
        k.as_i64()
            .or_else(|| k.as_str().and_then(|s| s.parse().ok()))
    })
}

// ---------------------------------------------------------------- store additions used above

/// `store.returns(which)`: filed/saved/all returns, decoded.
pub fn returns(which: &str) -> Vec<ReturnRow> {
    store::read(&format!("returns:{which}"), Vec::new(), || {
        bir::call1("list_returns", json!(which))
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    })
}

/// Filed returns only (notifications, queued watch).
pub fn returns_filed() -> Vec<ReturnRow> {
    returns("filed")
}

/// Saved (draft) returns (Home's Drafts tile).
pub fn saved_returns() -> Vec<ReturnRow> {
    returns("saved")
}

/// `store.deadlines(from, to)` filtered to what the session may see.
pub fn deadlines(from: &str, to: &str) -> Vec<crate::types::DeadlineRow> {
    let rows: Vec<crate::types::DeadlineRow> =
        store::read(&format!("deadlines:{from}:{to}"), Vec::new(), || {
            bir::call("deadlines_between", vec![json!(from), json!(to)])
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        });
    rows.into_iter()
        .filter(|d| store::can_see(&d.tin_root))
        .collect()
}

/// Today's ISO date (local).
pub fn today_iso() -> String {
    iso_date(chrono::Local::now())
}

/// `store.asNeeded(year)`: the as-needed forms each branch files in year.
pub fn as_needed(year: i64) -> Vec<crate::types::AsNeededForm> {
    let rows: Vec<crate::types::AsNeededForm> =
        store::read(&format!("as_needed:{year}"), Vec::new(), || {
            bir::call1("as_needed_forms", json!(year))
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        });
    rows.into_iter()
        .filter(|f| store::can_see(&f.tin_root))
        .collect()
}

/// `store.taxpayerCreds(root)`: PIN and authenticator of a taxpayer.
pub fn taxpayer_creds(root: &str) -> crate::types::ProfileAuth {
    store::read(
        &format!("tpauth:{root}"),
        crate::types::ProfileAuth::default(),
        || {
            bir::call1("taxpayer_auth", json!(root))
                .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
        },
    )
}

/// `store.setting(key)`: a host setting's text, or null.
pub fn setting(key: &str) -> Option<String> {
    store::read(&format!("setting:{key}"), None, || {
        bir::call1("get_setting", json!(key)).map(|v| v.as_str().map(str::to_owned))
    })
}

/// `store.backgroundJobs()`.
pub fn background_jobs() -> Vec<crate::types::BackgroundJob> {
    store::read("jobs", Vec::new(), || {
        bir::call0("list_background_jobs")
            .and_then(|v| serde_json::from_value(v).map_err(|e| e.to_string()))
    })
}

/// `store.rdoOptions()`: RDO choices as { value, label }. Static, kept past bump().
pub fn rdo_options() -> Vec<(String, String)> {
    static CACHE: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            bir::call0("list_rdos")
                .ok()
                .and_then(|v| serde_json::from_value::<Vec<serde_json::Value>>(v).ok())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|r| {
                    let code = r.get("code")?.as_str()?.to_owned();
                    let desc = r.get("description")?.as_str()?.to_owned();
                    Some((code.clone(), format!("{code} - {desc}")))
                })
                .collect()
        })
        .clone()
}

/// `store.zipOptions()`: ZIP choices as { value, label }. Static.
pub fn zip_options() -> Vec<(String, String)> {
    static CACHE: std::sync::OnceLock<Vec<(String, String)>> = std::sync::OnceLock::new();
    CACHE
        .get_or_init(|| {
            bir::call0("list_zip_codes")
                .ok()
                .and_then(|v| serde_json::from_value::<Vec<String>>(v).ok())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|z| {
                    let value = z.split(" - ").next()?.trim().to_owned();
                    Some((value, z))
                })
                .collect()
        })
        .clone()
}

/// One branch's facts for a tax year, who holds each and which forms read them (`store.yearFacts`);
/// Null when the read failed.
pub fn year_facts(tin: &str, year: i64) -> Value {
    store::read(&format!("year_facts:{tin}:{year}"), Value::Null, || {
        bir::call("year_facts", vec![json!(tin), json!(year)])
    })
}

/// Whether the year's income-tax election can be recorded (`store.electionReadiness`).
pub fn election_readiness(tin: &str, year: i64) -> Value {
    store::read(&format!("election_ready:{tin}:{year}"), Value::Null, || {
        bir::call(
            "income_tax_election_readiness",
            vec![json!(tin), json!(year)],
        )
    })
}

/// `store.screenIndex()`: the form library's index, built once and kept past bump() (about a second
/// in a debug build). A failed build answers [] until the backoff is due.
pub fn screen_index() -> Vec<ScreenFormIndexEntry> {
    static CACHE: std::sync::OnceLock<Vec<ScreenFormIndexEntry>> = std::sync::OnceLock::new();
    if let Some(rows) = CACHE.get() {
        return rows.clone();
    }
    let rows: Vec<ScreenFormIndexEntry> = bir::call0("screen_forms_index")
        .ok()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    if !rows.is_empty() {
        let _ = CACHE.set(rows.clone());
    }
    rows
}

/// "Cancel queue": the return is an editable draft again, or the person is told why not
/// (shell/queue.ts cancelQueue). True when the queue was cancelled.
pub fn cancel_queue(
    app: &mut AppState,
    tin: &str,
    code: &str,
    year: i64,
    period: Option<i64>,
) -> bool {
    match bir::call(
        "cancel_queued",
        vec![json!(tin), json!(code), json!(year), json!(period)],
    ) {
        Ok(_) => {
            app.toast(
                "Queue cancelled. The return is an editable draft again.",
                None,
                None,
            );
            true
        }
        Err(e) => {
            if !crate::lib::queue_message::is_already_sent(&e) {
                store::log_screen("error", &format!("bir.cancel_queued: {}", error_text(&e)));
            }
            app.toast(
                crate::lib::queue_message::cancel_queue_message(&e),
                Some("triangle-alert"),
                None,
            );
            false
        }
    }
    // store.bump("cancel_queued") either way.
    // (bump is called after the match — see below)
}
