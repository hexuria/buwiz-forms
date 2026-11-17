//! Filings (port of `nsh/screens/filings/`): every return the taxpayers have, saved or filed,
//! on a board with one column per status group when the page column is at least 720 points,
//! or as a list under status tabs when narrow. Above it a search box, four multi-select
//! combos (taxpayers, forms, periods, statuses — a null selection means every value), the
//! "Changed in" month chip a Home tile sets, Clear filters and a count; narrow, the four
//! combos sit in a Filters sheet. A card is its return and opens it; the one button a card
//! keeps is Cancel queue. Home's tiles arrive as a preset in the route params, applied once.
//! The pure model lives in `lib::filings_model` (model.ts + filters.ts' preset half).

use std::cell::RefCell;
use std::collections::HashSet;

use cui::{Align, ButtonVariant, Size, Ui};
use serde_json::Value;

use crate::lib::filings_model::{self, Column, FilterValues};
use crate::lib::format::format_tin;
use crate::lib::periods::FormIndexEntry;
use crate::state::{self, AppState, BREAK_NARROW_CONTENT};
use crate::theme;
use crate::types::ReturnRow;
use crate::ui::icons;
use crate::ui::kit;

/// Page width (the main surface less its padding) from which the board shows; under it, the
/// list with tabs (Filings.tsx BOARD_MIN).
const BOARD_MIN: f32 = 720.0;

thread_local! {
    static FILTERS: RefCell<FilterValues> = RefCell::new(FilterValues::default());
    static TAB: RefCell<String> = const { RefCell::new(String::new()) };
    /// The preset's params last applied, so a Back to the same page does not apply it twice.
    static APPLIED_FROM: RefCell<Option<String>> = const { RefCell::new(None) };
    /// The open multi-combo dialog ("taxpayers" | "forms" | "periods" | "statuses").
    static COMBO: RefCell<Option<&'static str>> = const { RefCell::new(None) };
    static COMBO_SEARCH: RefCell<String> = const { RefCell::new(String::new()) };
    /// The narrow Filters sheet.
    static SHEET: RefCell<bool> = const { RefCell::new(false) };
    /// A combo dialog or the Filters sheet is open: custom hit-tested widgets under it must
    /// not take the click (CUI's own controls are modal already; press_hit is not).
    static MODAL: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a filings dialog covers the page this frame (set at the top of `draw`).
fn modal() -> bool {
    MODAL.with(|m| m.get())
}

/// The next session starts from the defaults (filters.reset + tab "all" on session change).
pub fn reset_for_session() {
    FILTERS.with(|f| *f.borrow_mut() = FilterValues::default());
    TAB.with(|t| *t.borrow_mut() = "all".to_owned());
    APPLIED_FROM.with(|a| *a.borrow_mut() = None);
    COMBO.with(|c| *c.borrow_mut() = None);
    COMBO_SEARCH.with(|s| s.borrow_mut().clear());
    SHEET.with(|s| *s.borrow_mut() = false);
}

// ---------------------------------------------------------------- data

/// The returns Filings lists: the filed and saved lists, newest change first (Filings.tsx `all`).
fn all_rows() -> Vec<ReturnRow> {
    let mut v = state::returns("filed");
    v.extend(state::returns("saved"));
    filings_model::newest_first(&v)
}

/// The forms index as Filings reads it (periodKey/returnTitle take FormIndexEntry).
fn form_index() -> Vec<FormIndexEntry> {
    state::screen_index()
        .iter()
        .map(|f| FormIndexEntry {
            c: f.c.clone(),
            freq: crate::lib::periods::Frequency::parse(&f.freq),
        })
        .collect()
}

/// Whether the board shows: the page column (main surface less its padding) is at least
/// BOARD_MIN points (Filings.tsx `wide`).
fn wide(app: &AppState) -> bool {
    let pad = if app.main_width() < BREAK_NARROW_CONTENT {
        16.0
    } else {
        32.0
    };
    (app.main_width() - pad * 2.0).max(200.0) >= BOARD_MIN
}

/// Apply a Home tile's preset once per navigation (filters.applyPreset).
fn apply_preset(app: &AppState) {
    let params = app.params();
    let key = serde_json::to_string(params).unwrap_or_default();
    let Some(preset) = filings_model::preset_of(&Value::Object(params.clone())) else {
        return;
    };
    let already = APPLIED_FROM.with(|a| *a.borrow() == Some(key.clone()));
    if already {
        return;
    }
    APPLIED_FROM.with(|a| *a.borrow_mut() = Some(key));
    FILTERS.with(|f| {
        let (statuses, updated_month) = filings_model::preset_filters(&preset);
        // applyPreset resets every filter, then sets the two the preset names.
        *f.borrow_mut() = FilterValues {
            statuses,
            updated_month,
            ..Default::default()
        };
    });
    TAB.with(|t| *t.borrow_mut() = "all".to_owned());
}

// ---------------------------------------------------------------- multi combos (ui/choice.tsx MultiCombo + logic.ts)

/// The four combos, in bar order.
const KINDS: [&str; 4] = ["taxpayers", "forms", "periods", "statuses"];

/// A combo's words (MultiComboWords): the "all" summary and the plural noun.
fn kind_words(kind: &str) -> (&'static str, &'static str, &'static str) {
    match kind {
        "taxpayers" => ("All taxpayers", "taxpayers", "users"),
        "forms" => ("All forms", "forms", "file-text"),
        "periods" => ("All periods", "periods", "calendar"),
        _ => ("All statuses", "statuses", "circle-check"),
    }
}

/// A combo's dialog title.
fn kind_title(kind: &str) -> &'static str {
    match kind {
        "taxpayers" => "Taxpayers",
        "forms" => "Forms",
        "periods" => "Periods",
        _ => "Statuses",
    }
}

