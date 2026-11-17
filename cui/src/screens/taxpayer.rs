//! The taxpayer screens (port of `nsh/screens/taxpayer/`): the page's header and Profile / Forms /
//! Templates tabs, and the New taxpayer / New branch pages. Route `profile`, `forms`, `templates`
//! are the page's three tabs; `new_taxpayer`, `new_branch` are their own pages.

use std::cell::RefCell;

use chrono::Datelike;
use cui::Ui;
use serde_json::{Value, json};

use crate::lib::format::format_tin;
use crate::state::{self, AppState, ViewName};
use crate::store;
use crate::types::{Branch, Taxpayer};
use crate::ui::kit::UiExt;

const TAXPAYER_TYPES: [&str; 6] = [
    "Individual",
    "Corporation",
    "Partnership",
    "Cooperative",
    "Estate",
    "Trust",
];

const FREQS: [&str; 5] = ["All", "Monthly", "Quarterly", "Annual", "As needed"];

/// The profile's collapsible sections, as a first visit finds them (actions.ts `firstVisit`).
thread_local! {
    static SEC_OPEN: RefCell<std::collections::HashMap<&'static str, bool>> = RefCell::new(
        [
            ("taxpayer", true),
            ("branch", true),
            ("taxtypes", true),
            ("facts", true),
            ("address", true),
            ("inbox", true),
            ("agent", false),
        ]
        .into_iter()
        .collect()
    );
}

fn sec_open(id: &str) -> bool {
    SEC_OPEN.with(|m| *m.borrow().get(id).unwrap_or(&true))
}

/// `toggle` in ProfileTab: flip the section; closing it cancels its edit, fact edit or inbox edit.
fn toggle_sec(app: &mut AppState, id: &'static str) {
    let on = !sec_open(id);
    eprintln!("[sec] toggle {id} -> {on}");
    SEC_OPEN.with(|m| {
        m.borrow_mut().insert(id, on);
    });
    if !on {
        EDIT.with(|e| {
            if e.borrow().as_ref().is_some_and(|x| x.sec == id) {
                *e.borrow_mut() = None;
            }
        });
        if id == "facts" {
            cancel_fact_edit();
        }
        if id == "inbox" {
            INBOX_EDIT.with(|e| *e.borrow_mut() = None);
        }
    }
    let _ = app;
}

/// The Disclosure header: chevron + title (toggles), the section's meta, and Edit when editable.
/// Returns whether the body should draw.
fn disclosure_head(
    ui: &mut Ui,
    app: &mut AppState,
    id: &'static str,
    title: &str,
    meta: &str,
    edit_sec: Option<&'static str>,
) -> bool {
    // CUI ids a button by its label: every section's chevron/Edit would share one widget
    // without a per-section id scope.
    let _g = ui.push_id(id);
    let open = sec_open(id);
    ui.row(&Default::default(), |ui| {
        let chevron = if open {
            "chevron-down"
        } else {
            "chevron-right"
        };
        if ui.icon_button_with(
            crate::ui::icons::glyph(chevron),
            &cui::Button {
                variant: cui::ButtonVariant::Ghost,
                size: cui::ControlSize::Sm,
                tooltip: Some(if open { "Collapse" } else { "Expand" }),
                ..Default::default()
            },
        ) {
            toggle_sec(app, id);
        }
        if ui.ghost_button(title, true) {
            toggle_sec(app, id);
        }
        if !meta.is_empty() {
            ui.muted(meta);
        }
        ui.spacer();
        if open && let Some(sec) = edit_sec {
            if ui.ghost_button("Edit", true) {
                // startEdit also opens the section (page.ts)
                SEC_OPEN.with(|m| {
                    m.borrow_mut().insert(sec, true);
                });
                start_edit(app, sec);
            }
        }
    });
    open
}

// ---------------------------------------------------------------- shared bits

fn who(app: &AppState) -> Option<(Taxpayer, Option<Branch>)> {
    let tp = app.taxpayer()?;
    Some((tp.clone(), app.branch()))
}

fn tin_line(app: &AppState) -> String {
    let tp = app.taxpayer();
    let br = app.branch();
    format_tin(
        tp.as_ref().map(|t| t.tin_root.as_str()).unwrap_or(""),
        br.as_ref().map(|b| b.code.as_str()),
    )
}

/// The header: name, "TIN · branch · type", New return; then the three tabs.
fn draw_header(ui: &mut Ui, app: &mut AppState) {
    let (tp_name, tp_type) = app
        .taxpayer()
        .map(|t| (t.name.clone(), t.taxpayer_type.clone()))
        .unwrap_or_default();
    let branch_name = app.branch().map(|b| b.name.clone()).unwrap_or_default();
    let meta = [tin_line(app), branch_name, tp_type]
        .into_iter()
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" · ");
    ui.heading(&tp_name, 1);
    ui.muted(&meta);
    ui.row(&Default::default(), |ui| {
        ui.spacer();
        if ui.primary_button("New return", true) {
            app.start_new_return(None);
        }
    });
    // tabs
    let view = app.view();
    ui.row(&Default::default(), |ui| {
        for (v, label) in [
            (ViewName::Profile, "Profile"),
            (ViewName::Forms, "Forms"),
            (ViewName::Templates, "Templates"),
        ] {
            let active = view == v;
            let count = match v {
                ViewName::Forms => app.year_forms().len(),
                ViewName::Templates => app.templates().len(),
                _ => 0,
            };
            let text = if count > 0 {
                format!("{label} ({count})")
            } else {
                label.to_owned()
            };
            if ui.ghost_button(&text, true) && !active {
                app.go(v, Default::default());
            }
        }
    });
}

// ---------------------------------------------------------------- profile tab

/// The section being edited (page.ts SectionEdit): every value as typed, which branch fields
/// follow the head office, touched fields, errors, and the tax year a year-fields edit belongs to.
#[derive(Clone, Default)]
struct SectionEdit {
    /// taxpayer | branch | address
    sec: String,
    values: Vec<(String, String)>,
    /// The taxpayer type being edited (taxpayer section).
    tp_type: String,
    /// Branch field -> "Same as head office".
    inherit: Vec<(String, bool)>,
    touched: Vec<String>,
    submitted: bool,
    /// The TIN or branch code move's own error (or the host's answer to it).
    tin_error: Option<String>,
    /// The registered activity has the year's line of business and start date (a year exists).
    year_fields: bool,
    /// The tax year those fields were read from: the edit belongs to it, and a save under another
    /// year is refused.
    year: Option<i64>,
    error: Option<String>,
}

thread_local! {
    static EDIT: RefCell<Option<SectionEdit>> = const { RefCell::new(None) };
}

fn branch_field(branch: &Branch, name: &str) -> String {
    match name {
        "rdo_code" => branch.facts.rdo_code.clone().unwrap_or_default(),
        "registered_address" => branch.facts.registered_address.clone().unwrap_or_default(),
        "zip_code" => branch.facts.zip_code.clone().unwrap_or_default(),
        "phone" => branch.facts.phone.clone().unwrap_or_default(),
        "email" => branch.facts.email.clone().unwrap_or_default(),
        _ => String::new(),
    }
}

fn edit_get(edit: &SectionEdit, key: &str) -> String {
    edit.values
        .iter()
        .find(|(n, _)| n == key)
        .map(|(_, v)| v.clone())
        .unwrap_or_default()
}

fn edit_set(edit: &mut SectionEdit, key: &str, value: String) {
    if let Some(slot) = edit.values.iter_mut().find(|(n, _)| n == key) {
        slot.1 = value;
    } else {
        edit.values.push((key.to_owned(), value));
    }
}

fn edit_inherit(edit: &SectionEdit, field: &str) -> bool {
    edit.inherit
        .iter()
        .find(|(f, _)| f == field)
        .map(|(_, v)| *v)
        .unwrap_or(false)
}

/// The year_facts view of the branch's current year, when the host answered one.
fn year_facts_view(app: &AppState) -> Option<crate::lib::facts::YearFactsView> {
    let b = app.branch()?;
    let y = app.year()?;
    facts::year_facts_view(&state::year_facts(&b.tin, y))
}

/// startEdit (page.ts): the section's values as they stand, inherit flags, and the year binding.
fn start_edit(app: &AppState, sec: &str) {
    let Some((tp, br)) = who(app) else {
        return;
    };
    let mut e = SectionEdit {
        sec: sec.to_owned(),
        tp_type: tp.taxpayer_type.clone(),
        ..Default::default()
    };
    match sec {
        "taxpayer" => {
            e.values.push(("name".into(), tp.name.clone()));
            e.values.push(("tin".into(), tp.tin_root.clone()));
            if tp.taxpayer_type == "Individual" {
                e.values.push((
                    "birth_date".into(),
                    tp.birth_date.clone().unwrap_or_default(),
                ));
            }
        }
        "branch" => {
            if let Some(b) = &br {
                if !b.is_head_office {
                    e.values.push(("code".into(), b.code.clone()));
                }
                e.values.push(("name".into(), b.name.clone()));
                e.values
                    .push(("rdo_code".into(), branch_field(b, "rdo_code")));
                e.inherit.push((
                    "rdo_code".into(),
                    b.inherited.iter().any(|f| f == "rdo_code"),
                ));
                if let Some(view) = year_facts_view(app) {
                    e.year_fields = true;
                    e.year = Some(view.year);
                    e.values.push((
                        "line_of_business".into(),
                        view.facts
                            .get("line_of_business")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned(),
                    ));
                    e.values.push((
                        "business_start_date".into(),
                        view.facts
                            .get("business_start_date")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_owned(),
                    ));
                }
            }
        }
        "address" => {
            if let Some(b) = &br {
                for f in ["registered_address", "zip_code", "phone", "email"] {
                    e.values.push((f.into(), branch_field(b, f)));
                    e.inherit
                        .push((f.into(), b.inherited.iter().any(|x| x == f)));
                }
            }
        }
        _ => {}
    }
    EDIT.with(|cell| *cell.borrow_mut() = Some(e));
}

/// A year switch closes an edit that holds the year's facts (leaveYear): the taxpayer's own
/// section is the same in every year, so it stays open.
fn leave_year_edit() {
    EDIT.with(|cell| {
        if cell
            .borrow()
            .as_ref()
            .is_some_and(|e| e.sec == "branch" && e.year_fields)
        {
            *cell.borrow_mut() = None;
        }
    });
}

fn known_rdo(code: &str) -> bool {
    let list = state::rdo_options();
    list.is_empty() || list.iter().any(|(v, _)| v == code)
}

/// problems (page.ts): every problem with the section as typed, by field.
fn edit_problems(app: &AppState, e: &SectionEdit) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    let tp = app.taxpayer();
    let b = app.branch();
    let own = |f: &str| b.as_ref().is_none_or(|b| b.is_head_office) || !edit_inherit(e, f);
    if e.sec == "taxpayer" {
        let tin = edit_get(e, "tin");
        if tin.len() != 9 {
            out.push(("tin".into(), tlogic::TIN_ERROR.to_owned()));
        }
        if let Some(v) = e.values.iter().find(|(n, _)| n == "birth_date") {
            let err = tlogic::past_date_error(v.1.trim(), &chrono::Local::now().date_naive());
            if let Some(err) = err {
                out.push(("birth_date".into(), err));
            }
        }
    }
    if e.sec == "branch" {
        let code = edit_get(e, "code");
        if !code.is_empty()
            && let Some(b) = &b
            && code != b.code
        {
            if !(code.len() == 5 && code.bytes().all(|c| c.is_ascii_digit())) {
                out.push(("code".into(), tlogic::BRANCH_CODE_ERROR.to_owned()));
            } else if code == "00000"
                || tp
                    .as_ref()
                    .is_some_and(|t| t.branches.iter().any(|x| x.code == code))
            {
                out.push(("code".into(), format!("Branch {code} already exists.")));
            }
        }
        for f in ["rdo_code", "zip_code", "email"] {
            let v = edit_get(e, f);
            if v.is_empty() || !own(f) {
                continue;
            }
            if let Some(err) = tlogic::fact_error(f, &v, Some(&known_rdo)) {
                out.push((f.into(), err));
            }
        }
        let lob = edit_get(e, "line_of_business");
        let _ = lob;
        for (f, key) in [("business_start_date", "business_start_date")] {
            let v = edit_get(e, key);
            if let Some(err) = tlogic::past_date_error(v.trim(), &chrono::Local::now().date_naive())
            {
                out.push((f.into(), err));
            }
        }
    }
    if e.sec == "address" {
        for f in ["registered_address", "zip_code", "phone", "email"] {
            let v = edit_get(e, f);
            if v.is_empty() || !own(f) {
                continue;
            }
            if let Some(err) = tlogic::fact_error(f, &v, Some(&known_rdo)) {
                out.push((f.into(), err));
            }
        }
    }
    out
}

