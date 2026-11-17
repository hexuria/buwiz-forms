//! The Form library (port of `nsh/screens/library/`): search, scope ("Filed in <year>" or "All
//! forms"), frequency, the "n of 43" count, and a grid of form cards. A card opens its form as a
//! sample: blank, with no period, and never a recent return.

use std::cell::RefCell;
use std::collections::HashSet;

use chrono::Datelike;
use cui::Ui;
use serde_json::json;

use crate::state::{self, AppState, OpenReturnOptions, ViewName};
use crate::types::ScreenFormIndexEntry;
use crate::ui::kit::UiExt;

const FREQS: [&str; 5] = ["All", "Monthly", "Quarterly", "Annual", "As needed"];

/// The filters are this screen's state; they live at module level, as the old app kept them on the
/// app object, so they survive a trip to another page.
thread_local! {
    static LIB_QUERY: RefCell<String> = const { RefCell::new(String::new()) };
    static LIB_FREQ: RefCell<usize> = const { RefCell::new(0) };
    /// null until the person picks a scope: the start route's, else with a taxpayer "owned".
    static LIB_SCOPE: RefCell<Option<bool>> = const { RefCell::new(None) }; // true = owned
}

/// The next session starts from the defaults.
pub fn reset_for_session() {
    LIB_QUERY.with(|q| q.borrow_mut().clear());
    LIB_FREQ.with(|f| *f.borrow_mut() = 0);
    LIB_SCOPE.with(|s| *s.borrow_mut() = None);
}

/// The forms a filter shows (library/rules.ts `matchingForms`).
fn matching(
    forms: &[ScreenFormIndexEntry],
    owned: &HashSet<String>,
    scope_owned: bool,
    freq: &str,
    query: &str,
) -> Vec<ScreenFormIndexEntry> {
    crate::lib::library_rules::matching_forms(
        forms,
        &crate::lib::library_rules::FormFilter {
            scope: if scope_owned {
                crate::lib::library_rules::FormScope::Owned
            } else {
                crate::lib::library_rules::FormScope::All
            },
            owned,
            freq: crate::lib::library_rules::as_frequency(freq),
            query,
        },
    )
}

/// A card's press: the form as a sample of the selected branch, for the selected year.
fn open_sample(app: &mut AppState, code: &str) {
    let Some(branch) = app.branch() else {
        app.toast(
            "Add a taxpayer first, then open a form for them.",
            None,
            None,
        );
        return;
    };
    app.open_return(OpenReturnOptions {
        tin: branch.tin,
        code: code.to_owned(),
        year: app
            .year()
            .unwrap_or_else(|| chrono::Local::now().year() as i64),
        period: 0,
        return_key: None,
        mode: Some("sample".to_owned()),
        snapshot_id: None,
        new_key: None,
        new_key_from: None,
    });
}

