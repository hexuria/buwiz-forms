//! `bir-host-shim`: a UI-free stand-in for the `gpui_shell` host types.
//!
//! `bir-app-api` depends on it as `gpui_shell = { package = "bir-host-shim" }`,
//! so code written against `gpui_shell::HostModule` and friends builds with no
//! GPUI in the graph. `bir-shell` adapts a module built here to a real
//! `gpui_shell::HostModule`; the NSH door calls it directly.

pub mod host_modules;

pub use host_modules::{
    FunctionKind, HostArguments, HostError, HostFuture, HostModule, HostObject, HostResult,
    HostValue, RESERVED_SPECIFIERS,
};
