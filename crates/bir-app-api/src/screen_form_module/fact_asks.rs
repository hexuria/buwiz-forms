//! The tax facts a return reads (owner decisions 3, 5 and 8).
//!
//! - **Asks.** A Draft (never a sample) asks, at the box that shows it, for
//!   each fact its form reads ([`facts_used_by`]) that the tax profile has
//!   not set for the return's year ([`asks`]). Where the fact goes on the
//!   form is read from the page's schema and the form package ([`place`]):
//!   the box bound to the draft field the fact fills, else the item the
//!   fact decides; with no such box, the top of the first part. The 8% or
//!   graduated choice is never a card: 2551Q's Item 13 asks it on its own,
//!   and 1701Q and 1701 choose it on their own rate item.
//! - **Queue.** A fact not set BLOCKS Queue when it changes what this
//!   return files ([`checks`]): a box the form requires and the fact fills
//!   is empty (the government category of 0619E and 0619F counts as empty,
//!   since the page only guessed Private), it decides the penalty rates of
//!   a return that computes penalties (late, or penalty amounts entered),
//!   it fills 2550Q's required taxpayer classification, or the year's rate a
//!   2551Q must record needs it (the income type). Otherwise Queue warns
//!   once. The block reads as a normal validation message on the row the
//!   card is on.
//! - **Follow.** The draft fields facts fill follow a later change of the
//!   facts on a Draft, like the profile boxes do ([`fact_fields`]).
//! - **Save to profile too.** The profile boxes a return may write back,
//!   each through the function that owns the field ([`profile_field`]).
//! - **Filed.** A return past Queued says when the profile changed after it
//!   was filed ([`filed_changes`]); nothing is rewritten.

use std::collections::BTreeSet;

use bir_core::db::Database;
use bir_core::forms::FilingStatus;
use bir_core::forms::agent::{self, AgentFormKind};
use bir_core::profile::TaxpayerType;
use bir_core::tax_profile::{BranchField, Fact, FactOwner, YearFacts, facts_used_by};
use chrono::NaiveDate;
use serde_json::{Value, json};

use super::{
    NoteStyle, bound_paths, lifecycle_pointer, penalties, profile_notes, schema_entry, schema_keys,
};

/// What the asks and the queue checks know about the return's taxpayer and
/// year, read once per open, check or queue.
#[derive(Debug, Clone)]
pub(crate) struct FactContext {
    pub year: u16,
    pub name: String,
    pub individual: bool,
    /// A branch return (not the head office): a head-office fact saved here
    /// is set for all branches.
    pub branch_return: bool,
    pub facts: YearFacts,
    pub birth_date: Option<NaiveDate>,
    /// The branch fields this branch takes from the head office.
    pub inherited: BTreeSet<BranchField>,
}

/// The context of `tin`'s `year`, or `None` when it cannot be read (no
/// profile): then nothing is asked and nothing blocks.
pub(crate) fn context(db: &Database, tin: &str, year: u16) -> Option<FactContext> {
    let profile = db.get_profile_for_year(tin, year).ok()??;
    let effective = db.effective_year_facts(tin, year).ok()?;
    let branch = db.get_branch(tin).ok().flatten();
    let inherited = branch
        .as_ref()
        .map(|branch| {
            branch
                .facts
                .iter()
                .filter(|(_, resolved)| resolved.inherited)
                .map(|(field, _)| *field)
                .collect()
        })
        .unwrap_or_default();
    Some(FactContext {
        year,
        name: profile.full_name.trim().to_string(),
        individual: profile.taxpayer_type == TaxpayerType::Individual,
        branch_return: effective.head_office_tin != tin,
        facts: effective.facts,
        birth_date: profile.birth_date,
        inherited,
    })
}

/// The context of the return `draft` is (its own TIN and taxable year).
pub(crate) fn context_of(db: &Database, kind: AgentFormKind, draft: &Value) -> Option<FactContext> {
    let identity = agent::identity(kind, draft).ok()?;
    context(db, &identity.tin, identity.taxable_year)
}

