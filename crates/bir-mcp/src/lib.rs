//! `bir-mcp`: a draft-only MCP adapter (stdio, or Streamable HTTP on
//! loopback via [`http`]) for all 43 BIR forms. It lists the running
//! BIR apps through upstream gpui-agent discovery ([`apps`]), works only on
//! the one the user selects with `bir_connect`, and exposes a closed set of
//! tools (see [`tools::TOOLS`]); there is no submit, queue, file or payment
//! tool (`bir_form_review` only brings a clean draft up for the user's own
//! Submit click).
pub mod apps;
pub mod client;
pub mod http;
pub mod server;
pub mod tokens;
pub mod tools;
