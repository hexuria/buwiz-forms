//! UI-thread host ops. Each one calls bir-core through the shell's database
//! and, when the screen should move, pushes a command the script applies.

use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};

use bir_core::db::Database;
use bir_core::forms::agent::{self, AgentFormKind, Binding, CardBox, FromProfile};
use bir_core::forms::{FilingStatus, FormSetSource, PerYearFormsSet};
use bir_core::profile::TaxpayerProfile;
use bir_core::profile::TaxpayerType;
use bir_core::tax_profile::{
    BranchFacts, BranchField, BranchUpdate, FieldSetting, NewTaxpayer, TaxpayerUpdate,
};
use chrono::Datelike;
use gpui_agent::protocol::{DeliveryMode, HelloAuth, HelloInfo, Op, PlatformKind};
use gpui_agent::tree::UiTree;
use gpui_agent::{AgentHost, DispatchResult, PROTOCOL_VERSION};
use serde_json::{Map, Value, json};

use super::{APP, database_kind, tree_from_debug};

/// Commands waiting for `bir.agent_take`.
#[derive(Clone)]
pub(crate) struct UiQueue {
    inner: Arc<Mutex<VecDeque<Value>>>,
}

impl UiQueue {
    pub(crate) fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    pub(crate) fn push(&self, value: Value) {
        self.inner.lock().expect("agent ui queue").push_back(value);
    }

    pub(crate) fn pop(&self) -> Option<Value> {
        self.inner.lock().expect("agent ui queue").pop_front()
    }
}

struct OpenForm {
    kind: AgentFormKind,
    year: u16,
    period: u8,
    tin: String,
    draft: Value,
    sources: BTreeMap<String, String>,
}

pub(crate) struct Session {
    db: Option<Arc<Mutex<Database>>>,
    commands: UiQueue,
    selected_tin: Option<String>,
    open: Option<OpenForm>,
    tree: UiTree,
    quit: bool,
}

impl Session {
    pub(crate) fn new(commands: UiQueue) -> Self {
        Self {
            db: None,
            commands,
            selected_tin: None,
            open: None,
            tree: tree_from_debug(None),
            quit: false,
        }
    }

    pub(crate) fn set_db(&mut self, db: Arc<Mutex<Database>>) {
        self.db = Some(db);
    }

    pub(crate) fn needs_db(&self) -> bool {
        self.db.is_none()
    }

    pub(crate) fn set_tree(&mut self, tree: UiTree) {
        self.tree = tree;
    }

