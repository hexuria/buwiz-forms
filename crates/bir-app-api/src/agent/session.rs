//! UI-thread host ops. Each one calls bir-core through the shell's database
//! and, when the screen should move, pushes a command the script applies.

use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::filing_queue::QueueAuthSource;
use bir_core::forms::agent::{self, AgentFormKind, Binding, CardBox, FromProfile};
use bir_core::forms::queueable::QueueableForm;
use bir_core::forms::{
    FilingStatus, Form1601CDraft, Form2551QDraft, FormSetSource, PerYearFormsSet,
};
use bir_core::profile::TaxpayerProfile;
use bir_core::profile::TaxpayerType;
use bir_core::tax_profile::{
    BranchFacts, BranchField, BranchUpdate, FieldSetting, FormTemplate, NewTaxpayer, TaxpayerUpdate,
};
use chrono::Datelike;
use gpui_agent::protocol::{DeliveryMode, HelloAuth, HelloInfo, Op, PlatformKind};
use gpui_agent::tree::UiTree;
use gpui_agent::{AgentHost, DispatchResult, PROTOCOL_VERSION};
use serde_json::{Map, Value, json};

use super::{APP, database_kind, tree_from_debug};

/// Commands waiting for `bir.agent_take`.
#[derive(Clone)]
pub struct UiQueue {
    inner: Arc<Mutex<VecDeque<Value>>>,
    discard: bool,
}

impl UiQueue {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            discard: false,
        }
    }

    pub fn discarding() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
            discard: true,
        }
    }

    pub fn push(&self, value: Value) {
        if !self.discard {
            self.inner.lock().expect("agent ui queue").push_back(value);
        }
    }

    /// Whether a window takes what is pushed.
    pub fn shown(&self) -> bool {
        !self.discard
    }

    pub fn pop(&self) -> Option<Value> {
        if self.discard {
            None
        } else {
            self.inner.lock().expect("agent ui queue").pop_front()
        }
    }
}

impl Default for UiQueue {
    fn default() -> Self {
        Self::new()
    }
}

struct OpenForm {
    kind: AgentFormKind,
    year: u16,
    period: u8,
    tin: String,
    draft: Value,
    sources: BTreeMap<String, String>,
    dirty: bool,
    /// A fill is held in the window for the filer's approval (it removes
    /// what they typed): nothing of it is stored, and every call reads the
    /// return again, as the filer's answer left it, until a fill is stored
    /// (the filer may answer at any time).
    awaiting: bool,
}

pub struct Session {
    db: Option<Arc<Mutex<Database>>>,
    commands: UiQueue,
    selected_tin: Option<String>,
    open: Option<OpenForm>,
    tree: UiTree,
    platform: PlatformKind,
    quit: bool,
}

impl Session {
    pub fn new(commands: UiQueue) -> Self {
        Self {
            db: None,
            commands,
            selected_tin: None,
            open: None,
            tree: tree_from_debug(None),
            platform: PlatformKind::Desktop,
            quit: false,
        }
    }

    pub fn headless() -> Self {
        let mut session = Self::new(UiQueue::discarding());
        session.platform = PlatformKind::Headless;
        session
    }

    pub fn set_db(&mut self, db: Arc<Mutex<Database>>) {
        self.db = Some(db);
    }

    pub fn needs_db(&self) -> bool {
        self.db.is_none()
    }

    pub fn set_tree(&mut self, tree: UiTree) {
        self.tree = tree;
    }

    pub fn should_quit(&self) -> bool {
        self.quit
    }