/// What leads a combo option: an avatar, a status dot, or nothing (MultiComboOption.lead).
#[derive(Clone)]
enum Lead {
    None,
    Avatar(String),
    /// A status dot's tone name ("accent" | "neutral" | "success").
    Dot(&'static str),
}

/// One choice of a combo (PickOption + lead).
#[derive(Clone)]
struct ComboOpt {
    value: String,
    label: String,
    sub: String,
    lead: Lead,
}

/// The status dot's tone (FilterCombos' STATUS_TONE: accent, quiet -> neutral, success).
fn status_dot(status: &str) -> &'static str {
    match status {
        "Draft" | "Queued" => "accent",
        "Submitted" => "neutral",
        _ => "success",
    }
}

/// The options of one combo, from every return (not only the ones the filters keep).
fn combo_options(
    kind: &str,
    app: &AppState,
    all: &[ReturnRow],
    index: &[FormIndexEntry],
) -> Vec<ComboOpt> {
    match kind {
        "taxpayers" => app
            .taxpayers()
            .iter()
            .map(|t| ComboOpt {
                value: t.tin_root.clone(),
                label: t.name.clone(),
                sub: if t.tin_display.is_empty() {
                    format_tin(&t.tin_root, None)
                } else {
                    t.tin_display.clone()
                },
                lead: Lead::Avatar(t.name.clone()),
            })
            .collect(),
        "forms" => {
            let titles: std::collections::HashMap<String, String> = state::screen_index()
                .iter()
                .map(|f| (f.c.clone(), f.t.clone()))
                .collect();
            let mut codes: Vec<String> = all.iter().map(|r| r.form_code.clone()).collect();
            codes.sort();
            codes.dedup();
            codes
                .into_iter()
                .map(|c| ComboOpt {
                    sub: titles.get(&c).cloned().unwrap_or_default(),
                    value: c.clone(),
                    label: c,
                    lead: Lead::None,
                })
                .collect()
        }
        "periods" => {
            let mut keys: Vec<String> = all
                .iter()
                .map(|r| filings_model::period_key(index, r))
                .collect();
            keys.sort_by(|a, b| filings_model::period_rank(b).cmp(&filings_model::period_rank(a)));
            keys.dedup();
            keys.into_iter()
                .map(|k| {
                    let n = all
                        .iter()
                        .filter(|r| filings_model::period_key(index, r) == k)
                        .count();
                    ComboOpt {
                        value: k.clone(),
                        label: k,
                        sub: filings_model::plural(n, "return"),
                        lead: Lead::None,
                    }
                })
                .collect()
        }
        _ => filings_model::STATUSES
            .iter()
            .map(|s| {
                let n = all.iter().filter(|r| r.status == *s).count();
                ComboOpt {
                    value: s.to_string(),
                    label: s.to_string(),
                    sub: filings_model::plural(n, "return"),
                    lead: Lead::Dot(status_dot(s)),
                }
            })
            .collect(),
    }
}

/// The selection a kind holds (FilterValues field).
fn get_sel<'a>(f: &'a FilterValues, kind: &str) -> &'a Option<HashSet<String>> {
    match kind {
        "taxpayers" => &f.tps,
        "forms" => &f.forms,
        "periods" => &f.periods,
        _ => &f.statuses,
    }
}

/// Write a kind's selection back.
fn put_sel(f: &mut FilterValues, kind: &str, v: Option<HashSet<String>>) {
    match kind {
        "taxpayers" => f.tps = v,
        "forms" => f.forms = v,
        "periods" => f.periods = v,
        _ => f.statuses = v,
    }
}

/// Every option's value (the "all" set a null selection stands for).
fn all_values(opts: &[ComboOpt]) -> HashSet<String> {
    opts.iter().map(|o| o.value.clone()).collect()
}

/// The values on: the selection, or every option when it is null.
fn on_values<'a>(sel: &Option<HashSet<String>>, opts: &'a [ComboOpt]) -> Vec<&'a ComboOpt> {
    match sel {
        None => opts.iter().collect(),
        Some(s) => opts.iter().filter(|o| s.contains(&o.value)).collect(),
    }
}

/// Whether the selection narrows the list (on() count differs from every option).
fn narrowed(sel: &Option<HashSet<String>>, opts: &[ComboOpt]) -> bool {
    on_values(sel, opts).len() != opts.len()
}

/// A selection handed to the page: null once it holds every option, so values that appear
/// later are in it too (normalizeSelection).
fn normalize(next: HashSet<String>, opts: &[ComboOpt]) -> Option<HashSet<String>> {
    if opts.iter().all(|o| next.contains(&o.value)) {
        None
    } else {
        Some(next)
    }
}

