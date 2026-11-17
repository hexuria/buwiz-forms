//! Pure helpers, ported from `nsh/lib/*.ts`: money, dates, formats, periods, queue messages, the chrome gate.
//! No host calls, no CUI imports — plain functions with the clock as a parameter where the TS took `now`.

pub mod calendar_model;
pub mod chrome_gate;
pub mod dashboard;
pub mod dates;
pub mod errors;
pub mod events;
pub mod facts;
pub mod filings_model;
pub mod format;
pub mod library_rules;
pub mod mask;
pub mod money;
pub mod periods;
pub mod queue_message;
pub mod recent;
pub mod settings_model;
pub mod start;
pub mod taxpayer_logic;
pub mod todo;
