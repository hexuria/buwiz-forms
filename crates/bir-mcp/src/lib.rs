//! `bir-mcp`: a draft-only MCP (stdio) adapter for BIR forms 2551Q and
//! 1601C. It finds a running BIR through the gpui-agent discovery records and
//! exposes a closed set of tools (see [`tools::TOOLS`]); there is no submit,
//! queue, file or payment tool.
pub mod client;
pub mod discovery;
pub mod server;
pub mod tools;