/// The errors the section shows now: a field's once it was left (touched), every one after the
/// first submit, and the move's own (errors()).
fn edit_errors(app: &AppState, e: &SectionEdit) -> Vec<(String, String)> {
    let mut out: Vec<(String, String)> = Vec::new();
    for (f, msg) in edit_problems(app, e) {
        if e.submitted || e.touched.iter().any(|t| t == &f) {
            out.push((f, msg));
        }
    }
    if let Some(err) = &e.tin_error {
        let key = if e.sec == "taxpayer" { "tin" } else { "code" };
        if !out.iter().any(|(f, _)| f == key) {
            out.push((key.to_owned(), err.clone()));
        }
    }
    out
}

fn save_edit(app: &mut AppState) {
    let taken = EDIT.with(|e| e.borrow_mut().take());
    let Some(mut edit) = taken else {
        return;
    };
    let Some(tp) = app.taxpayer() else {
        return;
    };
    let Some(b) = app.branch() else {
        return;
    };
    // An edit that holds a year's facts is that year's: if the year changed under it, nothing of
    // it is written into the new one.
    if edit.year_fields {
        let now_year = year_facts_view(app).map(|v| v.year);
        if edit.year != now_year {
            app.toast(
                "The tax year changed while this was open, so nothing was saved. Open the edit again for this year.",
                Some("triangle-alert"),
                None,
            );
            return;
        }
    }
    edit.submitted = true;
    if !edit_problems(app, &edit).is_empty() {
        EDIT.with(|e| *e.borrow_mut() = Some(edit));
        return;
    }
    match save_edit_inner(app, &edit, &tp, &b) {
        Ok(SaveOutcome::Done) => {
            store::bump(Some("taxpayer"));
        }
        Ok(SaveOutcome::MoveRefused(msg)) => {
            edit.tin_error = Some(msg);
            EDIT.with(|e| *e.borrow_mut() = Some(edit));
        }
        Err(msg) => {
            edit.tin_error = None;
            edit.error = Some(msg);
            EDIT.with(|e| *e.borrow_mut() = Some(edit));
        }
    }
}

/// What a save answered (page.ts saveSection's outcomes).
enum SaveOutcome {
    Done,
    /// The TIN/branch-code move was refused after the rest saved: the edit stays open with the
    /// move's own error.
    MoveRefused(String),
}

fn save_edit_inner(
    app: &mut AppState,
    edit: &SectionEdit,
    tp: &Taxpayer,
    b: &Branch,
) -> Result<SaveOutcome, String> {
    let get = |k: &str| edit_get(edit, k);
    if edit.sec == "taxpayer" {
        let digits = get("tin");
        let mut update = serde_json::Map::new();
        update.insert(
            "name".into(),
            json!(if get("name").trim().is_empty() {
                tp.name.clone()
            } else {
                get("name").trim().to_owned()
            }),
        );
        update.insert("taxpayer_type".into(), json!(edit.tp_type));
        if edit.values.iter().any(|(n, _)| n == "birth_date") {
            let born_v = get("birth_date");
            let born = born_v.trim();
            let cur = tp.birth_date.clone().unwrap_or_default();
            if born != cur {
                update.insert(
                    "birth_date".into(),
                    if born.is_empty() {
                        Value::Null
                    } else {
                        json!(born)
                    },
                );
            }
        }
        store::write(
            "update_taxpayer",
            vec![json!(tp.tin_root), Value::Object(update)],
        )
        .map_err(|e| crate::lib::errors::error_text(&e))?;
        if digits != tp.tin_root {
            // One transaction in bir-core: branches, tax years, returns and templates move
            // together. A refusal must not leave a moved TIN with the rest unsaved.
            if let Err(err) =
                store::write("move_taxpayer_tin", vec![json!(tp.tin_root), json!(digits)])
            {
                return Ok(SaveOutcome::MoveRefused(format!(
                    "Name and type saved. The TIN did not change: {}",
                    crate::lib::errors::error_text(&err)
                )));
            }
            let old = tp.tin_root.clone();
            let code = b.code.clone();
            let year = app.year();
            app.remap_recent(&|tin| {
                if tin.starts_with(&old) && tin.len() >= 9 {
                    format!("{}{}", digits, &tin[9..])
                } else {
                    tin.to_owned()
                }
            });
            app.select_taxpayer(&digits, true, Some(code));
            if let Some(y) = year {
                app.select_year(y);
            }
            let shown = tlogic::format_root(&digits);
            app.toast(
                format!("TIN changed to {shown}. Every branch, tax year, return and template moved with it."),
                Some("circle-check"),
                None,
            );
        } else {
            app.toast("Changes saved", Some("circle-check"), None);
        }
        return Ok(SaveOutcome::Done);
    }
    if edit.sec == "branch" {
        let mut current = b.clone();
        let code = get("code");
        if !code.is_empty() && code != b.code {
            let old_tin = b.tin.clone();
            let year = app.year();
            if let Ok(v) = store::write("move_branch_code", vec![json!(b.tin), json!(code)]) {
                let moved: Option<crate::types::Branch> = serde_json::from_value(v).ok();
                if let Some(m) = moved {
                    current = m;
                }
            }
            let new_tin = format!("{}{}", tp.tin_root, code);
            app.remap_recent(&|tin| {
                if tin == old_tin {
                    new_tin.clone()
                } else {
                    tin.to_owned()
                }
            });
            app.select_branch(&code);
            if let Some(y) = year {
                app.select_year(y);
            }
        }
        let mut fields = serde_json::Map::new();
        fields.insert(
            "rdo_code".into(),
            if !current.is_head_office && edit_inherit(edit, "rdo_code") {
                Value::Null
            } else {
                json!(get("rdo_code").trim())
            },
        );
        store::write(
            "update_branch",
            vec![
                json!(current.tin),
                json!({
                    "name": if get("name").trim().is_empty() { current.name.clone() } else { get("name").trim().to_owned() },
                    "fields": Value::Object(fields),
                }),
            ],
        )
        .map_err(|e| crate::lib::errors::error_text(&e))?;
        if let Some(view) = year_facts_view(app)
            && edit.year_fields
        {
            let mut patch = serde_json::Map::new();
            let line = get("line_of_business").trim().to_owned();
            let start = get("business_start_date").trim().to_owned();
            let cur_line = view
                .facts
                .get("line_of_business")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let cur_start = view
                .facts
                .get("business_start_date")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if line != cur_line {
                patch.insert(
                    "line_of_business".into(),
                    if line.is_empty() {
                        Value::Null
                    } else {
                        json!(line)
                    },
                );
            }
            if start != cur_start {
                patch.insert(
                    "business_start_date".into(),
                    if start.is_empty() {
                        Value::Null
                    } else {
                        json!(start)
                    },
                );
            }
            if !patch.is_empty() {
                store::write(
                    "set_year_facts",
                    vec![json!(current.tin), json!(view.year), Value::Object(patch)],
                )
                .map_err(|e| crate::lib::errors::error_text(&e))?;
            }
        }
        app.toast("Changes saved", Some("circle-check"), None);
        return Ok(SaveOutcome::Done);
    }
    // address
    let mut fields = serde_json::Map::new();
    for f in ["registered_address", "zip_code", "phone", "email"] {
        fields.insert(
            f.to_owned(),
            if !b.is_head_office && edit_inherit(edit, f) {
                Value::Null
            } else {
                json!(get(f).trim())
            },
        );
    }
    store::write(
        "update_branch",
        vec![json!(b.tin), json!({ "fields": Value::Object(fields) })],
    )
    .map_err(|e| crate::lib::errors::error_text(&e))?;
    app.toast("Changes saved", Some("circle-check"), None);
    Ok(SaveOutcome::Done)
}

fn field_label(name: &str) -> &'static str {
    match name {
        "name" => "Name",
        "tin" => "TIN (9 digits)",
        "tin_root" => "TIN (9 digits)",
        "birth_date" => "Birth date (YYYY-MM-DD)",
        "code" => "Branch code (5 digits)",
        "rdo_code" => "RDO",
        "registered_address" => "Registered address",
        "zip_code" => "ZIP code",
        "phone" => "Phone",
        "email" => "Email",
        "line_of_business" => "Line of business",
        "business_start_date" => "Business start date (YYYY-MM-DD)",
        _ => "Value",
    }
}

fn draw_section_edit(ui: &mut Ui, app: &mut AppState) {
    let Some(mut edit) = EDIT.with(|e| e.borrow_mut().take()) else {
        return;
    };
    let title = match edit.sec.as_str() {
        "taxpayer" => "Edit taxpayer",
        "branch" => "Edit branch",
        _ => "Edit address & contact",
    };
    let errors = edit_errors(app, &edit);
    let mut consumed = false;
    ui.card(&Default::default(), |ui| {
        ui.section_title(title);
        for (name, value) in edit.values.clone() {
            let field_err = errors
                .iter()
                .find(|(f, _)| f == &name)
                .map(|(_, m)| m.clone());
            let mut v = value;
            let is_date = matches!(name.as_str(), "birth_date" | "business_start_date");
            if ui.text_input(
                &format!("tp-edit-{name}"),
                &mut v,
                field_label(&name),
                is_date,
            ) {
                edit_set(&mut edit, &name, v);
                if !edit.touched.iter().any(|t| t == &name) {
                    edit.touched.push(name.clone());
                }
                if name == "tin" || name == "code" {
                    edit.tin_error = None;
                }
            }
            // "Same as head office" on a branch field (page.ts setInherit)
            if !matches!(edit.sec.as_str(), "taxpayer")
                && edit.inherit.iter().any(|(f, _)| f == &name)
                && app
                    .taxpayer()
                    .is_some_and(|t| !t.branches.iter().any(|x| x.is_head_office) || true)
                && app.branch().is_some_and(|b| !b.is_head_office)
            {
                let mut inh = edit_inherit(&edit, &name);
                if ui.checkbox("Same as head office", &mut inh) {
                    if let Some(slot) = edit.inherit.iter_mut().find(|(f, _)| f == &name) {
                        slot.1 = inh;
                    }
                    if inh
                        && let Some(head) = app
                            .taxpayer()
                            .and_then(|t| t.branches.iter().find(|x| x.is_head_office).cloned())
                    {
                        edit_set(&mut edit, &name, branch_field(&head, &name));
                    }
                }
            }
            if let Some(err) = field_err {
                ui.danger_text(&err);
            }
        }
        if edit.sec == "taxpayer" {
            let tp = app
                .taxpayer()
                .map(|t| t.taxpayer_type.clone())
                .unwrap_or_default();
            let idx = TAXPAYER_TYPES
                .iter()
                .position(|t| *t == edit.tp_type)
                .or_else(|| TAXPAYER_TYPES.iter().position(|t| *t == tp))
                .unwrap_or(0) as i32;
            let refs: Vec<&str> = TAXPAYER_TYPES.to_vec();
            let mut i = idx;
            if ui.select("tp-type", &refs, &mut i) {
                edit.tp_type = TAXPAYER_TYPES
                    .get(i.max(0) as usize)
                    .copied()
                    .unwrap_or("Individual")
                    .to_owned();
            }
        }
        if let Some(err) = &edit.error {
            ui.danger_text(err);
        }
        ui.row(&Default::default(), |ui| {
            if ui.ghost_button("Cancel", true) {
                consumed = true;
            }
            if ui.primary_button("Save", true) {
                consumed = true;
                EDIT.with(|e| *e.borrow_mut() = Some(edit.clone()));
                save_edit(app);
            }
        });
    });
    if !consumed {
        EDIT.with(|e| *e.borrow_mut() = Some(edit));
    }
}

fn fact_row(ui: &mut Ui, label: &str, value: &str) {
    ui.row(&Default::default(), |ui| {
        ui.text(label);
        ui.spacer();
        ui.muted(if value.is_empty() { "—" } else { value });
    });
}

// ---------------------------------------------------------------- facts your forms use (Facts.tsx)

use crate::lib::facts::{self, FactName};

#[derive(Clone)]
struct FactEdit {
    fact: FactName,
    tin: String,
    year: i64,
    value: Value,
    atc: String,
    error: Option<String>,
}