    fn lock_db(&self) -> Result<std::sync::MutexGuard<'_, Database>, String> {
        self.db
            .as_ref()
            .ok_or_else(|| "agent host has no database".to_string())?
            .lock()
            .map_err(|error| error.to_string())
    }

    fn invoke(&mut self, name: &str, args: &Value) -> Result<DispatchResult, String> {
        // Dismissing reads nothing of the return: a held return that can no
        // longer be read again must still be closable.
        if name != "form.dismiss" {
            self.settle()?;
        }
        match INVOKES.iter().find(|invoke| invoke.name == name) {
            Some(invoke) => (invoke.run)(self, args),
            None => Err(format!("unknown invoke `{name}`")),
        }
    }

    fn navigate(&mut self, page: &str) -> Result<DispatchResult, String> {
        let view = view_from_page(page)?;
        self.commands.push(json!({"op": "go", "view": view}));
        Ok(DispatchResult::json(json!({"view": view})))
    }

    fn click(&mut self, target: &str) -> Result<DispatchResult, String> {
        match target.strip_prefix("nav-") {
            Some(page) => self.navigate(page),
            None => Err(format!(
                "unhandled click `{target}`; bir-shell navigates `nav-*` and fills forms with form.fill"
            )),
        }
    }

    fn app_info(&self) -> Result<DispatchResult, String> {
        let profiles = self.profile_rows()?;
        let profiles: Vec<Value> = profiles
            .into_iter()
            .filter(|row| row.get("archived").and_then(Value::as_bool) != Some(true))
            .map(|mut row| {
                if let Value::Object(object) = &mut row {
                    object.remove("last4");
                    object.remove("branch");
                    object.remove("rdo");
                    object.remove("selected");
                    object.remove("archived");
                }
                row
            })
            .collect();
        Ok(DispatchResult::json(json!({
            "app": APP,
            "mode": "desktop",
            "database": database_kind(),
            "profiles": profiles,
        })))
    }

    fn profile_list(&self) -> Result<DispatchResult, String> {
        Ok(DispatchResult::json(Value::Array(self.profile_rows()?)))
    }

    fn profile_rows(&self) -> Result<Vec<Value>, String> {
        let db = self.lock_db()?;
        let taxpayers = db.list_taxpayers().map_err(|error| error.to_string())?;
        let mut rows = Vec::new();
        for taxpayer in taxpayers {
            for branch in &taxpayer.branches {
                rows.push(json!({
                    "tin": branch.tin,
                    "name": taxpayer.taxpayer.name,
                    "last4": last4(&branch.tin),
                    "branch": branch.branch_code,
                    "rdo": branch.value(BranchField::RdoCode),
                    "selected": self.selected_tin.as_deref() == Some(branch.tin.as_str()),
                    "archived": taxpayer.taxpayer.archived,
                }));
            }
        }
        Ok(rows)
    }

    fn profile_set(&mut self, args: &Value) -> Result<DispatchResult, String> {
        let view = match args.get("view").and_then(Value::as_str) {
            Some(page) => Some(view_from_page(page)?),
            None => None,
        };
        match self.resolve_tin(args)? {
            TinQuery::Missing => Err("profile.set requires args.tin or args.q".into()),
            TinQuery::NotFound { query } => Ok(DispatchResult::json(json!({
                "status": "not_found",
                "query": query,
                "candidates": []
            }))),
            TinQuery::Ambiguous { query, candidates } => Ok(DispatchResult::json(json!({
                "status": "ambiguous",
                "query": query,
                "candidates": candidates
            }))),
            TinQuery::One {
                tin,
                root,
                candidates,
            } => {
                self.selected_tin = Some(tin.clone());
                self.commands
                    .push(json!({"op": "refresh", "tin_root": root}));
                if let Some(view) = view {
                    self.commands.push(json!({"op": "go", "view": view}));
                }
                Ok(DispatchResult::json(json!({
                    "status": "ok",
                    "selected": {"tin": tin},
                    "candidates": candidates
                })))
            }
        }
    }

    fn profile_save(&mut self, args: &Value) -> Result<DispatchResult, String> {
        let (full, root) = tin_parts(
            args.get("tin")
                .and_then(Value::as_str)
                .or_else(|| args.get("tin_root").and_then(Value::as_str))
                .ok_or("profile.save requires args.tin")?,
        )?;
        let name = optional_str(args, "name");
        let birth = optional_str(args, "birth_date")
            .map(|raw| {
                chrono::NaiveDate::parse_from_str(&raw, "%Y-%m-%d")
                    .map_err(|_| format!("`{raw}` is not a date (YYYY-MM-DD)"))
            })
            .transpose()?;
        let facts = BranchFacts {
            rdo_code: optional_str(args, "rdo_code").unwrap_or_default(),
            registered_address: optional_str(args, "registered_address")
                .or_else(|| optional_str(args, "address"))
                .unwrap_or_default(),
            zip_code: optional_str(args, "zip_code")
                .or_else(|| optional_str(args, "zip"))
                .unwrap_or_default(),
            phone: optional_str(args, "phone").unwrap_or_default(),
            email: optional_str(args, "email").unwrap_or_default(),
        };
        let db = self.lock_db()?;
        let existing = db.get_taxpayer(&root).map_err(|error| error.to_string())?;
        let created = existing.is_none();
        let saved_name = if let Some(existing) = existing {
            if name.is_some() || birth.is_some() {
                db.update_taxpayer(
                    &root,
                    TaxpayerUpdate {
                        name: name.clone(),
                        birth_date: birth.map(Some),
                        ..TaxpayerUpdate::default()
                    },
                )
                .map_err(|error| error.to_string())?;
            }
            let saved_name = name
                .clone()
                .unwrap_or_else(|| existing.taxpayer.name.clone());
            if facts_present(args) {
                let mut fields = BTreeMap::new();
                if args.get("rdo_code").is_some() {
                    fields.insert(
                        BranchField::RdoCode,
                        FieldSetting::Override(facts.rdo_code.clone()),
                    );
                }
                if args.get("registered_address").is_some() || args.get("address").is_some() {
                    fields.insert(
                        BranchField::RegisteredAddress,
                        FieldSetting::Override(facts.registered_address.clone()),
                    );
                }
                if args.get("zip_code").is_some() || args.get("zip").is_some() {
                    fields.insert(
                        BranchField::ZipCode,
                        FieldSetting::Override(facts.zip_code.clone()),
                    );
                }
                if args.get("phone").is_some() {
                    fields.insert(
                        BranchField::Phone,
                        FieldSetting::Override(facts.phone.clone()),
                    );
                }
                if args.get("email").is_some() {
                    fields.insert(
                        BranchField::Email,
                        FieldSetting::Override(facts.email.clone()),
                    );
                }
                db.update_branch(&full, BranchUpdate { name: None, fields })
                    .map_err(|error| error.to_string())?;
            }
            saved_name
        } else {
            let name =
                name.ok_or("profile.save requires args.name when the taxpayer does not exist")?;
            let created = db
                .create_taxpayer(NewTaxpayer {
                    tin_root: root.clone(),
                    name: name.clone(),
                    taxpayer_type: taxpayer_type(optional_str(args, "taxpayer_type").as_deref()),
                    birth_date: birth,
                    head_office: facts,
                })
                .map_err(|error| error.to_string())?;
            created.taxpayer.name
        };
        if db
            .get_setting("access_mode")
            .map_err(|error| error.to_string())?
            .is_none()
        {
            db.set_setting("admin_tin_root", &root)
                .map_err(|error| error.to_string())?;
            db.set_setting("access_mode", "solo")
                .map_err(|error| error.to_string())?;
        }
        drop(db);
        self.selected_tin = Some(full.clone());
        self.commands
            .push(json!({"op": "refresh", "tin_root": root}));
        Ok(DispatchResult::json(json!({
            "status": "ok",
            "tin": full,
            "name": saved_name,
            "created": created,
        })))
    }

    fn profile_forms_set(&mut self, args: &Value) -> Result<DispatchResult, String> {
        require_forms_set_confirm(args, "profile.forms_set")?;
        let year = require_year(args, "profile.forms_set")?;
        let tin = match self.resolve_tin(args)? {
            TinQuery::Missing => self.selected_tin.clone().ok_or_else(|| {
                "profile.forms_set requires args.tin or args.q or a selected profile".to_string()
            })?,
            TinQuery::NotFound { query } => {
                return Ok(DispatchResult::json(json!({
                    "status": "not_found",
                    "query": query,
                    "candidates": []
                })));
            }
            TinQuery::Ambiguous { query, candidates } => {
                return Ok(DispatchResult::json(json!({
                    "status": "ambiguous",
                    "query": query,
                    "candidates": candidates
                })));
            }
            TinQuery::One { tin, .. } => tin,
        };
        let codes = parse_forms_set_codes(args)?;
        let reason = optional_str(args, "reason");
        let mut set = PerYearFormsSet::from_codes(year, codes, FormSetSource::Manual);
        if let Some(reason) = &reason {
            for entry in &mut set.entries {
                entry.reason = Some(reason.clone());
            }
        }
        let db = self.lock_db()?;
        db.save_per_year_forms(&tin, year, &set)
            .map_err(|error| error.to_string())?;
        Ok(DispatchResult::json(json!({
            "status": "ok",
            "tin": tin,
            "year": year,
            "codes": set.active_form_codes(),
            "source": FormSetSource::Manual.as_str(),
            "confirm": true,
            "reason": reason,
        })))
    }

    fn dues_list(&mut self, args: &Value) -> Result<DispatchResult, String> {
        let filter = match args
            .get("filter")
            .and_then(Value::as_str)
            .unwrap_or("upcoming")
        {
            "upcoming" | "overdue" | "all" => args
                .get("filter")
                .and_then(Value::as_str)
                .unwrap_or("upcoming")
                .to_string(),
            other => {
                return Err(format!(
                    "unknown dues filter `{other}` (expected upcoming, overdue, or all)"
                ));
            }
        };
        let scope = match args.get("scope").and_then(Value::as_str) {
            None if self.selected_tin.is_some() => "profile".to_string(),
            None => "global".to_string(),
            Some("profile") | Some("global") => args
                .get("scope")
                .and_then(Value::as_str)
                .unwrap_or("global")
                .to_string(),
            Some(other) => {
                return Err(format!(
                    "unknown dues scope `{other}` (expected profile or global)"
                ));
            }
        };
        if scope == "profile" && self.selected_tin.is_none() {
            return Err("dues.list scope=profile requires a selected profile".into());
        }
        let today = chrono::Local::now().date_naive();
        let db = self.lock_db()?;
        let deadlines = db.list_tax_deadlines().map_err(|error| error.to_string())?;
        let dues: Vec<Value> = deadlines
            .into_iter()
            .filter(
                |deadline| match (filter.as_str(), parse_due(&deadline.due_date)) {
                    ("all", _) => true,
                    ("upcoming", Some(date)) => date >= today,
                    ("overdue", Some(date)) => date < today,
                    _ => false,
                },
            )
            .map(|deadline| {
                json!({
                    "form_type": deadline.form_type,
                    "due_date": deadline.due_date,
                    "description": deadline.description,
                })
            })
            .collect();
        Ok(DispatchResult::json(json!({
            "filter": filter,
            "scope": scope,
            "as_of": today.to_string(),
            "date_basis": "local-calendar-date (chrono::Local::now().date_naive())",
            "dues": dues,
        })))
    }

    fn form_open(&mut self, args: &Value) -> Result<DispatchResult, String> {
        let code = args
            .get("code")
            .and_then(Value::as_str)
            .ok_or("filing.start requires args.code")?;
        let kind =
            AgentFormKind::from_code(code).ok_or_else(|| format!("unknown form `{code}`"))?;
        let year = optional_year(args).unwrap_or_else(|| chrono::Local::now().year() as u16);
        let period = optional_u8(args, "period").unwrap_or(0);
        let (tin, root) = match self.resolve_tin(args)? {
            TinQuery::Missing => {
                let tin = self
                    .selected_tin
                    .clone()
                    .ok_or("form.open requires a taxpayer (args.tin or profile.set)")?;
                let (_, root) = tin_parts(&tin)?;
                (tin, root)
            }
            TinQuery::NotFound { query } => {
                return Err(format!("filing.start taxpayer not found for {query}"));
            }
            TinQuery::Ambiguous { query, .. } => {
                return Err(format!(
                    "filing.start taxpayer {query} is ambiguous; use args.tin as 14 digits"
                ));
            }
            TinQuery::One { tin, root, .. } => (tin, root),
        };
        if let Some(open) = &self.open
            && !(open.kind == kind && open.year == year && open.period == period && open.tin == tin)
        {
            return Err(format!(
                "form already open: {} {} period {}; save it (form.save_draft) or dismiss it (form.dismiss) before opening another form",
                open.kind.form_code(),
                open.year,
                open.period
            ));
        }
        let db = self.lock_db()?;
        let profile = load_profile(&db, &tin, year)?;
        let mut draft =
            crate::screen_form_module::load_draft(&db, kind, &profile, year, period, false, None)?;
        let status = agent::identity(kind, &draft)?.status;
        if status == FilingStatus::Draft {
            // Loading some stored queueable drafts recomputes them for their
            // penalty context before this open save. Use the still-stored row
            // as the incoming image so a dormant value the open itself clears
            // is not mistaken for one the filer cleared by hand.
            let incoming = draft["id"]
                .as_i64()
                .and_then(|id| db.draft_summary_and_json(id).ok().flatten())
                .and_then(|(_, data_json)| serde_json::from_str::<Value>(&data_json).ok())
                .unwrap_or_else(|| draft.clone());
            agent::save_draft_with_clearance_context(
                kind,
                &db,
                &mut draft,
                incoming,
                BTreeSet::new(),
            )?;
        }
        let facts = stored_send_facts(&db, draft.get("id").and_then(Value::as_i64))?;
        let status = facts.status.unwrap_or(status);
        drop(db);
        self.selected_tin = Some(tin.clone());
        self.open = Some(OpenForm {
            kind,
            year,
            period,
            tin: tin.clone(),
            draft,
            sources: BTreeMap::new(),
            dirty: false,
            awaiting: false,
        });
        self.commands
            .push(json!({"op": "refresh", "tin_root": root}));
        self.commands.push(json!({
            "op": "open",
            "tin": tin,
            "code": kind.form_code(),
            "year": year,
            "period": period,
        }));
        Ok(DispatchResult::json(json!({
            "form": kind.form_code(),
            "year": year,
            "period": period,
            "tin": tin,
            "status": format!("{status:?}"),
            "dry_run": facts.dry_run,
            "send": facts.send,
        })))
    }

    fn form_fields(&self, args: &Value) -> Result<DispatchResult, String> {
        let open = self
            .open
            .as_ref()
            .ok_or("open a form first (bir_form_open)")?;
        let def = open.kind.definition();
        let map = agent::field_map(open.kind, &open.draft)?;
        let mut boxes: Vec<Value> = def
            .fillable()
            .map(|entry| {
                card_json(
                    &CardBox::new(def, entry, &map),
                    open.sources.get(&entry.key),
                )
            })
            .collect();
        if args.get("locked").and_then(Value::as_bool) == Some(true) {
            for entry in def.boxes.iter().filter(|entry| entry.is_question()) {
                let Some(lock) = def.bindings.get(&entry.key).and_then(|bound| bound.lock) else {
                    continue;
                };
                let card = CardBox::new(def, entry, &map);
                let mut value = card_json(&card, None);
                value["locked"] = json!(lock);
                boxes.push(value);
            }
        }
        let questions = def.boxes.iter().filter(|entry| entry.is_question()).count();
        let facts = stored_send_facts(
            &*self.lock_db()?,
            open.draft.get("id").and_then(Value::as_i64),
        )?;
        let status = match facts.status {
            Some(status) => status,
            None => agent::identity(open.kind, &open.draft)?.status,
        };
        Ok(DispatchResult::json(json!({
            "form": open.kind.form_code(),
            "status": format!("{status:?}"),
            "dry_run": facts.dry_run,
            "send": facts.send,
            "id": open.draft.get("id").cloned().unwrap_or(Value::Null),
            "boxes": boxes,
            "read_only": questions.saturating_sub(boxes.len()),
            "computed": def.boxes.iter().filter(|entry| entry.computed).count(),
        })))
    }

    /// After a fill held for the filer's approval, the stored return is the
    /// outcome (Continue stored the fill, Cancel or leaving did not): read it
    /// again before anything is done with the open form, on every call, since
    /// the answer can come between any two.
    fn settle(&mut self) -> Result<(), String> {
        let Some(open) = self.open.as_ref().filter(|open| open.awaiting) else {
            return Ok(());
        };
        let (kind, year, period, tin) = (open.kind, open.year, open.period, open.tin.clone());
        let db = self.lock_db()?;
        let profile = load_profile(&db, &tin, year)?;
        let draft =
            crate::screen_form_module::load_draft(&db, kind, &profile, year, period, false, None)?;
        drop(db);
        if let Some(open) = self.open.as_mut() {
            open.draft = draft;
        }
        Ok(())
    }

    fn form_fill(&mut self, args: &Value) -> Result<DispatchResult, String> {
        let open = self
            .open
            .as_ref()
            .ok_or("open a form first (bir_form_open)")?;
        let kind = open.kind;
        let year = open.year;
        let period = open.period;
        let tin = open.tin.clone();
        let before = open.draft.clone();
        let mut draft = before.clone();
        let sources = open.sources.clone();
        if agent::identity(kind, &draft)?.status != FilingStatus::Draft {
            return Err("this return is no longer a draft".into());
        }
        let fields = collect_fill_fields(args)?;
        let source = fill_source(args, &fields, kind, &draft)?;
        let db = self.lock_db()?;
        let profile = load_profile(&db, &tin, year)?;
        let from_profile = profile_context(&db, kind, &profile, year, period)?;
        let def = kind.definition();
        let (mut filled, mut kept) = (Vec::new(), Vec::new());
        let mut refused = Map::new();
        let (forgot, kept_answers) = forget_unanswered(kind, &mut draft, args, &fields)?;
        refused.extend(kept_answers);
        let mut chosen = BTreeMap::new();
        let mut pending: Vec<(String, Value)> = Vec::new();
        for (key, value) in &fields {
            if sources
                .get(key)
                .is_some_and(|kept_source| kept_source == "user")
                && source != "user"
            {
                kept.push(key.clone());
                continue;
            }
            pending.push((key.clone(), value.clone()));
        }
        // A batch means its final answers, not the accidental JSON order of its
        // boxes. The 1701MS source/rate/deduction choices reset Part V Item 6 in
        // the official runtime, so apply those reset choices before any dependent
        // amounts; the stable sort keeps unrelated input order unchanged.
        if kind.form_code() == "1701MS" {
            pending.sort_by_key(|(key, value)| !agent::ms_part_reset_choice(key, value));
        }
        // Enforce the form page's gates (`screen_form_module::page_gate_refusal`)
        // using the same reviewed reason a person sees. Defer gated keys until
        // earlier/sibling answers in the same call have been applied and computed
        // so opening an answer and filling its dependent box in one call works
        // regardless of key order.
        loop {
            let mut deferred: Vec<(String, Value, String)> = Vec::new();
            let mut progressed = false;
            for (key, value) in pending {
                if let Some(reason) =
                    crate::screen_form_module::page_gate_refusal(kind, &draft, &key, &value)
                {
                    deferred.push((key, value, reason));
                    continue;
                }
                if let Err(why) = agent::fill(kind, &mut draft, &key, &value, Some(&from_profile)) {
                    refused.insert(key, Value::String(why));
                    continue;
                }
                note_choice(kind, def, &key, &value, &mut chosen);
                filled.push(key);
                progressed = true;
            }
            if deferred.is_empty() {
                break;
            }
            if !progressed {
                for (key, _, reason) in deferred {
                    refused.insert(key, Value::String(reason));
                }
                break;
            }
            if bir_core::forms::agent::draft_locks::previous_payment::spec(kind).is_some() {
                let closes_previous_payment =
                    bir_core::forms::agent::draft_locks::previous_payment::moved_amended_yes_to_no(
                        &before, &draft,
                    );
                bir_core::forms::agent::draft_locks::previous_payment::clear_on_no_radio(
                    kind,
                    &mut draft,
                    closes_previous_payment,
                );
            }
            agent::compute(kind, &mut draft)?;
            pending = deferred
                .into_iter()
                .map(|(key, value, _)| (key, value))
                .collect();
        }
        if bir_core::forms::agent::draft_locks::previous_payment::spec(kind).is_some() {
            let closes_previous_payment =
                bir_core::forms::agent::draft_locks::previous_payment::moved_amended_yes_to_no(
                    &before, &draft,
                );
            bir_core::forms::agent::draft_locks::previous_payment::clear_on_no_radio(
                kind,
                &mut draft,
                closes_previous_payment,
            );
        }
        agent::compute(kind, &mut draft)?;
        let map = agent::field_map(kind, &draft)?;
        let not_shown = not_shown(def, &fields, &filled, &chosen, &map);
        let mut edits = Map::new();
        // A fill that removes what the filer typed is the filer's to approve:
        // the window asks (its own fill of the same edits finds the same
        // removal) and stores the result on Continue. Nothing is stored here,
        // so Cancel, or leaving with the question open, keeps the return.
        // Without a window nobody can be asked, and the fill is the answer.
        let asks = if self.commands.shown() && (!filled.is_empty() || !forgot.is_empty()) {
            let made: Map<String, Value> = filled
                .iter()
                .filter_map(|key| Some((key.clone(), fields.get(key)?.clone())))
                .collect();
            crate::screen_form_module::removes(kind, &before, &draft, &made)
        } else {
            None
        };
        if asks.is_some() {
            // The window's removal question is already this clearing's one
            // showing. Nothing is stored until Continue, but the draft id
            // exists from the open, so report the same gates the page's own
            // fill would have reported inline; otherwise the save after
            // Continue writes the pending notice and the event is said twice.
            let shown = crate::screen_form_module::inline_clearances_between(kind, &before, &draft);
            crate::screen_form_module::remember_inline_clearances(draft["id"].as_i64(), &shown);
        }
        let clearances =
            bir_core::forms::agent::draft_locks::pending_notices::clearance(kind, &before, &draft);
        if !filled.is_empty() || !forgot.is_empty() {
            if asks.is_none() {
                let before_notices =
                    crate::screen_form_module::pending_notices_before_save(&db, &draft);
                let reported: BTreeSet<String> =
                    clearances.iter().map(|(gate, _)| gate.clone()).collect();
                let id = agent::save_draft_with_clearance_context(
                    kind,
                    &db,
                    &mut draft,
                    before.clone(),
                    reported.clone(),
                )?;
                crate::screen_form_module::mark_inline_clearances_seen(
                    &db,
                    id,
                    before_notices,
                    &reported,
                );
            }
            for key in &filled {
                if not_shown.iter().any(|hidden| hidden == key) {
                    continue;
                }
                edits.insert(key.clone(), Value::String(def.box_value(key, &map)));
            }
        }
        drop(db);
        if let Some(open) = self.open.as_mut() {
            if asks.is_some() {
                open.awaiting = true;
            } else {
                open.awaiting = false;
                open.draft = draft;
                open.dirty |= !filled.is_empty() || !forgot.is_empty();
                for key in &filled {
                    open.sources.insert(key.clone(), source.clone());
                }
            }
        }
        // The window holds its own copy of the return and saves all of it: the
        // answers dropped go to it with the edits, or its next save would
        // store them again.
        if !edits.is_empty() || !forgot.is_empty() {
            let mut choose = json!({"op": "choose", "edits": edits});
            if !forgot.is_empty() {
                choose["clear"] = json!(forgot);
            }
            self.commands.push(choose);
        }
        if let Some(asks) = asks {
            let message = asks
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("The answer removes information the filer entered.");
            return Err(format!(
                "approval required: {message} Nothing was stored; the window asks the filer. Continue stores the fill, Cancel keeps the return. Read the return again with form.fields."
            ));
        }
        let mut result = json!({
            "form": kind.form_code(),
            "source": source,
            "applied": filled,
            "filled": filled.clone(),
            "kept_user_boxes": kept,
        });
        if !clearances.is_empty()
            && let Value::Object(object) = &mut result
        {
            object.insert(
                "clearances".into(),
                json!(
                    clearances
                        .iter()
                        .map(|(gate, text)| json!({"gate": gate, "text": text}))
                        .collect::<Vec<_>>()
                ),
            );
        }
        if !not_shown.is_empty()
            && let Value::Object(object) = &mut result
        {
            object.insert("not_shown".into(), json!(not_shown));
        }
        if !refused.is_empty()
            && let Value::Object(object) = &mut result
        {
            object.insert("refused".into(), Value::Object(refused));
        }
        Ok(DispatchResult::json(result))
    }

    fn form_needs_you(&mut self) -> Result<DispatchResult, String> {
        let open = self
            .open
            .as_ref()
            .ok_or("open a form first (bir_form_open)")?;
        let kind = open.kind;
        let mut draft = open.draft.clone();
        let sources = open.sources.clone();
        let errors = agent::validate(kind, &mut draft)?;
        let map = agent::field_map(kind, &draft)?;
        let needs = agent::needs_you(kind, &map, &errors);
        if let Some(open) = self.open.as_mut() {
            open.draft = draft;
        }
        Ok(DispatchResult::json(json!({
            "form": kind.form_code(),
            "ok": errors.is_empty(),
            "boxes": needs.boxes.iter().map(|card| card_json(card, sources.get(&card.key))).collect::<Vec<_>>(),
            "other_errors": field_errors(&needs.other_errors),
        })))
    }

    fn form_validate(&mut self) -> Result<DispatchResult, String> {
        let open = self
            .open
            .as_ref()
            .ok_or("open a form first (bir_form_open)")?;
        let kind = open.kind;
        let mut draft = open.draft.clone();
        let errors = agent::validate(kind, &mut draft)?;
        let code = kind.form_code();
        if let Some(open) = self.open.as_mut() {
            open.draft = draft;
        }
        Ok(DispatchResult::json(json!({
            "ok": errors.is_empty(),
            "errors": errors.len(),
            "field_errors": field_errors(&errors),
            "form": code,
        })))
    }

    fn form_schema(&self, args: &Value) -> Result<DispatchResult, String> {
        let kind = form_kind(args, "form.schema")?;
        Ok(DispatchResult::json(agent::recipe::form_schema(kind)))
    }

    /// The taxpayer an invoke names (`args.tin` / `args.q`), else the
    /// selected one.
    fn taxpayer(&self, args: &Value, invoke: &str) -> Result<String, String> {
        match self.resolve_tin(args)? {
            TinQuery::Missing => self
                .selected_tin
                .clone()
                .ok_or_else(|| format!("{invoke} requires a taxpayer (args.tin or profile.set)")),
            TinQuery::NotFound { query } => Err(format!("{invoke} taxpayer not found for {query}")),
            TinQuery::Ambiguous { query, .. } => Err(format!(
                "{invoke} taxpayer {query} is ambiguous; use args.tin as 14 digits"
            )),
            TinQuery::One { tin, .. } => Ok(tin),
        }
    }

    /// A dry run of `form.open` + `form.fill` + `form.validate`: nothing is
    /// saved, opened or shown. On a fresh in-memory return, or with
    /// `args.stored` on the return `form.open` would open for the period
    /// (the stored draft when there is one), which must still be a draft.
    /// `args.ask` lists choice boxes whose stored answer is dropped unless
    /// `fields` answers them (a stopped box is asked again where it applies).
    /// `discarded` lists the inputs the computed return does not hold.
    fn form_check(&self, args: &Value) -> Result<DispatchResult, String> {
        let kind = form_kind(args, "form.check")?;
        let year = require_year(args, "form.check")?;
        let period = optional_u8(args, "period").unwrap_or(0);
        let tin = self.taxpayer(args, "form.check")?;
        let fields = match args.get("fields") {
            None | Some(Value::Null) => Map::new(),
            Some(Value::Object(object)) => object.clone(),
            Some(_) => return Err("form.check requires args.fields as an object".into()),
        };
        let strict = args.get("strict").and_then(Value::as_bool) == Some(true);
        let db = self.lock_db()?;
        let profile = load_profile(&db, &tin, year)?;
        let mut from_profile = profile_context(&db, kind, &profile, year, period)?;
        // As `form.open` does: a later 2551Q quarter holds its filed Q1's
        // Item 13 answer.
        if kind == AgentFormKind::Form2551Q {
            let mut typed: bir_core::forms::form_2551q::Form2551QDraft =
                serde_json::from_value(from_profile.draft).map_err(|error| error.to_string())?;
            agent::draft_locks::item_13::refresh(&db, &mut typed)
                .map_err(|error| error.to_string())?;
            from_profile.draft = serde_json::to_value(typed).map_err(|error| error.to_string())?;
        }
        let stored = args.get("stored").and_then(Value::as_bool) == Some(true);
        let mut draft = if stored {
            crate::screen_form_module::load_draft(&db, kind, &profile, year, period, false, None)?
        } else {
            from_profile.draft.clone()
        };
        drop(db);
        let before = draft.clone();
        if agent::identity(kind, &draft)?.status != FilingStatus::Draft {
            return Err(format!(
                "form.check failed, nothing was written: Form {} for {year} period {period} is no longer a draft",
                kind.form_code()
            ));
        }
        let mut applied = Vec::new();
        let mut refused = Map::new();
        let (_, kept_answers) = forget_unanswered(kind, &mut draft, args, &fields)?;
        refused.extend(kept_answers);
        let def = kind.definition();
        let mut chosen = BTreeMap::new();
        let mut ordered: Vec<(&String, &Value)> = fields.iter().collect();
        if kind.form_code() == "1701MS" {
            ordered.sort_by_key(|(key, value)| !agent::ms_part_reset_choice(key, value));
        }
        for (key, value) in ordered {
            match agent::fill(kind, &mut draft, key, value, Some(&from_profile)) {
                Ok(()) => {
                    note_choice(kind, def, key, value, &mut chosen);
                    applied.push(key.clone());
                }
                Err(why) => {
                    refused.insert(key.clone(), Value::String(why));
                }
            }
        }
        if bir_core::forms::agent::draft_locks::previous_payment::spec(kind).is_some() {
            let closes_previous_payment =
                bir_core::forms::agent::draft_locks::previous_payment::moved_amended_yes_to_no(
                    &before, &draft,
                );
            bir_core::forms::agent::draft_locks::previous_payment::clear_on_no_radio(
                kind,
                &mut draft,
                closes_previous_payment,
            );
        }
        agent::compute(kind, &mut draft)?;
        let errors = agent::validate(kind, &mut draft)?;
        let map = agent::field_map(kind, &draft)?;
        let computed: Map<String, Value> = def
            .boxes
            .iter()
            .filter(|entry| entry.computed && entry.group.is_none())
            .filter_map(|entry| {
                let value = def.box_value(&entry.key, &map);
                (!agent::carry::is_blank_value(&value)).then(|| (entry.key.clone(), json!(value)))
            })
            .collect();
        let discarded = discarded(def, &fields, &applied, &chosen, &map);
        let ok = errors.is_empty() && refused.is_empty() && discarded.is_empty();
        if strict && !ok {
            let mut problems: Vec<String> = refused
                .iter()
                .map(|(key, why)| format!("{key}: {}", why.as_str().unwrap_or_default()))
                .collect();
            problems.extend(discarded.iter().map(|key| {
                format!("{key}: the return does not keep this value (its box is not in use with the other answers)")
            }));
            problems.extend(
                errors
                    .iter()
                    .map(|(field, message)| format!("{field}: {message}")),
            );
            return Err(format!(
                "form.check failed, nothing was written: {}",
                problems.join("; ")
            ));
        }
        Ok(DispatchResult::json(json!({
            "form": kind.form_code(),
            "year": year,
            "period": period,
            "tin": tin,
            "ok": ok,
            "saved": false,
            "applied": applied,
            "refused": refused,
            "discarded": discarded,
            "errors": errors.len(),
            "field_errors": field_errors(&errors),
            "computed": computed,
        })))
    }

    fn template_list(&self, args: &Value) -> Result<DispatchResult, String> {
        let tin = self.taxpayer(args, "template.list")?;
        let db = self.lock_db()?;
        let templates: Vec<Value> = db
            .list_form_templates(&tin)
            .map_err(|error| error.to_string())?
            .iter()
            .map(|template| template_json(template, false))
            .collect();
        Ok(DispatchResult::json(
            json!({"tin": tin, "templates": templates}),
        ))
    }

    /// One form's template. With `args.expect_stamp` it fails unless the
    /// template is still the one a recipe was generated from.
    fn template_get(&self, args: &Value) -> Result<DispatchResult, String> {
        let kind = form_kind(args, "template.get")?;
        let tin = self.taxpayer(args, "template.get")?;
        let db = self.lock_db()?;
        let template = db
            .get_form_template(&tin, kind.form_code())
            .map_err(|error| error.to_string())?;
        drop(db);
        let stamp = agent::recipe::template_stamp(template.as_ref());
        if let Some(expected) = optional_str(args, "expect_stamp")
            && expected != stamp
        {
            return Err(format!(
                "template for Form {} changed since this recipe was generated (expected {expected}, now {stamp}); run recipe.generate again",
                kind.form_code()
            ));
        }
        let mut out = match &template {
            Some(template) => template_json(template, true),
            None => json!({"form": kind.form_code(), "stamp": stamp}),
        };
        if let Value::Object(object) = &mut out {
            object.insert("tin".into(), json!(tin));
            object.insert("exists".into(), json!(template.is_some()));
        }
        Ok(DispatchResult::json(out))
    }

    /// The taxpayer's own recipe for a form, from its box definitions,
    /// their template and their previous return. Read-only.
    fn recipe_generate(&self, args: &Value) -> Result<DispatchResult, String> {
        let kind = form_kind(args, "recipe.generate")?;
        let tin = self.taxpayer(args, "recipe.generate")?;
        // The history ends before the period named. Left out, it ends at the
        // newest return; a year alone, at that year's last. Given, each must
        // be usable, and a period needs its year: the newest return is not
        // what a caller who named a period asked for.
        let year = match args.get("year") {
            None | Some(Value::Null) => None,
            Some(_) => Some(require_year(args, "recipe.generate")?),
        };
        let period = match args.get("period") {
            None | Some(Value::Null) => None,
            Some(given) => Some(
                json_u64(given)
                    .and_then(|period| u32::try_from(period).ok())
                    .ok_or("recipe.generate requires args.period as a JSON number")?,
            ),
        };
        if year.is_none() && period.is_some() {
            return Err(
                "recipe.generate requires args.year when args.period is given: a period alone names no return"
                    .into(),
            );
        }
        let options = agent::recipe::Options {
            check: agent::recipe::CheckMode::parse(args.get("check").and_then(Value::as_str))?,
            include_unused: args.get("include_unused").and_then(Value::as_bool) == Some(true),
            keep_open: args.get("keep_open").and_then(Value::as_bool) == Some(true),
        };
        let db = self.lock_db()?;
        let template = db
            .get_form_template(&tin, kind.form_code())
            .map_err(|error| error.to_string())?;
        let mut previous = Vec::new();
        let mut before = (year.unwrap_or(u16::MAX), period.unwrap_or(u32::MAX));
        while previous.len() < RECIPE_HISTORY {
            let Some(earlier) = db
                .previous_return(&tin, kind.form_code(), before.0, before.1)
                .map_err(|error| error.to_string())?
            else {
                break;
            };
            before = (earlier.taxable_year, earlier.period);
            previous.push(earlier);
        }
        drop(db);
        let drafts: Vec<Value> = previous
            .iter()
            .map(|earlier| earlier.draft.clone())
            .collect();
        let generated = agent::recipe::generate(&agent::recipe::Inputs {
            kind,
            tin: &tin,
            template: template.as_ref(),
            previous: &drafts,
            options,
        })?;
        Ok(DispatchResult::json(json!({
            "form": kind.form_code(),
            "tin": tin,
            "recipe": generated.recipe,
            "input_schema": generated.input_schema,
            "template_stamp": generated.template_stamp,
            "from": {
                "template": template.as_ref().map(|template| template.source.to_db()),
                "previous_returns": previous.iter().map(|earlier| json!({
                    "id": earlier.id,
                    "year": earlier.taxable_year,
                    "period": earlier.period,
                })).collect::<Vec<_>>(),
            },
            "constants": generated.constants,
            "params": generated.params,
            "left_out": generated.left_out,
        })))
    }

    fn form_save_draft(&mut self) -> Result<DispatchResult, String> {
        let open = self
            .open
            .as_ref()
            .ok_or("open a form first (bir_form_open)")?;
        let kind = open.kind;
        let mut draft = open.draft.clone();
        if agent::identity(kind, &draft)?.status != FilingStatus::Draft {
            return Err("this return is no longer a draft".into());
        }
        let db = self.lock_db()?;
        let id = agent::save_draft(kind, &db, &mut draft)?;
        let status = agent::identity(kind, &draft)?.status;
        drop(db);
        if let Some(open) = self.open.as_mut() {
            open.draft = draft;
            open.dirty = false;
        }
        Ok(DispatchResult::json(json!({
            "status": format!("{status:?}"),
            "saved": true,
            "form": kind.form_code(),
            "id": id,
        })))
    }

    fn form_context(&mut self) -> Result<DispatchResult, String> {
        let open = self
            .open
            .as_ref()
            .ok_or("open a form first (bir_form_open)")?;
        let kind = open.kind;
        let tin = open.tin.clone();
        let year = open.year;
        let period = open.period;
        let db = self.lock_db()?;
        let profile = load_profile(&db, &tin, year)?;
        let bounds = filing_bounds(kind, &open.draft);
        let overlaps = |version: &bir_core::profile::TaxProfileVersion| match bounds {
            None => true,
            Some((start, end)) => {
                version.effective_from.is_none_or(|from| from <= end)
                    && version.effective_until.is_none_or(|until| until >= start)
            }
        };
        let period_versions: Vec<_> = profile
            .profile_versions
            .iter()
            .filter(|version| overlaps(version))
            .collect();
        let trade_name = period_versions
            .iter()
            .find(|version| version.status == bir_core::profile::TaxProfileVersionStatus::Confirmed)
            .or(period_versions.first())
            .and_then(|version| version.cor.trade_name.clone());
        let cor_versions: Vec<Value> = profile
            .profile_versions
            .iter()
            .map(|version| {
                json!({
                    "label": version.label,
                    "status": version.status,
                    "source": version.source,
                    "effective_from": version.effective_from,
                    "effective_until": version.effective_until,
                    "covers_period": overlaps(version),
                    "cor": version.cor,
                    "registered_tax_types": version.registered_tax_types,
                })
            })
            .collect();
        let elections: Vec<Value> = profile
            .tax_elections
            .iter()
            .map(|election| {
                json!({
                    "taxable_year": election.taxable_year,
                    "election": election.election,
                    "elected_at": election.elected_at,
                    "source_form": election.source_form,
                })
            })
            .collect();
        let drafts = db
            .list_draft_summaries_all_years(&tin)
            .map_err(|error| error.to_string())?;
        let past_returns: Vec<Value> = drafts
            .into_iter()
            .map(|summary| {
                let facts = stored_send_facts(&db, Some(summary.id))?;
                Ok(json!({
                    "kind": "saved_return",
                    "form": summary.form_code,
                    "year": summary.taxable_year,
                    "quarter": summary.quarter,
                    "month": summary.month,
                    "status": format!("{:?}", summary.status),
                    "dry_run": facts.dry_run,
                    "send": facts.send,
                    "updated_at": summary.updated_at,
                }))
            })
            .collect::<Result<_, String>>()?;
        let submissions = db
            .list_submissions_for_tin(&tin)
            .map_err(|error| error.to_string())?;
        let filed: Vec<Value> = submissions
            .into_iter()
            .map(|submission| {
                json!({
                    "kind": "submission",
                    "form": submission.form_type,
                    "period": submission.period,
                    "status": submission.status,
                    "submitted_at": submission.submitted_at,
                    "filename": submission.filename,
                })
            })
            .collect();
        drop(db);
        Ok(DispatchResult::json(json!({
            "form": kind.form_code(),
            "period": {
                "year": year,
                "period": period,
                "start": bounds.map(|(start, _)| start),
                "end": bounds.map(|(_, end)| end),
            },
            "profile": {
                "tin": profile.tin.full(),
                "registered_name": profile.full_name,
                "trade_name": trade_name,
                "registered_address": profile.registered_address,
                "zip_code": profile.zip_code,
                "rdo_code": profile.rdo_code,
                "line_of_business": profile.line_of_business,
                "taxpayer_type": profile.taxpayer_type,
                "tax_classification": profile.tax_classification,
                "is_vat_registered": profile.is_vat_registered,
                "eopt_tier": profile.eopt_tier,
                "business_start_date": profile.business_start_date,
                "contact_number": profile.phone,
                "email": profile.email,
                "cor_versions": cor_versions,
            },
            "elections": elections,
            "past_returns": past_returns,
            "submissions": filed,
            "documents": period_versions.iter().flat_map(|version| {
                version.evidence.iter().map(move |document| json!({
                    "profile_version": version.label,
                    "file_name": document.file_name,
                    "document_type": document.document_type,
                    "uploaded_at": document.uploaded_at,
                    "extracted_form_codes": document.extracted_form_codes,
                }))
            }).collect::<Vec<_>>(),
        })))
    }

    fn form_review_in_app(&mut self) -> Result<DispatchResult, String> {
        let open = self
            .open
            .as_ref()
            .ok_or("open a form first (bir_form_open)")?;
        let kind = open.kind;
        let year = open.year;
        let period = open.period;
        let tin = open.tin.clone();
        let mut draft = open.draft.clone();
        if agent::identity(kind, &draft)?.status != FilingStatus::Draft {
            return Err("this return is no longer a draft; there is nothing to hand over".into());
        }
        let errors = agent::validate(kind, &mut draft)?;
        if !errors.is_empty() {
            if let Some(open) = self.open.as_mut() {
                open.draft = draft;
            }
            return Err(format!(
                "fix the {} validation error(s) first (bir_form_validate); review is only for a return that validates",
                errors.len()
            ));
        }
        let db = self.lock_db()?;
        agent::save_draft(kind, &db, &mut draft)?;
        drop(db);
        if let Some(open) = self.open.as_mut() {
            open.draft = draft;
        }
        self.commands.push(json!({
            "op": "open",
            "tin": tin,
            "code": kind.form_code(),
            "year": year,
            "period": period,
        }));
        self.commands.push(json!({
            "op": "review",
            "form": kind.form_code(),
            "message": AGENT_REVIEW_MESSAGE,
        }));
        Ok(DispatchResult::json(json!({
            "form": kind.form_code(),
            "status": "Draft",
            "saved": true,
            "validated": true,
            "queued": false,
            "filed": false,
            "message": AGENT_REVIEW_MESSAGE,
        })))
    }

    /// Queue the open draft for upload from this computer.
    ///
    /// The gate is [`agent::validate`]: the same checks the page's Validate
    /// runs (`form.validate` / `bir_form_validate`). An invalid draft is not
    /// queued, and those field errors come back to the assistant. A missing
    /// standing OK returns `approval_required` without queueing.
    fn form_send(&mut self) -> Result<DispatchResult, String> {
        let open = self
            .open
            .as_ref()
            .ok_or("open a form first (bir_form_open)")?;
        let kind = open.kind;
        let mut draft = open.draft.clone();
        let db = self.lock_db()?;
        let outcome = send_draft(&db, kind, &mut draft)?;
        drop(db);
        if let Some(open) = self.open.as_mut() {
            open.draft = draft;
        }
        if outcome.get("queued").and_then(Value::as_bool) == Some(true) {
            crate::bir_module::wake_submission_worker();
        }
        Ok(DispatchResult::json(outcome))
    }

    /// With `discard: true`, closes without writing the agent's copy. The NSH
    /// window saves the filer's typing on leave; classic GPUI `enterView`
    /// switches views without saving unsaved typing.
    fn form_dismiss(&mut self, args: &Value) -> Result<DispatchResult, String> {
        let discard = args.get("discard").and_then(Value::as_bool) == Some(true);
        if !discard && self.open.as_ref().is_some_and(|open| open.dirty) {
            return Err("the form has unsaved edits; save the draft before dismissing it".into());
        }
        let Some(open) = self.open.take() else {
            return Err("no form is open to dismiss".into());
        };
        self.commands.push(json!({"op": "go", "view": "home"}));
        let mut result = json!({
            "dismissed": true,
            "form": open.kind.form_code(),
        });
        if discard {
            result["discarded"] = json!(true);
        }
        Ok(DispatchResult::json(result))
    }

    fn resolve_tin(&self, args: &Value) -> Result<TinQuery, String> {
        let tin_arg = optional_str(args, "tin");
        let query = optional_str(args, "q");
        if tin_arg.is_none() && query.is_none() {
            return Ok(TinQuery::Missing);
        }
        let rows = self.profile_rows()?;
        let wanted = tin_arg.clone().or(query.clone()).unwrap_or_default();
        let matches: Vec<Value> = if let Some(tin) = &tin_arg {
            rows.into_iter()
                .filter(|row| {
                    row.get("tin")
                        .and_then(Value::as_str)
                        .is_some_and(|stored| same_tin(stored, tin))
                })
                .collect()
        } else {
            let q = wanted.to_lowercase();
            rows.into_iter()
                .filter(|row| {
                    let tin = row.get("tin").and_then(Value::as_str).unwrap_or("");
                    let name = row.get("name").and_then(Value::as_str).unwrap_or("");
                    tin.to_lowercase().contains(&q) || name.to_lowercase().contains(&q)
                })
                .collect()
        };
        match matches.as_slice() {
            [] => Ok(TinQuery::NotFound { query: wanted }),
            [only] => {
                let tin = only
                    .get("tin")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let (_, root) = tin_parts(&tin)?;
                Ok(TinQuery::One {
                    tin,
                    root,
                    candidates: matches,
                })
            }
            _ => Ok(TinQuery::Ambiguous {
                query: wanted,
                candidates: matches,
            }),
        }
    }
}

