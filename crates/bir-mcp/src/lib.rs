//! `bir-mcp`: a draft-only MCP adapter (stdio, or Streamable HTTP on
//! loopback via [`http`]) for BIR forms 2551Q and 1601C. It lists the running
//! BIR apps through upstream gpui-agent discovery ([`apps`]), works only on
//! the one the user selects with `bir_connect`, and exposes a closed set of
//! tools (see [`tools::TOOLS`]); there is no submit, queue, file or payment
//! tool.
pub mod apps;
pub mod client;
pub mod http;
pub mod server;
pub mod tokens;
pub mod tools;