/// The values currently on, as a mutable set (current()).
fn current_set(sel: &Option<HashSet<String>>, opts: &[ComboOpt]) -> HashSet<String> {
    match sel {
        None => all_values(opts),
        Some(s) => s.clone(),
    }
}

/// Toggle one value and store the normalized selection (toggle + choose).
fn toggle_value(f: &mut FilterValues, kind: &str, value: &str, opts: &[ComboOpt]) {
    let mut next = current_set(get_sel(f, kind), opts);
    if !next.remove(value) {
        next.insert(value.to_owned());
    }
    let v = normalize(next, opts);
    put_sel(f, kind, v);
}

/// Select all / Clear scoped to the options the search shows (selectAll / clear).
fn bulk_value(f: &mut FilterValues, kind: &str, opts: &[ComboOpt], found: &[&ComboOpt], add: bool) {
    let mut next = current_set(get_sel(f, kind), opts);
    for o in found {
        if add {
            next.insert(o.value.clone());
        } else {
            next.remove(&o.value);
        }
    }
    let v = normalize(next, opts);
    put_sel(f, kind, v);
}

/// Options whose label and second line hold every typed word (matchOptions).
fn match_options<'a>(opts: &'a [ComboOpt], text: &str) -> Vec<&'a ComboOpt> {
    let words: Vec<String> = text
        .trim()
        .to_lowercase()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    opts.iter()
        .filter(|o| {
            let hay = format!("{} {}", o.label, o.sub).to_lowercase();
            words.iter().all(|w| hay.contains(w.as_str()))
        })
        .collect()
}

/// The closed trigger's summary (multiComboLabel): "All taxpayers", the one label,
/// "3 taxpayers" or "No taxpayers".
fn combo_label(kind: &str, opts: &[ComboOpt], sel: &Option<HashSet<String>>) -> String {
    let (all_word, nouns, _) = kind_words(kind);
    let on = on_values(sel, opts);
    if on.len() == opts.len() {
        all_word.to_owned()
    } else if on.is_empty() {
        format!("No {nouns}")
    } else if on.len() == 1 {
        on[0].label.clone()
    } else {
        format!("{} {nouns}", on.len())
    }
}

/// How many of the four filters and the month are narrowed — the Filters button's badge;
/// the search is visible and not counted (filters.narrowed).
fn narrowed_count(f: &FilterValues) -> usize {
    [&f.tps, &f.forms, &f.periods, &f.statuses]
        .iter()
        .filter(|s| s.is_some())
        .count()
        + usize::from(f.updated_month.is_some())
}

/// True when the search or any filter holds the list back — shows Clear filters (filters.active).
fn filters_active(f: &FilterValues) -> bool {
    f.query.trim() != "" || narrowed_count(f) > 0
}

// ---------------------------------------------------------------- combo drawing

/// Draw a lead element: an avatar, a status dot, or nothing.
fn draw_lead(ui: &mut Ui, lead: &Lead) {
    let p = kit::frame_palette();
    match lead {
        Lead::None => {}
        Lead::Avatar(name) => {
            ui.avatar_with(
                name,
                &cui::Avatar {
                    size: 20.0,
                    ..Default::default()
                },
            );
        }
        Lead::Dot(tone) => {
            ui.box_(
                &cui::BoxOpts {
                    width: Size::px(8.0),
                    height: Size::px(8.0),
                    bg: kit::tone_color(tone),
                    radius: theme::radius::FULL,
                    ..Default::default()
                },
                |_| {},
            );
        }
    }
    let _ = p;
}

/// A check box for a combo row: a 14pt rounded square, accent with a check when on.
fn draw_check(ui: &mut Ui, on: bool) {
    let p = kit::frame_palette();
    ui.box_(
        &cui::BoxOpts {
            width: Size::px(14.0),
            height: Size::px(14.0),
            border: 1.0,
            border_color: if on { p.accent_blue } else { p.border_strong },
            bg: if on { p.accent_blue } else { p.card },
            radius: theme::radius::CTL,
            align: Align::Center,
            justify: cui::Justify::Center,
            ..Default::default()
        },
        |ui| {
            if on {
                ui.icon(icons::glyph("check"), 10.0, p.primary_foreground);
            }
        },
    );
}

/// The multi-combo trigger: a bordered h-8 row — the kind's icon (or the one selected
/// option's lead), the summary, a chevron. Press opens the combo dialog (MultiCombo trigger).
fn combo_trigger(
    ui: &mut Ui,
    id: &str,
    kind: &str,
    label: &str,
    lead: Option<&Lead>,
    is_narrowed: bool,
    is_open: bool,
) -> bool {
    let p = kit::frame_palette();
    let wid = ui.make_id(id);
    let hovered = kit::press_hovered(id, ui.mouse_pos().x, ui.mouse_pos().y);
    ui.box_(
        &cui::BoxOpts {
            id: Some(id),
            dir: cui::Dir::Row,
            align: Align::Center,
            gap: theme::space(2),
            pad_x: 10.0,
            min_height: theme::size::CONTROL,
            max_width: 220.0,
            bg: if hovered { p.hover } else { p.card },
            border: 1.0,
            border_color: if is_narrowed || is_open {
                p.subtle_foreground
            } else {
                p.border_strong
            },
            radius: theme::radius::ROW,
            ..Default::default()
        },
        |ui| {
            match lead {
                Some(l @ (Lead::Avatar(_) | Lead::Dot(_))) => draw_lead(ui, l),
                _ => ui.icon(
                    icons::glyph(kind_words(kind).2),
                    theme::size::ICON,
                    p.foreground,
                ),
            }
            ui.text_with(
                label,
                &cui::Text {
                    size: theme::r#type::BODY.0,
                    ..Default::default()
                },
            );
            ui.icon(
                icons::glyph("chevron-down"),
                theme::size::ICON_SMALL,
                p.subtle_foreground,
            );
        },
    );
    let rect = ui.get_rect(wid);
    kit::press_remember(id, rect);
    let mouse = ui.mouse_pos();
    ui.mouse_clicked(cui::MouseButton::Left) && kit::press_hit(rect, mouse.x, mouse.y)
}

