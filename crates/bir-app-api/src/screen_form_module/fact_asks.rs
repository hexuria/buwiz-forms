//! The tax facts a return reads (owner decisions 3, 5 and 8).
//!
//! - **Asks.** A Draft (never a sample) asks, at the box that shows it, for
//!   each fact its form reads ([`facts_used_by`]) that the tax profile has
//!   not set for the return's year ([`asks`]). Where the fact goes on the
//!   form is read from the page's schema and the form package ([`place`]):
//!   the box bound to the draft field the fact fills, else the item the
//!   fact decides; with no such box, the top of the first part. A fact with
//!   a writable box of its own gets no card (Q1 P1b): the filer answers it
//!   in the box, and once a complete value is left the page asks once
//!   whether to keep it on the tax profile. The 8% or graduated choice is
//!   never a card: 2551Q's Item 13 asks it on its own, and 1701Q and 1701
//!   choose it on their own rate item.
//! - **Queue.** A fact not set BLOCKS Queue when it changes what this
//!   return files ([`checks`]): a box the form requires and the fact fills
//!   is empty (the government category of 0619E and 0619F counts as empty
//!   until the filer picks it on the return, since the page only guessed
//!   Private), it decides the penalty rates of
//!   a return that computes penalties (late, or penalty amounts entered),
//!   it fills 2550Q's required taxpayer classification, or the year's rate a
//!   2551Q must record needs it (the income type). Otherwise Queue warns
//!   once. The block reads as a normal validation message on the row the
//!   card is on.
//! - **Follow.** The draft fields facts fill follow a later change of the
//!   facts on a Draft, like the profile boxes do ([`fact_fields`]).
//! - **Save to profile.** The profile boxes a return may write back,
//!   each through the function that owns the field ([`profile_field`]), and
//!   the boxes that show a fact ([`answer`]: the box's value as the profile
//!   takes it). A boxed fact's box is one of them too (Q1 P1b): its value
//!   is offered to the profile once, after the filer leaves a complete
//!   answer ([`boxed_boxes`]).
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
    NoteStyle, bound_paths, lifecycle_pointer, penalties, profile_notes, schema_computed,
    schema_entry, schema_keys,
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

/// Whether `placed`'s boxes already answer the fact for this return (Q1
/// P1b): a radio or select pick (one box ticked), or every text or date
/// box of it filled — a split date counts only when all its parts are.
fn box_answered(placed: &Placed, values: &Value) -> bool {
    let shown = |key: &str| values[key].as_str().unwrap_or_default().trim();
    if placed.keys.iter().any(|key| shown(key) == "true") {
        return true;
    }
    !placed.keys.is_empty()
        && placed.keys.iter().all(|key| {
            let value = shown(key);
            !value.is_empty() && value != "false"
        })
}

/// Whether `key` is a box the filer can write on `kind`'s page: bound (or
/// one of the unbound 0619E/F category radios the filer picks), never
/// computed or hidden, and not a box the host writes through its own
/// editors — except the category radios, which the page still lets the
/// filer pick (`answers_on_return`).
fn writable_key(kind: AgentFormKind, key: &str) -> bool {
    let def = kind.definition();
    let pick = answers_on_return(kind, key);
    (def.bindings.contains_key(key) || pick)
        && !schema_computed(kind.form_code(), key)
        && (!agent::host_writes(kind, key) || pick)
        && !def
            .box_def(key)
            .is_some_and(|entry| entry.computed || entry.required == "computed")
}

/// Whether `placed` is a writable box the fact is answered in (Q1 P1b):
/// at least one of its boxes is writable and the fact has a route to the
/// profile through one of them. A computed or derived box (1702RT's
/// incorporation date) and an unbound one (1700's `lobContainer`) do not
/// count, so those facts keep their card.
fn boxed_placed(kind: AgentFormKind, placed: &Placed) -> bool {
    placed.fills
        && placed.keys.iter().any(|key| writable_key(kind, key))
        && placed
            .keys
            .iter()
            .any(|key| profile_field(kind, key).is_some())
}

