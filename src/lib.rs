#![deny(warnings)]
#![recursion_limit = "256"]

// Library crate for fileio-mcp.
// Protocol dispatch, transport framing and CLI are provided by mcp-core.

pub mod error;
pub mod operations;
pub mod path_guard;
pub mod service;
pub mod tools;