/// One option row of the combo dialog: check, lead, label over sub. Press toggles.
fn combo_row(ui: &mut Ui, kind: &str, opt: &ComboOpt, checked: bool) -> bool {
    kit::pressable_row(ui, &format!("combo-{kind}-{}", opt.value), |ui| {
        draw_check(ui, checked);
        draw_lead(ui, &opt.lead);
        ui.column(
            &cui::Column {
                width: Size::FILL,
                ..Default::default()
            },
            |ui| {
                ui.text(&opt.label);
                if !opt.sub.is_empty() {
                    ui.muted(&opt.sub);
                }
            },
        );
    })
}

/// The combo popover as a dialog: search, Select all, Clear, the option rows (MultiCombo).
fn draw_combo_dialog(ui: &mut Ui, app: &AppState, all: &[ReturnRow]) {
    let Some(kind) = COMBO.with(|c| c.borrow().clone()) else {
        return;
    };
    let index = form_index();
    let opts = combo_options(&kind, app, all, &index);
    let mut search = COMBO_SEARCH.with(|s| s.borrow().clone());
    let found = match_options(&opts, &search);
    let mut open = true;
    let mut done = false;
    ui.dialog_with(
        kind_title(&kind),
        &mut open,
        &cui::Dialog {
            width: 360.0,
            ..Default::default()
        },
        |ui| {
            // Search takes the whole first row (MultiCombo's flex-1 field).
            let mut q = search.clone();
            if q.capacity() < 256 {
                q.reserve(256);
            }
            if ui
                .input_with(
                    "combo-search",
                    &mut q,
                    &cui::Input {
                        placeholder: Some(match kind {
                            "taxpayers" => "Find a taxpayer",
                            "forms" => "Find a form",
                            "periods" => "Find a period",
                            _ => "Find a status",
                        }),
                        icon: Some(icons::glyph("search")),
                        ..Default::default()
                    },
                )
                .changed()
            {
                search = q;
            }
            // Select all / Clear on their own line under the search.
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    gap: theme::space(2),
                    align: Align::Center,
                    ..Default::default()
                },
                |ui| {
                    if kit::small_button(ui, "Select all", ButtonVariant::Ghost, None) {
                        FILTERS
                            .with(|f| bulk_value(&mut f.borrow_mut(), &kind, &opts, &found, true));
                    }
                    if kit::small_button(ui, "Clear", ButtonVariant::Ghost, None) {
                        FILTERS
                            .with(|f| bulk_value(&mut f.borrow_mut(), &kind, &opts, &found, false));
                    }
                },
            );
            ui.separator();
            ui.scroll(
                &cui::Scroll {
                    id: Some("combo-list"),
                    height: Size::px(300.0),
                    ..Default::default()
                },
                |ui| {
                    if found.is_empty() {
                        ui.muted("Nothing matches.");
                    }
                    for opt in &found {
                        let checked = FILTERS.with(|f| {
                            let f = f.borrow();
                            match get_sel(&f, &kind) {
                                None => true,
                                Some(s) => s.contains(&opt.value),
                            }
                        });
                        if combo_row(ui, &kind, opt, checked) {
                            FILTERS.with(|f| {
                                toggle_value(&mut f.borrow_mut(), &kind, &opt.value, &opts)
                            });
                        }
                    }
                },
            );
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    ..Default::default()
                },
                |ui| {
                    ui.spacer();
                    if kit::primary_button(ui, "Done", true) {
                        done = true;
                    }
                },
            );
        },
    );
    COMBO_SEARCH.with(|s| *s.borrow_mut() = search);
    if done || !open {
        COMBO.with(|c| *c.borrow_mut() = None);
        COMBO_SEARCH.with(|s| s.borrow_mut().clear());
    }
}

/// `found` borrows `opts`; bulk_value only reads `value`, so the refs pass straight through.

// ---------------------------------------------------------------- return card

fn open_return(app: &mut AppState, r: &ReturnRow) {
    app.open_return(state::OpenReturnOptions {
        tin: r.tin.clone(),
        code: r.form_code.clone(),
        year: r.taxable_year,
        period: r.period,
        return_key: r.return_key.map(serde_json::Value::from),
        mode: None,
        snapshot_id: None,
        new_key: None,
        new_key_from: None,
    });
}