/// One invoke the session answers.
struct Invoke {
    name: &'static str,
    run: fn(&mut Session, &Value) -> Result<DispatchResult, String>,
}

const fn invoke(
    name: &'static str,
    run: fn(&mut Session, &Value) -> Result<DispatchResult, String>,
) -> Invoke {
    Invoke { name, run }
}

fn not_allow_listed(name: &str) -> Result<DispatchResult, String> {
    Err(format!(
        "invoke `{name}` is not allow-listed; queue with form.queue/filing.queue confirm=true, then let cron PUT"
    ))
}

/// Every invoke the session answers: [`Session::invoke`] dispatches from this
/// list, and a test holds `agent-recipes/bir.schema.json` to it. An invoke
/// added here is offered to recipes only once that file lists it too; the
/// test names every invoke and argument the file leaves out, and why.
const INVOKES: &[Invoke] = &[
    invoke("nav.go", |session, args| {
        let page = args
            .get("page")
            .and_then(Value::as_str)
            .ok_or("nav.go requires args.page")?;
        session.navigate(page)
    }),
    invoke("app.info", |session, _| session.app_info()),
    invoke("profile.list", |session, _| session.profile_list()),
    invoke("profile.set", |session, args| session.profile_set(args)),
    invoke("profile.save", |session, args| session.profile_save(args)),
    invoke("profile.forms_set", |session, args| {
        session.profile_forms_set(args)
    }),
    invoke("profile.forms_set.set", |session, args| {
        session.profile_forms_set(args)
    }),
    invoke("dues.list", |session, args| session.dues_list(args)),
    invoke("form.open", |session, args| session.form_open(args)),
    invoke("filing.start", |session, args| session.form_open(args)),
    invoke("form.fields", |session, args| session.form_fields(args)),
    invoke("form.fill", |session, args| session.form_fill(args)),
    invoke("form.needs_you", |session, _| session.form_needs_you()),
    invoke("form.validate", |session, _| session.form_validate()),
    invoke("filing.validate", |session, _| session.form_validate()),
    invoke("form.context", |session, _| session.form_context()),
    invoke("form.schema", |session, args| session.form_schema(args)),
    invoke("form.check", |session, args| session.form_check(args)),
    invoke("template.list", |session, args| session.template_list(args)),
    invoke("template.get", |session, args| session.template_get(args)),
    invoke("recipe.generate", |session, args| {
        session.recipe_generate(args)
    }),
    invoke("form.review_in_app", |session, _| {
        session.form_review_in_app()
    }),
    invoke("form.send", |session, _| session.form_send()),
    invoke("form.save_draft", |session, _| session.form_save_draft()),
    invoke("form.dismiss", |session, args| session.form_dismiss(args)),
    // Named so the refusal says why; none of them does anything.
    invoke("form.file", |_, _| not_allow_listed("form.file")),
    invoke("form.submit_external", |_, _| {
        not_allow_listed("form.submit_external")
    }),
    invoke("filing.file", |_, _| not_allow_listed("filing.file")),
    invoke("profile.ensure", |_, _| {
        Err(
            "profile.ensure is not implemented; the host will not auto-create a taxpayer. Save it explicitly with profile.save (tin, name, rdo_code, ...)"
                .into(),
        )
    }),
];

impl AgentHost for Session {
    fn hello(&self) -> HelloInfo {
        HelloInfo {
            protocol: PROTOCOL_VERSION,
            app: APP.into(),
            platform: self.platform,
            os: gpui_agent::protocol::host_os(),
            ready: true,
            deliveries: vec![DeliveryMode::Semantic],
            auth: HelloAuth::None,
            bundle_id: None,
            executable: None,
            features: Vec::new(),
            framework: None,
            device: None,
        }
    }

    fn snapshot(&self) -> UiTree {
        self.tree.clone()
    }

    fn dispatch(&mut self, op: &Op) -> Result<DispatchResult, String> {
        match op {
            Op::Invoke { name, args } => self.invoke(name, args),
            Op::Click { target, .. } => self.click(target),
            Op::Shutdown => {
                self.quit = true;
                Ok(DispatchResult::json(json!({"shutdown": true})))
            }
            Op::Type { .. } | Op::SetValue { .. } | Op::Key { .. } => {
                Err("bir-shell fills forms with form.fill; virtual typing is not wired".into())
            }
            other => Err(format!("unsupported op {other:?}")),
        }
    }
}

enum TinQuery {
    Missing,
    NotFound {
        query: String,
    },
    One {
        tin: String,
        root: String,
        candidates: Vec<Value>,
    },
    Ambiguous {
        query: String,
        candidates: Vec<Value>,
    },
}

/// The branch's profile for tax year `year` (its year facts over the flat
/// fields), as every return of that year reads it.
fn load_profile(db: &Database, tin: &str, year: u16) -> Result<TaxpayerProfile, String> {
    db.get_profile_for_year(tin, year)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("no taxpayer profile for {tin}"))
}

fn profile_context(
    db: &Database,
    kind: AgentFormKind,
    profile: &TaxpayerProfile,
    year: u16,
    period: u8,
) -> Result<FromProfile, String> {
    let draft = crate::screen_form_module::load_draft(db, kind, profile, year, period, true, None)?;
    let values =
        serde_json::to_value(agent::field_map(kind, &draft)?).map_err(|error| error.to_string())?;
    Ok(FromProfile { draft, values })
}

/// How many earlier returns `recipe.generate` reads.
const RECIPE_HISTORY: usize = 8;

fn form_kind(args: &Value, invoke: &str) -> Result<AgentFormKind, String> {
    let code = args
        .get("code")
        .and_then(Value::as_str)
        .ok_or_else(|| format!("{invoke} requires args.code"))?;
    AgentFormKind::from_code(code).ok_or_else(|| format!("unknown form `{code}`"))
}

fn template_json(template: &FormTemplate, with_fields: bool) -> Value {
    let mut out = json!({
        "form": template.form_code,
        "stamp": agent::recipe::template_stamp(Some(template)),
        "source": template.source.to_db(),
        "updated_at": template.updated_at,
        "pinned": template.pinned().len(),
        "stops_on": template.stops_on(),
    });
    if with_fields && let Value::Object(object) = &mut out {
        object.insert("answers".into(), json!(template.pinned()));
        object.insert("stops".into(), json!(template.stop_edits()));
    }
    out
}

fn view_from_page(page: &str) -> Result<&'static str, String> {
    match page.trim().to_ascii_lowercase().as_str() {
        "home" | "dashboard" => Ok("home"),
        "library" => Ok("library"),
        "calendar" => Ok("calendar"),
        "filings" => Ok("filings"),
        "profile" | "profile-manager" => Ok("profile"),
        "settings" => Ok("settings"),
        other => Err(format!(
            "unknown page `{other}` (expected home, library, calendar, filings, profile, or settings)"
        )),
    }
}

fn optional_str(args: &Value, key: &str) -> Option<String> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn facts_present(args: &Value) -> bool {
    [
        "rdo_code",
        "registered_address",
        "address",
        "zip_code",
        "zip",
        "phone",
        "email",
    ]
    .iter()
    .any(|key| args.get(*key).is_some())
}

fn taxpayer_type(raw: Option<&str>) -> TaxpayerType {
    match raw.unwrap_or("").trim() {
        "Corporation" | "corporation" => TaxpayerType::Corporation,
        "Partnership" | "partnership" => TaxpayerType::Partnership,
        "Cooperative" | "cooperative" => TaxpayerType::Cooperative,
        "Estate" | "estate" => TaxpayerType::Estate,
        "Trust" | "trust" => TaxpayerType::Trust,
        _ => TaxpayerType::Individual,
    }
}

fn digits(raw: &str) -> String {
    raw.chars().filter(|ch| ch.is_ascii_digit()).collect()
}

fn same_tin(left: &str, right: &str) -> bool {
    let left = digits(left);
    let right = digits(right);
    !left.is_empty() && left == right
}

/// `(full 14-digit TIN, 9-digit root)`.
fn tin_parts(raw: &str) -> Result<(String, String), String> {
    let digits = digits(raw);
    match digits.len() {
        9 => Ok((format!("{digits}00000"), digits)),
        14 => Ok((digits.clone(), digits[..9].to_string())),
        _ => Err(format!(
            "a TIN is 9 digits or 14 digits (root + branch); got `{raw}`"
        )),
    }
}

fn last4(tin: &str) -> String {
    let digits = digits(tin);
    let start = digits.len().saturating_sub(4);
    digits[start..].to_string()
}

fn require_year(args: &Value, invoke: &str) -> Result<u16, String> {
    optional_year(args).ok_or_else(|| format!("{invoke} requires args.year as a JSON number"))
}

fn optional_year(args: &Value) -> Option<u16> {
    json_u64(args.get("year")?).and_then(|year| u16::try_from(year).ok())
}

fn optional_u8(args: &Value, key: &str) -> Option<u8> {
    json_u64(args.get(key)?).and_then(|value| u8::try_from(value).ok())
}

fn json_u64(value: &Value) -> Option<u64> {
    match value {
        Value::Number(number) => number.as_u64(),
        Value::String(text) => text.trim().parse().ok(),
        _ => None,
    }
}

fn require_forms_set_confirm(args: &Value, invoke: &str) -> Result<(), String> {
    match args.get("confirm") {
        Some(Value::Bool(true)) => Ok(()),
        Some(Value::Bool(false)) | None => Err(format!(
            "{invoke} requires args.confirm=true (JSON boolean) after a human confirmed the Forms Set change; the host will not write per_year_forms without it"
        )),
        Some(_) => Err(format!(
            "{invoke} args.confirm must be boolean true, not a string or number"
        )),
    }
}

fn parse_forms_set_codes(args: &Value) -> Result<Vec<String>, String> {
    let raw = args
        .get("codes")
        .ok_or_else(|| "profile.forms_set requires args.codes (comma list or array)".to_string())?;
    let codes = match raw {
        Value::String(text) => text
            .split([',', ' '])
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .map(str::to_string)
            .collect(),
        Value::Array(items) => items
            .iter()
            .filter_map(|item| item.as_str().map(str::trim).filter(|part| !part.is_empty()))
            .map(str::to_string)
            .collect(),
        _ => {
            return Err("profile.forms_set args.codes must be a comma list or an array".into());
        }
    };
    Ok(codes)
}

fn collect_fill_fields(args: &Value) -> Result<Map<String, Value>, String> {
    let mut fields = Map::new();
    match args.get("fields") {
        Some(Value::Object(object)) => fields.extend(object.clone()),
        Some(_) => return Err("form.fill requires args.fields as an object".into()),
        None => {}
    }
    if let Some(object) = args.as_object() {
        for (key, value) in object {
            if !matches!(key.as_str(), "fields" | "source" | "from_card" | "ask") {
                fields.insert(key.clone(), value.clone());
            }
        }
    }
    // A request that only drops stored answers (`ask`) fills nothing.
    let asks = args
        .get("ask")
        .and_then(Value::as_array)
        .is_some_and(|keys| !keys.is_empty());
    if fields.is_empty() && !asks {
        return Err(
            "form.fill requires args.fields as an object, or fillable KEY=VALUE args".into(),
        );
    }
    Ok(fields)
}

fn fill_source(
    args: &Value,
    fields: &Map<String, Value>,
    kind: AgentFormKind,
    draft: &Value,
) -> Result<String, String> {
    let from_card = args.get("from_card").and_then(Value::as_bool) == Some(true);
    let raw = args.get("source").and_then(Value::as_str);
    if !(from_card && raw.map(str::trim) == Some("user")) {
        return match raw.map(str::trim).filter(|value| !value.is_empty()) {
            None | Some("ai") => Ok("ai".into()),
            Some("profile") => Ok("profile".into()),
            Some("past_return") => Ok("past_return".into()),
            Some("document") => Ok("document".into()),
            Some("user") => Err(
                "source `user` is reserved for boxes the user typed; use profile, past_return, document or ai"
                    .into(),
            ),
            Some(other) => Err(format!(
                "unknown source `{other}`; use profile, past_return, document or ai"
            )),
        };
    }
    let mut draft = draft.clone();
    let errors = agent::validate(kind, &mut draft)?;
    let map = agent::field_map(kind, &draft)?;
    let (boxes, _) = agent::card_boxes(kind, &map, &errors);
    if let Some(key) = fields
        .keys()
        .find(|key| !boxes.iter().any(|card| &card.key == *key))
    {
        return Err(format!(
            "`{key}` is not on the form card; source `user` is only for the user's own answers to bir_form_card"
        ));
    }
    Ok("user".into())
}

fn card_json(card: &CardBox, source: Option<&String>) -> Value {
    let mut value = serde_json::to_value(card).unwrap_or(Value::Null);
    if let Value::Object(object) = &mut value {
        object.insert("field".into(), json!(card.key));
        if let Some(source) = source {
            object.insert("source".into(), json!(source));
        }
    }
    value
}

fn field_errors(errors: &[(String, String)]) -> Vec<Value> {
    errors
        .iter()
        .map(|(field, message)| json!({"field": field, "message": message}))
        .collect()
}

/// Drop the stored answer of every `args.ask` box that `fields` leaves
/// unanswered. The boxes whose answer was dropped, then the boxes the return
/// cannot hold unanswered, each with why: those keep their answer, as the
/// window's own fill keeps them.
fn forget_unanswered(
    kind: AgentFormKind,
    draft: &mut Value,
    args: &Value,
    fields: &Map<String, Value>,
) -> Result<(Vec<String>, Map<String, Value>), String> {
    let ask = match args.get("ask") {
        None | Some(Value::Null) => return Ok((Vec::new(), Map::new())),
        Some(Value::Array(keys)) => keys,
        Some(_) => return Err("args.ask must be a list of box keys".into()),
    };
    let mut forgotten = Vec::new();
    let mut refused = Map::new();
    for key in ask.iter().filter_map(Value::as_str) {
        let answered = match fields.get(key) {
            None | Some(Value::Null) => false,
            Some(Value::String(text)) => !text.trim().is_empty(),
            Some(_) => true,
        };
        if !answered {
            let before = draft.clone();
            match agent::forget(kind, draft, key) {
                Ok(()) if *draft != before => forgotten.push(key.to_string()),
                Ok(()) => {}
                Err(why) => {
                    refused.insert(key.to_string(), Value::String(why));
                }
            }
        }
    }
    Ok((forgotten, refused))
}

/// After a radio item is filled: the option that fill chose, by the write's
/// own matching of labels and aliases ([`agent::chosen_option`]). Not read
/// back from the draft: a box whose answer the form derives (the spouse's
/// rate, from the ATC) shows the old answer until the form recomputes.
fn note_choice(
    kind: AgentFormKind,
    def: &agent::AgentFormDef,
    key: &str,
    value: &Value,
    chosen: &mut BTreeMap<String, String>,
) {
    if def.box_def(key).is_some_and(|entry| entry.radio_group)
        && let Some(option) = agent::chosen_option(kind, key, value)
    {
        chosen.insert(key.to_string(), option);
    }
}

/// The filled boxes the return does not show as filled. `chosen` holds the
/// option each filled radio item selected ([`note_choice`]): an item whose
/// selection the computation replaced is not shown.
fn not_shown(
    def: &agent::AgentFormDef,
    fields: &Map<String, Value>,
    filled: &[String],
    chosen: &BTreeMap<String, String>,
    map: &BTreeMap<String, String>,
) -> Vec<String> {
    let text = |value: &Value| match value {
        Value::String(text) => text.trim().to_string(),
        other => other.to_string(),
    };
    filled
        .iter()
        .filter(|key| {
            let wanted = fields.get(*key).map(text).unwrap_or_default();
            let shown = def.box_value(key, map);
            if shown.is_empty()
                && !map.contains_key(*key)
                && def.box_def(key).is_none_or(|entry| !entry.radio_group)
            {
                return true;
            }
            if def.box_def(key).is_some_and(|entry| entry.radio_group) {
                return shown.is_empty() || chosen.get(*key).is_some_and(|option| *option != shown);
            }
            match def.bindings.get(*key).map(|bound| &bound.binding) {
                Some(Binding::Amount { .. }) => {
                    match (agent::parse_amount(&wanted), agent::parse_amount(&shown)) {
                        (Some(wanted), Some(shown)) => (wanted - shown).abs() >= 0.5,
                        _ => !wanted.is_empty(),
                    }
                }
                Some(Binding::Whole { .. }) => {
                    wanted.parse::<i64>().ok() != shown.trim().parse::<i64>().ok()
                }
                Some(Binding::Text { date: None, .. }) => {
                    !shown.to_uppercase().contains(&wanted.to_uppercase())
                }
                Some(Binding::Text { .. }) => shown.is_empty(),
                Some(Binding::Choice { .. }) | None => {
                    !wanted.eq_ignore_ascii_case(&shown) && shown != "true"
                }
            }
        })
        .cloned()
        .collect()
}

/// The applied inputs the computed return does not hold: a box the other
/// answers switch off, or a value the computation replaced. A blank input,
/// and a zero for an amount or a count, ask for nothing and are never listed.
fn discarded(
    def: &agent::AgentFormDef,
    fields: &Map<String, Value>,
    applied: &[String],
    chosen: &BTreeMap<String, String>,
    map: &BTreeMap<String, String>,
) -> Vec<String> {
    not_shown(def, fields, applied, chosen, map)
        .into_iter()
        .filter(|key| {
            let wanted = match fields.get(key) {
                Some(Value::String(text)) => text.trim().to_string(),
                Some(Value::Null) | None => String::new(),
                Some(other) => other.to_string(),
            };
            if wanted.is_empty() {
                return false;
            }
            match def.bindings.get(key).map(|bound| &bound.binding) {
                Some(Binding::Amount { .. }) => {
                    agent::parse_amount(&wanted).is_none_or(|amount| amount.abs() >= 0.005)
                }
                Some(Binding::Whole { .. }) => wanted.parse::<i64>().ok() != Some(0),
                _ => true,
            }
        })
        .collect()
}