/// The facts `kind` reads that are not set and that a card asks: the 8%
/// or graduated choice never (its own items ask it); the birth date and the
/// income type only for an individual.
pub(super) fn unset(kind: AgentFormKind, ctx: &FactContext) -> Vec<Fact> {
    facts_used_by(kind.form_code())
        .into_iter()
        .filter(|fact| match fact {
            Fact::IncomeTaxElection => false,
            Fact::BirthDate => ctx.individual && ctx.birth_date.is_none(),
            Fact::TaxClassification => ctx.individual && !ctx.facts.is_set(*fact),
            other => !ctx.facts.is_set(*other),
        })
        .collect()
}

/// The draft fields a fact fills on a return: what follows the fact on a
/// Draft, and what an answer saved from a card brings onto the return.
pub(super) fn fills(fact: Fact) -> &'static [&'static str] {
    match fact {
        Fact::BirthDate => &["/date_of_birth", "/birth_date"],
        Fact::TaxClassification => &["/taxpayer_type"],
        Fact::IncomeTaxElection => &[],
        Fact::EoptTier => &["/taxpayer_classification"],
        Fact::BusinessStartDate => &["/date_of_incorporation", "/incorporation_date"],
        Fact::LineOfBusiness => &["/line_of_business"],
        Fact::IsGovernmentWithholdingEntity => &["/withholding_agent_category"],
        Fact::AtcCodes => &["/atc"],
    }
}

/// Where a card goes besides the boxes the fact fills: the item the fact
/// decides (2551Q's Item 13 for the income type and the start date).
fn decides(fact: Fact) -> &'static [&'static str] {
    match fact {
        Fact::TaxClassification | Fact::BusinessStartDate => &["/item_13_election"],
        _ => &[],
    }
}

/// A printed item's caption word a fact's box has when no binding names it
/// (0619E's and 0619F's category radios, written without a draft field).
fn caption_word(fact: Fact) -> Option<&'static str> {
    match fact {
        Fact::IsGovernmentWithholdingEntity => Some("category"),
        Fact::EoptTier => Some("surcharge"),
        _ => None,
    }
}

/// The draft fields facts fill that follow the facts on a Draft of `kind`
/// (beside its profile boxes, which follow already). 2551Q follows its
/// facts through its own profile sync and is not here; the line of business
/// is a profile box.
pub(super) fn fact_fields(kind: AgentFormKind) -> BTreeSet<&'static str> {
    if kind == AgentFormKind::Form2551Q {
        return BTreeSet::new();
    }
    facts_used_by(kind.form_code())
        .into_iter()
        .filter(|fact| *fact != Fact::LineOfBusiness)
        .flat_map(|fact| fills(fact).iter().copied())
        .collect()
}

/// Where a fact sits on the page: the item, the box a card and a message
/// go on, and whether the form requires it.
#[derive(Debug, Clone)]
pub(super) struct Placed {
    /// The box the card and the message are keyed to.
    pub key: String,
    /// The item's boxes that show the fact (all of the item's when it was
    /// found by its caption).
    pub keys: Vec<String>,
    /// Found by a draft field the fact fills (not by the item it decides).
    pub fills: bool,
    pub required: bool,
    /// "Item 11" or "the top of Part II".
    pub at: String,
}

fn matches_path(path: &str, wanted: &[&str]) -> bool {
    wanted
        .iter()
        .any(|want| path == *want || path.starts_with(&format!("{want}/")))
}

/// The draft paths a page box writes or shows: its binding's, and the ones
/// a derived box's package note names (1702RT's incorporation date).
fn box_paths(kind: AgentFormKind, key: &str) -> Vec<String> {
    let def = kind.definition();
    let mut paths: Vec<String> = bound_paths(def, key)
        .into_iter()
        .map(str::to_string)
        .collect();
    if let Some(note) = def.unmapped.get(key) {
        paths.extend(
            note.split([' ', ',', ':'])
                .filter(|word| word.starts_with('/'))
                .map(str::to_string),
        );
    }
    paths
}