/// Cancel queue on a queued return (Filings.tsx `cancel`).
fn cancel_queue(app: &mut AppState, r: &ReturnRow) {
    state::cancel_queue(
        app,
        &r.tin,
        &r.form_code,
        r.taxable_year,
        (r.period != 0).then_some(r.period),
    );
}

/// One return as a card (ReturnCard.tsx): the emph title, the status chip only where the
/// column or tab does not already say it (Paid under Confirmed, every card on All), the
/// taxpayer, the branch when it is not head office, the date and Cancel queue.
fn draw_card(
    ui: &mut Ui,
    app: &mut AppState,
    index: &[FormIndexEntry],
    r: &ReturnRow,
    column: Option<&str>,
) {
    let _g = ui.push_id(format!("card-{}", r.id));
    let pressed = ui.card(
        &cui::Card {
            pad: theme::space(2),
            gap: theme::space(1),
            hoverable: true,
            ..Default::default()
        },
        |ui| {
            // head: title + status chip when column title != status
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    gap: theme::space(1) + 2.0,
                    align: Align::Center,
                    ..Default::default()
                },
                |ui| {
                    ui.text_with(
                        &filings_model::return_title(index, r),
                        &cui::Text {
                            size: theme::r#type::EMPH.0,
                            font: cui::font::MEDIUM,
                            ..Default::default()
                        },
                    );
                    ui.spacer();
                    if column != Some(r.status.as_str()) {
                        let chip = crate::ui::logic::status_chip(&r.status.to_lowercase(), None);
                        kit::draw_chip(ui, &chip);
                    }
                },
            );
            ui.text_with(
                &r.taxpayer_name,
                &cui::Text {
                    size: theme::r#type::META.0,
                    color: kit::frame_palette().muted_foreground,
                    ..Default::default()
                },
            );
            let branch = filings_model::named_branch(r);
            if !branch.is_empty() {
                ui.text_with(
                    &branch,
                    &cui::Text {
                        size: theme::r#type::META.0,
                        color: kit::frame_palette().subtle_foreground,
                        ..Default::default()
                    },
                );
            }
            // meta: date + Cancel queue
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    gap: theme::space(1) + 2.0,
                    align: Align::Center,
                    ..Default::default()
                },
                |ui| {
                    ui.text_with(
                        &filings_model::date_line(r),
                        &cui::Text {
                            size: theme::r#type::CAP.0,
                            color: kit::frame_palette().subtle_foreground,
                            ..Default::default()
                        },
                    );
                    ui.spacer();
                    if r.status == "Queued"
                        && kit::small_button(ui, "Cancel queue", ButtonVariant::Ghost, None)
                    {
                        cancel_queue(app, r);
                    }
                },
            );
        },
    );
    if pressed {
        open_return(app, r);
    }
}

// ---------------------------------------------------------------- bars

/// "Changed in Nov 2026" — the month chip a Home tile sets; a press removes it (MonthChip).
fn draw_month_chip(ui: &mut Ui) -> bool {
    let Some(m) = FILTERS.with(|f| f.borrow().updated_month.clone()) else {
        return false;
    };
    let label = format!("Changed in {}", filings_model::month_label(&m));
    if kit::small_button(ui, &label, ButtonVariant::Ghost, Some("x")) {
        FILTERS.with(|f| f.borrow_mut().updated_month = None);
        return true;
    }
    false
}

/// Clear filters (ghost, x icon): the search and every filter go back to all.
fn draw_clear(ui: &mut Ui) -> bool {
    let active = FILTERS.with(|f| filters_active(&f.borrow()));
    active && kit::small_button(ui, "Clear filters", ButtonVariant::Ghost, Some("x"))
}

/// The four multi-combo triggers in a wrapping row (FilterCombos, inline).
fn draw_combos(ui: &mut Ui, app: &AppState, all: &[ReturnRow], stacked: bool) {
    FILTERS.with(|f| {
        let flt = f.borrow();
        for kind in KINDS {
            if stacked {
                let (all_word, _, _) = kind_words(kind);
                ui.text_with(
                    &kind_title(kind),
                    &cui::Text {
                        size: theme::r#type::META.0,
                        color: kit::frame_palette().muted_foreground,
                        ..Default::default()
                    },
                );
                let _ = all_word;
            }
            let index = form_index();
            let opts = combo_options(kind, app, all, &index);
            let sel = get_sel(&flt, kind).clone();
            let is_narrowed = narrowed(&sel, &opts);
            let label = combo_label(kind, &opts, &sel);
            // The one selected option's lead shows in the trigger when narrowed.
            let single = if is_narrowed && on_values(&sel, &opts).len() == 1 {
                on_values(&sel, &opts)[0].lead.clone()
            } else {
                Lead::None
            };
            let lead = match single {
                Lead::None => None,
                l => Some(l),
            };
            let is_open = COMBO.with(|c| c.borrow().as_deref() == Some(kind));
            let trigger_id = format!("combo-{}", kind);
            if combo_trigger(
                ui,
                &trigger_id,
                kind,
                &label,
                lead.as_ref(),
                is_narrowed,
                is_open,
            ) && !modal()
            {
                COMBO.with(|c| *c.borrow_mut() = Some(kind));
                COMBO_SEARCH.with(|s| s.borrow_mut().clear());
            }
        }
    });
}