thread_local! {
    static FACT_EDIT: RefCell<Option<FactEdit>> = const { RefCell::new(None) };
}

/// The selected branch's `year_facts` view for the picked year.
fn facts_view(app: &AppState) -> Option<facts::YearFactsView> {
    let b = app.branch()?;
    let y = app.year()?;
    facts::year_facts_view(&state::year_facts(&b.tin, y))
}

fn election_ready(app: &AppState) -> Option<facts::ElectionReadiness> {
    let b = app.branch()?;
    let y = app.year()?;
    facts::readiness(&state::election_readiness(&b.tin, y))
}

/// The facts the year's forms read, in ROW_FACTS order (shownFacts).
fn shown_facts(view: &facts::YearFactsView) -> Vec<FactName> {
    facts::ROW_FACTS
        .iter()
        .copied()
        .filter(|f| !view.readers(*f).is_empty())
        .collect()
}

/// Whether a row is flagged "Not set": the rate only when the taxpayer chooses one (rowFlagged).
fn row_flagged(
    view: &facts::YearFactsView,
    ready: Option<&facts::ElectionReadiness>,
    fact: FactName,
) -> bool {
    if !view.not_set.contains(&fact) {
        return false;
    }
    if fact != FactName::IncomeTaxElection {
        return true;
    }
    ready.is_some_and(|r| r.gap.as_deref() == Some("ready"))
}

fn start_fact_edit(app: &AppState, fact: FactName) {
    let (Some(view), Some(b), Some(y)) = (facts_view(app), app.branch(), app.year()) else {
        return;
    };
    let current = view.facts.get(fact.wire()).cloned().unwrap_or(Value::Null);
    let atc = if fact == FactName::AtcCodes {
        current
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .unwrap_or_default()
    } else {
        String::new()
    };
    FACT_EDIT.with(|e| {
        *e.borrow_mut() = Some(FactEdit {
            fact,
            tin: b.tin.clone(),
            year: y,
            value: current,
            atc,
            error: None,
        })
    });
}

/// A fact's choices as (value, label) pairs (choices in Facts.tsx).
fn fact_choices(fact: FactName, taxpayer_type: Option<&str>) -> Vec<(String, String)> {
    match fact {
        FactName::TaxClassification => facts::income_type_choices(taxpayer_type)
            .into_iter()
            .map(|(v, l)| (v.to_owned(), l.to_owned()))
            .collect(),
        FactName::IncomeTaxElection => facts::ELECTION_CHOICES
            .iter()
            .map(|(v, l)| (v.to_string(), l.to_string()))
            .collect(),
        FactName::EoptTier => facts::EOPT_TIERS
            .iter()
            .map(|(v, l)| (v.to_string(), l.to_string()))
            .collect(),
        _ => vec![
            ("true".to_owned(), "Yes".to_owned()),
            ("false".to_owned(), "No".to_owned()),
        ],
    }
}

/// The displayed value of a fact row (value() in FactRow).
fn fact_display(
    view: &facts::YearFactsView,
    ready: Option<&facts::ElectionReadiness>,
    fact: FactName,
) -> String {
    let v = facts::fact_value(fact, Some(&view.facts), view.year as i64);
    let chooses = fact != FactName::IncomeTaxElection
        || ready.is_some_and(|r| r.gap.as_deref() == Some("ready"));
    if v.is_some() || chooses {
        return v.unwrap_or_default();
    }
    if ready.is_some_and(|r| r.gap.as_deref() == Some("not_individual")) {
        "Does not apply".to_owned()
    } else {
        "Not chosen".to_owned()
    }
}

fn cancel_fact_edit() {
    FACT_EDIT.with(|e| *e.borrow_mut() = None);
}

/// saveFactEdit: set_year_facts with the one patched key; errors stay on the row.
fn save_fact_edit(app: &mut AppState) {
    let taken = FACT_EDIT.with(|e| e.borrow_mut().take());
    let Some(edit) = taken else {
        return;
    };
    let mut value = edit.value.clone();
    if edit.fact == FactName::AtcCodes {
        match facts::parse_atc_codes(&edit.atc) {
            Some(codes) => value = json!(codes),
            None => {
                FACT_EDIT.with(|e| {
                    if let Some(x) = e.borrow_mut().as_mut() {
                        x.error = Some(
                            "Type ATC codes like WC160 or PT010, with commas between them."
                                .to_owned(),
                        );
                    }
                });
                return;
            }
        }
    } else if value.is_null() {
        FACT_EDIT.with(|e| {
            if let Some(x) = e.borrow_mut().as_mut() {
                x.error = Some("Choose one first.".to_owned());
            }
        });
        return;
    }
    let patch = json!({ edit.fact.wire(): value });
    match store::write(
        "set_year_facts",
        vec![json!(edit.tin), json!(edit.year), patch],
    ) {
        Ok(_) => {
            app.toast(
                format!("{} saved for {}.", facts::fact_label(edit.fact), edit.year),
                Some("circle-check"),
                None,
            );
            store::bump(Some("set_year_facts"));
        }
        Err(e) => {
            let msg = crate::lib::errors::error_text(&e)
                .trim_start_matches("Other error: ")
                .to_owned();
            FACT_EDIT.with(|cell| {
                if let Some(x) = cell.borrow_mut().as_mut() {
                    x.error = Some(msg);
                }
            });
        }
    }
}

fn draw_fact_edit(ui: &mut Ui, app: &mut AppState, edit: &mut FactEdit) {
    if edit.fact == FactName::AtcCodes {
        ui.text_input(
            "tp-fact-atc_codes-input",
            &mut edit.atc,
            "WC160, WI010",
            false,
        );
    } else {
        let tp_type = app.taxpayer().map(|t| t.taxpayer_type);
        // The edit's value as the choice strings compare it (selected() in FactRow).
        let raw = if edit.fact == FactName::IsGovernmentWithholdingEntity {
            edit.value
                .as_bool()
                .map(|b| b.to_string())
                .unwrap_or_default()
        } else {
            edit.value
                .as_str()
                .map(str::to_owned)
                .unwrap_or_else(|| edit.value.to_string())
        };
        for (v, label) in fact_choices(edit.fact, tp_type.as_deref()) {
            // "Graduated rates" also counts a GraduatedOsd/Itemized value as picked.
            let picked = if edit.fact == FactName::IncomeTaxElection && v == "GraduatedUnspecified"
            {
                raw == "GraduatedUnspecified" || raw.starts_with("Graduated")
            } else {
                raw == v
            };
            if ui.list_item_with(
                &label,
                &cui::ListItem {
                    selected: picked,
                    ..Default::default()
                },
            ) {
                let value = if edit.fact == FactName::IsGovernmentWithholdingEntity {
                    Value::Bool(v == "true")
                } else {
                    Value::String(v.clone())
                };
                // Apply directly to the live edit (pickFact).
                FACT_EDIT.with(|e| {
                    if let Some(x) = e.borrow_mut().as_mut() {
                        let next = if x.fact == FactName::IncomeTaxElection
                            && v == "GraduatedUnspecified"
                            && x.value.as_str().is_some_and(|s| s.starts_with("Graduated"))
                        {
                            x.value.clone()
                        } else {
                            value
                        };
                        x.value = next;
                        x.error = None;
                    }
                });
            }
        }
    }
    if let Some(err) = &edit.error {
        ui.danger_text(err);
    }
    ui.row(&Default::default(), |ui| {
        if ui.ghost_button("Cancel", true) {
            cancel_fact_edit();
        }
        if ui.primary_button(&format!("Save for {}", edit.year), true) {
            // Hand the edit back, then save (save_fact_edit takes it).
            FACT_EDIT.with(|e| {
                if e.borrow().is_none() {
                    *e.borrow_mut() = Some(edit.clone());
                }
            });
            save_fact_edit(app);
        }
    });
}

/// The Facts card (FactsBody + FactRow): one row per fact a form on the year's list reads.
fn draw_facts(ui: &mut Ui, app: &mut AppState) {
    let has_year_facts = app.branch().is_some() && app.year().is_some();
    let view = facts_view(app);
    let ready = election_ready(app);
    let shown = view.as_ref().map(shown_facts).unwrap_or_default();
    let flagged = shown
        .iter()
        .filter(|f| {
            view.as_ref()
                .is_some_and(|v| row_flagged(v, ready.as_ref(), **f))
        })
        .count();
    let facts_meta_text = facts::facts_meta(has_year_facts, shown.len(), flagged);
    ui.card(&Default::default(), |ui| {
        if !disclosure_head(
            ui,
            app,
            "facts",
            "Facts your forms use",
            &facts_meta_text,
            None,
        ) {
            return;
        }
        let Some(view) = &view else {
            ui.muted("Add a tax year to see the facts its forms use.");
            return;
        };
        for fact in &shown {
            let f = *fact;
            let _g = ui.push_id(format!("fact-{}", f.wire()));
            let is_editing = FACT_EDIT.with(|e| e.borrow().as_ref().is_some_and(|x| x.fact == f));
            ui.row(&Default::default(), |ui| {
                ui.text(facts::fact_label(f));
                ui.spacer();
                let display = if is_editing {
                    String::new()
                } else {
                    fact_display(view, ready.as_ref(), f)
                };
                ui.muted(if display.is_empty() {
                    "—".to_owned()
                } else {
                    display
                });
                if row_flagged(view, ready.as_ref(), f) && !is_editing {
                    ui.tone_badge("Not set", "warning");
                }
            });
            // "Read by" chips.
            let chips = facts::reader_chips(f, Some(view.readers(f)));
            if !chips.is_empty() {
                ui.row(&Default::default(), |ui| {
                    ui.meta_text("Read by");
                    for c in &chips {
                        ui.tone_badge(c, "muted");
                    }
                });
            }
            if f == FactName::IncomeTaxElection {
                if !is_editing {
                    if let Some(src) = facts::election_source_text(view.election_source.as_deref())
                    {
                        ui.meta_text(&src);
                    }
                }
                if view.election_locked {
                    ui.meta_text(&format!(
                        "A {} return that carries it was filed, so it cannot change for {}.",
                        view.year, view.year
                    ));
                } else if ready
                    .as_ref()
                    .is_some_and(|r| r.gap.as_deref() != Some("ready"))
                {
                    ui.meta_text(&election_why(ready.as_ref()));
                }
            }
            if view.owner_of(f) == Some("head_office")
                && !app.branch().is_some_and(|b| b.is_head_office)
            {
                ui.meta_text("Held by the head office. A change here changes it for every branch.");
            }
            if !is_editing {
                let chooses = f != FactName::IncomeTaxElection
                    || ready
                        .as_ref()
                        .is_some_and(|r| r.gap.as_deref() == Some("ready"));
                let locked = f == FactName::IncomeTaxElection && view.election_locked;
                if chooses && !locked {
                    ui.row(&Default::default(), |ui| {
                        ui.spacer();
                        let has_value =
                            facts::fact_value(f, Some(&view.facts), view.year).is_some();
                        if ui.ghost_button(if has_value { "Change" } else { "Set" }, true) {
                            start_fact_edit(app, f);
                        }
                    });
                }
            }
            if is_editing {
                let taken = FACT_EDIT.with(|e| e.borrow_mut().take());
                if let Some(mut edit) = taken {
                    draw_fact_edit(ui, app, &mut edit);
                    FACT_EDIT.with(|e| {
                        if e.borrow().is_none() {
                            *e.borrow_mut() = Some(edit);
                        }
                    });
                }
            }
        }
        if let Some(hidden) = facts::hidden_facts_text(facts::FactName::ALL.len() - shown.len()) {
            ui.meta_text(&hidden);
        }
    });
}

/// The line under the rate row when the taxpayer does not choose one (why() in FactRow).
fn election_why(ready: Option<&facts::ElectionReadiness>) -> String {
    if ready.is_some_and(|r| r.gap.as_deref() == Some("not_individual")) {
        "Only an individual chooses between 8% and graduated rates.".to_owned()
    } else {
        "Only a self-employed or mixed-income individual chooses between 8% and graduated rates. Set the income type first.".to_owned()
    }
}

/// "Change"/"Choose forms" on the Tax types section: the Forms tab with the checklist open
/// (FormsTab's `go("forms", { editForms: true })`).
pub fn open_forms_edit(app: &mut AppState) {
    let codes = app.year_forms();
    FORMS_EDIT.with(|e| *e.borrow_mut() = Some(codes));
    app.go(ViewName::Forms, Default::default());
}

