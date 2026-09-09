//! Opt-in gpui-agent host for bir-desktop.
//!
//! Stable IDs in [`ids`] are always compiled so GPUI widgets can mirror them
//! without pulling `gpui-agent`. The mailbox, semantic host, and UI-thread
//! drain live behind `--features agent`.

pub mod ids;

#[derive(Debug, Clone, Default)]
pub struct ProfileEditor {
    pub tin: String,
    pub full_name: String,
    pub rdo_code: String,
    pub line_of_business: String,
    pub registered_address: String,
    pub zip_code: String,
    pub phone: String,
    pub email: String,
    pub save_message: Option<String>,
    pub errors: Vec<String>,
}

#[cfg(feature = "agent")]
mod bridge;
#[cfg(feature = "agent")]
mod drain;
#[cfg(feature = "agent")]
mod host;

#[cfg(feature = "agent")]
pub use bridge::maybe_start;
#[cfg(feature = "agent")]
pub use drain::apply_agent;
#[cfg(feature = "agent")]
pub use host::{BirAgentHost, fixture_host};