/// The placement of `fact` when it has a writable box of its own (Q1 P1b),
/// `None` when it stays a card.
pub(super) fn boxed(kind: AgentFormKind, fact: Fact) -> Option<Placed> {
    place(kind, fact).filter(|placed| boxed_placed(kind, placed))
}

fn category_picked(draft: &Value, placed: &Placed) -> bool {
    let picked = super::profile_edits(draft);
    placed.keys.iter().any(|key| picked.contains(key))
}

/// The placed boxes whose opening values are guesses rather than the filer's
/// answer to the withholding-category fact.
pub(super) fn guessed_keys(kind: AgentFormKind, ctx: &FactContext, draft: &Value) -> Vec<String> {
    if !unset(kind, ctx).contains(&Fact::IsGovernmentWithholdingEntity) {
        return Vec::new();
    }
    let Some(placed) = place(kind, Fact::IsGovernmentWithholdingEntity) else {
        return Vec::new();
    };
    if boxed_placed(kind, &placed) && !category_picked(draft, &placed) {
        placed.keys
    } else {
        Vec::new()
    }
}

/// Whether `placed`'s boxes are pick controls (radio, select, checkbox):
/// its message says "pick it" rather than "type it".
fn pick_box(kind: AgentFormKind, placed: &Placed) -> bool {
    let Some(form) = schema_entry(kind.form_code()) else {
        return false;
    };
    let mut picks = 0;
    let mut others = 0;
    for field in form["parts"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|part| part["items"].as_array().into_iter().flatten())
        .flat_map(|item| item["f"].as_array().into_iter().flatten())
    {
        let Some(key) = field["k"].as_str() else {
            continue;
        };
        if !placed.keys.iter().any(|placed_key| placed_key == key) {
            continue;
        }
        match field["t"].as_str() {
            Some("radio") | Some("sel") | Some("chk") => picks += 1,
            _ => others += 1,
        }
    }
    picks > 0 && others == 0
}