/// Tax types · your forms for the year (TaxTypesBody): the year's forms with frequency, the tax
/// type chips, Change, and Save as a setup.
fn draw_taxtypes(ui: &mut Ui, app: &mut AppState, br: &Option<Branch>) {
    let year = app.year();
    let forms = app.year_forms();
    let index = store::screen_index();
    let entry = |code: &str| {
        index
            .iter()
            .find(|f| f.c == code)
            .cloned()
            .unwrap_or_else(|| crate::types::ScreenFormIndexEntry {
                c: code.to_owned(),
                t: code.to_owned(),
                ..Default::default()
            })
    };
    let chips = store::tax_types(br.as_ref().map(|b| b.tin.as_str()), year);
    let tt_title = match year {
        Some(y) => format!("Tax types · your forms for {y}"),
        None => "Tax types · your forms".to_owned(),
    };
    let tt_meta = match year {
        Some(_) => tlogic::plural(forms.len(), "form"),
        None => "Add a tax year first".to_owned(),
    };
    ui.card(&Default::default(), |ui| {
        if !disclosure_head(ui, app, "taxtypes", &tt_title, &tt_meta, None) {
            return;
        }
        if year.is_none() {
            ui.muted(&format!(
                "{} has no tax year yet. Add one to choose the forms it files.",
                br.as_ref().map(|b| b.name.as_str()).unwrap_or("")
            ));
            return;
        }
        if forms.is_empty() {
            ui.muted(&format!("No forms for {} yet.", year.unwrap_or(0)));
        }
        for code in &forms {
            let f = entry(code);
            ui.row(&Default::default(), |ui| {
                ui.text(code);
                ui.text(&f.t);
                ui.spacer();
                if !f.freq.is_empty() {
                    ui.tone_badge(&f.freq, "neutral");
                }
            });
        }
        if !chips.is_empty() {
            ui.row(&Default::default(), |ui| {
                for c in &chips {
                    ui.tone_badge(
                        &format!("{} · from {}", c.label, c.forms.join(", ")),
                        "accent",
                    );
                }
            });
        }
        ui.row(&Default::default(), |ui| {
            if ui.ghost_button(
                if forms.is_empty() { "Choose forms" } else { "Change" },
                true,
            ) {
                open_forms_edit(app);
            }
            if !forms.is_empty() && ui.ghost_button("Save as a setup", true) {
                SAVE_SETUP.with(|s| {
                    *s.borrow_mut() = Some((String::new(), None::<String>));
                });
            }
        });
        if let Some((name, error)) = SAVE_SETUP.with(|s| s.borrow().clone()) {
            let mut name_edit = name.clone();
            if ui.text_input("tp-save-setup-name", &mut name_edit, "Setup name", false) {
                SAVE_SETUP.with(|s| *s.borrow_mut() = Some((name_edit, error.clone())));
            }
            if let Some(err) = &error {
                ui.danger_text(err);
            }
            ui.muted(&format!(
                "Keeps the {} forms and the facts they use. Never a name, TIN, address, date or line of business.",
                year.unwrap_or(0)
            ));
            ui.row(&Default::default(), |ui| {
                if ui.ghost_button("Cancel", true) {
                    SAVE_SETUP.with(|s| *s.borrow_mut() = None);
                }
                if ui.primary_button("Save setup", true) {
                    let name_now = SAVE_SETUP.with(|s| s.borrow().clone().map(|(n, _)| n).unwrap_or_default());
                    let name_now = name_now.trim().to_owned();
                    if name_now.is_empty() {
                        SAVE_SETUP.with(|s| {
                            *s.borrow_mut() =
                                Some((name_now, Some("Type a name for the setup.".to_owned())));
                        });
                    } else if let Some(tin) = br.as_ref().map(|b| b.tin.clone())
                        && let Some(y) = year
                    {
                        match store::write(
                            "save_setup",
                            vec![json!(tin), json!(y), json!(name_now)],
                        ) {
                            Ok(_) => {
                                SAVE_SETUP.with(|s| *s.borrow_mut() = None);
                                app.toast(
                                    format!("Saved as the setup {name_now}. Pick it when you add a taxpayer."),
                                    Some("layout-template"),
                                    None,
                                );
                            }
                            Err(e) => {
                                SAVE_SETUP.with(|s| {
                                    *s.borrow_mut() = Some((
                                        name_now.clone(),
                                        Some(crate::lib::errors::error_text(&e)),
                                    ));
                                });
                            }
                        }
                    }
                }
            });
        }
    });
}

thread_local! {
    static SAVE_SETUP: RefCell<Option<(String, Option<String>)>> = const { RefCell::new(None) };
}

/// The inbox credentials being edited (page.ts `InboxEdit`).
#[derive(Clone)]
struct InboxEdit {
    /// "GoogleOAuth" | "AppPassword".
    method: String,
    email: String,
    host: String,
    password: String,
    error: Option<String>,
}

/// One inbox job in flight: which job it is decides the answer text.
enum InboxJob {
    Connect,
    Verify,
    Check,
}

struct InboxPending {
    job: InboxJob,
    rx: std::sync::mpsc::Receiver<Result<Value, String>>,
}

thread_local! {
    static INBOX_EDIT: RefCell<Option<InboxEdit>> = const { RefCell::new(None) };
    static INBOX_PENDING: RefCell<Option<InboxPending>> = const { RefCell::new(None) };
    /// The line under the buttons while a job runs and its answer after ({ok, text}).
    static INBOX_MSG: RefCell<Option<(bool, String)>> = const { RefCell::new(None) };
    /// The branch tin the inbox state belongs to; a switch resets the edit and the line.
    static INBOX_TIN: RefCell<String> = const { RefCell::new(String::new()) };
}

/// `inboxJob`: one job at a time; the "start" line shows while it runs.
fn inbox_job(start: &str, job: InboxJob, name: &str, args: Vec<Value>) {
    let busy = INBOX_PENDING.with(|p| p.borrow().is_some());
    if busy {
        return;
    }
    INBOX_MSG.with(|m| *m.borrow_mut() = Some((true, start.to_owned())));
    let rx = crate::bir::call_async_spawn(name, args);
    INBOX_PENDING.with(|p| {
        *p.borrow_mut() = Some(InboxPending { job, rx });
    });
}

/// Drains a finished inbox job into its message line and toast (actions.ts `inboxJob` tail).
fn inbox_drain(app: &mut AppState) {
    let mut done: Option<(InboxJob, Result<Value, String>)> = None;
    INBOX_PENDING.with(|p| {
        let mut b = p.borrow_mut();
        if let Some(pend) = b.as_mut()
            && let Ok(r) = pend.rx.try_recv()
        {
            let job = match pend.job {
                InboxJob::Connect => InboxJob::Connect,
                InboxJob::Verify => InboxJob::Verify,
                InboxJob::Check => InboxJob::Check,
            };
            done = Some((job, r));
            *b = None;
        }
    });
    let Some((job, result)) = done else {
        return;
    };
    match result {
        Ok(v) => {
            let (ok, text, icon, clears) = match job {
                InboxJob::Connect => (
                    true,
                    "Google account connected. Email tracking is on.".to_owned(),
                    Some("key-round"),
                    true,
                ),
                InboxJob::Verify => {
                    let ok = v.get("ok").and_then(Value::as_bool).unwrap_or(false);
                    let msg = v
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or("The check failed.")
                        .to_owned();
                    (ok, msg, None, false)
                }
                InboxJob::Check => {
                    let ok = v.get("ok").and_then(Value::as_bool).unwrap_or(false);
                    let pending = v
                        .get("still_pending")
                        .and_then(Value::as_bool)
                        .unwrap_or(false);
                    let err = v.get("error").and_then(Value::as_str);
                    (ok, tlogic::check_text(ok, pending, err), None, false)
                }
            };
            // a connect's key-round answer clears the line; every other answer shows it
            INBOX_MSG.with(|m| {
                *m.borrow_mut() = if clears {
                    None
                } else {
                    Some((ok, text.clone()))
                };
            });
            app.toast(text, icon, None);
        }
        Err(e) => {
            let text = crate::lib::errors::error_text(&e);
            INBOX_MSG.with(|m| *m.borrow_mut() = Some((false, text.clone())));
            app.toast(text, Some("triangle-alert"), None);
        }
    }
    store::bump(None);
}

/// The staged method pick or a fresh edit from the stored state (startInboxEdit).
fn inbox_start_edit(st: Option<&crate::types::InboxState>, method: Option<&str>) {
    let m = method
        .map(str::to_owned)
        .or_else(|| {
            st.map(|s| {
                if s.auth_method == "GoogleOAuth" {
                    "GoogleOAuth".to_owned()
                } else {
                    "AppPassword".to_owned()
                }
            })
        })
        .unwrap_or_else(|| "AppPassword".to_owned());
    INBOX_EDIT.with(|e| {
        *e.borrow_mut() = Some(InboxEdit {
            method: m,
            email: st.map(|s| s.imap_login.clone()).unwrap_or_default(),
            host: st
                .map(|s| {
                    if s.imap_host.is_empty() {
                        "imap.gmail.com".to_owned()
                    } else {
                        s.imap_host.clone()
                    }
                })
                .unwrap_or_else(|| "imap.gmail.com".to_owned()),
            password: String::new(),
            error: None,
        });
    });
}