/// Where `fact` sits on `kind`'s page, or `None` (the card then goes at
/// the top of the first part).
pub(super) fn place(kind: AgentFormKind, fact: Fact) -> Option<Placed> {
    let form = schema_entry(kind.form_code())?;
    let def = kind.definition();
    let items: Vec<&'static Value> = form["parts"]
        .as_array()?
        .iter()
        .flat_map(|part| part["items"].as_array().into_iter().flatten())
        .collect();
    let keys_of = |item: &Value| -> Vec<String> {
        item["f"]
            .as_array()
            .into_iter()
            .flatten()
            // The page draws no hidden box.
            .filter(|field| field["t"] != "hidden")
            .filter_map(|field| field["k"].as_str().map(str::to_string))
            .collect()
    };
    let placed = |item: &Value, keys: Vec<String>, fills: bool| {
        let required = keys.iter().any(|key| {
            item["f"]
                .as_array()
                .into_iter()
                .flatten()
                .any(|field| field["k"] == key.as_str() && is_truthy(&field["r"]))
                || def
                    .box_def(key)
                    .is_some_and(|entry| entry.required == "required")
        });
        let at = match item["no"]
            .as_str()
            .map(str::trim)
            .filter(|no| !no.is_empty())
        {
            Some(no) => format!("Item {}", no.trim_start_matches('0')),
            None => format!("\"{}\"", item["l"].as_str().unwrap_or_default().trim()),
        };
        Placed {
            key: keys[0].clone(),
            keys,
            fills,
            required,
            at,
        }
    };
    // By the draft fields the fact fills, then by the item it decides.
    for (wanted, fills) in [(fills(fact), true), (decides(fact), false)] {
        if wanted.is_empty() {
            continue;
        }
        for item in &items {
            let keys: Vec<String> = keys_of(item)
                .into_iter()
                .filter(|key| {
                    box_paths(kind, key)
                        .iter()
                        .any(|path| matches_path(path, wanted))
                })
                .collect();
            if !keys.is_empty() {
                return Some(placed(item, keys, fills));
            }
        }
    }
    // EOPT tier: the penalties the tier sets the rates of.
    if fact == Fact::EoptTier {
        let on_screen = schema_keys(kind.form_code());
        if let Some(key) = penalties::computed_groups(kind)
            .into_iter()
            .flat_map(|group| group.keys)
            .find(|key| on_screen.contains(key))
            && let Some(item) = items.iter().find(|item| keys_of(item).contains(&key))
        {
            return Some(placed(item, vec![key], false));
        }
    }
    let word = caption_word(fact)?;
    items.iter().find_map(|item| {
        let caption = item["l"].as_str().unwrap_or_default().to_lowercase();
        let keys = keys_of(item);
        (caption.contains(word) && !keys.is_empty())
            .then(|| placed(item, keys, fact == Fact::IsGovernmentWithholdingEntity))
    })
}

fn is_truthy(value: &Value) -> bool {
    match value {
        Value::Bool(on) => *on,
        Value::Number(n) => n.as_f64().is_some_and(|n| n != 0.0),
        Value::String(text) => !text.is_empty() && text != "0",
        _ => false,
    }
}

/// Whether the page shows nothing in `placed`'s boxes (`values`: the
/// page's values; a radio counts when ticked).
fn shows_nothing(placed: &Placed, values: &Value) -> bool {
    placed.keys.iter().all(|key| {
        let shown = values[key.as_str()].as_str().unwrap_or_default().trim();
        shown.is_empty() || shown == "false"
    })
}

/// The fact's name in a sentence's subject and in a message.
fn label(fact: Fact, ctx: &FactContext) -> &'static str {
    match fact {
        Fact::BirthDate => "Date of birth",
        Fact::TaxClassification => "Income type",
        Fact::IncomeTaxElection => "Income-tax rate",
        Fact::EoptTier => "EOPT tier",
        Fact::BusinessStartDate if ctx.individual => "Business start date",
        Fact::BusinessStartDate => "Date of incorporation",
        Fact::LineOfBusiness => "Line of business",
        Fact::IsGovernmentWithholdingEntity => "Government withholding agent",
        Fact::AtcCodes => "Usual ATC",
    }
}