    pub(crate) fn should_quit(&self) -> bool {
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
        match name {
            "nav.go" => {
                let page = args
                    .get("page")
                    .and_then(Value::as_str)
                    .ok_or("nav.go requires args.page")?;
                self.navigate(page)
            }
            "app.info" => self.app_info(),
            "profile.list" => self.profile_list(),
            "profile.set" => self.profile_set(args),
            "profile.save" => self.profile_save(args),
            "profile.forms_set" | "profile.forms_set.set" => self.profile_forms_set(args),
            "dues.list" => self.dues_list(args),
            "filing.start" | "form.open" => self.form_open(args),
            "form.fields" => self.form_fields(),
            "form.fill" => self.form_fill(args),
            "form.needs_you" => self.form_needs_you(),
            "filing.validate" | "form.validate" => self.form_validate(),
            "form.save_draft" => self.form_save_draft(),
            "form.dismiss" => self.form_dismiss(),
            "form.file" | "form.submit_external" | "filing.file" => Err(format!(
                "invoke `{name}` is not allow-listed; queue with form.queue/filing.queue confirm=true, then let cron PUT"
            )),
            "profile.ensure" => Err(
                "profile.ensure is not implemented; the host will not auto-create a taxpayer. Use profile.create to open the editor; profile.save stays confirm-gated"
                    .into(),
            ),
            other => Err(format!("unknown invoke `{other}`")),
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
            let saved_name = if let Some(name) = name.clone() {
                db.update_taxpayer(
                    &root,
                    TaxpayerUpdate {
                        name: Some(name.clone()),
                        ..TaxpayerUpdate::default()
                    },
                )
                .map_err(|error| error.to_string())?;
                name
            } else {
                existing.taxpayer.name.clone()
            };
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
        let profile = load_profile(&db, &tin)?;
        let mut draft =
            crate::screen_form_module::load_draft(&db, kind, &profile, year, period, false, None)?;
        if agent::identity(kind, &draft)?.status == FilingStatus::Draft {
            agent::save_draft(kind, &db, &mut draft)?;
        }
        drop(db);
        self.selected_tin = Some(tin.clone());
        self.open = Some(OpenForm {
            kind,
            year,
            period,
            tin: tin.clone(),
            draft,
            sources: BTreeMap::new(),
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
        })))
    }

    fn form_fields(&self) -> Result<DispatchResult, String> {
        let open = self
            .open
            .as_ref()
            .ok_or("open a form first (bir_form_open)")?;
        let def = open.kind.definition();
        let map = agent::field_map(open.kind, &open.draft)?;
        let boxes: Vec<Value> = def
            .fillable()
            .map(|entry| {
                card_json(
                    &CardBox::new(def, entry, &map),
                    open.sources.get(&entry.key),
                )
            })
            .collect();
        let questions = def.boxes.iter().filter(|entry| entry.is_question()).count();
        let status = agent::identity(open.kind, &open.draft)?.status;
        Ok(DispatchResult::json(json!({
            "form": open.kind.form_code(),
            "status": format!("{status:?}"),
            "id": open.draft.get("id").cloned().unwrap_or(Value::Null),
            "boxes": boxes,
            "read_only": questions.saturating_sub(boxes.len()),
            "computed": def.boxes.iter().filter(|entry| entry.computed).count(),
        })))
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
        let mut draft = open.draft.clone();
        let sources = open.sources.clone();
        if agent::identity(kind, &draft)?.status != FilingStatus::Draft {
            return Err("this return is no longer a draft".into());
        }
        let fields = collect_fill_fields(args)?;
        let source = fill_source(args, &fields, kind, &draft)?;
        let db = self.lock_db()?;
        let profile = load_profile(&db, &tin)?;
        let from_profile = profile_context(&db, kind, &profile, year, period)?;
        let def = kind.definition();
        let (mut filled, mut kept) = (Vec::new(), Vec::new());
        let mut refused = Map::new();
        for (key, value) in &fields {
            if sources
                .get(key)
                .is_some_and(|kept_source| kept_source == "user")
                && source != "user"
            {
                kept.push(key.clone());
                continue;
            }
            if let Err(why) = agent::fill(kind, &mut draft, key, value, Some(&from_profile)) {
                refused.insert(key.clone(), Value::String(why));
                continue;
            }
            filled.push(key.clone());
        }
        agent::compute(kind, &mut draft)?;
        let map = agent::field_map(kind, &draft)?;
        let not_shown = not_shown(def, &fields, &filled, &map);
        let mut edits = Map::new();
        if !filled.is_empty() {
            agent::save_draft(kind, &db, &mut draft)?;
            for key in &filled {
                if not_shown.iter().any(|hidden| hidden == key) {
                    continue;
                }
                edits.insert(key.clone(), Value::String(def.box_value(key, &map)));
            }
        }
        drop(db);
        if let Some(open) = self.open.as_mut() {
            open.draft = draft;
            for key in &filled {
                open.sources.insert(key.clone(), source.clone());
            }
        }
        if !edits.is_empty() {
            self.commands.push(json!({"op": "choose", "edits": edits}));
        }
        let mut result = json!({
            "form": kind.form_code(),
            "source": source,
            "applied": filled,
            "filled": filled.clone(),
            "kept_user_boxes": kept,
        });
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
        }
        Ok(DispatchResult::json(json!({
            "status": format!("{status:?}"),
            "saved": true,
            "form": kind.form_code(),
            "id": id,
        })))
    }

    fn form_dismiss(&mut self) -> Result<DispatchResult, String> {
        let Some(open) = self.open.take() else {
            return Err("no form is open to dismiss".into());
        };
        self.commands.push(json!({"op": "go", "view": "home"}));
        Ok(DispatchResult::json(json!({
            "dismissed": true,
            "form": open.kind.form_code(),
        })))
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

impl AgentHost for Session {
    fn hello(&self) -> HelloInfo {
        HelloInfo {
            protocol: PROTOCOL_VERSION,
            app: APP.into(),
            platform: PlatformKind::Desktop,
            os: gpui_agent::protocol::host_os(),
            ready: true,
            deliveries: vec![DeliveryMode::Semantic],
            auth: HelloAuth::None,
            bundle_id: None,
            executable: None,
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

fn load_profile(db: &Database, tin: &str) -> Result<TaxpayerProfile, String> {
    db.get_profile_by_tin(tin)
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
            if !matches!(key.as_str(), "fields" | "source" | "from_card") {
                fields.insert(key.clone(), value.clone());
            }
        }
    }
    if fields.is_empty() {
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

fn not_shown(
    def: &agent::AgentFormDef,
    fields: &Map<String, Value>,
    filled: &[String],
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
                return shown.is_empty();
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

fn parse_due(raw: &str) -> Option<chrono::NaiveDate> {
    let date = raw.get(..10).unwrap_or(raw);
    chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui_agent::protocol::{Op, Request, ScreenshotMode};
    use gpui_agent::{handle_request, screenshot_unavailable};

    const TIN: &str = "12345678800000";
    const ROOT: &str = "123456788";

    fn session() -> Session {
        let db = Arc::new(Mutex::new(
            Database::open_ephemeral().expect("ephemeral database"),
        ));
        let mut session = Session::new(UiQueue::new());
        session.set_db(db);
        session
    }

    fn call(session: &mut Session, name: &str, args: Value) -> Result<Value, String> {
        session
            .invoke(name, &args)
            .map(|result| result.value.unwrap_or(Value::Null))
    }

    fn save_scratch(session: &mut Session) -> Value {
        call(
            session,
            "profile.save",
            json!({"tin": TIN, "name": "Scratch Taxpayer", "rdo_code": "040"}),
        )
        .unwrap()
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
        assert_eq!(listed[0]["name"], "Scratch Taxpayer");
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
}