pub fn draw(ui: &mut Ui, app: &mut AppState) {
    let forms = state::screen_index();
    let owned: HashSet<String> = app.year_forms().into_iter().collect();

    // scope in effect
    let has_tp = app.taxpayer().is_some();
    let scope_owned = LIB_SCOPE.with(|s| *s.borrow()).unwrap_or_else(|| {
        if has_tp {
            match app.start_params.get("scope").map(String::as_str) {
                Some("all") => false,
                Some("owned") => true,
                _ => true,
            }
        } else {
            false
        }
    });

    let year = app
        .year()
        .unwrap_or_else(|| chrono::Local::now().year() as i64);
    let query = LIB_QUERY.with(|q| q.borrow().clone());
    let freq_idx = LIB_FREQ.with(|f| *f.borrow());
    let freq = FREQS[freq_idx.min(FREQS.len() - 1)];

    // filters row
    let mut q = query.clone();
    if ui.text_input(
        "library-search",
        &mut q,
        "Find a form, like 1601C or VAT",
        false,
    ) {
        LIB_QUERY.with(|c| *c.borrow_mut() = q);
    }
    ui.row(&Default::default(), |ui| {
        if has_tp {
            let owned_label = format!("Filed in {year} ({})", owned.len());
            let all_label = format!("All forms ({})", forms.len());
            for (label, is_owned) in [(owned_label, true), (all_label, false)] {
                let active = scope_owned == is_owned;
                if ui.ghost_button(&label, true) && !active {
                    LIB_SCOPE.with(|s| *s.borrow_mut() = Some(is_owned));
                }
            }
        }
        for (i, f) in FREQS.iter().enumerate() {
            let active = i == freq_idx;
            if ui.ghost_button(f, true) && !active {
                LIB_FREQ.with(|c| *c.borrow_mut() = i);
            }
        }
    });

    let shown = matching(&forms, &owned, scope_owned, freq, &query);
    ui.muted(&format!("{} of {}", shown.len(), forms.len()));

    // lede
    if let Some(tp) = app.taxpayer() {
        let branch = if tp.branches.len() > 1 {
            app.branch()
                .map(|b| format!(" ({})", b.name))
                .unwrap_or_default()
        } else {
            String::new()
        };
        ui.muted(&format!(
            "Forms {}{} files in {year}. Choose All forms to see every return eBIRForms can prepare.",
            tp.name, branch
        ));
    } else {
        ui.muted("Every return eBIRForms can prepare. Add a taxpayer to fill one in.");
    }
    ui.heading("Form library", 1);

    if shown.is_empty() {
        let empty = if has_tp && scope_owned && owned.is_empty() {
            format!(
                "{} has no forms for {year} yet. Choose them under Forms, or see All forms.",
                app.taxpayer().map(|t| t.name).unwrap_or_default()
            )
        } else {
            "No form matches. Try a code like 1601C or a word like VAT.".to_owned()
        };
        ui.muted(&empty);
    } else {
        // grid: `repeat(auto-fill, minmax(250px, 1fr))` with gap-3 — as many 250pt columns as
        // fit, each stretched to an equal share of the row (FormTile.tsx).
        let gap = crate::theme::space(3);
        let w = state::content_column(crate::state::ViewLayout::Page, app.main_width());
        let cols = (((w + gap) / (250.0 + gap)).floor() as usize).max(1);
        let col_w = (w - gap * (cols - 1) as f32) / cols as f32;
        for (ri, chunk) in shown.chunks(cols).enumerate() {
            if ri > 0 {
                ui.gap(gap);
            }
            ui.row(
                &cui::Row {
                    width: cui::Size::FILL,
                    gap,
                    ..Default::default()
                },
                |ui| {
                    for f in chunk {
                        let code = f.c.clone();
                        let title = f.t.clone();
                        let freq = f.freq.clone();
                        let pages = f.pages;
                        let nfields = f.n;
                        let draft = f.q < 0.8;
                        let pressed = ui.card(
                            &cui::Card {
                                width: cui::Size::px(col_w),
                                pad: crate::theme::space(3),
                                gap: crate::theme::space(2),
                                hoverable: true,
                                ..Default::default()
                            },
                            |ui| {
                                ui.row(
                                    &cui::Row {
                                        width: cui::Size::FILL,
                                        gap: crate::theme::space(2),
                                        align: cui::Align::Center,
                                        ..Default::default()
                                    },
                                    |ui| {
                                        ui.text_with(
                                            &code,
                                            &cui::Text {
                                                size: crate::theme::r#type::TITLE.0,
                                                font: cui::font::MEDIUM,
                                                ..Default::default()
                                            },
                                        );
                                        ui.spacer();
                                        if draft {
                                            ui.tone_badge("Draft layout", "warning");
                                        }
                                    },
                                );
                                // The title is clamped to two lines (h-9, leading 18).
                                ui.box_(
                                    &cui::BoxOpts {
                                        width: cui::Size::FILL,
                                        height: cui::Size::px(36.0),
                                        clip: true,
                                        ..Default::default()
                                    },
                                    |ui| {
                                        ui.text_with(
                                            &title,
                                            &cui::Text {
                                                size: crate::theme::r#type::CONTROL.0,
                                                width: cui::Size::FILL,
                                                line_height: 18.0,
                                                ..Default::default()
                                            },
                                        );
                                    },
                                );
                                ui.row(
                                    &cui::Row {
                                        width: cui::Size::FILL,
                                        gap: 6.0,
                                        wrap: true,
                                        ..Default::default()
                                    },
                                    |ui| {
                                        ui.tone_badge(&freq, "neutral");
                                        ui.tone_badge(&format!("{pages}p"), "neutral");
                                        ui.tone_badge(&format!("{nfields} fields"), "neutral");
                                    },
                                );
                            },
                        );
                        if pressed {
                            open_sample(app, &code);
                        }
                    }
                },
            );
        }
    }
}