/// The card's one sentence: what is asked, and that the answer goes to the
/// tax profile for every later return (for a head-office fact on a branch
/// return, for all branches too).
fn note(fact: Fact, ctx: &FactContext) -> String {
    let name = if ctx.name.is_empty() {
        "This taxpayer".to_string()
    } else {
        ctx.name.clone()
    };
    let year = ctx.year;
    let what = match fact {
        Fact::BirthDate => format!("{name}'s date of birth is"),
        Fact::TaxClassification => format!("{name}'s income type for {year} is"),
        Fact::IncomeTaxElection => format!("{name}'s income-tax rate for {year} is"),
        Fact::EoptTier => format!("{name}'s EOPT tier (taxpayer size) for {year} is"),
        Fact::BusinessStartDate if ctx.individual => format!("{name}'s business start date is"),
        Fact::BusinessStartDate => format!("{name}'s date of incorporation is"),
        Fact::LineOfBusiness => format!("{name}'s line of business for {year} is"),
        Fact::IsGovernmentWithholdingEntity => {
            format!("Whether {name} withholds as a government office in {year} is")
        }
        Fact::AtcCodes => format!("{name}'s usual ATC for {year} is"),
    };
    let all = if ctx.branch_return && fact.owner() == Some(FactOwner::HeadOffice) {
        " and for all branches"
    } else {
        ""
    };
    format!(
        "{what} not in the tax profile; what you save here goes there for every later return{all}."
    )
}

/// The card's control: `date`, `text` or `choice`.
fn control(fact: Fact) -> &'static str {
    match fact {
        Fact::BirthDate | Fact::BusinessStartDate => "date",
        Fact::LineOfBusiness => "text",
        _ => "choice",
    }
}

/// A choice card's answers: the value `set_year_facts` takes, and its label.
fn choices(fact: Fact) -> Value {
    let pairs: Vec<(Value, &str)> = match fact {
        Fact::TaxClassification => vec![
            (json!("SelfEmployed"), "Self-employed or professional"),
            (
                json!("MixedIncome"),
                "Mixed income (compensation and business)",
            ),
            (json!("PurelyCompensation"), "Compensation only"),
        ],
        Fact::EoptTier => vec![
            (json!("Micro"), "Micro: under ₱3 million gross sales"),
            (json!("Small"), "Small: ₱3 million to under ₱20 million"),
            (json!("Medium"), "Medium: ₱20 million to under ₱1 billion"),
            (json!("Large"), "Large: ₱1 billion or more"),
        ],
        Fact::IsGovernmentWithholdingEntity => vec![
            (json!(false), "Private"),
            (json!(true), "Government office"),
        ],
        Fact::AtcCodes => vec![
            (json!(["II012"]), "II012 Business income, graduated rates"),
            (json!(["II014"]), "II014 Profession, graduated rates"),
            (json!(["II013"]), "II013 Mixed income, graduated rates"),
            (json!(["II015"]), "II015 Business income, 8%"),
            (json!(["II017"]), "II017 Profession, 8%"),
            (json!(["II016"]), "II016 Mixed income, 8%"),
        ],
        _ => return Value::Null,
    };
    Value::Array(
        pairs
            .into_iter()
            .map(|(value, label)| json!({ "value": value, "label": label }))
            .collect(),
    )
}

/// The cards a return shows: one per fact not set, in [`Fact::ALL`] order.
/// `key` is the box the card sits at (null: the top of the first part).
pub(crate) fn asks(kind: AgentFormKind, ctx: &FactContext) -> Value {
    Value::Array(
        unset(kind, ctx)
            .into_iter()
            .map(|fact| {
                json!({
                    "fact": fact.key(),
                    "key": place(kind, fact).map(|placed| placed.key),
                    "label": label(fact, ctx),
                    "control": control(fact),
                    "choices": choices(fact),
                    "note": note(fact, ctx),
                })
            })
            .collect(),
    )
}