/// The Confirmation inbox section (InboxBody + the page's inbox edit and actions): method pick,
/// credentials, Google connect, Verify, Check now, tracking and the status line.
fn draw_inbox(ui: &mut Ui, app: &mut AppState, br: &Option<Branch>) {
    inbox_drain(app);
    let Some(b) = br else {
        return;
    };
    let tin = b.tin.clone();
    // a branch switch resets the edit and the job line (page.ts `reset`)
    let changed = INBOX_TIN.with(|t| {
        let mut t = t.borrow_mut();
        let same = *t == tin;
        *t = tin.clone();
        !same
    });
    if changed {
        INBOX_EDIT.with(|e| *e.borrow_mut() = None);
        INBOX_MSG.with(|m| *m.borrow_mut() = None);
    }
    let raw = store::inbox_state(Some(&tin));
    let st = if raw.is_null() {
        None
    } else {
        serde_json::from_value::<crate::types::InboxState>(raw).ok()
    };
    let ed = INBOX_EDIT.with(|e| e.borrow().clone());
    let method = ed
        .as_ref()
        .map(|e| e.method.clone())
        .or_else(|| st.as_ref().map(|s| s.auth_method.clone()))
        .filter(|m| !m.is_empty())
        .unwrap_or_else(|| "AppPassword".to_owned());
    let pw = method == "AppPassword";
    let busy = INBOX_PENDING.with(|p| p.borrow().is_some());

    let inbox_meta_text = match &st {
        Some(s) => {
            let input = tlogic::InboxMetaInput {
                tracking_enabled: s.tracking_enabled,
                inbox_email: s.inbox_email.clone(),
                oauth_connected: s.oauth_connected,
                has_app_password: s.has_app_password,
                oauth_inbox_email: s.oauth_inbox_email.clone(),
            };
            tlogic::inbox_meta(Some(&input))
        }
        None => tlogic::inbox_meta(None),
    };
    ui.card(&Default::default(), |ui| {
        if !disclosure_head(ui, app, "inbox", "Confirmation inbox", &inbox_meta_text, None) {
            return;
        }
        ui.muted("Authentication method");
        ui.row(&Default::default(), |ui| {
            for (m, label) in [
                ("GoogleOAuth", "Google account (OAuth2)"),
                ("AppPassword", "App password"),
            ] {
                let active = method == m;
                if ui.ghost_button(label, true) && !active {
                    // pickInboxMethod: stage the method; it applies on the next write
                    match INBOX_EDIT.with(|e| e.borrow_mut().take()) {
                        Some(mut e) => {
                            e.method = m.to_owned();
                            INBOX_EDIT.with(|e2| *e2.borrow_mut() = Some(e));
                        }
                        None => inbox_start_edit(st.as_ref(), Some(m)),
                    }
                }
            }
        });
        if pw {
            if let Some(e) = &ed {
                let mut email = e.email.clone();
                if ui.text_input("tp-inbox-email", &mut email, "Inbox email", false) {
                    INBOX_EDIT.with(|x| {
                        if let Some(x) = x.borrow_mut().as_mut() {
                            x.email = email;
                            x.error = None;
                        }
                    });
                }
                let mut host = e.host.clone();
                if ui.text_input("tp-inbox-host", &mut host, "IMAP host", false) {
                    INBOX_EDIT.with(|x| {
                        if let Some(x) = x.borrow_mut().as_mut() {
                            x.host = host;
                            x.error = None;
                        }
                    });
                }
                let mut pwv = e.password.clone();
                if ui.text_input("tp-inbox-password", &mut pwv, "App password", true) {
                    INBOX_EDIT.with(|x| {
                        if let Some(x) = x.borrow_mut().as_mut() {
                            x.password = pwv;
                            x.error = None;
                        }
                    });
                }
                if let Some(err) = &e.error {
                    ui.danger_text(err);
                }
                ui.muted(
                    "An app password needs 2-Step Verification on the mail account first (Google / Outlook / Yahoo security settings).",
                );
            } else if let Some(s) = &st {
                let login = if !s.imap_login.is_empty() {
                    s.imap_login.clone()
                } else if !s.inbox_email.is_empty() {
                    s.inbox_email.clone()
                } else {
                    "Not set".to_owned()
                };
                ui.muted(&format!("Inbox email: {login}"));
                ui.muted(&format!("IMAP host: {}", s.imap_host));
                ui.muted(&format!(
                    "App password: {}",
                    if s.has_app_password { "Stored" } else { "Not set" }
                ));
            }
        } else {
            let connected = st.as_ref().map(|s| s.oauth_connected).unwrap_or(false);
            let who = st
                .as_ref()
                .and_then(|s| {
                    s.oauth_inbox_email.clone().or_else(|| {
                        if s.inbox_email.is_empty() {
                            None
                        } else {
                            Some(s.inbox_email.clone())
                        }
                    })
                })
                .unwrap_or_default();
            ui.muted(&format!(
                "Google account: {}",
                if connected {
                    format!("Connected as {who}")
                } else {
                    "Not connected".to_owned()
                }
            ));
            ui.muted(
                "Profiles that share this inbox share one Google OAuth credential set. Re-authorize or Disconnect updates every profile using this email.",
            );
        }
        if let Some(s) = &st
            && let Some(at) = &s.last_check_at
        {
            ui.muted(&format!(
                "Last check: {}{}",
                crate::lib::dates::short_date(at),
                s.last_check_status
                    .as_ref()
                    .map(|x| format!(" — {x}"))
                    .unwrap_or_default()
            ));
        }

        // the button row
        ui.row(&Default::default(), |ui| {
            if ed.is_some() {
                if ui.ghost_button("Cancel", true) {
                    INBOX_EDIT.with(|e| *e.borrow_mut() = None);
                }
                if pw && ui.primary_button("Save", true) {
                    let e = INBOX_EDIT.with(|x| x.borrow().clone());
                    if let Some(e) = e {
                        let email = e.email.trim().to_owned();
                        let re = regex::Regex::new(tlogic::EMAIL_RE).unwrap();
                        if !re.is_match(&email) {
                            INBOX_EDIT.with(|x| {
                                if let Some(x) = x.borrow_mut().as_mut() {
                                    x.error =
                                        Some("Type an email like name@example.com.".to_owned());
                                }
                            });
                        } else {
                            let input = json!({
                                "imap_email": email,
                                "imap_host": if e.host.trim().is_empty() { Value::Null } else { json!(e.host.trim()) },
                                "app_password": if e.password.trim().is_empty() { Value::Null } else { json!(e.password.trim()) },
                            });
                            match store::write(
                                "set_inbox_app_password",
                                vec![json!(tin), input],
                            ) {
                                Ok(_) => {
                                    INBOX_EDIT.with(|x| *x.borrow_mut() = None);
                                    app.toast(
                                        "Inbox credentials saved. Verify the connection to start tracking.",
                                        None,
                                        None,
                                    );
                                    store::bump(Some("set_inbox_app_password"));
                                }
                                Err(err) => {
                                    let text = crate::lib::errors::error_text(&err);
                                    INBOX_EDIT.with(|x| {
                                        if let Some(x) = x.borrow_mut().as_mut() {
                                            x.error = Some(text);
                                        }
                                    });
                                }
                            }
                        }
                    }
                }
            } else if pw && ui.ghost_button("Edit credentials", true) {
                inbox_start_edit(st.as_ref(), None);
            }
            if !pw {
                let connected = st.as_ref().map(|s| s.oauth_connected).unwrap_or(false);
                if connected {
                    if ui.primary_button("Re-authorize", !busy) {
                        inbox_job(
                            "Google sign-in opened in your browser…",
                            InboxJob::Connect,
                            "connect_inbox_oauth",
                            vec![json!(tin)],
                        );
                    }
                    if ui.ghost_button("Disconnect", true) {
                        match store::write("disconnect_inbox_oauth", vec![json!(tin)]) {
                            Ok(_) => {
                                app.toast(
                                    "Google account disconnected for every profile sharing this inbox.",
                                    None,
                                    None,
                                );
                            }
                            Err(e) => {
                                app.toast(
                                    crate::lib::errors::error_text(&e),
                                    Some("triangle-alert"),
                                    None,
                                );
                            }
                        }
                    }
                } else if ui.primary_button("Connect Google", !busy) {
                    inbox_job(
                        "Google sign-in opened in your browser…",
                        InboxJob::Connect,
                        "connect_inbox_oauth",
                        vec![json!(tin)],
                    );
                }
            }
            if ui.ghost_button("Verify connection", !busy) {
                inbox_job(
                    "Testing connection…",
                    InboxJob::Verify,
                    "test_inbox_connection",
                    vec![json!(tin)],
                );
            }
            if ui.ghost_button("Check now", !busy) {
                inbox_job(
                    "Checking the inbox for BIR confirmations…",
                    InboxJob::Check,
                    "check_inbox_now",
                    vec![json!(tin)],
                );
            }
        });

        // Receipt tracking switch
        let tracking = st.as_ref().map(|s| s.tracking_enabled).unwrap_or(false);
        let mut on = tracking;
        if ui.switch("Receipt tracking", &mut on) && on != tracking {
            match store::write("set_inbox_tracking", vec![json!(tin), json!(on)]) {
                Ok(_) => {
                    app.toast(
                        if on {
                            "Automated receipt tracking on."
                        } else {
                            "Automated receipt tracking off."
                        },
                        None,
                        None,
                    );
                }
                Err(e) => {
                    app.toast(
                        crate::lib::errors::error_text(&e),
                        Some("triangle-alert"),
                        None,
                    );
                }
            }
            store::bump(Some("set_inbox_tracking"));
        }
        if let Some((ok, text)) = INBOX_MSG.with(|m| m.borrow().clone()) {
            if ok {
                ui.secondary(&text);
            } else {
                ui.danger_text(&text);
            }
        }
        ui.muted(&tlogic::tracking_line(
            tracking,
            st.as_ref().map(|s| s.still_pending).unwrap_or(false),
        ));
    });
}
/// this branch.
fn draw_agent(ui: &mut Ui, app: &mut AppState, br: &Option<Branch>) {
    let Some(b) = br else {
        return;
    };
    let forms = {
        let list = app.year_forms();
        if list.is_empty() {
            vec!["2551Q".to_owned()]
        } else {
            list
        }
    };
    let grants = store::send_grants(Some(&b.tin));
    let granted: Vec<String> = grants.iter().map(|g| g.form.clone()).collect();
    let agent_meta_text = tlogic::agent_meta(&granted);
    ui.card(&Default::default(), |ui| {
        if !disclosure_head(ui, app, "agent", "Agent filing", &agent_meta_text, None) {
            return;
        }
        ui.muted("A standing OK lets an assistant file that form for this branch without asking each time. The Queue button does not need it. You can turn it off here.");
        for code in &forms {
            let mut on = granted.contains(code);
            if ui.switch(&format!("Allow the assistant to file {code}"), &mut on)
                && on != granted.contains(code)
            {
                let result = if on {
                    store::write("grant_send", vec![json!(b.tin), json!(code)])
                } else {
                    store::write("revoke_send", vec![json!(b.tin), json!(code)])
                };
                match result {
                    Ok(_) => {
                        app.toast(
                            if on {
                                format!("Assistant may file {code}.")
                            } else {
                                format!("Assistant may no longer file {code}.")
                            },
                            None,
                            None,
                        );
                    }
                    Err(e) => {
                        app.toast(
                            crate::lib::errors::error_text(&e),
                            Some("triangle-alert"),
                            None,
                        );
                    }
                }
            }
            ui.text(code);
        }
    });
}

/// Archive or delete (RemoveCard): admin only, never the admin root itself (canRemove).
fn draw_remove(ui: &mut Ui, app: &mut AppState, tp: &Taxpayer) {
    if !app.is_admin() || Some(tp.tin_root.as_str()) == app.auth_state().admin_root.as_deref() {
        return;
    }
    let mut armed = DELETE_ARMED.with(|a| *a.borrow());
    ui.card(&Default::default(), |ui| {
        ui.section_title("Archive or delete");
        ui.row(&Default::default(), |ui| {
            ui.column(&Default::default(), |ui| {
                ui.text(if tp.archived {
                    "Archived"
                } else {
                    "Archive this taxpayer"
                });
                ui.muted(if tp.archived {
                    "Kept with its returns, out of new work. Unarchive to file for it again"
                } else {
                    "Keeps every return; it just leaves new work"
                });
            });
            ui.spacer();
            if ui.ghost_button(if tp.archived { "Unarchive" } else { "Archive" }, true) {
                match store::write(
                    "set_taxpayer_archived",
                    vec![json!(tp.tin_root), json!(!tp.archived)],
                ) {
                    Ok(_) => {
                        app.toast(
                            if tp.archived {
                                format!("{} is back in new work.", tp.name)
                            } else {
                                format!("{} archived.", tp.name)
                            },
                            Some("archive"),
                            None,
                        );
                    }
                    Err(e) => {
                        app.toast(
                            crate::lib::errors::error_text(&e),
                            Some("triangle-alert"),
                            None,
                        );
                    }
                }
            }
        });
        ui.row(&Default::default(), |ui| {
            ui.column(&Default::default(), |ui| {
                ui.text("Delete this taxpayer");
                ui.muted(if armed {
                    "Every branch and return goes. This cannot be undone"
                } else {
                    "Removes it and every branch and return"
                });
            });
            ui.spacer();
            if armed && ui.ghost_button("Keep it", true) {
                armed = false;
            }
            if ui.danger_button(if armed { "Delete for good" } else { "Delete" }, true) {
                if !armed {
                    armed = true;
                } else {
                    match store::write("delete_taxpayer", vec![json!(tp.tin_root)]) {
                        Ok(_) => {
                            armed = false;
                            app.toast("Taxpayer deleted.", Some("trash"), None);
                            app.go(ViewName::Home, Default::default());
                        }
                        Err(e) => {
                            app.toast(
                                crate::lib::errors::error_text(&e),
                                Some("triangle-alert"),
                                None,
                            );
                        }
                    }
                }
            }
        });
    });
    DELETE_ARMED.with(|a| *a.borrow_mut() = armed);
}

thread_local! {
    static DELETE_ARMED: RefCell<bool> = const { RefCell::new(false) };
}

thread_local! {
    /// The (tin, branch code) the page's edits belong to; a switch drops every edit in progress
    /// (page.ts `reset`).
    static TP_WATCH: RefCell<(String, String)> = const { RefCell::new((String::new(), String::new())) };
}