/// The wide filter bar: search (200), the four combos, the month chip, Clear filters, count.
fn draw_wide_bar(ui: &mut Ui, app: &AppState, all: &[ReturnRow], rows: &[ReturnRow]) {
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: theme::space(2),
            align: Align::Center,
            wrap: true,
            ..Default::default()
        },
        |ui| {
            // Search is 200pt (TextField width={200}).
            ui.box_(
                &cui::BoxOpts {
                    width: Size::px(200.0),
                    ..Default::default()
                },
                |ui| {
                    FILTERS.with(|f| {
                        let mut flt = f.borrow_mut();
                        let mut v = flt.query.clone();
                        if v.capacity() < 256 {
                            v.reserve(256);
                        }
                        let changed =
                            kit::text_input(ui, "filings-search", &mut v, "Search returns", false);
                        if changed {
                            flt.query = v;
                        }
                    });
                },
            );
            draw_combos(ui, app, all, false);
            draw_month_chip(ui);
            // Count then Clear filters, pushed to the row's right edge; the FILL box keeps
            // them right-aligned when the wrapping row breaks them onto a second line.
            ui.box_(
                &cui::BoxOpts {
                    width: Size::FILL,
                    ..Default::default()
                },
                |ui| {
                    ui.row(
                        &cui::Row {
                            width: Size::FILL,
                            gap: theme::space(2),
                            align: Align::Center,
                            ..Default::default()
                        },
                        |ui| {
                            ui.spacer();
                            ui.muted(&filings_model::plural(rows.len(), "return"));
                            draw_clear(ui);
                        },
                    );
                },
            );
        },
    );
}

/// The narrow bar: search and a Filters button (accent badge = narrowed count); the month
/// chip stays in view under it (NarrowBar).
fn draw_narrow_bar(ui: &mut Ui) {
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: theme::space(2),
            align: Align::Center,
            ..Default::default()
        },
        |ui| {
            ui.box_(
                &cui::BoxOpts {
                    width: Size::FILL,
                    ..Default::default()
                },
                |ui| {
                    FILTERS.with(|f| {
                        let mut flt = f.borrow_mut();
                        let mut v = flt.query.clone();
                        if v.capacity() < 256 {
                            v.reserve(256);
                        }
                        let changed =
                            kit::text_input(ui, "filings-search", &mut v, "Search returns", false);
                        if changed {
                            flt.query = v;
                        }
                    });
                },
            );
            let n = FILTERS.with(|f| narrowed_count(&f.borrow()));
            let label = if n > 0 {
                format!("Filters {n}")
            } else {
                "Filters".to_owned()
            };
            if ui.button(&label) {
                SHEET.with(|s| *s.borrow_mut() = true);
            }
        },
    );
    draw_month_chip(ui);
}

// ---------------------------------------------------------------- board / list

/// One board column: title + count chip, "Nothing here." or the cards (BoardColumn).
fn draw_column(
    ui: &mut Ui,
    app: &mut AppState,
    index: &[FormIndexEntry],
    col: &Column,
    rows: &[ReturnRow],
) {
    let cards: Vec<&ReturnRow> = rows
        .iter()
        .filter(|r| col.statuses.contains(&r.status.as_str()))
        .collect();
    ui.box_(
        &cui::BoxOpts {
            width: Size::FILL,
            dir: cui::Dir::Column,
            gap: theme::space(2),
            pad: theme::space(2) + 2.0,
            bg: kit::frame_palette().background,
            border: 1.0,
            border_color: kit::frame_palette().border,
            radius: theme::radius::CARD,
            ..Default::default()
        },
        |ui| {
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    gap: theme::space(1) + 2.0,
                    align: Align::Center,
                    ..Default::default()
                },
                |ui| {
                    ui.text_with(
                        col.title,
                        &cui::Text {
                            size: theme::r#type::CONTROL.0,
                            font: cui::font::MEDIUM,
                            ..Default::default()
                        },
                    );
                    kit::tone_badge(ui, &cards.len().to_string(), "neutral");
                },
            );
            if cards.is_empty() {
                ui.text_with(
                    "Nothing here.",
                    &cui::Text {
                        size: theme::r#type::META.0,
                        color: kit::frame_palette().subtle_foreground,
                        ..Default::default()
                    },
                );
            }
            for r in cards {
                draw_card(ui, app, index, r, Some(col.title));
            }
        },
    );
}

/// The wide board: four columns side by side (Board).
fn draw_board(ui: &mut Ui, app: &mut AppState, rows: &[ReturnRow], index: &[FormIndexEntry]) {
    ui.gap(theme::space(4));
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: theme::space(3),
            align: Align::Start,
            ..Default::default()
        },
        |ui| {
            for col in filings_model::COLUMNS {
                draw_column(ui, app, index, &col, rows);
            }
        },
    );
}