/// One fact not set, as Queue sees it.
#[derive(Debug, Clone)]
pub(crate) struct Check {
    pub fact: Fact,
    pub key: Option<String>,
    pub blocks: bool,
    pub message: String,
}

/// Whether the return computes penalties: a party late by its computed
/// penalties' days, penalty amounts entered (overrides), or (2551Q, which
/// keeps its own) a penalty amount above zero. `draft` is computed.
pub(super) fn computes_penalties(kind: AgentFormKind, draft: &Value) -> bool {
    fn scan(value: &Value) -> bool {
        match value {
            Value::Object(object) => {
                let late = object
                    .get("computed_penalties")
                    .and_then(|computed| computed["inputs"]["days_late"].as_i64())
                    .is_some_and(|days| days > 0);
                let entered = object.get("penalty_overrides").is_some_and(|overrides| {
                    ["surcharge", "interest", "compromise"]
                        .iter()
                        .any(|component| overrides[*component].is_number())
                });
                late || entered || object.values().any(scan)
            }
            Value::Array(list) => list.iter().any(scan),
            _ => false,
        }
    }
    let own = kind == AgentFormKind::Form2551Q
        && ["surcharge", "interest", "compromise"]
            .iter()
            .any(|component| {
                draft[*component]
                    .as_f64()
                    .is_some_and(|amount| amount > 0.0)
            });
    own || scan(draft)
}

/// Each fact `kind` reads that is not set, blocking or not. `draft` is
/// computed, `values` its page values, `errors` the form's own messages'
/// fields.
pub(crate) fn checks(
    kind: AgentFormKind,
    ctx: &FactContext,
    draft: &Value,
    values: &Value,
    errors: &BTreeSet<String>,
) -> Vec<Check> {
    unset(kind, ctx)
        .into_iter()
        .map(|fact| {
            let placed = place(kind, fact);
            let fills_required = placed
                .as_ref()
                .is_some_and(|placed| placed.fills && placed.required);
            let blocks = match fact {
                // The page's Private is a guess, not an answer.
                Fact::IsGovernmentWithholdingEntity => fills_required,
                Fact::EoptTier => {
                    computes_penalties(kind, draft)
                        || (fills_required
                            && placed
                                .as_ref()
                                .is_some_and(|placed| shows_nothing(placed, values)))
                }
                // The year's rate a 2551Q must record needs the income type.
                Fact::TaxClassification if kind == AgentFormKind::Form2551Q => {
                    errors.contains("annual_income_tax_election")
                }
                _ => {
                    fills_required
                        && placed
                            .as_ref()
                            .is_some_and(|placed| shows_nothing(placed, values))
                }
            };
            let name = label(fact, ctx);
            let at = placed
                .as_ref()
                .map(|placed| format!("at {}", placed.at))
                .unwrap_or_else(|| "at the top of the form".to_string());
            let message = if blocks {
                match fact {
                    Fact::EoptTier if computes_penalties(kind, draft) => format!(
                        "EOPT tier is not set in the tax profile, and it sets the penalty rates on this return. Answer it in the card {at}, then queue."
                    ),
                    _ => format!(
                        "{name} is not set in the tax profile, and this return needs it. Answer it in the card {at}, then queue."
                    ),
                }
            } else {
                match fact {
                    Fact::EoptTier => "EOPT tier is not set. It would set the penalty rates if this return were late.".to_string(),
                    _ => format!("{name} is not set in the tax profile. This return can go without it."),
                }
            };
            Check {
                fact,
                key: placed.map(|placed| placed.key),
                blocks,
                message,
            }
        })
        .collect()
}