pub fn draw_profile(ui: &mut Ui, app: &mut AppState) {
    draw_header(ui, app);
    let Some((tp, br)) = who(app) else {
        ui.muted("Pick a taxpayer first.");
        return;
    };
    let watch = (
        br.as_ref().map(|b| b.tin.clone()).unwrap_or_default(),
        br.as_ref().map(|b| b.code.clone()).unwrap_or_default(),
    );
    let switched = TP_WATCH.with(|w| {
        let mut w = w.borrow_mut();
        let same = *w == watch;
        *w = watch.clone();
        !same
    });
    if switched {
        EDIT.with(|e| *e.borrow_mut() = None);
        cancel_fact_edit();
        SAVE_SETUP.with(|e| *e.borrow_mut() = None);
        DELETE_ARMED.with(|a| *a.borrow_mut() = false);
        INBOX_EDIT.with(|e| *e.borrow_mut() = None);
        INBOX_MSG.with(|m| *m.borrow_mut() = None);
    }
    draw_section_edit(ui, app);
    // ReplaceBanner: shared by the Profile and Forms tabs
    let profile_year = app.year().unwrap_or(0);
    let profile_conflicts = store::annual_conflicts(&app.year_forms());
    draw_banners(ui, app, &profile_conflicts, profile_year);

    // taxpayer section
    ui.card(&Default::default(), |ui| {
        if !disclosure_head(
            ui,
            app,
            "taxpayer",
            "Taxpayer",
            "Shared by every branch",
            Some("taxpayer"),
        ) {
            return;
        }
        fact_row(ui, "Name", &tp.name);
        fact_row(ui, "TIN", &format_tin(&tp.tin_root, None));
        fact_row(ui, "Type", &tp.taxpayer_type);
        if let Some(bd) = &tp.birth_date {
            fact_row(ui, "Birth date", bd);
        }
    });

    // registered activity (ProfileTab's "branch" section: code, name, RDO, the year's line of
    // business and start date)
    if let Some(b) = &br {
        let act_meta = format!(
            "{}{}",
            if b.is_head_office {
                "Head office".to_owned()
            } else {
                format!("Branch {}", b.code)
            },
            app.year().map(|y| format!(" · {y}")).unwrap_or_default()
        );
        let act_title = if b.is_head_office {
            "Registered activity".to_owned()
        } else {
            "Registered activity".to_owned()
        };
        ui.card(&Default::default(), |ui| {
            if !disclosure_head(ui, app, "branch", &act_title, &act_meta, Some("branch")) {
                return;
            }
            if !b.is_head_office {
                fact_row(ui, "Branch code", &b.code);
            }
            fact_row(ui, "Name", &b.name);
            fact_row(ui, "RDO", &b.facts.rdo_code.clone().unwrap_or_default());
            if let Some(view) = year_facts_view(app) {
                let lob = view
                    .facts
                    .get("line_of_business")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let start = view
                    .facts
                    .get("business_start_date")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                fact_row(ui, "Line of business", lob);
                fact_row(ui, "Business start date", start);
            } else {
                ui.muted("Add a tax year to record the line of business and the start date.");
            }
        });
    }

    // tax types · your forms for the year (TaxTypesBody)
    draw_taxtypes(ui, app, &br);

    // address and contact (ProfileTab's "address" section)
    if let Some(b) = &br {
        let inherited: Vec<String> = b.inherited.clone();
        let addr_meta = tlogic::address_meta(
            b.is_head_office,
            tlogic::ADDRESS_FIELDS
                .iter()
                .filter(|(f, _, _)| b.inherited.iter().any(|x| x == f))
                .count(),
        );
        ui.card(&Default::default(), |ui| {
            if !disclosure_head(
                ui,
                app,
                "address",
                "Address and contact",
                &addr_meta,
                Some("address"),
            ) {
                return;
            }
            for (label, field) in [
                ("Registered address", "registered_address"),
                ("ZIP code", "zip_code"),
                ("Phone", "phone"),
                ("Email", "email"),
            ] {
                let val = branch_field(b, field);
                let from_head = !b.is_head_office && inherited.iter().any(|f| f == field);
                ui.row(&Default::default(), |ui| {
                    ui.text(label);
                    ui.spacer();
                    ui.muted(if val.is_empty() {
                        "—".to_owned()
                    } else if from_head {
                        format!("{val} (head office)")
                    } else {
                        val
                    });
                });
            }
        });
    }

    // confirmation inbox + agent filing + archive/delete (ProfileTab's remaining sections)
    draw_inbox(ui, app, &br);
    draw_agent(ui, app, &br);
    draw_remove(ui, app, &tp);

    // tax years + forms summary
    ui.card(&Default::default(), |ui| {
        ui.section_title("Tax years");
        let years = app.years();
        if years.is_empty() {
            ui.muted("No tax years yet.");
        }
        ui.row(&Default::default(), |ui| {
            for y in years.iter().take(8) {
                let active = app.year() == Some(*y);
                if ui.ghost_button(&y.to_string(), true) && !active {
                    leave_year_edit();
                    app.select_year(*y);
                }
            }
            ui.spacer();
            if ui.ghost_button("+ Add year", true) {
                let tin = br.as_ref().map(|b| b.tin.clone()).unwrap_or_default();
                let year = chrono::Local::now().year() as i64 + 1;
                match store::write("add_tax_year", vec![json!(tin), json!(year), json!(true)]) {
                    Ok(_) => {
                        app.toast(
                            format!("Added tax year {year}."),
                            Some("circle-check"),
                            None,
                        );
                        store::bump(Some("add_tax_year"));
                    }
                    Err(e) => {
                        app.toast(
                            crate::lib::errors::error_text(&e),
                            Some("triangle-alert"),
                            None,
                        );
                    }
                }
            }
        });
        let forms = app.year_forms();
        if !forms.is_empty() {
            ui.muted(&format!(
                "{} forms for {}",
                forms.len(),
                app.year().unwrap_or(0)
            ));
        }
    });

    // facts your forms use (Facts.tsx)
    draw_facts(ui, app);

    // branches list
    if tp.branches.len() > 1 {
        ui.card(&Default::default(), |ui| {
            ui.section_title("Branches");
            for b in &tp.branches {
                let _g = ui.push_id(format!("br-{}", b.tin));
                let active = br.as_ref().map(|x| &x.tin) == Some(&b.tin);
                ui.row(&Default::default(), |ui| {
                    ui.text(&format!("{} — {}", b.code, b.name));
                    ui.spacer();
                    if ui.ghost_button("Open", true) && !active {
                        app.open_branch(&tp.tin_root, &b.code);
                    }
                });
            }
            if ui.ghost_button("+ New branch", true) {
                app.go(ViewName::NewBranch, Default::default());
            }
        });
    }

    // archive
    ui.card(&Default::default(), |ui| {
        ui.row(&Default::default(), |ui| {
            ui.text("Archive this taxpayer");
            ui.spacer();
            if ui.danger_button(if tp.archived { "Unarchive" } else { "Archive" }, true) {
                let root = tp.tin_root.clone();
                match store::write(
                    "set_taxpayer_archived",
                    vec![json!(root), json!(!tp.archived)],
                ) {
                    Ok(_) => {
                        store::bump(Some("archive"));
                    }
                    Err(e) => {
                        app.toast(
                            crate::lib::errors::error_text(&e),
                            Some("triangle-alert"),
                            None,
                        );
                    }
                }
            }
        });
    });
}

// ---------------------------------------------------------------- forms tab

thread_local! {
    static FORMS_EDIT: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
    static FORMS_QUERY: RefCell<String> = const { RefCell::new(String::new()) };
    static FORMS_FREQ: RefCell<usize> = const { RefCell::new(0) };
    static FORMS_ADD_YEAR: RefCell<bool> = const { RefCell::new(false) };
    static FORMS_ADD_YEAR_SEL: RefCell<Option<i64>> = const { RefCell::new(None) };
    static FORMS_ADD_YEAR_COPY: RefCell<bool> = RefCell::new(true);
    static REPLACE_ASK: RefCell<Option<crate::lib::taxpayer_logic::ReplaceAsk>> = const { RefCell::new(None) };
}

use crate::lib::taxpayer_logic as tlogic;

/// The toast and the keep-question after a setup or copy, as actions.ts does (applySetup /
/// startFromChoice): the write answers what it kept, the toast says it, the banner asks once.
fn apply_setup_write(
    app: &mut AppState,
    tin: &str,
    year: i64,
    kind: &str,
    id: Option<&str>,
    from: Option<&str>,
    label: &str,
    replace: bool,
) -> bool {
    let result = if kind == "copy" {
        store::write(
            "copy_setup_from",
            vec![
                json!(from.unwrap_or("")),
                json!(tin),
                json!(year),
                json!(replace),
            ],
        )
    } else {
        store::write(
            "apply_setup",
            vec![
                json!(tin),
                json!(year),
                json!(id.unwrap_or("")),
                json!(replace),
            ],
        )
    };
    match result {
        Ok(v) => {
            let applied: crate::types::SetupApplied = serde_json::from_value(v).unwrap_or_default();
            let seed = tlogic::ReplaceAskSeed {
                kind: kind.to_owned(),
                tin: tin.to_owned(),
                year,
                id: id.map(str::to_owned),
                from: from.map(str::to_owned),
                label: label.to_owned(),
            };
            let (toast, ask) = tlogic::after_apply(&applied, &seed);
            REPLACE_ASK.with(|a| *a.borrow_mut() = ask);
            app.toast(toast, Some("layout-template"), None);
            true
        }
        Err(e) => {
            app.toast(
                crate::lib::errors::error_text(&e),
                Some("triangle-alert"),
                None,
            );
            false
        }
    }
}

/// "Replace what is set?" answered Replace (confirmReplace): the same setup or copy again,
/// replacing.
fn confirm_replace(app: &mut AppState) {
    let Some(a) = REPLACE_ASK.with(|r| r.borrow_mut().take()) else {
        return;
    };
    if apply_setup_write(
        app,
        &a.tin,
        a.year,
        &a.kind,
        a.id.as_deref(),
        a.from.as_deref(),
        &a.label,
        true,
    ) {
        app.toast(format!("Replaced with {}.", a.label), None, None);
    }
}

/// The Add year band (AddYearBand): which year, whether to copy the latest year's forms, Add.
fn draw_add_year_band(ui: &mut Ui, app: &mut AppState, tin: &str, years: &[i64]) {
    let bounds = store::year_bounds();
    let allowed = tlogic::allowed_years((bounds.min, bounds.max), years);
    let mut sel = FORMS_ADD_YEAR_SEL.with(|s| *s.borrow());
    if sel.is_none_or(|y| !allowed.contains(&y)) {
        sel = tlogic::default_add_year(&allowed, years, chrono::Local::now().year() as i64);
    }
    if allowed.is_empty() {
        ui.muted(&format!(
            "Every tax year from {} to {} is already added.",
            bounds.min, bounds.max
        ));
        if !years.is_empty() && ui.ghost_button("Cancel", true) {
            FORMS_ADD_YEAR.with(|a| *a.borrow_mut() = false);
        }
        return;
    }
    ui.text("Add tax year");
    // year dropdown
    let labels: Vec<String> = allowed.iter().map(|y| y.to_string()).collect();
    let refs: Vec<&str> = labels.iter().map(String::as_str).collect();
    let mut idx = allowed.iter().position(|y| Some(*y) == sel).unwrap_or(0) as i32;
    if ui.select("tp-add-year-select", &refs, &mut idx) {
        FORMS_ADD_YEAR_SEL.with(|s| *s.borrow_mut() = allowed.get(idx as usize).copied());
    }
    let pick = FORMS_ADD_YEAR_SEL
        .with(|s| *s.borrow())
        .filter(|y| allowed.contains(y))
        .or(sel);
    let last = years.last().copied();
    let last_count = last.map(|y| store::yearly_forms(tin, y).len()).unwrap_or(0);
    let mut copy = FORMS_ADD_YEAR_COPY.with(|c| *c.borrow());
    if last.is_some() {
        let label = format!(
            "Copy {} from {}",
            tlogic::plural(last_count, "form"),
            last.unwrap()
        );
        if ui.checkbox(&label, &mut copy) {
            FORMS_ADD_YEAR_COPY.with(|c| *c.borrow_mut() = copy);
        }
    }
    ui.row(&Default::default(), |ui| {
        ui.spacer();
        if !years.is_empty() && ui.ghost_button("Cancel", true) {
            FORMS_ADD_YEAR.with(|a| *a.borrow_mut() = false);
        }
        if let Some(year) = pick
            && ui.primary_button(&format!("Add {year}"), true)
        {
            let r = store::write(
                "add_tax_year",
                vec![json!(tin), json!(year), json!(!years.is_empty() && copy)],
            );
            match r {
                Ok(v) => {
                    let added: crate::types::AddedTaxYear =
                        serde_json::from_value(v).unwrap_or(crate::types::AddedTaxYear {
                            year,
                            ..Default::default()
                        });
                    leave_year_edit();
                    app.select_year(year);
                    FORMS_ADD_YEAR.with(|a| *a.borrow_mut() = false);
                    FORMS_EDIT.with(|e| *e.borrow_mut() = None);
                    app.toast(
                        tlogic::year_added_text(year, added.copied_from, &added.forms),
                        Some("circle-check"),
                        None,
                    );
                }
                Err(e) => {
                    app.toast(
                        crate::lib::errors::error_text(&e),
                        Some("triangle-alert"),
                        None,
                    );
                }
            }
        }
    });
}

