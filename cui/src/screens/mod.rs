//! The screens: one module per view, dispatched by `state::ViewName`.

pub mod auth;
pub mod calendar;
pub mod filings;
pub mod form;
pub mod home;
pub mod library;
pub mod settings;
pub mod taxpayer;

use cui::Ui;

use crate::screens::auth::SharedAuth;
use crate::state::{AppState, ViewName};

/// Draws the current view inside the frame's main surface.
pub fn draw(ui: &mut Ui, app: &mut AppState, auth: &SharedAuth) {
    form::set_active(matches!(app.view(), ViewName::Form));
    match app.view() {
        ViewName::Home => home::draw(ui, app),
        ViewName::Library => library::draw(ui, app),
        ViewName::Calendar => calendar::draw(ui, app),
        ViewName::Filings => filings::draw(ui, app),
        ViewName::Settings => settings::draw(ui, app),
        ViewName::Profile => taxpayer::draw_profile(ui, app),
        ViewName::Forms => taxpayer::draw_forms(ui, app),
        ViewName::Templates => taxpayer::draw_templates(ui, app),
        ViewName::NewTaxpayer => taxpayer::draw_new(ui, app, auth),
        ViewName::NewBranch => taxpayer::draw_new_branch(ui, app),
        ViewName::Form => form::draw(ui, app),
        ViewName::Kit | ViewName::Theme => crate::screens::home::draw(ui, app),
    }
}