/// One tab of the narrow strip: label + count, fg with a 2pt underline when selected
/// (ui/lists.tsx Tabs).
fn draw_tab(ui: &mut Ui, id: &str, label: &str, count: usize, selected: bool) -> bool {
    let p = kit::frame_palette();
    let wid = ui.make_id(id);
    let hovered = kit::press_hovered(id, ui.mouse_pos().x, ui.mouse_pos().y);
    ui.box_(
        &cui::BoxOpts {
            id: Some(id),
            dir: cui::Dir::Column,
            ..Default::default()
        },
        |ui| {
            ui.box_(
                &cui::BoxOpts {
                    dir: cui::Dir::Row,
                    align: Align::Center,
                    gap: theme::space(1) + 2.0,
                    pad_x: 10.0,
                    min_height: theme::size::CONTROL,
                    bg: if hovered { p.hover } else { p.background },
                    ..Default::default()
                },
                |ui| {
                    ui.text_with(
                        label,
                        &cui::Text {
                            size: theme::r#type::CONTROL.0,
                            color: if selected {
                                p.foreground
                            } else {
                                p.muted_foreground
                            },
                            ..Default::default()
                        },
                    );
                    ui.box_(
                        &cui::BoxOpts {
                            pad_x: 6.0,
                            bg: p.hover,
                            radius: theme::radius::FULL,
                            ..Default::default()
                        },
                        |ui| {
                            ui.text_with(
                                &count.to_string(),
                                &cui::Text {
                                    size: theme::r#type::CAP.0,
                                    color: p.muted_foreground,
                                    tabular: true,
                                    ..Default::default()
                                },
                            );
                        },
                    );
                },
            );
            // The 2pt indicator under the selected tab.
            ui.box_(
                &cui::BoxOpts {
                    width: Size::FILL,
                    height: Size::px(2.0),
                    bg: if selected { p.foreground } else { p.background },
                    ..Default::default()
                },
                |_| {},
            );
        },
    );
    let rect = ui.get_rect(wid);
    kit::press_remember(id, rect);
    let mouse = ui.mouse_pos();
    ui.mouse_clicked(cui::MouseButton::Left) && kit::press_hit(rect, mouse.x, mouse.y)
}

/// The narrow tab strip with a bottom border: All + the four columns, counts included.
fn draw_tabs(ui: &mut Ui, rows: &[ReturnRow]) {
    let tab = TAB.with(|t| t.borrow().clone());
    let mut tabs: Vec<(String, String, usize)> =
        vec![("all".to_owned(), "All".to_owned(), rows.len())];
    for col in filings_model::COLUMNS {
        let n = rows
            .iter()
            .filter(|r| col.statuses.contains(&r.status.as_str()))
            .count();
        tabs.push((col.id.to_owned(), col.title.to_owned(), n));
    }
    let mut picked: Option<String> = None;
    ui.row(
        &cui::Row {
            width: Size::FILL,
            gap: theme::space(1),
            align: Align::End,
            ..Default::default()
        },
        |ui| {
            for (i, (id, label, count)) in tabs.iter().enumerate() {
                if draw_tab(ui, &format!("ftab-{i}"), label, *count, tab == *id) && !modal() {
                    picked = Some(id.clone());
                }
            }
        },
    );
    // The strip's bottom border (Tabs' border-b border-border).
    ui.row(
        &cui::Row {
            width: Size::FILL,
            ..Default::default()
        },
        |ui| {
            ui.separator();
        },
    );
    if let Some(id) = picked {
        TAB.with(|t| *t.borrow_mut() = id);
    }
}

/// The count + Clear filters row under the tabs (Filings.tsx narrow middle row).
fn draw_count_row(ui: &mut Ui, shown: usize) {
    ui.row(
        &cui::Row {
            width: Size::FILL,
            align: Align::Center,
            ..Default::default()
        },
        |ui| {
            ui.muted(&filings_model::plural(shown, "return"));
            ui.spacer();
            draw_clear(ui);
        },
    );
}

/// The narrow list: the cards of the chosen tab, or "Nothing here." (NarrowList).
fn draw_narrow_list(ui: &mut Ui, app: &mut AppState, rows: &[ReturnRow], index: &[FormIndexEntry]) {
    let tab = TAB.with(|t| t.borrow().clone());
    let col = filings_model::COLUMNS.iter().find(|c| c.id == tab);
    let shown: Vec<&ReturnRow> = match col {
        Some(c) => rows
            .iter()
            .filter(|r| c.statuses.contains(&r.status.as_str()))
            .collect(),
        None => rows.iter().collect(),
    };
    if shown.is_empty() {
        kit::list_group(ui, |ui| {
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    pad_y: theme::space(3),
                    pad_x: theme::space(2),
                    ..Default::default()
                },
                |ui| {
                    ui.muted("Nothing here.");
                },
            );
        });
        return;
    }
    let column_title = col.map(|c| c.title);
    ui.gap(theme::space(2));
    for r in shown {
        draw_card(ui, app, index, r, column_title);
    }
}

/// Empty states: nothing saved or filed yet; returns exist but the filters keep none.
fn draw_empty(ui: &mut Ui, filtered: bool) {
    ui.gap(theme::space(4));
    kit::list_group(ui, |ui| {
        ui.row(
            &cui::Row {
                width: Size::FILL,
                pad_y: theme::space(3),
                pad_x: theme::space(2),
                gap: theme::space(2),
                align: Align::Center,
                ..Default::default()
            },
            |ui| {
                if filtered {
                    ui.muted("No returns match these filters.");
                    if kit::small_button(ui, "Clear filters", ButtonVariant::Secondary, None) {
                        FILTERS.with(|f| *f.borrow_mut() = FilterValues::default());
                    }
                } else {
                    ui.muted("Saved and filed returns show here.");
                }
            },
        );
    });
}