/// The keep-question banner (ReplaceBanner / AnnualConflictBanner).
fn draw_banners(ui: &mut Ui, app: &mut AppState, conflicts: &[Vec<String>], year: i64) {
    if let Some(ask) = REPLACE_ASK.with(|a| a.borrow().clone()) {
        ui.muted(&tlogic::replace_sentence(&ask));
        ui.row(&Default::default(), |ui| {
            if ui.ghost_button("Keep mine", true) {
                REPLACE_ASK.with(|a| *a.borrow_mut() = None);
            }
            if ui.primary_button("Replace", true) {
                confirm_replace(app);
            }
        });
    }
    for group in conflicts {
        ui.muted(&tlogic::annual_conflict_sentence(group, year));
    }
}

/// One form's row: the tick (editing), code, name with its next period, frequency, due date,
/// template (FormRow).
fn draw_form_row(
    ui: &mut Ui,
    app: &AppState,
    f: &crate::types::ScreenFormIndexEntry,
    tin: &str,
    year: i64,
    editing: bool,
    on: bool,
    has_template: bool,
) -> bool {
    let mut toggle = false;
    ui.row(&Default::default(), |ui| {
        if editing {
            let mut v = on;
            if ui.checkbox(&format!("File {} in {year}", f.c), &mut v) {
                toggle = true;
            }
        }
        ui.column(&Default::default(), |ui| {
            ui.row(&Default::default(), |ui| {
                ui.text(&f.c);
                ui.text(&f.t);
            });
            // the schedule line: next open period and its due date
            if on {
                let from = format!("{year}-01-01");
                let to = format!("{}-12-31", year + 1);
                let rows: Vec<tlogic::DeadlineLike> =
                    store::form_deadlines(tin, &f.c, year, &from, &to)
                        .into_iter()
                        .map(|d| tlogic::DeadlineLike {
                            status: d.status,
                            due_date: d.due_date,
                            period_label: d.period_label,
                        })
                        .collect();
                let now = chrono::Local::now();
                let (next, due, none) = tlogic::schedule(
                    &rows,
                    |iso| crate::lib::dates::days_until(iso, now),
                    |iso| crate::lib::dates::short_date(iso),
                );
                let sub = tlogic::form_sub(on, editing, next.as_deref(), none, year);
                ui.muted(&sub);
                if !due.is_empty() {
                    ui.muted(&format!("Due {due}"));
                }
            } else {
                ui.muted(&tlogic::form_sub(on, editing, None, false, year));
            }
        });
        ui.spacer();
        if !f.freq.is_empty() {
            ui.tone_badge(&f.freq, "neutral");
        }
        if on {
            if has_template {
                ui.tone_badge("Template", "success");
            } else {
                ui.tone_badge("No template", "neutral");
            }
        }
    });
    toggle
}

pub fn draw_forms(ui: &mut Ui, app: &mut AppState) {
    let Some((tp, br)) = who(app) else {
        ui.muted("Pick a taxpayer first.");
        return;
    };
    draw_header(ui, app);
    let year = app
        .year()
        .unwrap_or_else(|| chrono::Local::now().year() as i64);
    let Some(br) = br else {
        ui.muted("Pick a branch first.");
        return;
    };
    let tin = br.tin.clone();
    let years = app.years();
    let index = store::screen_index();
    let saved = app.year_forms();
    let editing = FORMS_EDIT.with(|e| e.borrow().is_some());
    let staged: Vec<String> = FORMS_EDIT
        .with(|e| e.borrow().clone())
        .unwrap_or_else(|| saved.clone());
    let templates = app.templates();
    let has_tpl = |code: &str| templates.iter().any(|t| t.form_code == code);

    if years.is_empty() {
        ui.muted(&format!(
            "{} has no tax year yet. Add one to choose the forms it files.",
            br.name
        ));
        let add_open = FORMS_ADD_YEAR.with(|a| *a.borrow());
        if !add_open && ui.ghost_button("Add year", true) {
            FORMS_ADD_YEAR.with(|a| *a.borrow_mut() = true);
        }
        if FORMS_ADD_YEAR.with(|a| *a.borrow()) {
            draw_add_year_band(ui, app, &tin, &years);
        }
        return;
    }

    // the toolbar: Add year / Edit forms or the count + Cancel + Save
    ui.row(&Default::default(), |ui| {
        let add_open = FORMS_ADD_YEAR.with(|a| *a.borrow());
        if !add_open && ui.ghost_button("+ Add year", true) {
            FORMS_ADD_YEAR.with(|a| *a.borrow_mut() = true);
            FORMS_ADD_YEAR_SEL.with(|s| *s.borrow_mut() = None);
        }
        ui.spacer();
        if editing {
            ui.muted(&format!("{} selected", staged.len()));
            if ui.ghost_button("Cancel", true) {
                FORMS_EDIT.with(|e| *e.borrow_mut() = None);
            }
            if ui.primary_button("Save forms", true) {
                let before = app.year_forms();
                match store::write(
                    "set_yearly_forms",
                    vec![json!(tin), json!(year), json!(staged)],
                ) {
                    Ok(_) => {
                        FORMS_EDIT.with(|e| *e.borrow_mut() = None);
                        app.toast(
                            tlogic::forms_saved_text(year, &before, &staged),
                            Some("circle-check"),
                            None,
                        );
                    }
                    Err(e) => {
                        app.toast(
                            crate::lib::errors::error_text(&e),
                            Some("triangle-alert"),
                            None,
                        );
                    }
                }
            }
        } else if ui.primary_button("Edit forms", true) {
            FORMS_EDIT.with(|e| *e.borrow_mut() = Some(saved.clone()));
        }
    });

    if FORMS_ADD_YEAR.with(|a| *a.borrow()) {
        draw_add_year_band(ui, app, &tin, &years);
    }

    let conflicts = store::annual_conflicts(if editing { &staged } else { &saved });
    draw_banners(ui, app, &conflicts, year);

    if !editing {
        // view mode: the year's forms with their schedule line
        let shown: Vec<_> = index.iter().filter(|f| saved.contains(&f.c)).collect();
        if shown.is_empty() {
            ui.muted(&format!("No forms for {year} yet."));
            if ui.ghost_button("Choose forms", true) {
                FORMS_EDIT.with(|e| *e.borrow_mut() = Some(saved.clone()));
            }
        }
        for f in shown {
            draw_form_row(ui, app, f, &tin, year, false, true, has_tpl(&f.c));
        }
        return;
    }

    // edit mode: search + frequency over the whole index, with the setups
    let mut q = FORMS_QUERY.with(|c| c.borrow().clone());
    if ui.text_input(
        "tp-forms-search",
        &mut q,
        "Find a form, like 1601C or VAT",
        false,
    ) {
        FORMS_QUERY.with(|c| *c.borrow_mut() = q.clone());
    }
    let freq_idx = FORMS_FREQ.with(|f| *f.borrow());
    let needle = q.trim().to_lowercase();
    let filtered: Vec<_> = index
        .iter()
        .filter(|f| {
            (freq_idx == 0 || f.freq == FREQS[freq_idx])
                && (needle.is_empty() || tlogic::search_text(&f.c, &f.t).contains(&needle))
        })
        .collect();
    ui.row(&Default::default(), |ui| {
        for (i, f) in FREQS.iter().enumerate() {
            let active = i == freq_idx;
            if ui.ghost_button(f, true) && !active {
                FORMS_FREQ.with(|c| *c.borrow_mut() = i);
            }
        }
    });

    let setups = store::setups(Some(&tp.taxpayer_type));
    if !setups.is_empty() {
        ui.muted("Setups");
        ui.row(&Default::default(), |ui| {
            for setup in &setups {
                let _g = ui.push_id(format!("setup-{}", setup.id));
                if ui.ghost_button(&setup.name, true) {
                    if apply_setup_write(
                        app,
                        &tin,
                        year,
                        "setup",
                        Some(&setup.id),
                        None,
                        &setup.name,
                        false,
                    ) {
                        FORMS_EDIT.with(|e| *e.borrow_mut() = None);
                    }
                }
                if !setup.built_in && ui.ghost_button("Delete", true) {
                    match store::write("delete_setup", vec![json!(setup.id)]) {
                        Ok(_) => {
                            app.toast(
                                format!("Setup {} deleted.", setup.name),
                                Some("trash"),
                                None,
                            );
                        }
                        Err(e) => {
                            app.toast(
                                crate::lib::errors::error_text(&e),
                                Some("triangle-alert"),
                                None,
                            );
                        }
                    }
                }
            }
        });
    }

    if filtered.is_empty() {
        ui.muted("No form matches. Try a code like 1601C or a word like VAT.");
    }
    let mut staged_now = staged.clone();
    for f in &filtered {
        if draw_form_row(
            ui,
            app,
            f,
            &tin,
            year,
            true,
            staged_now.contains(&f.c),
            has_tpl(&f.c),
        ) {
            // the checkbox toggled: flip it in the staged list
            if let Some(pos) = staged_now.iter().position(|c| c == &f.c) {
                staged_now.remove(pos);
            } else {
                staged_now.push(f.c.clone());
            }
            staged_now.sort();
            FORMS_EDIT.with(|e| *e.borrow_mut() = Some(staged_now.clone()));
        }
    }
}

// ---------------------------------------------------------------- templates tab

pub fn draw_templates(ui: &mut Ui, app: &mut AppState) {
    let Some((tp, br)) = who(app) else {
        ui.muted("Pick a taxpayer first.");
        return;
    };
    draw_header(ui, app);
    let Some(br) = br else {
        ui.muted("Pick a branch first.");
        return;
    };
    let year = app
        .year()
        .unwrap_or_else(|| chrono::Local::now().year() as i64);
    let tin = br.tin.clone();
    let index = store::screen_index();
    let saved = app.year_forms();
    let templates = app.templates();
    let tpl_of = |code: &str| templates.iter().find(|t| t.form_code == code);

    ui.muted(&format!(
        "A new return starts with the answers of your last one. A template fixes the answers to start from instead. {} can have one per form, kept across tax years. These are the forms it files in {year}. A template never holds amounts or dates.",
        br.name
    ));

    if app.years().is_empty() {
        ui.muted("Add a tax year first, then choose the forms it files.");
        return;
    }
    let shown: Vec<_> = index.iter().filter(|f| saved.contains(&f.c)).collect();
    if shown.is_empty() {
        ui.muted(&format!(
            "No forms for {year} yet. Choose forms on the Forms tab."
        ));
        return;
    }
    for f in shown {
        let _g = ui.push_id(format!("tpl-{}", f.c));
        let tpl = tpl_of(&f.c);
        ui.row(&Default::default(), |ui| {
            ui.column(&Default::default(), |ui| {
                ui.text(&f.c);
                ui.text(&f.t);
                match tpl {
                    Some(t) => {
                        ui.muted(&format!(
                            "Updated {} · {}",
                            crate::lib::dates::short_date(&t.updated_at),
                            tlogic::template_counts(t)
                        ));
                    }
                    None => ui.muted("No template yet"),
                }
            });
            ui.spacer();
            match tpl {
                Some(_) => {
                    if ui.ghost_button("Update", true) {
                        make_template(app, &tin, &f.c);
                    }
                    if ui.ghost_button("Delete", true) {
                        match store::write("delete_form_template", vec![json!(tin), json!(f.c)]) {
                            Ok(_) => {
                                app.toast(
                                    format!("{} template deleted.", f.c),
                                    Some("trash"),
                                    None,
                                );
                            }
                            Err(e) => {
                                app.toast(
                                    crate::lib::errors::error_text(&e),
                                    Some("triangle-alert"),
                                    None,
                                );
                            }
                        }
                    }
                }
                None => {
                    if ui.primary_button("Create", true) {
                        make_template(app, &tin, &f.c);
                    }
                }
            }
        });
    }
    let _ = tp;
}

/// makeTemplate (actions.ts): the template starts from the newest return of the form.
fn make_template(app: &mut AppState, tin: &str, code: &str) {
    match store::write("create_template_from_latest", vec![json!(tin), json!(code)]) {
        Ok(_) => {
            app.toast(
                format!("{code} template saved from the newest return."),
                Some("layout-template"),
                None,
            );
        }
        Err(e) => {
            app.toast(
                tlogic::template_error_text(code, &crate::lib::errors::error_text(&e)),
                Some("triangle-alert"),
                None,
            );
        }
    }
}

// ---------------------------------------------------------------- new taxpayer

#[derive(Default, Clone)]
struct NewTaxpayerForm {
    tin: String,
    name: String,
    tp_type: usize,
    birth: String,
    /// head office fields
    rdo: String,
    address: String,
    zip: String,
    phone: String,
    email: String,
    error: Option<String>,
    busy: bool,
}