/// Every box of `kind` that answers a fact with a writable box of its own
/// (Q1 P1b), with the profile field each writes back through: the
/// "Save to profile" entries of a boxed fact's box.
pub(super) fn boxed_boxes(kind: AgentFormKind) -> Vec<(String, ProfileField)> {
    facts_used_by(kind.form_code())
        .into_iter()
        .filter(|fact| *fact != Fact::IncomeTaxElection)
        .filter_map(|fact| boxed(kind, fact))
        .flat_map(|placed| {
            placed
                .keys
                .into_iter()
                .filter(|key| writable_key(kind, key))
                .filter_map(|key| profile_field(kind, &key).map(|field| (key, field)))
                .collect::<Vec<_>>()
        })
        .collect()
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

/// A choice card's answers: the value `set_year_facts` takes, its label,
/// and a `short` label for the inline chips (F2).
fn choices(fact: Fact) -> Value {
    let pairs: Vec<(Value, &str, &str)> = match fact {
        Fact::TaxClassification => vec![
            (
                json!("SelfEmployed"),
                "Self-employed or professional",
                "Self-employed",
            ),
            (
                json!("MixedIncome"),
                "Mixed income (compensation and business)",
                "Mixed income",
            ),
            (
                json!("PurelyCompensation"),
                "Compensation only",
                "Compensation only",
            ),
        ],
        Fact::EoptTier => vec![
            (
                json!("Micro"),
                "Micro: under ₱3 million gross sales",
                "Micro",
            ),
            (
                json!("Small"),
                "Small: ₱3 million to under ₱20 million",
                "Small",
            ),
            (
                json!("Medium"),
                "Medium: ₱20 million to under ₱1 billion",
                "Medium",
            ),
            (json!("Large"), "Large: ₱1 billion or more", "Large"),
        ],
        Fact::IsGovernmentWithholdingEntity => vec![
            (json!(false), "Private", "Private"),
            (json!(true), "Government office", "Government"),
        ],
        Fact::AtcCodes => vec![
            (
                json!(["II012"]),
                "II012 Business income, graduated rates",
                "II012",
            ),
            (
                json!(["II014"]),
                "II014 Profession, graduated rates",
                "II014",
            ),
            (
                json!(["II013"]),
                "II013 Mixed income, graduated rates",
                "II013",
            ),
            (json!(["II015"]), "II015 Business income, 8%", "II015"),
            (json!(["II017"]), "II017 Profession, 8%", "II017"),
            (json!(["II016"]), "II016 Mixed income, 8%", "II016"),
        ],
        _ => return Value::Null,
    };
    Value::Array(
        pairs
            .into_iter()
            .map(|(value, label, short)| json!({ "value": value, "label": label, "short": short }))
            .collect(),
    )
}

/// The cards a return shows: one per fact not set that has no writable box
/// of its own (Q1 P1b), in [`Fact::ALL`] order. `key` is the box the card
/// sits at (null: the top of the first part).
pub(crate) fn asks(kind: AgentFormKind, ctx: &FactContext) -> Value {
    Value::Array(
        unset(kind, ctx)
            .into_iter()
            .filter(|fact| boxed(kind, *fact).is_none())
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
    /// Every box the fact is answered in (a split date's parts, a radio
    /// group): the form's own message on these keys is the fact's to say.
    pub keys: Vec<String>,
    /// The fact has a writable box of its own (Q1 P1b): its message says
    /// "type it"/"pick it" there instead of "answer it in the card".
    pub boxed: bool,
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
        .filter_map(|fact| {
            let placed = place(kind, fact);
            let boxed = placed.as_ref().is_some_and(|p| boxed_placed(kind, p));
            // A boxed fact the box already answers for this return raises
            // no warning and no block (Q1 P1b). The government category is
            // the exception: the page's Private is a guess, so only the
            // filer's own pick counts (B8).
            let answered = boxed
                && placed.as_ref().is_some_and(|p| {
                    if fact == Fact::IsGovernmentWithholdingEntity {
                        category_picked(draft, p)
                    } else {
                        box_answered(p, values)
                    }
                });
            if answered {
                return None;
            }
            let fills_required = placed
                .as_ref()
                .is_some_and(|placed| placed.fills && placed.required);
            let blocks = match fact {
                Fact::IsGovernmentWithholdingEntity => {
                    fills_required
                        && !placed.as_ref().is_some_and(|placed| category_picked(draft, placed))
                }
                Fact::EoptTier if !boxed => {
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
                _ if boxed => {
                    fills_required
                        && placed
                            .as_ref()
                            .is_some_and(|placed| !box_answered(placed, values))
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
                if boxed {
                    if placed
                        .as_ref()
                        .is_some_and(|placed| pick_box(kind, placed))
                    {
                        format!(
                            "{name} is not in the tax profile, and this return needs it. Pick it {at}."
                        )
                    } else {
                        format!(
                            "{name} is not in the tax profile, and this return needs it. Type it {at}."
                        )
                    }
                } else {
                    match fact {
                        Fact::EoptTier if computes_penalties(kind, draft) => format!(
                            "EOPT tier is not set in the tax profile, and it sets the penalty rates on this return. Answer it {at}, then queue."
                        ),
                        Fact::IsGovernmentWithholdingEntity => format!(
                            "Category of withholding agent is not set in the tax profile, and this return needs it. Pick Private or Government {at}, then queue."
                        ),
                        _ => format!(
                            "{name} is not set in the tax profile, and this return needs it. Answer it {at}, then queue."
                        ),
                    }
                }
            } else {
                match fact {
                    Fact::EoptTier => "EOPT tier is not set. It would set the penalty rates if this return were late.".to_string(),
                    _ => format!("{name} is not set in the tax profile. This return can go without it."),
                }
            };
            let keys = placed
                .as_ref()
                .map(|placed| placed.keys.clone())
                .unwrap_or_default();
            Some(Check {
                fact,
                keys,
                boxed,
                blocks,
                message,
            })
        })
        .collect()
}

/// The blocking checks as the page's validation messages, on the row the
/// card or box is on.
pub(crate) fn block_errors(checks: &[Check]) -> Value {
    Value::Array(
        checks
            .iter()
            .filter(|check| check.blocks)
            .map(|check| {
                json!({
                    "field": format!("tax_profile.{}", check.fact.key()),
                    "message": check.message,
                    "keys": check.keys.iter().collect::<Vec<_>>(),
                })
            })
            .collect(),
    )
}

/// The form's own messages a blocking boxed fact already says (Q1 P1b,
/// B5/B7): an error whose every named box is one the fact is answered in
/// is dropped, so the filer reads one sentence, not two.
pub(crate) fn covered_errors(errors: &mut Value, checks: &[Check]) {
    let covered: BTreeSet<&str> = checks
        .iter()
        .filter(|check| check.blocks && check.boxed)
        .flat_map(|check| check.keys.iter().map(String::as_str))
        .collect();
    if covered.is_empty() {
        return;
    }
    if let Value::Array(list) = errors {
        list.retain(|error| {
            let keys: Vec<&str> = error["keys"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(Value::as_str)
                .collect();
            keys.is_empty() || !keys.iter().all(|key| covered.contains(key))
        });
    }
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

/// Whether a draft field holds nothing.
fn blank(value: &Value) -> bool {
    match value {
        Value::Null => true,
        Value::String(text) => text.trim().is_empty(),
        Value::Array(list) => list.is_empty(),
        _ => false,
    }
}

/// Whether `key` is a box whose pick answers a fact for this return alone:
/// the category radios of 0619E and 0619F (B8), which no binding names and
/// [`place`] finds by their caption. The fill records the pick
/// (`profile_edits`), so the page's opening Private is told from an answer.
pub(super) fn answers_on_return(kind: AgentFormKind, key: &str) -> bool {
    let fact = Fact::IsGovernmentWithholdingEntity;
    facts_used_by(kind.form_code()).contains(&fact)
        && place(kind, fact)
            .is_some_and(|placed| placed.fills && placed.keys.iter().any(|k| k == key))
}

/// The draft fields the filer answered by a pick on this return
/// ([`answers_on_return`]).
fn picked_paths(kind: AgentFormKind, draft: &Value) -> BTreeSet<&'static str> {
    if super::profile_edits(draft)
        .iter()
        .any(|key| answers_on_return(kind, key))
    {
        fills(Fact::IsGovernmentWithholdingEntity)
            .iter()
            .copied()
            .collect()
    } else {
        BTreeSet::new()
    }
}

/// A Draft just answered `facts` from its cards (B6): each draft field
/// those facts fill takes what the profile makes now (`from_profile`) when
/// the return holds nothing there, or still holds what the profile made
/// (its `profile_baseline`; with none, the category the page only guessed).
/// What the filer typed or picked there on this return stays (`kept`: the
/// draft fields of the profile boxes edited on it). No other field moves,
/// and the baseline follows for these fields only.
pub(super) fn take_facts(
    kind: AgentFormKind,
    draft: &mut Value,
    from_profile: &Value,
    facts: &[Fact],
    kept: &BTreeSet<&str>,
) {
    let pointer = lifecycle_pointer(draft, "profile_baseline");
    let mut baseline = draft.pointer(&pointer).and_then(Value::as_object).cloned();
    let picked = picked_paths(kind, draft);
    for fact in facts {
        for path in fills(*fact) {
            let (Some(want), Some(here)) = (from_profile.pointer(path), draft.pointer(path)) else {
                continue;
            };
            let untouched = match baseline.as_ref().and_then(|baseline| baseline.get(*path)) {
                Some(was) => was == here,
                None => *fact == Fact::IsGovernmentWithholdingEntity,
            };
            let free = !kept.contains(path) && !picked.contains(path);
            if free && (blank(here) || untouched) && here != want {
                agent::set_draft_path(draft, path, want.clone());
            }
            if let Some(baseline) = baseline.as_mut() {
                baseline.insert(path.to_string(), want.clone());
            }
        }
    }
    if let Some(baseline) = baseline {
        agent::set_draft_path(draft, &pointer, Value::Object(baseline));
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
    /// A fact the box shows (B9): the birth date, the start date, the EOPT
    /// tier, the income type, the withholding category or the usual ATC.
    Fact(Fact),
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
            Self::Fact(fact) => fact.key(),
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
            Self::Fact(Fact::BirthDate) => "Date of birth",
            Self::Fact(Fact::BusinessStartDate) => "Business start date",
            Self::Fact(Fact::EoptTier) => "EOPT tier",
            Self::Fact(Fact::TaxClassification) => "Income type",
            Self::Fact(Fact::IsGovernmentWithholdingEntity) => "Withholding category",
            Self::Fact(Fact::AtcCodes) => "Usual ATC",
            Self::Fact(Fact::LineOfBusiness) => "Line of business",
            Self::Fact(Fact::IncomeTaxElection) => "Income-tax rate",
        }
    }

    /// The draft field the box writes (a fact's: the first it may fill).
    pub fn path(self) -> &'static str {
        match self {
            Self::Name => "/taxpayer_name",
            Self::Branch(BranchField::RdoCode) => "/rdo_code",
            Self::Branch(BranchField::RegisteredAddress) => "/registered_address",
            Self::Branch(BranchField::ZipCode) => "/zip_code",
            Self::Branch(BranchField::Phone) => "/contact_number",
            Self::Branch(BranchField::Email) => "/email",
            Self::LineOfBusiness => "/line_of_business",
            Self::Fact(fact) => fills(fact).first().copied().unwrap_or_default(),
        }
    }
}

/// The facts a form box saves to the tax profile from the return (B9), the
/// line of business aside (a profile box with its own route).
const FACT_ROUTES: [Fact; 6] = [
    Fact::BirthDate,
    Fact::BusinessStartDate,
    Fact::EoptTier,
    Fact::TaxClassification,
    Fact::IsGovernmentWithholdingEntity,
    Fact::AtcCodes,
];

/// What the tax profile takes for a fact, read from the return's own box.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum Answer {
    Date(NaiveDate),
    Tier(bir_core::profile::EoptTier),
    Classification(bir_core::profile::TaxClassification),
    Government(bool),
    Atc(String),
}

/// A date as a draft keeps it: `MM/DD/YYYY`, `YYYY-MM-DD`, or its parts.
fn draft_date(value: &Value) -> Option<NaiveDate> {
    match value {
        Value::String(text) => {
            let text = text.trim();
            NaiveDate::parse_from_str(text, "%m/%d/%Y")
                .or_else(|_| NaiveDate::parse_from_str(text, "%Y-%m-%d"))
                .ok()
        }
        Value::Object(parts) => NaiveDate::from_ymd_opt(
            i32::try_from(parts.get("year")?.as_i64()?).ok()?,
            u32::try_from(parts.get("month")?.as_i64()?).ok()?,
            u32::try_from(parts.get("day")?.as_i64()?).ok()?,
        ),
        _ => None,
    }
}

/// The draft field of `fact` the return has, with what it holds.
fn held(fact: Fact, draft: &Value) -> Option<(&'static str, &Value)> {
    fills(fact)
        .iter()
        .find_map(|path| Some((*path, draft.pointer(path)?)))
}

/// What the profile takes for `fact` as the return's box has it, or `None`
/// when the box holds nothing or an answer with no profile meaning (1701
/// Item 6's Estate for an individual's income type). The choices are found
/// through the profile's own way onto the form, never a second table:
/// `made(profile)` is the draft a profile makes, and the answer is the one
/// whose draft holds what this return holds. 1701's Item 6 is not made
/// from the income type (the ATC decides it), so it is read in Item 6's own
/// terms: a compensation earner, or a single proprietor or professional
/// whose ATC says whether compensation income comes with it.
pub(super) fn answer(
    kind: AgentFormKind,
    fact: Fact,
    draft: &Value,
    profile: &bir_core::profile::TaxpayerProfile,
    made: &dyn Fn(&bir_core::profile::TaxpayerProfile) -> Option<Value>,
) -> Option<Answer> {
    use bir_core::profile::{EoptTier, TaxClassification};
    let (path, here) = held(fact, draft)?;
    if blank(here) {
        return None;
    }
    let found = |candidates: Vec<Answer>| {
        candidates.into_iter().find(|candidate| {
            let mut profile = profile.clone();
            candidate.clone().put(fact, &mut profile);
            made(&profile).is_some_and(|draft| {
                let made = draft.pointer(path);
                if fact == Fact::AtcCodes {
                    match (made.and_then(Value::as_str), here.as_str()) {
                        (Some(made), Some(here)) => {
                            made.trim().to_uppercase() == here.trim().to_uppercase()
                        }
                        _ => false,
                    }
                } else {
                    made == Some(here)
                }
            })
        })
    };
    match fact {
        Fact::BirthDate | Fact::BusinessStartDate => draft_date(here).map(Answer::Date),
        Fact::EoptTier => found(
            [
                EoptTier::Micro,
                EoptTier::Small,
                EoptTier::Medium,
                EoptTier::Large,
            ]
            .map(Answer::Tier)
            .to_vec(),
        ),
        Fact::IsGovernmentWithholdingEntity => {
            found([false, true].map(Answer::Government).to_vec())
        }
        Fact::AtcCodes => found(vec![Answer::Atc(here.as_str()?.trim().to_uppercase())]),
        Fact::TaxClassification if kind.form_code() == "1701" => {
            let mixed = draft
                .pointer("/atc")
                .and_then(Value::as_str)
                .is_some_and(|atc| matches!(atc.to_uppercase().as_str(), "II013" | "II016"));
            let now = profile.effective_classification();
            Some(Answer::Classification(match here.as_str()? {
                "CompensationEarner" => TaxClassification::PurelyCompensation,
                "SingleProprietor" | "Professional" if mixed => TaxClassification::MixedIncome,
                // No ATC says it: a profile that already holds one of the
                // two keeps it.
                "SingleProprietor" | "Professional" => match now {
                    Some(TaxClassification::MixedIncome) => TaxClassification::MixedIncome,
                    _ => TaxClassification::SelfEmployed,
                },
                _ => return None,
            }))
        }
        Fact::TaxClassification => found(
            [
                TaxClassification::SelfEmployed,
                TaxClassification::MixedIncome,
                TaxClassification::PurelyCompensation,
            ]
            .map(Answer::Classification)
            .to_vec(),
        ),
        Fact::IncomeTaxElection | Fact::LineOfBusiness => None,
    }
}

/// Whether two ATC codes are of one kind (`II012` and `II014`: the same
/// letters before the digits), so a usual ATC replaces only its own kind.
pub(super) fn same_atc_family(a: &str, b: &str) -> bool {
    let letters = |code: &str| -> String {
        code.trim()
            .chars()
            .take_while(char::is_ascii_alphabetic)
            .flat_map(char::to_uppercase)
            .collect()
    };
    letters(a) == letters(b)
}

impl Answer {
    /// The answer on a profile (a copy the search builds a draft from).
    fn put(self, fact: Fact, profile: &mut bir_core::profile::TaxpayerProfile) {
        match self {
            Self::Date(date) if fact == Fact::BirthDate => profile.birth_date = Some(date),
            Self::Date(date) => profile.business_start_date = Some(date),
            Self::Tier(tier) => profile.eopt_tier = Some(tier),
            Self::Classification(class) => profile.tax_classification = Some(class),
            Self::Government(yes) => profile.is_government_withholding_entity = yes,
            Self::Atc(code) => profile.atc_codes = vec![code],
        }
    }
}

pub(super) fn profile_field(kind: AgentFormKind, key: &str) -> Option<ProfileField> {
    let def = kind.definition();
    if let Some(agent::Binding::Text { path, .. }) = def.bindings.get(key).map(|b| &b.binding) {
        match path.as_str() {
            "/taxpayer_name" => return Some(ProfileField::Name),
            "/rdo_code" => return Some(ProfileField::Branch(BranchField::RdoCode)),
            "/registered_address" => {
                return Some(ProfileField::Branch(BranchField::RegisteredAddress));
            }
            "/zip_code" => return Some(ProfileField::Branch(BranchField::ZipCode)),
            "/contact_number" => return Some(ProfileField::Branch(BranchField::Phone)),
            "/email" | "/email_address" => return Some(ProfileField::Branch(BranchField::Email)),
            "/line_of_business" => return Some(ProfileField::LineOfBusiness),
            _ => {}
        }
    }
    fact_field(kind, key).map(ProfileField::Fact)
}

/// The fact a box shows, whatever its binding (a date's parts, a radio, a
/// select, the unbound category radios): one of [`FACT_ROUTES`] the form
/// reads, placed on the box's item by a draft field it fills.
fn fact_field(kind: AgentFormKind, key: &str) -> Option<Fact> {
    let used = facts_used_by(kind.form_code());
    FACT_ROUTES
        .into_iter()
        .filter(|fact| used.contains(fact))
        .find(|fact| {
            place(kind, *fact)
                .is_some_and(|placed| placed.fills && placed.keys.iter().any(|k| k == key))
        })
}

/// The page's "Save to profile" entries (Q1 P1b, Q1a): for each profile
/// box it may open with Edit that has a safe write path, and for each box
/// that answers a fact with a writable box of its own ([`boxed_boxes`]),
/// the field and a note for the row (a branch field this branch takes from
/// the head office says it gets its own). The page asks once per field
/// whether to keep a typed answer on the profile; the host remembers the
/// answer per taxpayer, year and field.
pub(crate) fn profile_save(kind: AgentFormKind, edit_keys: &Value, ctx: &FactContext) -> Value {
    let mut out = serde_json::Map::new();
    let boxed = boxed_boxes(kind);
    let keys = edit_keys
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .chain(boxed.iter().map(|(key, _)| key.as_str()));
    for key in keys {
        if out.contains_key(key) {
            continue;
        }
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
            ProfileField::Fact(Fact::BirthDate) => "Saved for this taxpayer.".to_string(),
            ProfileField::LineOfBusiness | ProfileField::Fact(_) => {
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

/// The "Save to profile?" answers remembered for this return's taxpayer
/// and year (Q1·a): the field ids of the `profile_ask:<tin>:<year>:*`
/// settings, so the page asks each field once. A field of a later tax
/// year asks again. A return with no identity yet has asked nothing; a
/// failed settings read is the caller's error to report.
pub(crate) fn profile_asked(
    db: &Database,
    kind: AgentFormKind,
    draft: &Value,
) -> Result<Vec<String>, bir_core::db::DbError> {
    let Ok(identity) = agent::identity(kind, draft) else {
        return Ok(Vec::new());
    };
    let prefix = format!("profile_ask:{}:{}:", identity.tin, identity.taxable_year);
    Ok(db
        .settings_with_prefix(&prefix)?
        .into_iter()
        .filter_map(|(key, _)| key.strip_prefix(&prefix).map(str::to_string))
        .collect())
}

/// Remember the "Save to profile?" answer for this return's taxpayer,
/// year and field (Q1·a): `profile_ask:<tin>:<year>:<field>` holds `save`
/// or `only`. The return itself is not changed.
pub(crate) fn remember_profile_asked(
    db: &Database,
    tin: &str,
    year: u16,
    field: &str,
    answer: &str,
) -> Result<(), bir_core::db::DbError> {
    db.set_setting(&format!("profile_ask:{tin}:{year}:{field}"), answer)
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