// ---------------------------------------------------------------- sheets

/// The narrow Filters sheet: the four combos stacked with labels, Clear filters and
/// "Show N returns" in the footer (Filings.tsx Sheet).
fn draw_sheet(ui: &mut Ui, app: &AppState, all: &[ReturnRow], rows: &[ReturnRow]) {
    if !SHEET.with(|s| *s.borrow()) {
        return;
    }
    let mut open = true;
    ui.dialog_with(
        "Filters",
        &mut open,
        &cui::Dialog {
            width: 360.0,
            ..Default::default()
        },
        |ui| {
            // The combos stacked: a muted label over each trigger, in a column.
            ui.column(
                &cui::Column {
                    width: Size::FILL,
                    gap: theme::space(4),
                    ..Default::default()
                },
                |ui| {
                    FILTERS.with(|f| {
                        let flt = f.borrow();
                        for kind in KINDS {
                            let index = form_index();
                            let opts = combo_options(kind, app, all, &index);
                            let sel = get_sel(&flt, kind).clone();
                            let is_narrowed = narrowed(&sel, &opts);
                            let label = combo_label(kind, &opts, &sel);
                            let single = if is_narrowed && on_values(&sel, &opts).len() == 1 {
                                on_values(&sel, &opts)[0].lead.clone()
                            } else {
                                Lead::None
                            };
                            let lead = match single {
                                Lead::None => None,
                                l => Some(l),
                            };
                            ui.text_with(
                                &kind_title(kind),
                                &cui::Text {
                                    size: theme::r#type::META.0,
                                    color: kit::frame_palette().muted_foreground,
                                    ..Default::default()
                                },
                            );
                            let is_open = COMBO.with(|c| c.borrow().as_deref() == Some(kind));
                            if combo_trigger(
                                ui,
                                &format!("sheet-combo-{kind}"),
                                kind,
                                &label,
                                lead.as_ref(),
                                is_narrowed,
                                is_open,
                            ) {
                                COMBO.with(|c| *c.borrow_mut() = Some(kind));
                                COMBO_SEARCH.with(|s| s.borrow_mut().clear());
                            }
                        }
                    });
                },
            );
            ui.row(
                &cui::Row {
                    width: Size::FILL,
                    gap: theme::space(2),
                    ..Default::default()
                },
                |ui| {
                    ui.spacer();
                    let active = FILTERS.with(|f| filters_active(&f.borrow()));
                    if active
                        && kit::small_button(ui, "Clear filters", ButtonVariant::Secondary, None)
                    {
                        FILTERS.with(|f| *f.borrow_mut() = FilterValues::default());
                    }
                    let n = rows.len();
                    if kit::primary_button(
                        ui,
                        &format!("Show {}", filings_model::plural(n, "return")),
                        true,
                    ) {
                        SHEET.with(|s| *s.borrow_mut() = false);
                    }
                },
            );
        },
    );
    if !open {
        SHEET.with(|s| *s.borrow_mut() = false);
    }
}

// ---------------------------------------------------------------- page

pub fn draw(ui: &mut Ui, app: &mut AppState) {
    apply_preset(app);
    // Dialogs drawn last frame cover the page: custom hit tests below must not fire.
    MODAL.with(|m| m.set(COMBO.with(|c| c.borrow().is_some()) || SHEET.with(|s| *s.borrow())));
    // PageHeader: the title and the lede.
    ui.heading("Filings", 1);
    ui.muted("Every return, from draft to confirmed. Filed returns stay exactly as submitted.");
    ui.gap(theme::space(if wide(app) { 6 } else { 4 }));

    let all = all_rows();
    // The form library's index names each form's frequency and title; it is not read while
    // there is nothing to name.
    let index = if all.is_empty() {
        Vec::new()
    } else {
        form_index()
    };
    let rows = FILTERS.with(|f| filings_model::filter_rows(&all, &index, &f.borrow()));

    let is_wide = wide(app);
    if is_wide {
        draw_wide_bar(ui, app, &all, &rows);
        if all.is_empty() {
            draw_empty(ui, false);
        } else if rows.is_empty() {
            draw_empty(ui, true);
        } else {
            draw_board(ui, app, &rows, &index);
        }
    } else {
        draw_narrow_bar(ui);
        if all.is_empty() {
            draw_empty(ui, false);
        } else {
            draw_tabs(ui, &rows);
            // shownCount: the chosen tab's count (All shows every filtered row).
            let tab = TAB.with(|t| t.borrow().clone());
            let shown = filings_model::COLUMNS
                .iter()
                .find(|c| c.id == tab)
                .map(|c| {
                    rows.iter()
                        .filter(|r| c.statuses.contains(&r.status.as_str()))
                        .count()
                })
                .unwrap_or(rows.len());
            draw_count_row(ui, shown);
            if rows.is_empty() {
                draw_empty(ui, true);
            } else {
                draw_narrow_list(ui, app, &rows, &index);
            }
        }
    }
    draw_combo_dialog(ui, app, &all);
    draw_sheet(ui, app, &all, &rows);
}