thread_local! {
    static NEW_TP: RefCell<NewTaxpayerForm> = RefCell::new(NewTaxpayerForm::default());
}

fn digits_only(s: &str, n: usize) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).take(n).collect()
}

fn draw_new_form(ui: &mut Ui, app: &mut AppState, _auth: &crate::screens::auth::SharedAuth) {
    let mut f = NEW_TP.with(|c| (*c.borrow()).clone());
    ui.heading("New taxpayer", 1);
    ui.muted("TIN, registered name and head-office address. This decides who the admin is.");
    ui.card(&Default::default(), |ui| {
        ui.section_title("Taxpayer");
        let mut tin = f.tin.clone();
        if ui.text_input("nt-tin", &mut tin, "TIN (9 digits)", false) {
            f.tin = digits_only(&tin, 9);
        }
        let mut name = f.name.clone();
        if ui.text_input("nt-name", &mut name, "Registered name", false) {
            f.name = name;
        }
        let mut idx = f.tp_type as i32;
        let refs: Vec<&str> = TAXPAYER_TYPES.to_vec();
        if ui.select("nt-type", &refs, &mut idx) {
            f.tp_type = idx.max(0) as usize;
        }
        if TAXPAYER_TYPES.get(f.tp_type) == Some(&"Individual") {
            let mut bd = f.birth.clone();
            if ui.text_input("nt-birth", &mut bd, "Birth date (YYYY-MM-DD)", false) {
                f.birth = bd;
            }
        }
    });
    ui.card(&Default::default(), |ui| {
        ui.section_title("Head office");
        let mut v = f.rdo.clone();
        if ui.text_input("nt-rdo", &mut v, "RDO code", false) {
            f.rdo = v;
        }
        let mut v = f.address.clone();
        if ui.text_input("nt-address", &mut v, "Registered address", false) {
            f.address = v;
        }
        let mut v = f.zip.clone();
        if ui.text_input("nt-zip", &mut v, "ZIP code", false) {
            f.zip = v;
        }
        let mut v = f.phone.clone();
        if ui.text_input("nt-phone", &mut v, "Phone", false) {
            f.phone = v;
        }
        let mut v = f.email.clone();
        if ui.text_input("nt-email", &mut v, "Email", false) {
            f.email = v;
        }
    });
    if let Some(e) = &f.error {
        ui.danger_text(e);
    }
    ui.row(&Default::default(), |ui| {
        if ui.primary_button(if f.busy { "Adding…" } else { "Add taxpayer" }, !f.busy) {
            // newTaxpayerProblems: TIN shape and in-use, the name, then the field errors in order
            let mut problems: Vec<String> = Vec::new();
            if f.tin.len() != 9 {
                problems.push(tlogic::TIN_ERROR.to_owned());
            } else if let Some(taken) = store::taxpayers().iter().find(|t| t.tin_root == f.tin) {
                problems.push(format!(
                    "TIN {} is already {}.",
                    tlogic::format_root(&f.tin),
                    taken.name
                ));
            }
            if f.name.trim().is_empty() {
                problems.push("Type the registered name.".to_owned());
            }
            let known_rdo = |code: &str| {
                let list = state::rdo_options();
                list.is_empty() || list.iter().any(|(v, _)| v == code)
            };
            for (field, raw) in [
                ("zip_code", f.zip.as_str()),
                ("rdo_code", f.rdo.as_str()),
                ("email", f.email.as_str()),
            ] {
                if let Some(e) = tlogic::fact_error(field, raw, Some(&known_rdo)) {
                    problems.push(e);
                }
            }
            if let Some(e) = problems.into_iter().next() {
                f.error = Some(e);
            } else {
                f.busy = true;
                let input = json!({
                    "tin_root": f.tin,
                    "name": f.name.trim(),
                    "taxpayer_type": TAXPAYER_TYPES.get(f.tp_type).copied().unwrap_or("Individual"),
                    "birth_date": if f.birth.is_empty() { Value::Null } else { json!(f.birth) },
                    "head_office": {
                        "rdo_code": f.rdo, "registered_address": f.address,
                        "zip_code": f.zip, "phone": f.phone, "email": f.email,
                    },
                });
                match store::write("create_taxpayer", vec![input]) {
                    Ok(v) => {
                        NEW_TP.with(|c| *c.borrow_mut() = NewTaxpayerForm::default());
                        // solo setup completes first-run; helpdesk just selects the new taxpayer
                        if app.needs_setup() {
                            app.setup_pending = None;
                            app.setup_step = "choose".to_owned();
                        }
                        if let Some(root) = v.get("tin_root").and_then(Value::as_str) {
                            app.select_taxpayer(root, false, None);
                        }
                        store::bump(Some("create_taxpayer"));
                        app.go(ViewName::Home, Default::default());
                    }
                    Err(e) => {
                        f.busy = false;
                        f.error = Some(crate::lib::errors::error_text(&e));
                    }
                }
            }
        }
        if ui.ghost_button("Cancel", true) {
            app.back();
        }
    });
    NEW_TP.with(|c| *c.borrow_mut() = f);
}

// ---------------------------------------------------------------- new branch

/// The NewBranchPage draft: suggested code, head-office values with "Same as head office" on, and
/// per-field errors once touched or after a refused save (NewBranch.tsx `Draft`).
#[derive(Default, Clone)]
struct NewBranchForm {
    started: bool,
    code: String,
    name: String,
    /// Values aligned with `tlogic::BRANCH_FIELDS` order.
    values: Vec<String>,
    inherit: Vec<bool>,
    touched: Vec<String>,
    submitted: bool,
    host_errors: std::collections::HashMap<String, String>,
    error: Option<String>,
}

thread_local! {
    static NEW_BR: RefCell<NewBranchForm> = RefCell::new(NewBranchForm::default());
}

/// Every problem with the branch as typed, by field (NewBranch.tsx `problems`).
fn new_branch_problems(f: &NewBranchForm) -> std::collections::HashMap<String, String> {
    let mut e = std::collections::HashMap::new();
    if f.code.len() != 5 || !f.code.chars().all(|c| c.is_ascii_digit()) {
        e.insert("code".to_owned(), tlogic::BRANCH_CODE_ERROR.to_owned());
    }
    if f.name.trim().is_empty() {
        e.insert(
            "name".to_owned(),
            "Type a branch name, like Cebu branch.".to_owned(),
        );
    }
    for (i, (field, _)) in tlogic::BRANCH_FIELDS.iter().enumerate() {
        if f.inherit.get(i).copied().unwrap_or(true) {
            continue;
        }
        let v = f.values.get(i).cloned().unwrap_or_default();
        if let Some(err) = tlogic::fact_error(field, &v, Some(&known_rdo)) {
            e.insert(field.to_string(), err);
        }
    }
    e
}

fn draw_new_branch_form(ui: &mut Ui, app: &mut AppState) {
    let mut f = NEW_BR.with(|c| (*c.borrow()).clone());
    if !f.started {
        f.started = true;
        f.values = vec![String::new(); tlogic::BRANCH_FIELDS.len()];
        f.inherit = vec![false; tlogic::BRANCH_FIELDS.len()];
        if let Some(t) = app.taxpayer() {
            f.code = store::call("suggest_next_branch_code", vec![json!(t.tin_root)])
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
                .unwrap_or_default();
            if let Some(head) = t
                .branches
                .iter()
                .find(|b| b.is_head_office)
                .or_else(|| t.branches.first())
            {
                for (i, (field, _)) in tlogic::BRANCH_FIELDS.iter().enumerate() {
                    f.values[i] = branch_field(head, field);
                    f.inherit[i] = true;
                }
            }
        }
    }
    ui.heading("New branch", 1);
    let tp_name = app.taxpayer().map(|t| t.name.clone()).unwrap_or_default();
    ui.muted(&format!(
        "A branch of {tp_name}. It starts with the head office's details; switch any of them off to set its own."
    ));
    ui.card(&Default::default(), |ui| {
        ui.section_title("Branch");
        let mut code = f.code.clone();
        if ui.text_input("nb-code", &mut code, "Branch code", false) {
            f.code = digits_only(&code, 5);
            f.host_errors.remove("code");
            f.error = None;
        }
        let mut name = f.name.clone();
        if ui.text_input("nb-name", &mut name, "Branch name", false) {
            f.name = name;
            f.error = None;
        }
        // head office facts for the inherit switches
        let head = app.taxpayer().and_then(|t| {
            t.branches
                .iter()
                .find(|b| b.is_head_office)
                .or_else(|| t.branches.first())
                .cloned()
        });
        for (i, (field, label)) in tlogic::BRANCH_FIELDS.iter().enumerate() {
            let mut inherit = f.inherit.get(i).copied().unwrap_or(false);
            let head_val = head
                .as_ref()
                .map(|h| branch_field(h, field))
                .unwrap_or_default();
            let shown = if inherit {
                head_val.clone()
            } else {
                f.values.get(i).cloned().unwrap_or_default()
            };
            if ui.checkbox(&format!("Same as head office: {label}"), &mut inherit)
                && inherit != f.inherit.get(i).copied().unwrap_or(false)
            {
                f.inherit[i] = inherit;
                if inherit {
                    f.values[i] = head_val.clone();
                }
            }
            let mut v = shown.clone();
            if !inherit && ui.text_input(&format!("nb-{field}"), &mut v, label, false) {
                f.values[i] = v;
                f.host_errors.remove(*field);
                f.error = None;
                if !f.touched.iter().any(|t| t == field) {
                    f.touched.push(field.to_string());
                }
            }
        }
    });
    // error lines: shownErrors(problems, touched, submitted, hostErrors)
    let problems = new_branch_problems(&f);
    let mut keys: Vec<String> = problems.keys().cloned().collect();
    keys.sort();
    for key in keys {
        let shown =
            f.submitted || f.touched.iter().any(|t| t == &key) || f.host_errors.contains_key(&key);
        if shown {
            let msg = f
                .host_errors
                .get(&key)
                .cloned()
                .unwrap_or_else(|| problems[&key].clone());
            ui.danger_text(&msg);
        }
    }
    if let Some(e) = &f.error {
        ui.danger_text(e);
    }
    ui.row(&Default::default(), |ui| {
        if ui.primary_button("Add branch", true) {
            f.submitted = true;
            if !new_branch_problems(&f).is_empty() {
                f.error = None;
            } else {
                let root = app
                    .taxpayer()
                    .map(|t| t.tin_root.clone())
                    .unwrap_or_default();
                let mut fields = serde_json::Map::new();
                for (i, (field, _)) in tlogic::BRANCH_FIELDS.iter().enumerate() {
                    let v = if f.inherit.get(i).copied().unwrap_or(true) {
                        Value::Null
                    } else {
                        json!(f.values.get(i).cloned().unwrap_or_default().trim())
                    };
                    fields.insert(field.to_string(), v);
                }
                let input = json!({
                    "branch_code": f.code,
                    "name": f.name.trim(),
                    "fields": Value::Object(fields),
                });
                match store::write("create_branch", vec![json!(root), input]) {
                    Ok(_) => {
                        let name = f.name.trim().to_owned();
                        let code = f.code.clone();
                        NEW_BR.with(|c| *c.borrow_mut() = NewBranchForm::default());
                        app.select_branch(&code);
                        app.go(ViewName::Profile, Default::default());
                        app.toast(format!("{name} added."), Some("circle-check"), None);
                    }
                    Err(e) => {
                        let text = crate::lib::errors::error_text(&e);
                        if text.to_lowercase().contains("branch code") {
                            f.host_errors.insert("code".to_owned(), text);
                        } else {
                            f.error = Some(text);
                        }
                    }
                }
            }
        }
        if ui.ghost_button("Cancel", true) {
            app.back();
        }
    });
    NEW_BR.with(|c| *c.borrow_mut() = f);
}

// ---------------------------------------------------------------- dispatch

/// The taxpayer page for one of its views (profile | forms | templates).
pub fn draw(ui: &mut Ui, app: &mut AppState) {
    match app.view() {
        ViewName::Forms => draw_forms(ui, app),
        ViewName::Templates => draw_templates(ui, app),
        _ => draw_profile(ui, app),
    }
}

/// New taxpayer (also the solo first-run page with no chrome).
pub fn draw_new(ui: &mut Ui, app: &mut AppState, auth: &crate::screens::auth::SharedAuth) {
    draw_new_form(ui, app, auth);
}

/// New branch.
pub fn draw_new_branch(ui: &mut Ui, app: &mut AppState) {
    draw_new_branch_form(ui, app);
}