/// The blocking checks as the page's validation messages, on the row the
/// card is on.
pub(crate) fn block_errors(checks: &[Check]) -> Value {
    Value::Array(
        checks
            .iter()
            .filter(|check| check.blocks)
            .map(|check| {
                json!({
                    "field": format!("tax_profile.{}", check.fact.key()),
                    "message": check.message,
                    "keys": check.key.iter().collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

/// The fields of the form's own messages (an `errors_json` array).
pub(crate) fn error_fields(errors: &Value) -> BTreeSet<String> {
    errors
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|error| error["field"].as_str().map(str::to_string))
        .collect()
}

/// A Draft just answered `facts` from its cards: the draft fields those
/// facts fill take what the profile makes now (`from_profile`).
pub(super) fn take_facts(draft: &mut Value, from_profile: &Value, facts: &[Fact]) {
    for path in facts.iter().flat_map(|fact| fills(*fact).iter()) {
        if let (Some(want), Some(_)) = (from_profile.pointer(path), draft.pointer(path)) {
            agent::set_draft_path(draft, path, want.clone());
        }
    }
}

/// The profile field a profile box writes back with "Save to profile too":
/// the taxpayer's name, a branch field, or the year's line of business.
/// `None` for a box with no safe write path (a TIN, a split name, an
/// address line, an agent's or a donor's name).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum ProfileField {
    Name,
    Branch(BranchField),
    LineOfBusiness,
}

impl ProfileField {
    pub fn id(self) -> &'static str {
        match self {
            Self::Name => "name",
            Self::Branch(BranchField::RdoCode) => "rdo_code",
            Self::Branch(BranchField::RegisteredAddress) => "registered_address",
            Self::Branch(BranchField::ZipCode) => "zip_code",
            Self::Branch(BranchField::Phone) => "phone",
            Self::Branch(BranchField::Email) => "email",
            Self::LineOfBusiness => "line_of_business",
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Name => "Name",
            Self::Branch(BranchField::RdoCode) => "RDO",
            Self::Branch(BranchField::RegisteredAddress) => "Address",
            Self::Branch(BranchField::ZipCode) => "ZIP code",
            Self::Branch(BranchField::Phone) => "Phone",
            Self::Branch(BranchField::Email) => "Email",
            Self::LineOfBusiness => "Line of business",
        }
    }

    /// The draft field the box writes.
    pub fn path(self) -> &'static str {
        match self {
            Self::Name => "/taxpayer_name",
            Self::Branch(BranchField::RdoCode) => "/rdo_code",
            Self::Branch(BranchField::RegisteredAddress) => "/registered_address",
            Self::Branch(BranchField::ZipCode) => "/zip_code",
            Self::Branch(BranchField::Phone) => "/contact_number",
            Self::Branch(BranchField::Email) => "/email",
            Self::LineOfBusiness => "/line_of_business",
        }
    }
}

pub(super) fn profile_field(kind: AgentFormKind, key: &str) -> Option<ProfileField> {
    let def = kind.definition();
    let bound = def.bindings.get(key)?;
    let agent::Binding::Text { path, .. } = &bound.binding else {
        return None;
    };
    Some(match path.as_str() {
        "/taxpayer_name" => ProfileField::Name,
        "/rdo_code" => ProfileField::Branch(BranchField::RdoCode),
        "/registered_address" => ProfileField::Branch(BranchField::RegisteredAddress),
        "/zip_code" => ProfileField::Branch(BranchField::ZipCode),
        "/contact_number" => ProfileField::Branch(BranchField::Phone),
        "/email" | "/email_address" => ProfileField::Branch(BranchField::Email),
        "/line_of_business" => ProfileField::LineOfBusiness,
        _ => return None,
    })
}

/// The page's "Save to profile too" switches: for each profile box it may
/// open with Edit that has a safe write path, the field and a note for the
/// row (a branch field this branch takes from the head office says it
/// gets its own).
pub(crate) fn profile_save(kind: AgentFormKind, edit_keys: &Value, ctx: &FactContext) -> Value {
    let mut out = serde_json::Map::new();
    for key in edit_keys.as_array().into_iter().flatten() {
        let Some(key) = key.as_str() else {
            continue;
        };
        let Some(field) = profile_field(kind, key) else {
            continue;
        };
        let note = match field {
            ProfileField::Name => "The name is saved for every branch.".to_string(),
            ProfileField::Branch(branch) if ctx.inherited.contains(&branch) => format!(
                "This branch uses the head office's {}. Saved here, the branch gets its own.",
                field.label()
            ),
            ProfileField::Branch(_) => "Saved for this branch.".to_string(),
            ProfileField::LineOfBusiness => {
                format!("Saved to this branch's tax facts for {}.", ctx.year)
            }
        };
        out.insert(
            key.to_string(),
            json!({ "field": field.id(), "label": field.label(), "note": note }),
        );
    }
    Value::Object(out)
}

/// What a return past Queued (Submitted, Confirmed, Paid) shows when the
/// tax profile changed after it was filed: each profile box whose profile
/// value moved since the return was made (its `profile_baseline`, else what
/// it holds, its edits aside), and each fact the profile now sets
/// differently from the return ("EOPT tier"). Empty on any other status.
/// Read only: nothing is rewritten.
pub(super) fn filed_changes(
    kind: AgentFormKind,
    status: &FilingStatus,
    draft: &Value,
    from_profile: &Value,
) -> Vec<String> {
    if !matches!(
        status,
        FilingStatus::Submitted | FilingStatus::Confirmed | FilingStatus::Paid
    ) {
        return Vec::new();
    }
    // What the profile made when the return was made, laid over what it
    // makes now: the boxes compared are only the profile's.
    let mut then = from_profile.clone();
    let baseline = draft
        .pointer(&lifecycle_pointer(draft, "profile_baseline"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let edited = super::edited_profile_paths(kind, draft);
    let paths = super::profile_paths(kind);
    for path in &paths {
        if edited.contains(path) {
            continue;
        }
        let was = baseline.get(*path).or_else(|| draft.pointer(path));
        if let Some(was) = was {
            agent::set_draft_path(&mut then, path, was.clone());
        }
    }
    let mut notes = profile_notes(kind, &then, from_profile, NoteStyle::Filed).unwrap_or_default();
    // Facts the profile sets now that the return holds otherwise.
    let ctx_free_label = |fact: Fact| match fact {
        Fact::BirthDate => "Date of birth",
        Fact::TaxClassification => "Income type",
        Fact::IncomeTaxElection => "Income-tax rate",
        Fact::EoptTier => "EOPT tier",
        Fact::BusinessStartDate => "Business start date",
        Fact::LineOfBusiness => "Line of business",
        Fact::IsGovernmentWithholdingEntity => "Government withholding agent",
        Fact::AtcCodes => "Usual ATC",
    };
    let blank = |value: Option<&Value>| match value {
        None | Some(Value::Null) => true,
        Some(Value::String(text)) => text.trim().is_empty(),
        Some(Value::Array(list)) => list.is_empty(),
        _ => false,
    };
    for fact in facts_used_by(kind.form_code()) {
        let mut pointers: Vec<String> = fills(fact)
            .iter()
            .filter(|path| !paths.contains(**path))
            .map(|path| path.to_string())
            .collect();
        match fact {
            Fact::EoptTier => {
                pointers.extend(pointers_named(from_profile, "penalty_taxpayer_class"));
                pointers.push("/eopt_tier".to_string());
            }
            Fact::IncomeTaxElection => pointers.push("/annual_income_tax_election".to_string()),
            _ => {}
        }
        let moved = pointers.iter().any(|pointer| {
            let now = from_profile.pointer(pointer);
            !blank(now) && draft.pointer(pointer) != now
        });
        let text = ctx_free_label(fact).to_string();
        if moved && !notes.iter().any(|note| note.starts_with(&text)) {
            notes.push(text);
        }
    }
    notes
}

/// Every JSON pointer in `value` to a key named `name`.
fn pointers_named(value: &Value, name: &str) -> Vec<String> {
    fn walk(value: &Value, at: &str, name: &str, out: &mut Vec<String>) {
        if let Value::Object(object) = value {
            for (key, inner) in object {
                let here = format!("{at}/{key}");
                if key == name {
                    out.push(here.clone());
                }
                walk(inner, &here, name, out);
            }
        }
    }
    let mut out = Vec::new();
    walk(value, "", name, &mut out);
    out
}