fn parse_due(raw: &str) -> Option<chrono::NaiveDate> {
    let date = raw.get(..10).unwrap_or(raw);
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

const AGENT_REVIEW_MESSAGE: &str =
    "Prepared by the assistant. Review every box, then click Queue yourself.";

fn filing_bounds(
    kind: AgentFormKind,
    draft: &Value,
) -> Option<(chrono::NaiveDate, chrono::NaiveDate)> {
    if kind == AgentFormKind::Form2551Q {
        return serde_json::from_value::<Form2551QDraft>(draft.clone())
            .ok()
            .and_then(|typed| typed.filing_period_bounds());
    }
    None
}

/// Queue `draft` after the page's Validate has already passed.
fn queue_validated(kind: AgentFormKind, db: &Database, draft: &mut Value) -> Result<i64, String> {
    let code = kind.form_code();
    if !bir_core::forms::can_queue_for_submission(code) {
        return Err(format!("{code} cannot be queued for submission yet"));
    }
    let join = |errors: Vec<(String, String)>| {
        errors
            .iter()
            .map(|(field, message)| format!("{field}: {message}"))
            .collect::<Vec<_>>()
            .join("; ")
    };
    match kind {
        AgentFormKind::Form2551Q => {
            let mut typed: Form2551QDraft =
                serde_json::from_value(draft.clone()).map_err(|error| error.to_string())?;
            typed.transition_to_queued().map_err(join)?;
            typed.set_queue_auth_source(QueueAuthSource::AgentConfirm);
            let id = db
                .save_queued_2551q_draft_and_election(&typed)
                .map_err(|error| error.to_string())?;
            *draft = serde_json::to_value(&typed).map_err(|error| error.to_string())?;
            Ok(id)
        }
        AgentFormKind::Form1601C => {
            let mut typed: Form1601CDraft =
                serde_json::from_value(draft.clone()).map_err(|error| error.to_string())?;
            typed.transition_to_queued().map_err(join)?;
            typed.set_queue_auth_source(QueueAuthSource::AgentConfirm);
            let id = db
                .save_queued_1601c_draft(&typed)
                .map_err(|error| error.to_string())?;
            *draft = serde_json::to_value(&typed).map_err(|error| error.to_string())?;
            Ok(id)
        }
        AgentFormKind::Queueable(queueable) => {
            bir_core::with_queueable_kind!(queueable, F => {
                let mut typed: F =
                    serde_json::from_value(draft.clone()).map_err(|error| error.to_string())?;
                typed.queue(QueueAuthSource::AgentConfirm).map_err(join)?;
                let id = db
                    .save_queued_queueable(&typed)
                    .map_err(|error| error.to_string())?;
                *draft = serde_json::to_value(&typed).map_err(|error| error.to_string())?;
                Ok(id)
            })
        }
    }
}

/// Shared send for `form.send` and a route-3 pickup.
///
/// Calls [`agent::validate`] (the page's Validate) before any queue write.
/// Errors from that call are the `field_errors` returned to the assistant.
pub fn send_draft(db: &Database, kind: AgentFormKind, draft: &mut Value) -> Result<Value, String> {
    let identity = agent::identity(kind, draft)?;
    let code = kind.form_code().to_string();
    if !db
        .has_send_grant(&identity.tin, &code)
        .map_err(|error| error.to_string())?
    {
        return Ok(json!({
            "ok": false,
            "status": "approval_required",
            "pending": true,
            "queued": false,
            "filed": false,
            "form": code,
            "message": "This send needs a standing OK for this taxpayer and form. Grant it on the taxpayer page. Nothing was queued.",
        }));
    }
    if identity.status != FilingStatus::Draft {
        return Err(format!("this return is {:?}, not a Draft", identity.status));
    }
    if kind == AgentFormKind::Form2551Q
        && let Some(profile) = db
            .get_profile_for_year(&identity.tin, identity.taxable_year)
            .map_err(|error| error.to_string())?
        && let Ok(mut typed) = serde_json::from_value::<Form2551QDraft>(draft.clone())
    {
        let _ = typed.reconcile_with_effective_profile(&profile);
        *draft = serde_json::to_value(&typed).map_err(|error| error.to_string())?;
    }
    let errors = agent::validate(kind, draft)?;
    if !errors.is_empty() {
        if agent::identity(kind, draft)?.status == FilingStatus::Draft {
            let _ = agent::save_draft(kind, db, draft);
        }
        return Ok(json!({
            "ok": false,
            "status": "invalid",
            "queued": false,
            "filed": false,
            "form": code,
            "field_errors": field_errors(&errors),
            "message": "Validation failed. Nothing was queued.",
        }));
    }
    let id = queue_validated(kind, db, draft)?;
    Ok(queued_answer(
        &code,
        id,
        bir_core::transport::sends_through_buwiz(),
        bir_core::transport::is_sftp_dry_run(),
    ))
}

/// What `form.send` says of a return it queued, by where the send goes. Through the Buwiz relay the server decides
/// whether it is a real send or a test send (a test TIN), so `dry_run` is unknown (null) until it is sent; the agent
/// reads it then from `form.open` (`dry_run`, `send`). Otherwise this build's dry-run setting says it.
fn queued_answer(code: &str, id: i64, through_buwiz: bool, dry_run: bool) -> Value {
    let (dry_run, message) = if through_buwiz {
        (
            Value::Null,
            "Queued. It goes to the BIR through the Buwiz relay; whether it is a real send or a test send is known once it is sent: open the form again (form.open) for `dry_run` and `send`.",
        )
    } else if dry_run {
        (
            json!(true),
            "Queued. This build is a dry run: nothing is sent to BIR.",
        )
    } else {
        (json!(false), "Queued for upload from this computer.")
    };
    json!({
        "ok": true,
        "status": "queued",
        "queued": true,
        "filed": false,
        "dry_run": dry_run,
        "form": code,
        "id": id,
        "message": message,
    })
}

/// What the agent reads of the stored return's send, from its one stored row, as the screens get it.
struct SendFacts {
    /// The stored row's status; None for a return not stored yet.
    status: Option<FilingStatus>,
    /// Whether the finished send reached nobody (the worker's `submitted_dry_run`). None until a send has finished
    /// (Submitted, Confirmed or Paid): a Draft or a Queued return has no send to tell of, and through the Buwiz relay
    /// only the server's answer says whether it was a test send.
    dry_run: Option<bool>,
    /// The send state (send_state.rs: stage, line ...); null where the screens show the status alone.
    send: Value,
}

fn stored_send_facts(db: &Database, id: Option<i64>) -> Result<SendFacts, String> {
    let Some(id) = id else {
        return Ok(SendFacts {
            status: None,
            dry_run: None,
            send: Value::Null,
        });
    };
    let Some((summary, json)) = db
        .draft_summary_and_json(id)
        .map_err(|error| error.to_string())?
    else {
        return Ok(SendFacts {
            status: None,
            dry_run: None,
            send: Value::Null,
        });
    };
    let dry_run = matches!(
        summary.status,
        FilingStatus::Submitted | FilingStatus::Confirmed | FilingStatus::Paid
    )
    .then(|| {
        serde_json::from_str::<Value>(&json).is_ok_and(|draft| {
            crate::screen_form_module::lifecycle_flag(&draft, "submitted_dry_run")
        })
    });
    let send = crate::send_state::stored_send(db, id).map_err(|error| error.to_string())?;
    Ok(SendFacts {
        status: Some(summary.status),
        dry_run,
        send,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_agent::protocol::{Op, Request, ScreenshotMode};
    use gpui_agent::{handle_request, screenshot_unavailable};

    const TIN: &str = "12345678800000";
    const ROOT: &str = "123456788";
    /// A spouse TIN with a valid check digit.
    const SPOUSE_TIN: &str = "12345678800000";

    fn session() -> Session {
        let db = Arc::new(Mutex::new(
            Database::open_ephemeral().expect("ephemeral database"),
        ));
        let mut session = Session::new(UiQueue::new());
        session.set_db(db);
        session
    }

    /// Every top-level argument each invoke's handler reads. `call` refuses
    /// an argument that is not here, so this cannot fall behind the handlers
    /// without a test noticing, and the schema test below compares the file
    /// with it.
    const INVOKE_ARGS: &[(&str, &[&str])] = &[
        ("nav.go", &["page"]),
        ("app.info", &[]),
        ("profile.list", &[]),
        ("profile.set", &["tin", "q", "view"]),
        ("profile.save", PROFILE_SAVE_ARGS),
        ("profile.forms_set", FORMS_SET_ARGS),
        ("profile.forms_set.set", FORMS_SET_ARGS),
        ("dues.list", &["filter", "scope"]),
        ("form.open", FORM_OPEN_ARGS),
        ("filing.start", FORM_OPEN_ARGS),
        ("form.fields", &["locked"]),
        ("form.fill", &["fields", "source", "from_card", "ask"]),
        ("form.needs_you", &[]),
        ("form.validate", &[]),
        ("filing.validate", &[]),
        ("form.context", &[]),
        ("form.schema", &["code"]),
        (
            "form.check",
            &[
                "code", "year", "period", "tin", "q", "fields", "stored", "strict", "ask",
            ],
        ),
        ("template.list", &["tin", "q"]),
        ("template.get", &["code", "tin", "q", "expect_stamp"]),
        (
            "recipe.generate",
            &[
                "code",
                "tin",
                "q",
                "year",
                "period",
                "check",
                "include_unused",
                "keep_open",
            ],
        ),
        ("form.review_in_app", &[]),
        ("form.send", &[]),
        ("form.save_draft", &[]),
        ("form.dismiss", &["discard"]),
        ("form.file", &[]),
        ("form.submit_external", &[]),
        ("filing.file", &[]),
        ("profile.ensure", &[]),
    ];
    const PROFILE_SAVE_ARGS: &[&str] = &[
        "tin",
        "tin_root",
        "name",
        "rdo_code",
        "taxpayer_type",
        "birth_date",
        "address",
        "registered_address",
        "zip",
        "zip_code",
        "phone",
        "email",
    ];
    const FORMS_SET_ARGS: &[&str] = &["confirm", "year", "tin", "q", "codes", "reason"];
    const FORM_OPEN_ARGS: &[&str] = &["code", "year", "period", "tin", "q"];
    /// Invokes whose handler takes any other top-level key too: `form.fill`
    /// reads them as boxes.
    const LOOSE: &[&str] = &["form.fill"];

    fn declared_args(name: &str) -> Option<&'static [&'static str]> {
        INVOKE_ARGS
            .iter()
            .find(|(invoke, _)| *invoke == name)
            .map(|(_, args)| *args)
    }

    /// Invokes as the wire does. Every argument a test passes must be one
    /// [`INVOKE_ARGS`] declares for that invoke.
    fn call(session: &mut Session, name: &str, args: Value) -> Result<Value, String> {
        if let Some(declared) = declared_args(name)
            && !LOOSE.contains(&name)
        {
            for key in args.as_object().into_iter().flatten().map(|(key, _)| key) {
                assert!(
                    declared.contains(&key.as_str()),
                    "`{name}` is passed `{key}`, which INVOKE_ARGS does not declare"
                );
            }
        }
        session
            .invoke(name, &args)
            .map(|result| result.value.unwrap_or(Value::Null))
    }

    /// Invokes the session answers that `agent-recipes/bir.schema.json`
    /// leaves out, so a recipe cannot name them, and why.
    const NOT_IN_THE_RECIPE_SCHEMA: &[(&str, &str)] = &[
        ("filing.start", "alias of form.open"),
        ("filing.validate", "alias of form.validate"),
        ("profile.forms_set.set", "alias of profile.forms_set"),
        ("form.file", "always refused"),
        ("form.submit_external", "always refused"),
        ("filing.file", "always refused"),
        ("profile.ensure", "always refused: not implemented"),
        (
            "form.send",
            "sends the return; offered through the MCP tool bir_form_send",
        ),
        (
            "form.review_in_app",
            "hands the return to the person; offered through the MCP tool bir_form_review",
        ),
        (
            "profile.forms_set",
            "writes only with args.confirm, after a person confirmed",
        ),
    ];

    /// Arguments a handler reads that the schema file does not offer to a
    /// recipe, and why.
    const ARGS_NOT_IN_THE_RECIPE_SCHEMA: &[(&str, &str, &str)] = &[
        (
            "form.open",
            "q",
            "a search that can match several taxpayers; a recipe names one by tin",
        ),
        ("form.check", "q", "as form.open"),
        ("template.list", "q", "as form.open"),
        ("template.get", "q", "as form.open"),
        ("recipe.generate", "q", "as form.open"),
        (
            "form.fill",
            "from_card",
            "marks the person's own answers to a form card; an MCP flow",
        ),
    ];

    /// The schema file is what `gpui-agent recipe validate` judges a recipe
    /// by, and it is written by hand. It must list every invoke and argument
    /// the session answers, or the difference must be named above.
    #[test]
    fn the_recipe_schema_file_matches_the_invoke_table() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../agent-recipes/bir.schema.json"
        );
        let file: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
        let rows = file["schemas"].as_array().unwrap();
        let listed = |name: &str| rows.iter().find(|row| row["name"] == name);
        let left_out = |name: &str| NOT_IN_THE_RECIPE_SCHEMA.iter().any(|(n, _)| *n == name);

        assert_eq!(
            INVOKES.iter().map(|invoke| invoke.name).collect::<Vec<_>>(),
            INVOKE_ARGS
                .iter()
                .map(|(name, _)| *name)
                .collect::<Vec<_>>(),
            "INVOKE_ARGS has one entry per invoke, in the order of INVOKES"
        );
        for invoke in INVOKES {
            let name = invoke.name;
            let reads = declared_args(name).unwrap();
            let Some(row) = listed(name) else {
                assert!(
                    left_out(name),
                    "`{name}` is answered by the session but is not in bir.schema.json; add it there, or to NOT_IN_THE_RECIPE_SCHEMA with the reason"
                );
                continue;
            };
            assert!(
                !left_out(name),
                "`{name}` is in bir.schema.json and in NOT_IN_THE_RECIPE_SCHEMA"
            );
            assert_eq!(row["kind"], "invoke", "{name}");
            let mut offered: Vec<&str> = row["args"]
                .as_object()
                .unwrap_or_else(|| panic!("`{name}` lists its args in bir.schema.json"))
                .keys()
                .map(String::as_str)
                .collect();
            for required in row["required"].as_array().into_iter().flatten() {
                let required = required.as_str().unwrap();
                assert!(
                    offered.contains(&required),
                    "`{name}` requires `{required}` in bir.schema.json without listing it"
                );
            }
            for arg in &offered {
                assert!(
                    reads.contains(arg),
                    "bir.schema.json offers `{name}` the arg `{arg}`, which its handler does not read"
                );
            }
            let mut withheld: Vec<&str> = ARGS_NOT_IN_THE_RECIPE_SCHEMA
                .iter()
                .filter(|(n, _, _)| *n == name)
                .map(|(_, arg, _)| *arg)
                .collect();
            offered.append(&mut withheld);
            for arg in reads {
                assert!(
                    offered.contains(arg),
                    "`{name}` reads `{arg}`, which is not in bir.schema.json; add it there, or to ARGS_NOT_IN_THE_RECIPE_SCHEMA with the reason"
                );
            }
            assert_eq!(
                offered.len(),
                reads.len(),
                "`{name}`: an arg is both in bir.schema.json and in ARGS_NOT_IN_THE_RECIPE_SCHEMA"
            );
        }
        for row in rows {
            let name = row["name"].as_str().unwrap();
            assert!(
                INVOKES.iter().any(|invoke| invoke.name == name),
                "bir.schema.json lists `{name}`, which the session does not answer"
            );
        }
        for (name, _) in NOT_IN_THE_RECIPE_SCHEMA {
            assert!(
                INVOKES.iter().any(|invoke| invoke.name == *name),
                "NOT_IN_THE_RECIPE_SCHEMA names `{name}`, which the session does not answer"
            );
        }
        for (name, arg, _) in ARGS_NOT_IN_THE_RECIPE_SCHEMA {
            assert!(
                declared_args(name).is_some_and(|reads| reads.contains(arg)),
                "ARGS_NOT_IN_THE_RECIPE_SCHEMA names `{name}`.`{arg}`, which the handler does not read"
            );
        }
    }

    fn save_scratch(session: &mut Session) -> Value {
        call(
            session,
            "profile.save",
            json!({"tin": TIN, "name": "TEST", "rdo_code": "040"}),
        )
        .unwrap()
    }

    /// The assistant's 2551Q send (`send_draft`) for a year whose facts
    /// were just set and a profile with no confirmed version: it queues,
    /// and Item 13 becomes the year's election on the head office.
    #[test]
    fn the_2551q_send_needs_no_confirmed_profile_version() {
        let db = Database::open_ephemeral().expect("ephemeral database");
        db.create_taxpayer(bir_core::tax_profile::NewTaxpayer {
            tin_root: "000123453".into(),
            name: "Juana Santos".into(),
            taxpayer_type: bir_core::profile::TaxpayerType::Individual,
            birth_date: None,
            head_office: bir_core::tax_profile::BranchFacts {
                rdo_code: "039".into(),
                registered_address: "Quezon City".into(),
                zip_code: "1100".into(),
                phone: "09170000000".into(),
                email: "juana@example.com".into(),
            },
        })
        .unwrap();
        let head = "00012345300000";
        db.set_year_facts(
            head,
            2024,
            &bir_core::tax_profile::YearFactsPatch {
                tax_classification: Some(Some(bir_core::profile::TaxClassification::SelfEmployed)),
                ..Default::default()
            },
        )
        .unwrap();
        db.grant_send(head, "2551Q").unwrap();
        let profile = db.get_profile_for_year(head, 2024).unwrap().unwrap();
        assert!(profile.profile_versions.is_empty());
        let mut draft = Form2551QDraft::new_from_effective_profile(&profile, 2024, 1);
        draft.item_13_election = bir_core::forms::form_2551q::Item13Election::Graduated;
        let mut value = serde_json::to_value(&draft).unwrap();

        let sent = send_draft(&db, AgentFormKind::Form2551Q, &mut value).unwrap();

        assert_eq!(sent["queued"], true, "{sent}");
        let facts = db.year_facts(head, 2024).unwrap().unwrap();
        assert_eq!(
            facts.income_tax_election,
            Some(bir_core::profile::IncomeTaxElection::GraduatedUnspecified)
        );
        assert_eq!(facts.election_source.as_deref(), Some("2551Q Q1"));
    }

    #[test]
    fn hello_names_bir_shell_and_screenshot_stays_unavailable() {
        let mut session = Session::new(UiQueue::new());
        let hello = handle_request(&mut session, Request::new("1", Op::Hello), None, None);
        assert!(hello.ok, "{hello:?}");
        let info = hello.hello.expect("hello");
        assert_eq!(info.app, "bir-shell");
        assert_eq!(info.protocol, PROTOCOL_VERSION);
        assert_eq!(info.platform, PlatformKind::Desktop);
        assert!(info.ready);
        assert_eq!(info.deliveries, vec![DeliveryMode::Semantic]);
        let shot = handle_request(
            &mut session,
            Request::new(
                "2",
                Op::Screenshot {
                    path: None,
                    mode: ScreenshotMode::Viewport,
                    target: None,
                    max_height_px: None,
                    inline: false,
                },
            ),
            None,
            None,
        );
        let error = shot.error.expect("screenshot error");
        assert!(screenshot_unavailable("x").starts_with("screenshot_unavailable"));
        assert!(error.contains("screenshot_unavailable"), "{error}");
    }

    #[test]
    fn profile_save_lists_and_sets_solo_on_a_scratch_database() {
        let mut session = session();
        let saved = save_scratch(&mut session);
        assert_eq!(saved["tin"], TIN);
        assert_eq!(saved["created"], true);
        let mode = session
            .lock_db()
            .unwrap()
            .get_setting("access_mode")
            .unwrap();
        assert_eq!(mode.as_deref(), Some("solo"));
        let listed = call(&mut session, "profile.list", json!({})).unwrap();
        assert_eq!(listed[0]["tin"], TIN);
        assert_eq!(listed[0]["name"], "TEST");
        assert_eq!(listed[0]["rdo"], "040");
        let info = call(&mut session, "app.info", json!({})).unwrap();
        assert_eq!(info["app"], "bir-shell");
        assert_eq!(info["mode"], "desktop");
        assert_eq!(info["profiles"][0]["tin"], TIN);
        let set = call(
            &mut session,
            "profile.set",
            json!({"tin": TIN, "view": "dashboard"}),
        )
        .unwrap();
        assert_eq!(set["status"], "ok");
        let mut saw_refresh = false;
        let mut saw_home = false;
        while let Some(command) = session.commands.pop() {
            if command["op"] == "refresh" && command["tin_root"] == ROOT {
                saw_refresh = true;
            }
            if command["op"] == "go" && command["view"] == "home" {
                saw_home = true;
            }
        }
        assert!(saw_refresh, "profile.set refreshes the selected taxpayer");
        assert!(saw_home, "dashboard navigates to home");
    }

    /// `profile.save` on a taxpayer that already exists stores `birth_date`
    /// and leaves the name alone when none is given.
    #[test]
    fn profile_save_stores_birth_date_on_an_existing_taxpayer() {
        let mut session = session();
        save_scratch(&mut session);
        let saved = call(
            &mut session,
            "profile.save",
            json!({"tin": TIN, "birth_date": "1975-10-10"}),
        )
        .unwrap();
        assert_eq!(saved["created"], false);
        assert_eq!(saved["name"], "TEST");
        let stored = session
            .lock_db()
            .unwrap()
            .get_taxpayer(ROOT)
            .unwrap()
            .expect("taxpayer");
        assert_eq!(
            stored.taxpayer.birth_date,
            chrono::NaiveDate::from_ymd_opt(1975, 10, 10)
        );
        assert_eq!(stored.taxpayer.name, "TEST");
    }

    #[test]
    fn form_open_fill_and_validate_use_the_scratch_draft() {
        let mut session = session();
        save_scratch(&mut session);
        while session.commands.pop().is_some() {}
        let opened = call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        assert_eq!(opened["form"], "1701Q");
        assert_eq!(session.commands.pop().unwrap()["op"], "refresh");
        assert_eq!(session.commands.pop().unwrap()["op"], "open");
        let fields = call(&mut session, "form.fields", json!({})).unwrap();
        assert_eq!(fields["form"], "1701Q");
        assert!(
            fields["boxes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|box_| box_["key"] == "frm1701q:txtSheets")
        );
        let filled = call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701q:txtSheets": "3"}, "source": "ai"}),
        )
        .unwrap();
        assert_eq!(filled["filled"][0], "frm1701q:txtSheets");
        assert_eq!(filled["source"], "ai");
        let choose = session.commands.pop().unwrap();
        assert_eq!(choose["op"], "choose");
        assert_eq!(choose["edits"]["frm1701q:txtSheets"], "3");
        let bogus = call(
            &mut session,
            "form.fill",
            json!({"fields": {"note": "nope"}, "source": "ai"}),
        )
        .unwrap();
        assert!(
            bogus["refused"]["note"]
                .as_str()
                .unwrap()
                .contains("not a box on Form 1701Q"),
            "{bogus}"
        );
        let validated = call(&mut session, "form.validate", json!({})).unwrap();
        assert_eq!(validated["ok"], false);
        assert!(validated["errors"].as_u64().unwrap() >= 1);
        assert!(validated["field_errors"].as_array().unwrap()[0]["field"].is_string());
        let needs = call(&mut session, "form.needs_you", json!({})).unwrap();
        assert_eq!(needs["ok"], false);
        let again = call(
            &mut session,
            "form.open",
            json!({"code": "1601C", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap_err();
        assert!(again.contains("form already open"), "{again}");
        let saved = call(&mut session, "form.save_draft", json!({})).unwrap();
        assert_eq!(saved["saved"], true);
        assert_eq!(saved["form"], "1701Q");
    }

    #[test]
    fn form_fields_can_read_locked_profile_email_without_making_it_fillable() {
        let mut session = session();
        save_complete(&mut session);
        store_1701q(&mut session, 1, &graduated_1701q());
        call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();

        let fields = call(&mut session, "form.fields", json!({})).unwrap();
        assert!(
            !fields["boxes"]
                .as_array()
                .unwrap()
                .iter()
                .any(|box_| box_["key"] == "txtEmail"),
            "{fields}"
        );

        let fields = call(&mut session, "form.fields", json!({"locked": true})).unwrap();
        let email = fields["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|box_| box_["key"] == "txtEmail")
            .expect("locked profile email is readable");
        assert_eq!(email["value"], "test@example.com", "{email}");
        assert_eq!(email["locked"], "profile", "{email}");

        let fill = call(
            &mut session,
            "form.fill",
            json!({"fields": {"txtEmail": "changed@example.com"}}),
        )
        .unwrap();
        assert!(fill["filled"].as_array().unwrap().is_empty(), "{fill}");
        assert!(
            fill["refused"]["txtEmail"]
                .as_str()
                .unwrap()
                .contains("taxpayer profile"),
            "{fill}"
        );
    }

    #[test]
    fn recipe_is_generated_from_the_template_and_the_previous_return() {
        let mut session = session();
        save_scratch(&mut session);
        let blank = call(&mut session, "recipe.generate", json!({"code": "1701Q"})).unwrap();
        assert_eq!(blank["template_stamp"], "none", "{blank}");
        assert!(blank["constants"].as_object().unwrap().is_empty());
        let required = |made: &Value| -> Vec<String> {
            serde_json::from_value(made["input_schema"]["required"].clone()).unwrap()
        };

        call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let filled = call(
            &mut session,
            "form.fill",
            json!({"fields": {
                "frm1701q:txtCitizenship": "FILIPINO",
                "frm1701q:optTaxRate": "Graduated",
                "frm1701q:optMethodOfDeduction": "I",
                "frm1701q:txt36A": "250000.00",
            }}),
        )
        .unwrap();
        assert!(filled.get("refused").is_none(), "{filled}");
        call(&mut session, "form.save_draft", json!({})).unwrap();
        call(&mut session, "form.dismiss", json!({})).unwrap();
        session
            .lock_db()
            .unwrap()
            .snapshot_template_from_latest_return(TIN, "1701Q")
            .unwrap();

        let listed = call(&mut session, "template.list", json!({"tin": TIN})).unwrap();
        assert_eq!(listed["templates"][0]["form"], "1701Q", "{listed}");
        let generated = call(
            &mut session,
            "recipe.generate",
            json!({"code": "1701Q", "tin": TIN}),
        )
        .unwrap();
        // An election the history answered is asked every time, never defaulted.
        assert!(required(&generated).contains(&"opttaxrate".to_string()));
        let rate = &generated["input_schema"]["properties"]["opttaxrate"];
        assert!(rate.get("default").is_none(), "{rate}");
        assert_eq!(generated["from"]["previous_returns"][0]["period"], 1);
        let stamp = generated["template_stamp"].as_str().unwrap().to_string();
        assert_ne!(stamp, "none");
        assert_eq!(listed["templates"][0]["stamp"], stamp.as_str());
        assert_eq!(
            generated["constants"]["frm1701q:txtCitizenship"], "FILIPINO",
            "{generated}"
        );
        let recipe = &generated["recipe"];
        let fill = recipe["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|step| step["id"] == "fill")
            .unwrap();
        assert_eq!(
            fill["args"]["fields"]["frm1701q:txtCitizenship"],
            "FILIPINO"
        );
        assert_eq!(
            fill["args"]["fields"]["frm1701q:txt36A"], "$txt36a",
            "{generated:#}"
        );
        let sales = recipe["params"]
            .as_array()
            .unwrap()
            .iter()
            .find(|param| param["name"] == "txt36a")
            .unwrap();
        assert!(
            sales.get("default").is_none(),
            "an amount is never carried: {sales}"
        );
        assert!(sales["pattern"].is_string(), "{sales}");
        assert_eq!(
            generated["input_schema"]["properties"]["txt36a"]["x-box"],
            "frm1701q:txt36A"
        );
        let names: Vec<&str> = recipe["params"]
            .as_array()
            .unwrap()
            .iter()
            .map(|param| param["name"].as_str().unwrap())
            .collect();
        for placeholder in fill["args"]["fields"].as_object().unwrap().values() {
            if let Some(name) = placeholder.as_str().unwrap().strip_prefix('$') {
                assert!(names.contains(&name), "{name} is declared");
            }
        }

        let fresh = call(
            &mut session,
            "template.get",
            json!({"code": "1701Q", "tin": TIN, "expect_stamp": stamp}),
        )
        .unwrap();
        assert_eq!(fresh["exists"], true, "{fresh}");
        session
            .lock_db()
            .unwrap()
            .set_template_stop(TIN, "1701Q", "frm1701q:txtCitizenship", Some(true))
            .unwrap();
        let stale = call(
            &mut session,
            "template.get",
            json!({"code": "1701Q", "tin": TIN, "expect_stamp": stamp}),
        )
        .unwrap_err();
        assert!(stale.contains("run recipe.generate again"), "{stale}");
        let again = call(
            &mut session,
            "recipe.generate",
            json!({"code": "1701Q", "tin": TIN}),
        )
        .unwrap();
        assert!(
            again["constants"].get("frm1701q:txtCitizenship").is_none(),
            "a stop is asked, not pinned: {again}"
        );
        assert!(
            again["input_schema"]["required"]
                .as_array()
                .unwrap()
                .contains(&json!("txtcitizenship")),
            "{again}"
        );
    }

    /// A later 2551Q quarter's dry run starts from the filed Q1's Item 13,
    /// as `form.open` does.
    #[test]
    fn check_of_a_later_2551q_quarter_holds_the_filed_q1_item_13() {
        let mut session = session();
        save_scratch(&mut session);
        let later = json!({"code": "2551Q", "year": 2026, "period": 2, "tin": TIN});
        let before = call(&mut session, "form.check", later.clone()).unwrap();
        {
            let db = session.lock_db().unwrap();
            let profile = load_profile(&db, TIN, 2026).unwrap();
            let mut q1 = Form2551QDraft::new_from_effective_profile(&profile, 2026, 1);
            q1.item_13_election = bir_core::forms::form_2551q::Item13Election::Graduated;
            let id = db.save_2551q_draft(&q1).unwrap();
            q1.id = Some(id);
            q1.status = FilingStatus::Submitted;
            db.conn_for_tests()
                .execute(
                    "UPDATE form_drafts SET status = 'Submitted', data_json = ?1 WHERE id = ?2",
                    (serde_json::to_string(&q1).unwrap(), id),
                )
                .unwrap();
        }
        let after = call(&mut session, "form.check", later).unwrap();
        let unanswered = |checked: &Value| {
            checked["field_errors"]
                .as_array()
                .unwrap()
                .iter()
                .any(|error| error["field"] == "item_13_election")
        };
        assert!(unanswered(&before), "{before}");
        assert!(!unanswered(&after), "{after}");
    }

    /// A stop on a computed penalty box is not an input: `fill` refuses the
    /// box until Edit opens it in the app, so a strict check could not pass.
    #[test]
    fn a_penalty_stop_is_not_a_recipe_input() {
        let mut session = session();
        save_scratch(&mut session);
        let key = "frm2551Qv2018:txt20";
        session
            .lock_db()
            .unwrap()
            .set_template_stop(TIN, "2551Q", key, Some(true))
            .unwrap();
        let generated = call(
            &mut session,
            "recipe.generate",
            json!({"code": "2551Q", "tin": TIN}),
        )
        .unwrap();
        assert!(
            !generated["recipe"].to_string().contains(key),
            "{generated}"
        );
        let schema = call(&mut session, "form.schema", json!({"code": "2551Q"})).unwrap();
        let entry = schema["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["key"] == key)
            .unwrap();
        assert_eq!(entry["fillable"], false, "{entry}");
    }

    /// Every value `form.schema` offers for a fillable box is one `fill` takes.
    #[test]
    fn schema_choices_are_the_ones_fill_accepts() {
        let mut session = session();
        save_scratch(&mut session);
        let mut wrong = Vec::new();
        for kind in AgentFormKind::all() {
            let code = kind.form_code();
            let schema = call(&mut session, "form.schema", json!({"code": code})).unwrap();
            for entry in schema["boxes"].as_array().unwrap() {
                let (Some(key), Some(values)) =
                    (entry["key"].as_str(), entry["value"]["enum"].as_array())
                else {
                    continue;
                };
                if entry["fillable"] != true {
                    continue;
                }
                for value in values {
                    let checked = call(
                        &mut session,
                        "form.check",
                        json!({"code": code, "year": 2026, "period": 1, "tin": TIN,
                               "fields": {key: value}}),
                    );
                    match checked {
                        Ok(checked) if checked["refused"].get(key).is_none() => {}
                        Ok(checked) => {
                            wrong.push(format!("{code} {key}={value}: {}", checked["refused"][key]))
                        }
                        Err(error) => wrong.push(format!("{code} {key}={value}: {error}")),
                    }
                }
            }
        }
        assert!(
            wrong.is_empty(),
            "{} refused:\n{}",
            wrong.len(),
            wrong.join("\n")
        );
    }

    /// The review's three cases on the generated recipe's strict check.
    #[test]
    fn strict_check_is_on_the_stored_draft_and_refuses_discarded_inputs() {
        let mut session = session();
        save_scratch(&mut session);
        let valid = json!({
            "frm1701q:txtCitizenship": "FILIPINO",
            "frm1701q:optType": "Professional",
            "frm1701q:optATC": "II012",
            "frm1701q:txtBirthMonth": "01/15/1990",
            "frm1701q:optTaxRate": "Graduated",
            "frm1701q:optMethodOfDeduction": "O",
            "frm1701q:txt36A": "250000.00",
        });
        let check = |session: &mut Session, fields: &Value, stored: bool| {
            call(
                session,
                "form.check",
                json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN,
                       "fields": fields, "stored": stored}),
            )
            .unwrap()
        };
        // A draft stored while the profile had no email keeps its blank email.
        call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        call(&mut session, "form.save_draft", json!({})).unwrap();
        call(&mut session, "form.dismiss", json!({})).unwrap();
        call(
            &mut session,
            "profile.save",
            json!({"tin": TIN, "name": "TEST", "rdo_code": "040", "email": "test@example.com"}),
        )
        .unwrap();
        let fresh = check(&mut session, &valid, false);
        let stored = check(&mut session, &valid, true);
        let names = |checked: &Value| -> Vec<String> {
            checked["field_errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| error["field"].as_str().unwrap_or_default().to_string())
                .collect()
        };
        assert!(
            !names(&fresh).iter().any(|field| field == "email"),
            "{fresh}"
        );
        assert!(
            names(&stored).iter().any(|field| field == "email"),
            "{stored}"
        );
        let strict = call(
            &mut session,
            "form.check",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN,
                   "fields": valid, "stored": true, "strict": true}),
        )
        .unwrap_err();
        assert!(
            strict.contains("nothing was written") && strict.contains("email"),
            "{strict}"
        );

        // Cost of sales under the optional standard deduction is zeroed by the form.
        let mut discarded = valid.clone();
        discarded["frm1701q:txt37A"] = json!("40000.00");
        let checked = check(&mut session, &discarded, false);
        assert_eq!(
            checked["discarded"],
            json!(["frm1701q:txt37A"]),
            "{checked}"
        );
        assert_eq!(checked["ok"], false, "{checked}");
        let strict = call(
            &mut session,
            "form.check",
            json!({"code": "1701Q", "year": 2026, "period": 2, "tin": TIN,
                   "fields": discarded, "strict": true}),
        )
        .unwrap_err();
        assert!(strict.contains("frm1701q:txt37A"), "{strict}");
        // A zero in the same inactive box asks for nothing.
        let mut zero = valid.clone();
        zero["frm1701q:txt37A"] = json!("0.00");
        let checked = check(&mut session, &zero, false);
        assert_eq!(checked["discarded"], json!([]), "{checked}");
    }

    /// 1701Q Item 16A is asked only under graduated rates.
    #[test]
    fn eight_percent_recipe_does_not_require_the_deduction_method() {
        let mut session = session();
        save_scratch(&mut session);
        let made = call(&mut session, "recipe.generate", json!({"code": "1701Q"})).unwrap();
        let name = made["input_schema"]["properties"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, property)| property["x-box"] == "frm1701q:optMethodOfDeduction")
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| panic!("{made}"));
        let required = made["input_schema"]["required"].as_array().unwrap();
        assert!(!required.contains(&json!(name)), "{made}");
        let errors = |session: &mut Session, rate: &str| -> Vec<String> {
            let checked = call(
                session,
                "form.check",
                json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN,
                       "fields": {"frm1701q:optTaxRate": rate}}),
            )
            .unwrap();
            checked["field_errors"]
                .as_array()
                .unwrap()
                .iter()
                .map(|error| error["field"].as_str().unwrap_or_default().to_string())
                .collect()
        };
        let rates: Vec<String> = serde_json::from_value(
            made["input_schema"]["properties"]
                .as_object()
                .unwrap()
                .values()
                .find(|property| property["x-box"] == "frm1701q:optTaxRate")
                .unwrap()["enum"]
                .clone(),
        )
        .unwrap();
        let eight = rates
            .iter()
            .find(|rate| rate.as_str() != "Graduated")
            .unwrap();
        assert!(
            !errors(&mut session, eight)
                .iter()
                .any(|field| field == "deduction_method")
        );
        assert!(
            errors(&mut session, "Graduated")
                .iter()
                .any(|field| field == "deduction_method")
        );
    }

    /// A valid first-quarter 1701Q under graduated rates.
    fn graduated_1701q() -> Value {
        json!({
            "frm1701q:txtCitizenship": "FILIPINO",
            "frm1701q:optType": "Professional",
            "frm1701q:optATC": "II012",
            "frm1701q:optTaxRate": "Graduated",
            "frm1701q:optMethodOfDeduction": "O",
            "frm1701q:txt36A": "250000.00",
        })
    }

    /// A scratch profile complete enough for a return to validate.
    fn save_complete(session: &mut Session) {
        call(
            session,
            "profile.save",
            json!({"tin": TIN, "name": "TEST", "rdo_code": "040", "birth_date": "1990-01-15",
                   "address": "1 TEST STREET", "zip": "1100", "email": "test@example.com"}),
        )
        .unwrap();
    }

    /// Store `fields` as the period's draft, the history a recipe reads.
    fn store_1701q(session: &mut Session, period: u32, fields: &Value) {
        store_1701q_as(session, period, fields, false);
    }

    /// [`store_1701q`]; `married` turns the spouse's part on first, as the
    /// page's Civil Status does (no agent box does).
    fn store_1701q_as(session: &mut Session, period: u32, fields: &Value, married: bool) {
        call(
            session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": period, "tin": TIN}),
        )
        .unwrap();
        if married {
            let draft = &mut session.open.as_mut().unwrap().draft;
            draft["has_spouse"] = json!(true);
            draft["spouse_tin"] = json!(SPOUSE_TIN);
            draft["spouse_name"] = json!("TEST, SPOUSE");
            draft["spouse_rdo_code"] = json!("040");
        }
        let filled = call(session, "form.fill", json!({"fields": fields})).unwrap();
        assert!(filled.get("refused").is_none(), "{filled}");
        call(session, "form.save_draft", json!({})).unwrap();
        call(session, "form.dismiss", json!({})).unwrap();
    }

    /// The generated recipe's parameter for box `key`.
    fn param_of(made: &Value, key: &str) -> String {
        made["input_schema"]["properties"]
            .as_object()
            .unwrap()
            .iter()
            .find(|(_, property)| property["x-box"] == key)
            .map(|(name, _)| name.clone())
            .unwrap_or_else(|| panic!("no parameter for {key}: {made}"))
    }

    fn is_required(made: &Value, key: &str) -> bool {
        made["input_schema"]["required"]
            .as_array()
            .unwrap()
            .contains(&json!(param_of(made, key)))
    }

    /// Run a generated recipe the way `gpui-agent recipe run` does: bind
    /// `set` (box key, value) with the runner's own rules, an optional
    /// parameter left out dropping its argument, then invoke each step in
    /// order. The error names the step that stopped the run.
    fn run_recipe(
        session: &mut Session,
        made: &Value,
        period: u32,
        set: &[(&str, &str)],
    ) -> Result<BTreeMap<String, Value>, String> {
        use gpui_agent::protocol::Op;
        let recipe = gpui_agent_recipe::Recipe::from_value(&made["recipe"])
            .map_err(|error| format!("recipe: {error}"))?;
        let mut bound: BTreeMap<String, String> = set
            .iter()
            .map(|(key, value)| (param_of(made, key), (*value).to_string()))
            .collect();
        bound.insert("year".into(), "2026".into());
        bound.insert("period".into(), period.to_string());
        let recipe = gpui_agent_recipe::apply_params(&recipe, &bound)
            .map_err(|error| format!("params: {error}"))?;
        let mut done = BTreeMap::new();
        for step in &recipe.steps {
            let Op::Invoke { name, args } = &step.op else {
                panic!("step {} is not an invoke", step.id);
            };
            let value =
                call(session, name, args.clone()).map_err(|why| format!("{}: {why}", step.id))?;
            done.insert(step.id.clone(), value);
        }
        Ok(done)
    }

    /// Answers for a 1701Q under graduated rates, Item 16A left out.
    const GRADUATED_SET: [(&str, &str); 8] = [
        ("frm1701q:txtCitizenship", "FILIPINO"),
        ("frm1701q:optType", "Professional"),
        ("frm1701q:optATC", "II012"),
        ("frm1701q:txtBirthMonth", "01"),
        ("frm1701q:txtBirthDay", "15"),
        ("frm1701q:txtBirthYear", "1990"),
        ("frm1701q:optTaxRate", "Graduated"),
        ("frm1701q:txt36A", "250000.00"),
    ];

    /// The same return under the 8% rate.
    const EIGHT_PERCENT_SET: [(&str, &str); 7] = [
        ("frm1701q:txtCitizenship", "FILIPINO"),
        ("frm1701q:optType", "Professional"),
        ("frm1701q:optATC", "II015"),
        ("frm1701q:txtBirthMonth", "01"),
        ("frm1701q:txtBirthDay", "15"),
        ("frm1701q:txtBirthYear", "1990"),
        ("frm1701q:optTaxRate", "Percentage"),
    ];

    /// The answers of `set` the recipe has a parameter for.
    fn asked<'a>(made: &Value, set: &[(&'a str, &'a str)]) -> Vec<(&'a str, &'a str)> {
        set.iter()
            .copied()
            .filter(|(key, _)| made_asks(made, key))
            .collect()
    }

    fn with<'a>(
        base: &[(&'a str, &'a str)],
        more: &[(&'a str, &'a str)],
    ) -> Vec<(&'a str, &'a str)> {
        base.iter().chain(more).copied().collect()
    }

    /// The generated recipe itself runs an 8% return with Item 16A left
    /// out: the runner drops the argument from both the check and the fill.
    #[test]
    fn generated_recipe_runs_an_eight_percent_return_without_the_deduction_method() {
        let mut session = session();
        save_complete(&mut session);
        let made = call(
            &mut session,
            "recipe.generate",
            json!({"code": "1701Q", "tin": TIN, "check": "strict"}),
        )
        .unwrap();
        let done = run_recipe(&mut session, &made, 1, &asked(&made, &EIGHT_PERCENT_SET)).unwrap();
        assert_eq!(done["check"]["ok"], true, "{}", done["check"]);
        assert_eq!(done["validate"]["ok"], true, "{}", done["validate"]);
        assert_eq!(done["save"]["saved"], true, "{}", done["save"]);
        // Graduated rates still ask for it, and name it before any write.
        let stopped =
            run_recipe(&mut session, &made, 2, &asked(&made, &GRADUATED_SET)).unwrap_err();
        assert!(stopped.starts_with("check: "), "{stopped}");
        assert!(stopped.contains("Item 16A"), "{stopped}");
        let answered = with(
            &asked(&made, &GRADUATED_SET),
            &[("frm1701q:optMethodOfDeduction", "O")],
        );
        let done = run_recipe(&mut session, &made, 2, &answered).unwrap();
        assert_eq!(done["save"]["saved"], true, "{}", done["save"]);
    }

    #[test]
    fn generated_recipe_waits_for_approval_before_saving_a_rate_change() {
        let mut session = session();
        save_complete(&mut session);
        store_1701q(&mut session, 1, &graduated_1701q());
        let made = call(
            &mut session,
            "recipe.generate",
            json!({"code": "1701Q", "tin": TIN, "check": "strict"}),
        )
        .unwrap();

        let stopped =
            run_recipe(&mut session, &made, 1, &asked(&made, &EIGHT_PERCENT_SET)).unwrap_err();
        assert!(stopped.starts_with("fill: approval required:"), "{stopped}");
        assert!(!stopped.contains("save:"), "{stopped}");
        assert!(session.open.is_some(), "{stopped}");
        assert_eq!(stored_income(&session, 1), "250,000.00");
    }

    /// A spouse who filed under graduated rates may elect 8% later: Item
    /// 25A is then not asked for, and the recipe runs without it.
    #[test]
    fn spouse_eight_percent_recipe_does_not_require_the_spouse_deduction_method() {
        let mut session = session();
        save_complete(&mut session);
        let mut history = graduated_1701q();
        for (key, value) in SPOUSE_GRADUATED {
            history[key] = json!(value);
        }
        store_1701q_as(&mut session, 1, &history, true);
        // The period's draft is the married filer's, its elections unmade.
        store_1701q_as(
            &mut session,
            2,
            &json!({"frm1701q:txtCitizenship": "FILIPINO"}),
            true,
        );
        let made = call(
            &mut session,
            "recipe.generate",
            json!({"code": "1701Q", "tin": TIN, "check": "strict"}),
        )
        .unwrap();
        assert!(!is_required(&made, "frm1701q:optSpouseMethod"), "{made}");
        let filer = with(
            &asked(&made, &GRADUATED_SET),
            &[("frm1701q:optMethodOfDeduction", "O")],
        );
        let spouse: Vec<(&str, &str)> = SPOUSE_GRADUATED
            .iter()
            // Neither Item 25A nor graduated sales (Item 36B) is on an 8% column.
            .filter(|(key, _)| !matches!(*key, "frm1701q:optSpouseMethod" | "frm1701q:txt36B"))
            .map(|(key, value)| match *key {
                "frm1701q:optSpouseATC" => (*key, "II015"),
                _ => (*key, *value),
            })
            .chain([("frm1701q:optSpouseTaxRate", "Percentage")])
            .filter(|(key, _)| made_asks(&made, key))
            .collect();
        let done = run_recipe(&mut session, &made, 2, &with(&filer, &spouse)).unwrap();
        assert_eq!(done["check"]["ok"], true, "{}", done["check"]);
        assert_eq!(done["check"]["discarded"], json!([]), "{}", done["check"]);
        assert_eq!(done["save"]["saved"], true, "{}", done["save"]);
        // The 8% spouse return holds no method of deduction.
        assert_eq!(
            read(&done["readback"], "frm1701q:optSpouseTaxRate"),
            "Percentage"
        );
        assert_eq!(read(&done["readback"], "frm1701q:optSpouseMethod"), "");
    }

    /// A graduated spouse return.
    const SPOUSE_GRADUATED: [(&str, &str); 4] = [
        ("frm1701q:optSpouseType", "Professional"),
        ("frm1701q:optSpouseATC", "II012"),
        ("frm1701q:optSpouseMethod", "O"),
        ("frm1701q:txt36B", "100000.00"),
    ];

    /// The value `form.fields` read back for box `key`.
    fn read(readback: &Value, key: &str) -> String {
        readback["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["key"] == key)
            .unwrap_or_else(|| panic!("no box {key}"))["value"]
            .as_str()
            .unwrap_or_default()
            .to_string()
    }

    fn made_asks(made: &Value, key: &str) -> bool {
        made["input_schema"]["properties"]
            .as_object()
            .unwrap()
            .values()
            .any(|property| property["x-box"] == key)
    }

    /// A tax rate the ATC replaces is a discarded input: strict preflight
    /// stops the recipe before the form opens.
    #[test]
    fn strict_check_refuses_a_tax_rate_the_atc_replaces() {
        let mut session = session();
        save_complete(&mut session);
        let check = |session: &mut Session, fields: &Value, strict: bool| {
            call(
                session,
                "form.check",
                json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN,
                       "fields": fields, "strict": strict}),
            )
        };
        let mut conflict = graduated_1701q();
        conflict["frm1701q:optATC"] = json!("II015");
        let checked = check(&mut session, &conflict, false).unwrap();
        let discarded = checked["discarded"].as_array().unwrap();
        assert!(
            discarded.contains(&json!("frm1701q:optTaxRate")),
            "{checked}"
        );
        assert_eq!(checked["ok"], false, "{checked}");
        let refused = check(&mut session, &conflict, true).unwrap_err();
        assert!(refused.contains("frm1701q:optTaxRate"), "{refused}");
        // A label or alias fill accepts for the option the return keeps is
        // not a discard.
        for rate in ["Graduated", "graduated", "Graduated Rates"] {
            let mut same = graduated_1701q();
            same["frm1701q:optTaxRate"] = json!(rate);
            let Ok(checked) = check(&mut session, &same, false) else {
                continue;
            };
            if checked["refused"].get("frm1701q:optTaxRate").is_some() {
                continue;
            }
            assert_eq!(checked["discarded"], json!([]), "{rate}: {checked}");
        }
        let checked = check(&mut session, &graduated_1701q(), true).unwrap();
        assert_eq!(checked["ok"], true, "{checked}");

        // The generated recipe stops at its check, with the form unopened.
        let made = call(
            &mut session,
            "recipe.generate",
            json!({"code": "1701Q", "tin": TIN, "check": "strict"}),
        )
        .unwrap();
        let set: Vec<_> = asked(&made, &EIGHT_PERCENT_SET)
            .into_iter()
            .map(|(key, value)| match key {
                "frm1701q:optTaxRate" => (key, "Graduated"),
                _ => (key, value),
            })
            .chain([("frm1701q:optMethodOfDeduction", "O")])
            .collect();
        let stopped = run_recipe(&mut session, &made, 1, &set).unwrap_err();
        assert!(stopped.starts_with("check: "), "{stopped}");
        assert!(stopped.contains("frm1701q:optTaxRate"), "{stopped}");
        assert!(call(&mut session, "form.fields", json!({})).is_err());
    }

    /// The generated recipe refuses an amount the return discards, before
    /// it opens the form, and runs with a zero in the inactive box.
    #[test]
    fn generated_recipe_stops_on_a_discarded_amount() {
        let mut session = session();
        save_complete(&mut session);
        // An itemized return with cost of sales, so the recipe asks for it.
        let mut itemized = graduated_1701q();
        itemized["frm1701q:optMethodOfDeduction"] = json!("I");
        itemized["frm1701q:txt37A"] = json!("40000.00");
        store_1701q(&mut session, 1, &itemized);
        let made = call(
            &mut session,
            "recipe.generate",
            json!({"code": "1701Q", "tin": TIN, "check": "strict"}),
        )
        .unwrap();
        let set = with(
            &asked(&made, &GRADUATED_SET),
            &[("frm1701q:optMethodOfDeduction", "O")],
        );
        let stopped = run_recipe(
            &mut session,
            &made,
            2,
            &with(&set, &[("frm1701q:txt37A", "40000.00")]),
        )
        .unwrap_err();
        assert!(stopped.starts_with("check: "), "{stopped}");
        assert!(stopped.contains("frm1701q:txt37A"), "{stopped}");
        assert!(call(&mut session, "form.fields", json!({})).is_err());
        let done = run_recipe(
            &mut session,
            &made,
            2,
            &with(&set, &[("frm1701q:txt37A", "0.00")]),
        )
        .unwrap();
        assert_eq!(done["check"]["discarded"], json!([]), "{}", done["check"]);
        assert_eq!(done["save"]["saved"], true, "{}", done["save"]);
    }

    /// A stop on Item 16A asks again where the item applies: the method a
    /// stored draft holds is not reused. Under 8%, changing a typed draft
    /// needs approval before the recipe can continue.
    #[test]
    fn explicit_stop_on_the_deduction_method_is_asked_again() {
        let mut session = session();
        save_complete(&mut session);
        store_1701q(&mut session, 1, &graduated_1701q());
        session
            .lock_db()
            .unwrap()
            .snapshot_template_from_latest_return(TIN, "1701Q")
            .unwrap();
        session
            .lock_db()
            .unwrap()
            .set_template_stop(TIN, "1701Q", "frm1701q:optMethodOfDeduction", Some(true))
            .unwrap();
        let made = call(
            &mut session,
            "recipe.generate",
            json!({"code": "1701Q", "tin": TIN, "check": "strict"}),
        )
        .unwrap();
        assert!(made_asks(&made, "frm1701q:optMethodOfDeduction"), "{made}");
        assert!(
            !is_required(&made, "frm1701q:optMethodOfDeduction"),
            "{made}"
        );
        // The period's draft already holds a method of deduction.
        store_1701q(&mut session, 2, &graduated_1701q());
        let stopped =
            run_recipe(&mut session, &made, 2, &asked(&made, &GRADUATED_SET)).unwrap_err();
        assert!(stopped.starts_with("check: "), "{stopped}");
        assert!(stopped.contains("Item 16A"), "{stopped}");
        let answered = with(
            &asked(&made, &GRADUATED_SET),
            &[("frm1701q:optMethodOfDeduction", "I")],
        );
        let done = run_recipe(&mut session, &made, 2, &answered).unwrap();
        assert_eq!(
            read(&done["readback"], "frm1701q:optMethodOfDeduction"),
            "I"
        );
        // An 8% return runs with Item 16A left out, on a prefilled draft too.
        store_1701q(&mut session, 3, &graduated_1701q());
        let stopped =
            run_recipe(&mut session, &made, 3, &asked(&made, &EIGHT_PERCENT_SET)).unwrap_err();
        assert!(stopped.starts_with("fill: approval required:"), "{stopped}");
        assert!(!stopped.contains("save:"), "{stopped}");
        assert!(session.open.as_ref().unwrap().awaiting, "{stopped}");
        assert_eq!(stored_income(&session, 3), "250,000.00");
    }

    /// A strict recipe that moves a stored graduated spouse to 8%: the rate
    /// box writes the ATC and the form derives the rate afterwards, so the
    /// answer is kept, not discarded.
    #[test]
    fn strict_check_accepts_a_spouse_rate_that_replaces_a_stored_one() {
        // No window: the switch drops the spouse's graduated entries, which a
        // window would ask the filer about first.
        let mut session = session();
        session.commands = UiQueue::discarding();
        save_complete(&mut session);
        let mut stored = graduated_1701q();
        for (key, value) in SPOUSE_GRADUATED {
            stored[key] = json!(value);
        }
        stored["frm1701q:optSpouseTaxRate"] = json!("Graduated");
        store_1701q_as(&mut session, 1, &stored, true);
        store_1701q_as(&mut session, 2, &stored, true);
        let made = call(
            &mut session,
            "recipe.generate",
            json!({"code": "1701Q", "tin": TIN, "check": "strict"}),
        )
        .unwrap();
        let filer = with(
            &asked(&made, &GRADUATED_SET),
            &[("frm1701q:optMethodOfDeduction", "O")],
        );
        let spouse: Vec<(&str, &str)> = [
            ("frm1701q:optSpouseType", "Professional"),
            ("frm1701q:optSpouseATC", "II015"),
            ("frm1701q:optSpouseTaxRate", "Percentage"),
        ]
        .into_iter()
        .filter(|(key, _)| made_asks(&made, key))
        .collect();
        assert!(
            spouse
                .iter()
                .any(|(key, _)| *key == "frm1701q:optSpouseTaxRate"),
            "{made}"
        );
        let done = run_recipe(&mut session, &made, 2, &with(&filer, &spouse)).unwrap();
        assert_eq!(done["check"]["ok"], true, "{}", done["check"]);
        assert_eq!(done["check"]["discarded"], json!([]), "{}", done["check"]);
        assert_eq!(
            read(&done["readback"], "frm1701q:optSpouseTaxRate"),
            "Percentage"
        );
    }

    /// The income the database holds for the scratch 1701Q, read as another
    /// session without a window reads it.
    fn stored_income(session: &Session, period: u32) -> String {
        let mut other = Session::new(UiQueue::discarding());
        other.db = session.db.clone();
        call(
            &mut other,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": period, "tin": TIN}),
        )
        .unwrap();
        let fields = call(&mut other, "form.fields", json!({})).unwrap();
        read(&fields, "frm1701q:txt36A")
    }

    #[test]
    fn a_fill_that_removes_what_the_filer_typed_waits_for_the_window() {
        let mut session = session();
        save_complete(&mut session);
        store_1701q(&mut session, 1, &graduated_1701q());
        assert_eq!(stored_income(&session, 1), "250,000.00");
        call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        while session.commands.pop().is_some() {}
        let ask = json!(["frm1701q:optATC", "frm1701q:optTaxRate"]);
        let error = call(&mut session, "form.fill", json!({"fields": {}, "ask": ask})).unwrap_err();
        assert!(error.starts_with("approval required:"), "{error}");
        assert!(error.contains("Nothing was stored;"), "{error}");
        assert!(session.open.as_ref().unwrap().awaiting);
        // The window is asked, with the same answers to drop.
        let choose = session.commands.pop().expect("the window is told");
        assert_eq!(choose["clear"], ask, "{choose}");
        // Before approval, and after leaving with the question open (the
        // window stores nothing): the income is as the filer typed it.
        assert_eq!(stored_income(&session, 1), "250,000.00");
        let fields = call(&mut session, "form.fields", json!({})).unwrap();
        assert_eq!(read(&fields, "frm1701q:txt36A"), "250,000.00");
        assert_eq!(read(&fields, "frm1701q:optATC"), "II012");
        // The filer's Continue can come after any call: the window stores the
        // fill (here, another writer does), and the next read shows it.
        call(&mut session, "form.save_draft", json!({})).unwrap();
        assert_eq!(stored_income(&session, 1), "250,000.00");
        let mut window = Session::new(UiQueue::discarding());
        window.db = session.db.clone();
        call(
            &mut window,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        call(
            &mut window,
            "form.fill",
            json!({"fields": {"frm1701q:txt36A": "300000"}}),
        )
        .unwrap();
        let fields = call(&mut session, "form.fields", json!({})).unwrap();
        assert_eq!(read(&fields, "frm1701q:txt36A"), "300,000.00");
        call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701q:txt36A": "250000"}}),
        )
        .unwrap();
        call(&mut session, "form.save_draft", json!({})).unwrap();
        call(&mut session, "form.dismiss", json!({})).unwrap();
        assert_eq!(stored_income(&session, 1), "250,000.00");
        // Without a window nobody can be asked: the fill is the answer.
        let mut alone = Session::new(UiQueue::discarding());
        alone.db = session.db.clone();
        call(
            &mut alone,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let filled = call(&mut alone, "form.fill", json!({"fields": {}, "ask": ask})).unwrap();
        assert!(filled.get("awaiting_approval").is_none(), "{filled}");
    }

    /// A fill that only leaves a stored choice unanswered (`fields` empty,
    /// the box in `ask`) is taken, and the window is told which answer was
    /// dropped: it saves its own copy of the return whole.
    #[test]
    fn a_fill_that_only_drops_an_answer_tells_the_window() {
        let mut session = session();
        save_complete(&mut session);
        store_1701q(&mut session, 1, &graduated_1701q());
        call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        while session.commands.pop().is_some() {}
        let ask = json!(["frm1701q:optMethodOfDeduction"]);
        let filled = call(&mut session, "form.fill", json!({"fields": {}, "ask": ask})).unwrap();
        assert_eq!(filled["applied"], json!([]), "{filled}");
        let choose = session.commands.pop().expect("the window is told");
        assert_eq!(choose["op"], "choose", "{choose}");
        assert_eq!(choose["clear"], ask, "{choose}");
        assert_eq!(choose["edits"], json!({}), "{choose}");
        let fields = call(&mut session, "form.fields", json!({})).unwrap();
        assert_eq!(read(&fields, "frm1701q:optMethodOfDeduction"), "");
        // With an edit beside it, one message carries both.
        call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701q:optMethodOfDeduction": "O"}}),
        )
        .unwrap();
        while session.commands.pop().is_some() {}
        call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701q:txtSheets": "3"}, "ask": ask}),
        )
        .unwrap();
        let choose = session.commands.pop().expect("the window is told");
        assert_eq!(choose["clear"], ask, "{choose}");
        assert_eq!(choose["edits"]["frm1701q:txtSheets"], "3", "{choose}");
        // Nothing to fill and nothing to drop is still refused.
        assert!(call(&mut session, "form.fill", json!({"fields": {}})).is_err());
        assert!(call(&mut session, "form.fill", json!({"fields": {}, "ask": []})).is_err());
    }

    /// A held return that can no longer be read again (here its taxpayer is
    /// gone) is still dismissed: no other call can close it.
    #[test]
    fn a_held_return_that_cannot_be_reloaded_is_still_dismissed() {
        let mut session = session();
        save_complete(&mut session);
        store_1701q(&mut session, 1, &graduated_1701q());
        call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let ask = json!(["frm1701q:optATC", "frm1701q:optTaxRate"]);
        call(&mut session, "form.fill", json!({"fields": {}, "ask": ask})).unwrap_err();
        assert!(session.open.as_ref().unwrap().awaiting);
        session
            .lock_db()
            .unwrap()
            .delete_profile(TIN)
            .expect("the profile is deleted");
        let error = call(&mut session, "form.fields", json!({})).unwrap_err();
        assert!(error.contains("no taxpayer profile"), "{error}");
        let dismissed = call(&mut session, "form.dismiss", json!({"discard": true})).unwrap();
        assert_eq!(dismissed["dismissed"], true, "{dismissed}");
        assert!(session.open.is_none());
    }

    /// A year or period that is given and cannot be read is refused, never
    /// taken as "the newest return".
    #[test]
    fn recipe_generate_refuses_a_year_or_period_it_cannot_read() {
        let mut session = session();
        save_scratch(&mut session);
        store_1701q(&mut session, 2, &graduated_1701q());
        for (args, named) in [
            (json!({"code": "1701Q", "year": 70000}), "args.year"),
            (json!({"code": "1701Q", "year": "next"}), "args.year"),
            (
                json!({"code": "1701Q", "year": 2026, "period": "Q1"}),
                "args.period",
            ),
            (
                json!({"code": "1701Q", "year": 2026, "period": 4_294_967_298_u64}),
                "args.period",
            ),
        ] {
            let error = call(&mut session, "recipe.generate", args.clone()).unwrap_err();
            assert!(error.contains(named), "{args}: {error}");
        }
        for args in [
            json!({"code": "1701Q"}),
            json!({"code": "1701Q", "year": null, "period": null}),
            json!({"code": "1701Q", "year": 2026, "period": 3}),
        ] {
            call(&mut session, "recipe.generate", args.clone())
                .unwrap_or_else(|error| panic!("{args}: {error}"));
        }
    }

    /// The history a recipe reads ends before the period named. A year
    /// alone is the end of that year. A period alone names no return and is
    /// refused, never read as "the newest return".
    #[test]
    fn recipe_generate_reads_the_history_before_the_period_named() {
        let mut session = session();
        save_scratch(&mut session);
        store_1701q(&mut session, 1, &graduated_1701q());
        store_1701q(&mut session, 3, &graduated_1701q());
        let mut newest = |args: Value| {
            let made = call(&mut session, "recipe.generate", args.clone())
                .unwrap_or_else(|error| panic!("{args}: {error}"));
            made["from"]["previous_returns"][0]["period"].clone()
        };
        assert_eq!(newest(json!({"code": "1701Q"})), 3);
        assert_eq!(newest(json!({"code": "1701Q", "year": 2026})), 3);
        assert_eq!(
            newest(json!({"code": "1701Q", "year": 2026, "period": 2})),
            1
        );
        let error = call(
            &mut session,
            "recipe.generate",
            json!({"code": "1701Q", "period": 2}),
        )
        .unwrap_err();
        assert!(error.contains("args.year"), "{error}");
    }

    /// A box in `ask` the return cannot hold unanswered is refused by name,
    /// as the window's own fill refuses it: its answer stays, the fill's
    /// other boxes are taken and the window is not told to drop it.
    #[test]
    fn a_fill_refuses_to_drop_a_required_choice_and_keeps_its_fields() {
        let mut session = session();
        save_complete(&mut session);
        store_1701q(&mut session, 1, &graduated_1701q());
        call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        while session.commands.pop().is_some() {}
        let key = "frm1701q:AmendedRtn";
        let filled = call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701q:txtSheets": "3"}, "ask": [key]}),
        )
        .unwrap();
        assert_eq!(filled["applied"], json!(["frm1701q:txtSheets"]), "{filled}");
        let why = filled["refused"][key].as_str().unwrap_or_default();
        assert!(why.contains("cannot be left unanswered"), "{filled}");
        let choose = session.commands.pop().expect("the window is told");
        assert_eq!(choose["edits"]["frm1701q:txtSheets"], "3", "{choose}");
        assert!(choose.get("clear").is_none(), "{choose}");
        // The dry run names it the same way.
        let checked = call(
            &mut session,
            "form.check",
            json!({"code": "1701Q", "year": 2026, "period": 2, "tin": TIN, "ask": [key]}),
        )
        .unwrap();
        assert_eq!(checked["ok"], false, "{checked}");
        let why = checked["refused"][key].as_str().unwrap_or_default();
        assert!(why.contains("cannot be left unanswered"), "{checked}");
    }

    /// A 1701MS batch means its final answers, not the accidental JSON order
    /// of its boxes: the source/rate/deduction choices whose official clicks
    /// reset Part V Item 6 are applied before an Item 6 amount either way.
    #[test]
    fn form_fill_and_check_put_1701ms_reset_choices_before_amounts() {
        let amount = "frm1701MS:txtTaxPaidPreviouslyFiledNo6a";
        let choice = "frm1701MS:txtTaxpayerNo11group";
        let amended = "frm1701MS:txtAmendedYesNo2";
        for amount_first in [true, false] {
            let mut session = session();
            save_scratch(&mut session);
            call(
                &mut session,
                "form.open",
                json!({"code": "1701MS", "year": 2025, "period": 0, "tin": TIN}),
            )
            .unwrap();
            call(
                &mut session,
                "form.fill",
                json!({"fields": {amended: "true"}}),
            )
            .unwrap();
            let edits = if amount_first {
                json!({amount: "444", choice: "a"})
            } else {
                json!({choice: "a", amount: "444"})
            };
            let filled = call(&mut session, "form.fill", json!({"fields": edits})).unwrap();
            assert_eq!(
                session.open.as_ref().unwrap().draft["taxpayer"]["previously_filed"],
                json!(444.0),
                "form.fill amount_first={amount_first}: {filled}"
            );
            assert!(
                !filled["not_shown"]
                    .as_array()
                    .is_some_and(|keys| keys.contains(&json!(amount))),
                "form.fill amount_first={amount_first} keeps Item 6 visible: {filled}"
            );

            let check_edits = if amount_first {
                json!({amended: "true", amount: "444", choice: "a"})
            } else {
                json!({amended: "true", choice: "a", amount: "444"})
            };
            let checked = call(
                &mut session,
                "form.check",
                json!({"code": "1701MS", "year": 2025, "period": 0, "tin": TIN,
                       "fields": check_edits}),
            )
            .unwrap();
            assert!(
                !checked["discarded"]
                    .as_array()
                    .is_some_and(|keys| keys.contains(&json!(amount))),
                "form.check amount_first={amount_first} keeps Item 6: {checked}"
            );
        }
    }

    #[test]
    fn form_check_validates_without_writing() {
        let mut session = session();
        save_scratch(&mut session);
        while session.commands.pop().is_some() {}
        let schema = call(&mut session, "form.schema", json!({"code": "1701Q"})).unwrap();
        let sales = schema["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["key"] == "frm1701q:txt36A")
            .unwrap_or_else(|| panic!("{schema}"));
        assert_eq!(sales["fillable"], true, "{sales}");
        assert_eq!(sales["carry"], "amount", "{sales}");
        let checked = call(
            &mut session,
            "form.check",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN,
                   "fields": {"frm1701q:txt36A": "250000.00", "note": "nope"}}),
        )
        .unwrap();
        assert_eq!(checked["saved"], false);
        assert_eq!(checked["ok"], false, "{checked}");
        assert_eq!(checked["applied"], json!(["frm1701q:txt36A"]), "{checked}");
        assert!(checked["refused"]["note"].is_string(), "{checked}");
        let strict = call(
            &mut session,
            "form.check",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN,
                   "fields": {"note": "nope"}, "strict": true}),
        )
        .unwrap_err();
        assert!(strict.contains("nothing was written"), "{strict}");
        assert!(session.open.is_none(), "form.check opens nothing");
        assert!(session.commands.pop().is_none(), "form.check shows nothing");
        let db = session.lock_db().unwrap();
        assert!(
            db.previous_return(TIN, "1701Q", u16::MAX, u32::MAX)
                .unwrap()
                .is_none(),
            "form.check saves no draft"
        );
    }

    #[test]
    fn refusals_and_forms_set_stay_gated() {
        let mut session = session();
        save_scratch(&mut session);
        let file = call(&mut session, "form.file", json!({})).unwrap_err();
        assert!(file.contains("not allow-listed"), "{file}");
        let filing = call(&mut session, "filing.file", json!({})).unwrap_err();
        assert!(filing.contains("not allow-listed"), "{filing}");
        let ensure = call(&mut session, "profile.ensure", json!({})).unwrap_err();
        assert!(
            ensure.contains("profile.ensure is not implemented"),
            "{ensure}"
        );
        let ungated = call(
            &mut session,
            "profile.forms_set",
            json!({"year": 2026, "tin": TIN, "codes": ["1701Q"]}),
        )
        .unwrap_err();
        assert!(ungated.contains("args.confirm=true"), "{ungated}");
        let gated = call(
            &mut session,
            "profile.forms_set",
            json!({"year": 2026, "tin": TIN, "codes": "1701Q", "confirm": true}),
        )
        .unwrap();
        assert_eq!(gated["status"], "ok");
        assert_eq!(gated["confirm"], true);
        assert_eq!(gated["codes"][0], "1701Q");
        let dues = call(
            &mut session,
            "dues.list",
            json!({"filter": "all", "scope": "global"}),
        )
        .unwrap();
        assert!(dues["dues"].as_array().is_some());
        let nav = call(&mut session, "nav.go", json!({"page": "library"})).unwrap();
        assert_eq!(nav["view"], "library");
        assert_eq!(session.commands.pop().unwrap()["op"], "refresh");
        let click = session
            .dispatch(&Op::click("nav-calendar"))
            .unwrap()
            .value
            .unwrap();
        assert_eq!(click["view"], "calendar");
    }

    #[test]
    fn dues_profile_scope_needs_a_selected_taxpayer() {
        let mut session = session();
        let error = call(&mut session, "dues.list", json!({"scope": "profile"})).unwrap_err();
        assert!(error.contains("requires a selected profile"), "{error}");
        let bad = call(&mut session, "dues.list", json!({"filter": "soon"})).unwrap_err();
        assert!(bad.contains("unknown dues filter"), "{bad}");
    }

    #[test]
    fn send_without_a_grant_asks_and_an_invalid_draft_is_not_queued() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "2551Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let context = call(&mut session, "form.context", json!({})).unwrap();
        assert_eq!(context["form"], "2551Q");
        assert!(context["profile"]["pin_hash"].is_null());
        let refused = call(&mut session, "form.send", json!({})).unwrap();
        assert_eq!(refused["status"], "approval_required");
        assert_eq!(refused["pending"], true);
        assert_eq!(refused["queued"], false);
        session.lock_db().unwrap().grant_send(TIN, "2551Q").unwrap();
        let blocked = call(&mut session, "form.send", json!({})).unwrap();
        assert_eq!(blocked["ok"], false);
        assert_eq!(blocked["status"], "invalid");
        assert_eq!(blocked["queued"], false);
        assert_eq!(blocked["filed"], false);
        let same = call(&mut session, "form.validate", json!({})).unwrap();
        assert_eq!(blocked["field_errors"], same["field_errors"]);
        let still = session.lock_db().unwrap();
        let draft = still.get_2551q_draft(TIN, 2026, 1).unwrap().expect("saved");
        assert!(matches!(draft.status, FilingStatus::Draft));
    }

    fn session_with_a_valid_2551q() -> Session {
        let mut session = session();
        call(
            &mut session,
            "profile.save",
            json!({"tin": TIN, "name": "TEST", "rdo_code": "040", "birth_date": "1990-01-15",
                   "address": "1 TEST STREET", "zip": "1100", "phone": "09170000000",
                   "email": "test@example.com"}),
        )
        .unwrap();
        session.lock_db().unwrap().grant_send(TIN, "2551Q").unwrap();
        {
            let db = session.lock_db().unwrap();
            let profile = db.get_profile_for_year(TIN, 2026).unwrap().unwrap();
            let mut draft = Form2551QDraft::new_from_effective_profile(&profile, 2026, 1);
            draft.item_13_election = bir_core::forms::form_2551q::Item13Election::Graduated;
            draft.schedule_1[0].taxable_amount = 10_000.0;
            draft.creditable_tax_withheld = 12.5;
            draft.recompute(None);
            db.save_2551q_draft(&draft).unwrap();
        }
        call(
            &mut session,
            "form.open",
            json!({"code": "2551Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        session
    }

    #[test]
    fn an_assistant_yes_to_no_shown_in_the_window_reports_its_clearing() {
        let mut session = session();
        save_complete(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "1701A", "year": 2026, "period": 0, "tin": TIN}),
        )
        .unwrap();
        call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701A:optForeignTaxCredits_1": "true"}}),
        )
        .unwrap();
        call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701A:txtForeignTaxNumber": "FTN-42"}}),
        )
        .unwrap();
        let id = session.open.as_ref().unwrap().draft["id"]
            .as_i64()
            .expect("the open saved the draft");
        while session.commands.pop().is_some() {}

        let error = call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701A:optForeignTaxCredits_2": "true"}}),
        )
        .unwrap_err();
        assert!(error.starts_with("approval required:"), "{error}");
        let reported = crate::screen_form_module::reported_inline_clearances(Some(id));
        assert!(
            reported.contains("1701a_foreign_tax_number"),
            "the window's approval is the one showing of this clearing; reported: {reported:?}"
        );
    }

    #[test]
    fn form_open_records_a_dormant_2551q_item16_clearing_as_pending() {
        let mut session = session();
        call(
            &mut session,
            "profile.save",
            json!({"tin": TIN, "name": "TEST", "rdo_code": "040", "birth_date": "1990-01-15",
                   "address": "1 TEST STREET", "zip": "1100", "phone": "09170000000",
                   "email": "test@example.com"}),
        )
        .unwrap();
        {
            let db = session.lock_db().unwrap();
            let profile = db.get_profile_for_year(TIN, 2026).unwrap().unwrap();
            let mut draft = Form2551QDraft::new_from_effective_profile(&profile, 2026, 1);
            draft.item_13_election = bir_core::forms::form_2551q::Item13Election::Graduated;
            draft.tax_paid_previous = 42.0;
            draft.recompute(None);
            draft.tax_paid_previous = 42.0;
            db.save_2551q_draft(&draft).unwrap();
        }

        call(
            &mut session,
            "form.open",
            json!({"code": "2551Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        assert_eq!(
            session.open.as_ref().unwrap().draft["tax_paid_previous"],
            0.0
        );
        let db = session.lock_db().unwrap();
        let pending = db
            .settings_with_prefix("notice_pending:2551q_item16:")
            .unwrap();
        assert_eq!(pending.len(), 1, "{pending:?}");
        assert!(pending[0].1.contains("Item 16 was cleared"), "{pending:?}");
    }

    #[test]
    fn form_fill_lets_the_assistant_write_1601fq_schedule1_only_at_item_13_yes() {
        use bir_core::forms::agent::draft_locks::schedule1_1601fq as gate;
        use bir_core::forms::form_1601fq::{Form1601FqDraft, Form1601FqTaxRelief};

        let mut no_session = session();
        save_scratch(&mut no_session);
        call(
            &mut no_session,
            "form.open",
            json!({"code": "1601FQ", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let refused = call(
            &mut no_session,
            "form.fill",
            json!({"fields": {"txtAmtIncomePay0": "42"}}),
        )
        .unwrap();
        assert_eq!(
            refused["refused"]["txtAmtIncomePay0"],
            gate::LOCK,
            "{refused}"
        );

        // Item 13's own radio is the host's editor, so the Yes draft the
        // assistant writes into is one the page already holds at Yes.
        let mut yes_session = session();
        call(
            &mut yes_session,
            "profile.save",
            json!({"tin": TIN, "name": "TEST", "rdo_code": "040", "birth_date": "1990-01-15",
                   "address": "1 TEST STREET", "zip": "1100", "phone": "09170000000",
                   "email": "test@example.com"}),
        )
        .unwrap();
        {
            let db = yes_session.lock_db().unwrap();
            let profile = db.get_profile_for_year(TIN, 2026).unwrap().unwrap();
            let mut draft = Form1601FqDraft::new_from_profile(&profile, 2026, 1);
            draft.set_tax_relief(Some(Form1601FqTaxRelief::SpecialRate));
            db.save_queueable_draft(&draft).unwrap();
        }
        call(
            &mut yes_session,
            "form.open",
            json!({"code": "1601FQ", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let typed = call(
            &mut yes_session,
            "form.fill",
            json!({"fields": {"txtAmtIncomePay0": "42", "txtRate0": "10"}}),
        )
        .unwrap();
        assert!(typed.get("refused").is_none(), "{typed}");
        let row = &yes_session.open.as_ref().unwrap().draft["schedule1"][0];
        assert_eq!(row["income_payment"], 42.0);
        assert_eq!(row["tax_rate"], 10.0);
    }

    #[test]
    fn form_fill_1601fq_item4_no_asks_the_official_sentence_only_with_rows() {
        use bir_core::forms::form_1601fq::{Form1601FqAtcRow, Form1601FqCategory, Form1601FqDraft};

        fn fq_session(withheld: Option<bool>, with_part_ii_row: bool) -> Session {
            let mut session = session();
            save_scratch(&mut session);
            {
                let db = session.lock_db().unwrap();
                let profile = db.get_profile_for_year(TIN, 2026).unwrap().unwrap();
                let mut draft = Form1601FqDraft::new_from_profile(&profile, 2026, 1);
                draft.category = Some(Form1601FqCategory::Private);
                if let Some(withheld) = withheld {
                    draft.set_any_tax_withheld(withheld);
                }
                if with_part_ii_row {
                    draft.schedule.push(Form1601FqAtcRow {
                        popup_index: 0,
                        atc_code: "WI226".to_string(),
                        tax_base: 1_000.0,
                        tax_withheld: 0.0,
                    });
                }
                draft.recompute();
                db.save_queueable_draft(&draft).unwrap();
            }
            call(
                &mut session,
                "form.open",
                json!({"code": "1601FQ", "year": 2026, "period": 1, "tin": TIN}),
            )
            .unwrap();
            session
        }

        let mut rows_session = fq_session(Some(true), true);
        let error = call(
            &mut rows_session,
            "form.fill",
            json!({"fields": {"frm1601FQ:TaxWithheld": "2"}}),
        )
        .unwrap_err();
        assert!(
            error.contains(
                "You are about to change the value to 'No'. Doing this will clear the entries in Part II and Schedule II."
            ),
            "{error}"
        );
        {
            let db = rows_session.lock_db().unwrap();
            let stored: String = db
                .conn_for_tests()
                .query_row("SELECT data_json FROM form_drafts", [], |row| row.get(0))
                .unwrap();
            let stored: Value = serde_json::from_str(&stored).unwrap();
            assert_eq!(stored["any_tax_withheld"], true, "Cancel means no write");
            assert_eq!(stored["schedule"][0]["tax_base"], 1_000.0);
        }

        let mut empty_session = fq_session(Some(true), false);
        let answer = call(
            &mut empty_session,
            "form.fill",
            json!({"fields": {"frm1601FQ:TaxWithheld": "2"}}),
        )
        .unwrap();
        assert!(
            answer["asks"].is_null(),
            "no Part II row: no ask, nothing cleared; {answer}"
        );
        assert_eq!(
            empty_session.open.as_ref().unwrap().draft["any_tax_withheld"],
            false
        );

        let mut imported_session = fq_session(None, true);
        let error = call(
            &mut imported_session,
            "form.fill",
            json!({"fields": {"frm1601FQ:TaxWithheld": "2"}}),
        )
        .unwrap_err();
        assert!(
            error.contains(
                "You are about to change the value to 'No'. Doing this will clear the entries in Part II and Schedule II."
            ),
            "imported unanswered -> No asks once: {error}"
        );
    }

    #[test]
    fn form_open_records_a_dormant_1601fq_schedule1_clearing_by_stable_row() {
        use bir_core::forms::form_1601fq::{Form1601FqDraft, Form1601FqTaxRelief};

        let mut session = session();
        call(
            &mut session,
            "profile.save",
            json!({"tin": TIN, "name": "TEST", "rdo_code": "040", "birth_date": "1990-01-15",
                   "address": "1 TEST STREET", "zip": "1100", "phone": "09170000000",
                   "email": "test@example.com"}),
        )
        .unwrap();
        let id;
        {
            let db = session.lock_db().unwrap();
            let profile = db.get_profile_for_year(TIN, 2026).unwrap().unwrap();
            let mut draft = Form1601FqDraft::new_from_profile(&profile, 2026, 1);
            draft.set_tax_relief(Some(Form1601FqTaxRelief::SpecialRate));
            draft.schedule1[0].income_payment = 42.0;
            draft.schedule1[0].tax_rate = 10.0;
            draft.schedule1[1].income_payment = 24.0;
            draft.schedule1[1].tax_rate = 10.0;
            draft.recompute();
            id = db.save_queueable_draft(&draft).unwrap();

            // A pre-gate row: Item 13 is shut while its typed Schedule 1 row
            // is still in the stored bytes. The save path itself would clear
            // it, so this legacy state is written as bytes.
            let stored: String = db
                .conn_for_tests()
                .query_row(
                    "SELECT data_json FROM form_drafts WHERE id = ?1",
                    [id],
                    |row| row.get(0),
                )
                .unwrap();
            let mut legacy: Value = serde_json::from_str(&stored).unwrap();
            legacy["tax_relief"] = Value::Null;
            db.conn_for_tests()
                .execute(
                    "UPDATE form_drafts SET data_json = ?1 WHERE id = ?2",
                    (serde_json::to_string(&legacy).unwrap(), id),
                )
                .unwrap();
        }

        call(
            &mut session,
            "form.open",
            json!({"code": "1601FQ", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        assert_eq!(session.open.as_ref().unwrap().draft["schedule1"], json!([]));
        let db = session.lock_db().unwrap();
        let pending = db
            .settings_with_prefix("notice_pending:1601fq_schedule1:")
            .unwrap();
        assert_eq!(pending.len(), 1, "{pending:?}");
        assert!(
            pending[0].0.starts_with(
                "notice_pending:1601fq_schedule1:row-00000000000000000001,row-00000000000000000002:"
            ),
            "{pending:?}"
        );
        assert!(
            pending[0]
                .1
                .contains("Schedule 1's rows were cleared: they apply only when item 13 is Yes."),
            "{pending:?}"
        );
        assert!(
            !pending[0]
                .1
                .contains("Schedule 1 row row-00000000000000000001"),
            "{pending:?}"
        );
    }

    #[test]
    fn form_fill_records_an_assistant_recompute_clearing_from_the_stored_row() {
        let mut session = session();
        call(
            &mut session,
            "profile.save",
            json!({"tin": TIN, "name": "TEST", "rdo_code": "040", "birth_date": "1990-01-15",
                   "address": "1 TEST STREET", "zip": "1100", "phone": "09170000000",
                   "email": "test@example.com"}),
        )
        .unwrap();
        {
            let db = session.lock_db().unwrap();
            let profile = db.get_profile_for_year(TIN, 2026).unwrap().unwrap();
            let mut draft = Form2551QDraft::new_from_effective_profile(&profile, 2026, 1);
            draft.is_amended = true;
            draft.item_13_election = bir_core::forms::form_2551q::Item13Election::Graduated;
            draft.tax_paid_previous = 42.0;
            draft.recompute(None);
            assert_eq!(draft.tax_paid_previous, 42.0);
            db.save_2551q_draft(&draft).unwrap();
        }
        call(
            &mut session,
            "form.open",
            json!({"code": "2551Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        assert_eq!(
            session.open.as_ref().unwrap().draft["tax_paid_previous"],
            42.0
        );

        // The fill recomputes before it saves, so its in-memory before-image
        // is already zero. The save must compare with the stored row instead.
        let answer = call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm2551Qv2018:amendedRtn_2": "true"}}),
        )
        .unwrap();
        assert_eq!(
            session.open.as_ref().unwrap().draft["tax_paid_previous"],
            0.0
        );
        assert_eq!(
            answer["clearances"][0]["gate"], "2551q_item16",
            "the assistant's fill is the one announcement: {answer}"
        );
        assert!(
            answer["clearances"][0]["text"]
                .as_str()
                .is_some_and(|text| text.contains("Item 16 was cleared")),
            "{answer}"
        );
        let db = session.lock_db().unwrap();
        let pending = db
            .settings_with_prefix("notice_pending:2551q_item16:")
            .unwrap();
        assert_eq!(pending.len(), 0, "already said at the change: {pending:?}");
        let seen = db
            .settings_with_prefix("notice_seen:2551q_item16:")
            .unwrap();
        assert_eq!(seen.len(), 1, "{seen:?}");
    }

    fn open_1600pt(session: &mut Session) {
        save_scratch(session);
        call(
            session,
            "form.open",
            json!({"code": "1600PT", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
    }

    #[test]
    fn form_fill_holds_the_previous_payment_gate_and_accepts_zero_spellings() {
        let mut session = session();
        open_1600pt(&mut session);
        let kind = AgentFormKind::Queueable(bir_core::forms::queueable::QueueableKind::Form1600PT);
        let spec = bir_core::forms::agent::draft_locks::previous_payment::spec(kind).unwrap();

        let refused = call(
            &mut session,
            "form.fill",
            json!({"fields": {spec.key: "42"}}),
        )
        .unwrap();
        assert_eq!(refused["refused"][spec.key], spec.lock, "{refused}");
        for value in [json!("0"), json!("₱0.00"), json!("(0.00)"), json!(0)] {
            let zero = call(
                &mut session,
                "form.fill",
                json!({"fields": {spec.key: value}}),
            )
            .unwrap();
            assert!(
                zero.get("refused").is_none(),
                "zero spelling {value}: {zero}"
            );
            assert_eq!(
                session.open.as_ref().unwrap().draft[spec.path]
                    .as_f64()
                    .unwrap_or(0.0),
                0.0
            );
        }
    }

    #[test]
    fn form_fill_holds_the_three_new_previous_payment_gates() {
        for (code, key, path, lock) in [
            (
                "0619E",
                "frm0619E:txtTax15",
                "item_15_amount_remitted_previously",
                "Item 15 applies only to an amended return.",
            ),
            (
                "0619F",
                "frm0619F:txtTax16",
                "item_16_remitted_previously",
                "Item 16 applies only to an amended return.",
            ),
            (
                "2550M",
                "frm2550m:txtTax23D",
                "vat_paid_previous",
                "Item 23D applies only to an amended return.",
            ),
        ] {
            let mut session = session();
            save_scratch(&mut session);
            call(
                &mut session,
                "form.open",
                json!({"code": code, "year": 2026, "period": 1, "tin": TIN}),
            )
            .unwrap_or_else(|error| panic!("{code} opens: {error}"));

            let refused = call(&mut session, "form.fill", json!({"fields": {key: "42"}})).unwrap();
            assert_eq!(refused["refused"][key], lock, "{code}: {refused}");
            let zero = call(&mut session, "form.fill", json!({"fields": {key: "0"}})).unwrap();
            assert!(zero.get("refused").is_none(), "{code} zero: {zero}");
            assert_eq!(
                session.open.as_ref().unwrap().draft[path]
                    .as_f64()
                    .unwrap_or(0.0),
                0.0,
                "{code}: zero leaves the model empty"
            );
        }
    }

    #[test]
    fn form_fill_previous_payment_clears_for_every_yes_to_no_key_shape() {
        for (code, group, group_no, yes_key, key, path) in [
            (
                "0619E",
                "frm0619E:optAmend",
                "N",
                "frm0619E:optAmend:Y",
                "frm0619E:txtTax15",
                "item_15_amount_remitted_previously",
            ),
            (
                "0619F",
                "frm0619F:optAmend",
                "N",
                "frm0619F:optAmend:Y",
                "frm0619F:txtTax16",
                "item_16_remitted_previously",
            ),
            (
                "2550M",
                "frm2550m:OptAmendedYN",
                "2",
                "frm2550m:OptAmendedYN1",
                "frm2550m:txtTax23D",
                "vat_paid_previous",
            ),
        ] {
            for (shape, close_key, close_value) in [
                ("group", group.to_string(), json!(group_no)),
                ("yes untick", yes_key.to_string(), json!("false")),
                (
                    "no tick",
                    yes_key.replace(":Y", ":N").replace("YN1", "YN2"),
                    json!("true"),
                ),
            ] {
                let mut session = session();
                session.commands = UiQueue::discarding();
                save_scratch(&mut session);
                call(
                    &mut session,
                    "form.open",
                    json!({"code": code, "year": 2026, "period": 1, "tin": TIN}),
                )
                .unwrap_or_else(|error| panic!("{code} opens: {error}"));
                let opened = call(
                    &mut session,
                    "form.fill",
                    json!({"fields": {yes_key: "true", key: "42"}}),
                )
                .unwrap();
                assert!(opened.get("refused").is_none(), "{code} open: {opened}");
                let closed = call(
                    &mut session,
                    "form.fill",
                    json!({"fields": {close_key: close_value}}),
                )
                .unwrap();
                assert!(closed.get("refused").is_none(), "{code} {shape}: {closed}");
                assert_eq!(
                    session.open.as_ref().unwrap().draft[path]
                        .as_f64()
                        .unwrap_or(0.0),
                    0.0,
                    "{code} {shape} clears the previous payment"
                );
            }
        }
    }

    #[test]
    fn form_fill_already_no_previous_payment_no_click_zeroes_the_dormant_amount() {
        for spec in bir_core::forms::agent::draft_locks::previous_payment::SPECS {
            let code = spec.form_code;
            let mut session = session();
            session.commands = UiQueue::discarding();
            save_scratch(&mut session);
            call(
                &mut session,
                "form.open",
                json!({"code": code, "year": 2026, "period": 1, "tin": TIN}),
            )
            .unwrap_or_else(|error| panic!("{code} opens: {error}"));
            {
                let open = session.open.as_mut().unwrap();
                open.draft["is_amended"] = json!(false);
                open.draft[spec.path] = json!(42.0);
            }
            let clicked = call(
                &mut session,
                "form.fill",
                json!({"fields": {spec.no_key: "true"}}),
            )
            .unwrap();
            assert!(clicked.get("refused").is_none(), "{code}: {clicked}");
            assert_eq!(
                session.open.as_ref().unwrap().draft[spec.path]
                    .as_f64()
                    .unwrap_or(0.0),
                0.0,
                "{code}: the official No click zeroes even when the draft was already No"
            );
        }
    }

    #[test]
    fn form_check_previous_payment_clearing_agrees_with_fill() {
        let mut session = session();
        session.commands = UiQueue::discarding();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "0619E", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let opened = call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm0619E:optAmend:Y": "true", "frm0619E:optWithheld:Y": "true", "frm0619E:txtTax14": "20", "frm0619E:txtTax15": "42"}}),
        )
        .unwrap();
        assert!(opened.get("refused").is_none(), "{opened}");
        let checked = call(
            &mut session,
            "form.check",
            json!({"code": "0619E", "year": 2026, "period": 1, "stored": true, "fields": {"frm0619E:optAmend": "N"}}),
        )
        .unwrap();
        assert_eq!(
            checked["computed"]["frm0619E:txtTax16"], "20.00",
            "check clears the credit as fill does: {checked}"
        );
    }

    #[test]
    fn form_fill_previous_payment_yes_to_no_says_the_clearing_once() {
        let mut session = session();
        open_1600pt(&mut session);
        let kind = AgentFormKind::Queueable(bir_core::forms::queueable::QueueableKind::Form1600PT);
        let spec = bir_core::forms::agent::draft_locks::previous_payment::spec(kind).unwrap();

        let opened = call(
            &mut session,
            "form.fill",
            json!({"fields": {spec.yes_key: "true", spec.key: "42"}}),
        )
        .unwrap();
        assert!(opened.get("refused").is_none(), "{opened}");
        assert_eq!(
            session.open.as_ref().unwrap().draft[spec.path]
                .as_f64()
                .unwrap_or(0.0),
            42.0
        );
        let closed = call(
            &mut session,
            "form.fill",
            json!({"fields": {spec.no_key: "true"}}),
        )
        .unwrap();
        assert!(closed.get("refused").is_none(), "{closed}");
        assert_eq!(
            session.open.as_ref().unwrap().draft[spec.path]
                .as_f64()
                .unwrap_or(0.0),
            0.0
        );
        assert_eq!(closed["clearances"][0]["text"], spec.cleared, "{closed}");
        let db = session.lock_db().unwrap();
        let gate = format!(
            "{}_item{}",
            spec.form_code.to_lowercase(),
            spec.item.to_lowercase()
        );
        let pending = db
            .settings_with_prefix(&format!("notice_pending:{gate}:"))
            .unwrap();
        assert_eq!(pending.len(), 0, "already said at the change: {pending:?}");
    }

    #[test]
    fn form_fields_and_send_hold_2551q_item16_to_its_gate() {
        let mut session = session_with_a_valid_2551q();
        let item16 = bir_core::forms::agent::draft_locks::item16_2551q::KEY;
        let fields = call(&mut session, "form.fields", json!({})).unwrap();
        let box_ = fields["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|box_| box_["key"] == item16)
            .unwrap_or_else(|| panic!("{fields}"));
        assert_eq!(box_["value"], "0.00", "{box_}");
        let refused = call(&mut session, "form.fill", json!({"fields": {item16: "42"}})).unwrap();
        assert_eq!(
            refused["refused"][item16],
            bir_core::forms::agent::draft_locks::item16_2551q::AMENDED_LOCK,
            "{refused}"
        );
        let zero = call(
            &mut session,
            "form.fill",
            json!({"fields": {item16: "0.000"}}),
        )
        .unwrap();
        assert!(zero.get("refused").is_none(), "{zero}");
        assert_eq!(
            session.open.as_ref().unwrap().draft["tax_paid_previous"],
            0.0
        );

        let opened = call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm2551Qv2018:amendedRtn_1": "true", item16: "42"}}),
        )
        .unwrap();
        assert!(opened.get("refused").is_none(), "{opened}");
        let fields = call(&mut session, "form.fields", json!({})).unwrap();
        let box_ = fields["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|box_| box_["key"] == item16)
            .unwrap_or_else(|| panic!("{fields}"));
        assert_eq!(box_["value"], "42.00", "{box_}");

        let sent = call(&mut session, "form.send", json!({})).unwrap();
        assert_eq!(sent["queued"], true, "{sent}");
        let queued = session.open.as_ref().unwrap().draft.clone();
        assert_eq!(queued["tax_paid_previous"], 42.0, "{queued}");
        assert_eq!(
            agent::field_map(AgentFormKind::Form2551Q, &queued).unwrap()[item16],
            "42.00"
        );
    }

    #[test]
    fn form_send_payload_zeroes_2551q_item16_while_shut() {
        let mut session = session_with_a_valid_2551q();
        let item16 = bir_core::forms::agent::draft_locks::item16_2551q::KEY;
        let sent = call(&mut session, "form.send", json!({})).unwrap();
        assert_eq!(sent["queued"], true, "{sent}");
        let queued = session.open.as_ref().unwrap().draft.clone();
        assert_eq!(queued["tax_paid_previous"], 0.0, "{queued}");
        assert_eq!(
            agent::field_map(AgentFormKind::Form2551Q, &queued).unwrap()[item16],
            "0.00"
        );
    }

    #[test]
    fn form_send_queues_a_valid_form_with_gated_rows() {
        use bir_core::forms::form_2000::{
            Form2000AffixtureMode, Form2000Draft, Form2000OtherParty, Form2000PaymentRow,
            Form2000RemittanceRow, form_2000_atc_option,
        };

        let mut session = session();
        call(
            &mut session,
            "profile.save",
            json!({"tin": TIN, "name": "TEST", "rdo_code": "040", "birth_date": "1990-01-15",
                   "address": "1 TEST STREET", "zip": "1100", "phone": "09170000000",
                   "email": "test@example.com"}),
        )
        .unwrap();
        session.lock_db().unwrap().grant_send(TIN, "2000").unwrap();
        {
            let db = session.lock_db().unwrap();
            let profile = db.get_profile_for_year(TIN, 2026).unwrap().unwrap();
            let mut draft = Form2000Draft::new_from_profile(&profile, 2026, 6);
            draft.other_party = Form2000OtherParty::Creditor;
            draft.other_party_name = "Sample Creditor Corp".into();
            draft.other_party_tin = "987654321000".into();
            draft.affixture_mode = Form2000AffixtureMode::Constructive;
            let pick = |code, description| form_2000_atc_option(code, description).unwrap();
            draft
                .set_atc(0, pick("DS101", "ORIGINAL ISSUE OF SHARES OF STOCKS"))
                .unwrap();
            draft.schedule1[0].tax_base = 123_456.78;
            draft.ds106_term_under_a_year = Some(true);
            draft.ds106_term_days = 90;
            draft.schedule2 = vec![Form2000PaymentRow {
                date: "06/05/2026".into(),
                receipt_number: "Rcpt1001".into(),
                amount: 1_000.50,
            }];
            draft.schedule4 = vec![Form2000RemittanceRow {
                rco_code: "Rco01".into(),
                date: "06/25/2026".into(),
                bank: "Sample Bank".into(),
                amount: 5_000.0,
                number_from: "1001".into(),
                number_to: "1100".into(),
            }];
            draft.recompute();
            db.save_queueable_draft(&draft).unwrap();
        }
        call(
            &mut session,
            "form.open",
            json!({"code": "2000", "year": 2026, "period": 6, "tin": TIN}),
        )
        .unwrap();
        let sent = call(&mut session, "form.send", json!({})).unwrap();
        assert_eq!(sent["queued"], true, "{sent}");
    }

    #[test]
    fn context_and_review_are_known_invokes() {
        let mut session = session();
        save_scratch(&mut session);
        let unknown = call(&mut session, "form.context", json!({})).unwrap_err();
        assert!(unknown.contains("open a form"), "{unknown}");
        call(
            &mut session,
            "form.open",
            json!({"code": "2551Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let review = call(&mut session, "form.review_in_app", json!({})).unwrap_err();
        assert!(review.contains("validation error"), "{review}");
        assert!(review.contains("bir_form_validate"), "{review}");
    }

    #[test]
    fn one_form_lock_refuses_second_open() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "2551Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let error = call(
            &mut session,
            "form.open",
            json!({"code": "1601C", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap_err();
        assert!(error.contains("form already open"), "{error}");
    }

    #[test]
    fn dismiss_clean_form_closes_it() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "2551Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        assert_eq!(
            call(&mut session, "form.dismiss", json!({})).unwrap()["dismissed"],
            true
        );
        assert!(call(&mut session, "form.fields", json!({})).is_err());
    }

    #[test]
    fn agent_dismiss_with_unsaved_edits_is_refused() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701q:txtSheets": "3"}, "source": "document"}),
        )
        .unwrap();
        let error = call(&mut session, "form.dismiss", json!({})).unwrap_err();
        assert!(error.contains("unsaved edits"), "{error}");
    }

    #[test]
    fn form_dismiss_discard_keeps_another_sessions_saved_value() {
        let mut session = session();
        save_complete(&mut session);
        store_1701q(&mut session, 1, &graduated_1701q());
        call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701q:txt36A": "300000"}}),
        )
        .unwrap();
        assert_eq!(stored_income(&session, 1), "300,000.00");

        let mut other = Session::new(UiQueue::discarding());
        other.db = session.db.clone();
        call(
            &mut other,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        call(
            &mut other,
            "form.fill",
            json!({"fields": {"frm1701q:txt36A": "400000"}}),
        )
        .unwrap();
        assert_eq!(stored_income(&session, 1), "400,000.00");

        let dismissed = call(&mut session, "form.dismiss", json!({"discard": true})).unwrap();
        assert_eq!(dismissed["discarded"], true, "{dismissed}");
        assert_eq!(stored_income(&session, 1), "400,000.00");
        assert!(call(&mut session, "form.fields", json!({})).is_err());
    }

    #[test]
    fn agent_save_draft_clears_unsaved_edits_so_dismiss_succeeds() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "1701Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1701q:txtSheets": "3"}, "source": "ai"}),
        )
        .unwrap();
        call(&mut session, "form.needs_you", json!({})).unwrap();
        let error = call(&mut session, "form.dismiss", json!({})).unwrap_err();
        assert!(error.contains("unsaved edits"), "{error}");
        call(&mut session, "form.validate", json!({})).unwrap();
        let saved = call(&mut session, "form.save_draft", json!({})).unwrap();
        assert_eq!(saved["saved"], json!(true), "{saved}");
        call(&mut session, "form.dismiss", json!({})).unwrap();
        assert!(call(&mut session, "form.fields", json!({})).is_err());
    }

    #[test]
    fn fill_records_source_and_never_overwrites_user_box() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "1601C", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let key = "frm1601c:CatAgent";
        let typed = call(
            &mut session,
            "form.fill",
            json!({"fields": {key: "P"}, "source": "user", "from_card": true}),
        )
        .unwrap();
        assert_eq!(typed["source"], "user", "{typed}");
        assert_eq!(typed["filled"], json!([key]), "{typed}");
        let result = call(
            &mut session,
            "form.fill",
            json!({"fields": {key: "G"}, "source": "document"}),
        )
        .unwrap();
        assert_eq!(result["source"], "document", "{result}");
        assert_eq!(result["kept_user_boxes"], json!([key]), "{result}");
        assert_eq!(result["filled"], json!([]), "{result}");
        let fields = call(&mut session, "form.fields", json!({})).unwrap();
        let box_ = fields["boxes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|box_| box_["key"] == key)
            .unwrap_or_else(|| panic!("{fields}"));
        assert_eq!(box_["source"], "user", "{box_}");
        assert_eq!(box_["value"], "P", "{box_}");
    }

    #[test]
    fn needs_you_lists_required_empty_boxes() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "1601C", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let needs = call(&mut session, "form.needs_you", json!({})).unwrap();
        assert_eq!(needs["form"], "1601C");
        assert_eq!(needs["ok"], false);
        let boxes = needs["boxes"].as_array().unwrap();
        assert!(!boxes.is_empty(), "{needs}");
        for item in boxes {
            assert!(!item["label"].as_str().unwrap().is_empty(), "{item}");
            assert_eq!(item["required"], true, "{item}");
            assert_eq!(item["value"], "", "{item}");
            assert!(item["error"].is_string(), "{item}");
        }
        let fields: Vec<&str> = boxes
            .iter()
            .map(|item| item["field"].as_str().unwrap())
            .collect();
        assert!(fields.contains(&"frm1601c:TaxWithheld"), "{fields:?}");
        assert!(fields.contains(&"frm1601c:CatAgent"), "{fields:?}");
        // Pre-filled boxes are not listed.
        assert!(!fields.contains(&"frm1601c:AmendedRtn"), "{fields:?}");
        // Profile-backed identity fields are reported as other errors, not boxes.
        let others: Vec<&str> = needs["other_errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|item| item["field"].as_str().unwrap())
            .collect();
        assert!(others.contains(&"contact_number"), "{others:?}");
    }

    #[test]
    fn validate_returns_field_errors() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "2551Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let validation = call(&mut session, "form.validate", json!({})).unwrap();
        assert_eq!(validation["ok"], false);
        assert!(!validation["field_errors"].as_array().unwrap().is_empty());
    }

    #[test]
    fn form_context_exposes_profile_and_past_returns() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "2551Q", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();
        let context = call(&mut session, "form.context", json!({})).unwrap();
        assert_eq!(context["profile"]["registered_name"], "TEST");
        assert!(context["past_returns"].as_array().is_some());
    }

    /// Once the worker has sent a return, `form.open` (called again), `form.fields` and `form.context`'s
    /// `past_returns` say whether that send reached nobody (`dry_run`, the worker's `submitted_dry_run`) and the send
    /// state the screens show (`send`: stage, line ...), with the stored row's status. A Draft and a Queued return
    /// have no finished send to tell of: `dry_run` is null, as `form.send` says it.
    #[test]
    fn the_agent_reads_whether_a_send_was_a_test_send() {
        use crate::send_state::TEST_SENT_LINE;
        let mut session = session();
        save_scratch(&mut session);
        let args = json!({"code": "2551Q", "year": 2026, "period": 1, "tin": TIN});
        let draft = call(&mut session, "form.open", args.clone()).unwrap();
        assert_eq!(draft["status"], "Draft");
        assert!(draft["dry_run"].is_null(), "{draft}");
        assert!(draft["send"].is_null(), "{draft}");
        let id = session.open.as_ref().unwrap().draft["id"].as_i64().unwrap();
        // Queued and not sent yet: no send has finished, whatever this build's dry-run setting says.
        {
            let db = session.lock_db().unwrap();
            let (_, stored) = db.draft_summary_and_json(id).unwrap().unwrap();
            let mut queued: Value = serde_json::from_str(&stored).unwrap();
            queued["status"] = json!("Queued");
            db.conn_for_tests()
                .execute(
                    "UPDATE form_drafts SET status = 'Queued', data_json = ?1 WHERE id = ?2",
                    (queued.to_string(), id),
                )
                .unwrap();
        }
        // form.fields reads the status from the stored row its `send` comes from, not from the form held open.
        let fields = call(&mut session, "form.fields", json!({})).unwrap();
        assert_eq!(fields["status"], "Queued", "form.fields: {fields}");
        assert!(fields["dry_run"].is_null(), "form.fields: {fields}");
        let queued = call(&mut session, "form.open", args.clone()).unwrap();
        assert_eq!(queued["status"], "Queued");
        assert!(queued["dry_run"].is_null(), "form.open: {queued}");
        let context = call(&mut session, "form.context", json!({})).unwrap();
        let past = context["past_returns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|past| past["form"] == "2551Q" && past["year"] == 2026 && past["quarter"] == 1)
            .expect("the queued return")
            .clone();
        assert_eq!(past["status"], "Queued");
        assert!(past["dry_run"].is_null(), "form.context: {past}");
        // The worker's store of a relay dry_run result (bir-core: Submitted, submitted_dry_run, its receipt).
        {
            let db = session.lock_db().unwrap();
            let (_, stored) = db.draft_summary_and_json(id).unwrap().unwrap();
            let mut sent: Value = serde_json::from_str(&stored).unwrap();
            sent["status"] = json!("Submitted");
            sent["submitted_at"] = json!("2026-04-10T01:02:03+00:00");
            sent["submitted_dry_run"] = json!(true);
            sent["relay_receipt"] = json!("a.b.c");
            db.conn_for_tests()
                .execute(
                    "UPDATE form_drafts SET status = 'Submitted', data_json = ?1 WHERE id = ?2",
                    (sent.to_string(), id),
                )
                .unwrap();
        }
        let is_test_send = |answer: &Value, what: &str| {
            assert_eq!(answer["dry_run"], true, "{what}: {answer}");
            assert_eq!(answer["send"]["stage"], "test_sent", "{what}: {answer}");
            assert_eq!(answer["send"]["line"], TEST_SENT_LINE, "{what}: {answer}");
        };
        let opened = call(&mut session, "form.open", args).unwrap();
        assert_eq!(opened["status"], "Submitted");
        is_test_send(&opened, "form.open");
        let fields = call(&mut session, "form.fields", json!({})).unwrap();
        assert_eq!(fields["status"], "Submitted");
        is_test_send(&fields, "form.fields");
        let context = call(&mut session, "form.context", json!({})).unwrap();
        let past = context["past_returns"]
            .as_array()
            .unwrap()
            .iter()
            .find(|past| past["form"] == "2551Q" && past["year"] == 2026 && past["quarter"] == 1)
            .expect("the sent return")
            .clone();
        assert_eq!(past["status"], "Submitted");
        is_test_send(&past, "form.context");
    }

    /// `form.send`'s answer never claims a real or a test send it cannot know: through the Buwiz relay the server
    /// decides (a test TIN), so `dry_run` is null until it is sent, and the return does not go "from this computer".
    #[test]
    fn a_queued_answer_says_only_what_its_route_knows() {
        let relay = queued_answer("2551Q", 7, true, false);
        assert!(relay["dry_run"].is_null(), "{relay}");
        let message = relay["message"].as_str().unwrap();
        assert!(!message.contains("from this computer"), "{message}");
        assert!(message.contains("Buwiz relay"), "{message}");
        assert!(message.contains("form.open"), "{message}");
        assert_eq!(relay["queued"], true);
        assert_eq!(relay["filed"], false);
        assert_eq!(relay["id"], 7);
        // The Buwiz relay wins over this build's dry-run setting: the server decides there.
        assert!(queued_answer("2551Q", 7, true, true)["dry_run"].is_null());
        let dry = queued_answer("2551Q", 7, false, true);
        assert_eq!(dry["dry_run"], true);
        assert_eq!(
            dry["message"],
            "Queued. This build is a dry run: nothing is sent to BIR."
        );
        let local = queued_answer("2551Q", 7, false, false);
        assert_eq!(local["dry_run"], false);
        assert_eq!(local["message"], "Queued for upload from this computer.");
    }

    /// Official rules: `rules/forms/2552-v2018/fixtures/runtime-control-inventory-v796.json`
    /// (`txtPg1Pt1I12TotShare` lines 307-318, `txtPg2P4I2Amount..I5Amount` lines 823-906)
    /// and `rules/forms/2552-v2018/fixtures/source-v796.html` (lines 759, 1327-1357).
    /// `form.fill` must enforce the same page gates (`screen_form_module::off_boxes`)
    /// as the form page, refusing gated boxes with the reason a person sees until
    /// the opening answer is filled.
    #[test]
    fn form_fill_2552_refuses_page_gated_boxes_until_opened() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "2552", "year": 2026, "period": 1, "tin": TIN}),
        )
        .unwrap();

        // Fresh return: Item 12 total shares and Part IV row 2 are shut on the page.
        let fresh = call(
            &mut session,
            "form.fill",
            json!({
                "fields": {
                    "frm2552:txtPg1Pt1I12TotShare": "1000000",
                    "frm2552:txtPg2P4I2Amount": "500000",
                }
            }),
        )
        .unwrap();
        assert!(
            fresh["filled"].as_array().unwrap().is_empty(),
            "fresh 2552 should refuse both gated boxes: {fresh}"
        );
        let share_reason = fresh["refused"]["frm2552:txtPg1Pt1I12TotShare"]
            .as_str()
            .unwrap_or("");
        assert!(
            share_reason.starts_with("Needs "),
            "expected page gate reason for Item 12 total shares, got {fresh}"
        );
        let row2_reason = fresh["refused"]["frm2552:txtPg2P4I2Amount"]
            .as_str()
            .unwrap_or("");
        assert!(
            row2_reason.contains("Part IV row 1"),
            "expected Part IV row 1 gate reason, got {fresh}"
        );

        // Under LSE, Item 12 total shares stays shut.
        let under_lse = call(
            &mut session,
            "form.fill",
            json!({
                "fields": {
                    "frm2552:rdoPg1I11Transaction": "LSE",
                    "frm2552:txtPg1Pt1I12TotShare": "1000000",
                }
            }),
        )
        .unwrap();
        assert!(
            under_lse["refused"]["frm2552:txtPg1Pt1I12TotShare"]
                .as_str()
                .is_some_and(|s| s.starts_with("Needs ")),
            "LSE must keep Item 12 total shares shut: {under_lse}"
        );

        // Switching to Primary opens Item 12 total shares; filling Part IV row 1 opens row 2.
        let opened = call(
            &mut session,
            "form.fill",
            json!({
                "fields": {
                    "frm2552:rdoPg1I11Transaction": "Primary",
                    "frm2552:txtPg1Pt1I12TotShare": "1000000",
                    "frm2552:txtPg2P4I1TransClass": "Gift",
                    "frm2552:txtPg2P4I1Amount": "250000",
                    "frm2552:txtPg2P4I2TransClass": "Swap",
                    "frm2552:txtPg2P4I2Amount": "500000",
                }
            }),
        )
        .unwrap();
        let filled: Vec<&str> = opened["filled"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .collect();
        assert!(
            filled.contains(&"frm2552:txtPg1Pt1I12TotShare"),
            "Primary should open Item 12 total shares: {opened}"
        );
        assert!(
            filled.contains(&"frm2552:txtPg2P4I2Amount"),
            "Part IV row 1 should open row 2: {opened}"
        );
        assert!(
            opened.get("refused").is_none(),
            "no refusals expected once opened: {opened}"
        );
    }

    #[test]
    fn form_fill_1701_fresh_return_refuses_page_gated_boxes() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "1701", "year": 2025, "period": 0, "tin": TIN}),
        )
        .unwrap();

        // A fresh 1701 is a single filer's: item 17 (spouse income) waits on
        // item 16 Married, and Part IV's spouse boxes wait on a joint return
        // (items 16 Married, 17 Yes, 18 Joint Filing). The page fades them;
        // form.fill must refuse them with the same reasons.
        let fresh = call(
            &mut session,
            "form.fill",
            json!({
                "fields": {
                    "frm1701:rdoPg1I17SpouseIncomeYes": "true",
                    "frm1701:txtPg2I5SpouseName": "SPOUSE NAME",
                }
            }),
        )
        .unwrap();
        assert!(
            fresh["filled"].as_array().unwrap().is_empty(),
            "fresh 1701 should refuse both gated boxes: {fresh}"
        );
        let income_reason = fresh["refused"]["frm1701:rdoPg1I17SpouseIncomeYes"]
            .as_str()
            .unwrap_or("");
        assert!(
            income_reason.contains("married filer") && income_reason.contains("item 16"),
            "item 17 should carry the item-16-Married gate reason, got: {income_reason}"
        );
        let tin_reason = fresh["refused"]["frm1701:txtPg2I5SpouseName"]
            .as_str()
            .unwrap_or("");
        assert!(
            tin_reason.contains("joint return") && tin_reason.contains("item 18"),
            "spouse name should carry the joint-return gate reason, got: {tin_reason}"
        );

        // Counter-case: making the gating choices opens the same boxes.
        let opened = call(
            &mut session,
            "form.fill",
            json!({
                "fields": {
                    "frm1701:rdoPg1I16CivilStatusM": "true",
                    "frm1701:rdoPg1I17SpouseIncomeYes": "true",
                    "frm1701:rdoPg1I18FilingStatusJ": "true",
                    "frm1701:txtPg2I5SpouseName": "SPOUSE NAME",
                }
            }),
        )
        .unwrap();
        assert!(
            opened.get("refused").is_none(),
            "once married + spouse income + joint filing are chosen the same boxes fill: {opened}"
        );
    }

    /// Official 1702EX runtime fixture (`rules/forms/1702ex-v2018c/fixtures/
    /// runtime-control-inventory-v796.json`): Schedule 1 Item 17i and Schedule
    /// 2 Item 4 (including Legal Basis) are popup/attached-schedule-owned
    /// controls that BIR's program keeps shut. `form.fill` must refuse all
    /// five boxes and name the attached schedule rather than accepting Legal
    /// Basis or saying only "computed by BIR's own formulas".
    #[test]
    fn form_fill_1702ex_refuses_popup_owned_schedule_1_and_2_boxes_by_name() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "1702EX", "year": 2025, "period": 0, "tin": TIN}),
        )
        .unwrap();
        let reply = call(
            &mut session,
            "form.fill",
            json!({
                "fields": {
                    "frm1702EX:txtPg3Pt6I17OthersIDesc": "Other expense",
                    "frm1702EX:txtPg3Pt6I17OthersIAmt": "50000",
                    "frm1702EX:txtPg3Pt6S2I4Description": "Special deduction",
                    "frm1702EX:txtPg3Pt6S2I4LegalBasis": "RA 9999",
                    "frm1702EX:txtPg3Pt6S2I4Amount": "75000"
                }
            }),
        )
        .unwrap();
        assert_eq!(
            reply["filled"],
            json!([]),
            "all five 1702EX popup-owned boxes must be refused: {reply}"
        );
        for (key, expected_schedule) in [
            (
                "frm1702EX:txtPg3Pt6I17OthersIDesc",
                "Schedule 1 Item 17i (more...) attached schedule",
            ),
            (
                "frm1702EX:txtPg3Pt6I17OthersIAmt",
                "Schedule 1 Item 17i (more...) attached schedule",
            ),
            (
                "frm1702EX:txtPg3Pt6S2I4Description",
                "Schedule 2 Item 4 (more...) attached schedule",
            ),
            (
                "frm1702EX:txtPg3Pt6S2I4LegalBasis",
                "Schedule 2 Item 4 (more...) attached schedule",
            ),
            (
                "frm1702EX:txtPg3Pt6S2I4Amount",
                "Schedule 2 Item 4 (more...) attached schedule",
            ),
        ] {
            let reason = reply["refused"][key].as_str().unwrap_or("");
            assert!(
                reason.contains(expected_schedule),
                "expected `{key}` refusal to name `{expected_schedule}`, got `{reason}` ({reply})"
            );
        }
    }

    #[test]
    fn form_fill_1702mx_nolco_populates_official_xml() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "1702MX", "year": 2025, "period": 0, "tin": TIN}),
        )
        .unwrap();
        let reply = call(
            &mut session,
            "form.fill",
            json!({
                "fields": {
                    "frm1702MX:rdoPg1Pt1I13MethodOfDeducItemized": "true",
                    "frm1702MX:txtPg3IShed7_4Year": "2022",
                    "frm1702MX:txtPg3IShed7_4A": "300000",
                    "frm1702MX:txtPg3IShed7_4B": "50000",
                    "frm1702MX:txtPg3IShed7_4C": "20000",
                    "frm1702MX:txtPg3IShed7_4D": "80000",
                    "frm1702MX:txtPg3IShed8_4Year": "2022",
                    "frm1702MX:txtPg4IShed8_4A": "400000",
                    "frm1702MX:txtPg4IShed8_4B": "60000",
                    "frm1702MX:txtPg4IShed8_4C": "25000",
                    "frm1702MX:txtPg4IShed8_4D": "90000"
                }
            }),
        )
        .unwrap();
        assert!(
            reply.get("refused").is_none(),
            "unexpected refusals: {reply}"
        );

        let draft = session.open.as_ref().expect("open 1702MX").draft.clone();
        let typed: bir_core::forms::form_1702mx::Form1702MXDraft =
            serde_json::from_value(draft).unwrap();
        let layout = typed.official_layout().unwrap();
        let official_xml =
            bir_core::official_xml::write(&layout, &typed.to_official_field_map()).unwrap();
        let parsed = bir_core::bir_xml::parse_bir_xml_checked(&official_xml).unwrap();
        let print_map = typed.to_print_field_map();
        for (key, expected) in [
            ("frm1702MX:txtPg3IShed7_4A", "300,000.00"),
            ("frm1702MX:txtPg3IShed7_4B", "50,000.00"),
            ("frm1702MX:txtPg3IShed7_4C", "20,000.00"),
            ("frm1702MX:txtPg3IShed7_4D", "80,000.00"),
            ("frm1702MX:txtPg3IShed7_4E", "150,000"),
            ("frm1702MX:txtPg3IShed78D", "80,000"),
            ("frm1702MX:txtPg4IShed8_4A", "400,000.00"),
            ("frm1702MX:txtPg4IShed8_4B", "60,000.00"),
            ("frm1702MX:txtPg4IShed8_4C", "25,000.00"),
            ("frm1702MX:txtPg4IShed8_4D", "90,000.00"),
            ("frm1702MX:txtPg4IShed8_4E", "225,000"),
            ("frm1702MX:txtPg3IShed88D", "90,000"),
        ] {
            assert_eq!(
                parsed.get(key).map(String::as_str),
                Some(expected),
                "form.fill official upload XML missing {key}"
            );
            assert_eq!(
                print_map.get(key).map(String::as_str),
                Some(expected),
                "form.fill print map missing {key}"
            );
        }
    }

    #[test]
    fn form_fill_1702mx_nolco_matches_official_sample_xml_byte_for_byte() {
        let mut session = session();
        save_scratch(&mut session);
        call(
            &mut session,
            "form.open",
            json!({"code": "1702MX", "year": 2025, "period": 0, "tin": TIN}),
        )
        .unwrap();
        let reply = call(
            &mut session,
            "form.fill",
            json!({
                "fields": {
                    "frm1702MX:rdoPg1Pt1I13MethodOfDeducItemized": "true",
                    "frm1702MX:txtPg3IShed7_4Year": "2022",
                    "frm1702MX:txtPg3IShed7_4A": "300000",
                    "frm1702MX:txtPg3IShed7_4B": "100000",
                    "frm1702MX:txtPg3IShed7_4C": "25000",
                    "frm1702MX:txtPg3IShed7_4D": "50000"
                }
            }),
        )
        .unwrap();
        assert!(
            reply.get("refused").is_none(),
            "unexpected refusals: {reply}"
        );
        // Clear 4C back to 0 and verify it matches official "0".
        let cleared = call(
            &mut session,
            "form.fill",
            json!({"fields": {"frm1702MX:txtPg3IShed7_4C": "0"}}),
        )
        .unwrap();
        assert!(
            cleared.get("refused").is_none(),
            "unexpected refusals on clear: {cleared}"
        );

        let draft = session.open.as_ref().expect("open 1702MX").draft.clone();
        let typed: bir_core::forms::form_1702mx::Form1702MXDraft =
            serde_json::from_value(draft).unwrap();
        let layout = typed.official_layout().unwrap();
        let ours_xml =
            bir_core::official_xml::write(&layout, &typed.to_official_field_map()).unwrap();

        let official_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../bir-core/tests/sample-payloads/1702MX-1225-sched6.official.xml");
        let official_xml = std::fs::read_to_string(&official_path).unwrap();

        let nolco_block = |xml: &str| -> String {
            xml.lines()
                .filter(|line| {
                    line.contains("IShed7_")
                        || line.contains("IShed78D")
                        || line.contains("IShed8_")
                        || line.contains("IShed88D")
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        let want_block = nolco_block(&official_xml);
        let got_block = nolco_block(&ours_xml);
        assert_eq!(want_block.lines().count(), 50);
        assert_eq!(
            got_block, want_block,
            "form.fill official upload XML Schedule 7.1 & 8.1 block must match official sample byte for byte"
        );

        let print_map = typed.to_print_field_map();
        assert_eq!(print_map["frm1702MX:txtPg3IShed7_4E"], "150,000");
        assert_eq!(print_map["frm1702MX:txtPg3IShed78D"], "50,000");
    }
}
