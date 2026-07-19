#![deny(warnings)]

//! fileio-mcp binary entry-point.
//!
//! Protocol dispatch, transport framing, and the `serve` CLI are all provided
//! by mcp-core.  This binary only needs to parse its own extra flags
//! (`--block-path`, `--block-file`), build the `PathGuard`, and hand off to
//! mcp-core.

use clap::Args;
use fileio_mcp::path_guard::PathGuard;
use fileio_mcp::service::FileIoService;
use mcp_core::Result;

/// fileio-mcp-specific serve flags. mcp-core flattens `CommonServeArgs`
/// (including `--transport` / `--mode` alias) into the `serve` subcommand
/// automatically; this struct carries only what fileio-mcp adds on top.
#[derive(Args)]
struct Local {
    /// Additional paths to block (repeatable). Trailing / means directory prefix.
    #[arg(long = "block-path")]
    block_paths: Vec<String>,

    /// File containing additional paths to block (one per line, # comments).
    #[arg(long = "block-file")]
    block_file: Option<String>,
}

#[tokio::main]
async fn main() -> Result<()> {
    let config = fileio_mcp::server_config();

    mcp_core::run::<Local, FileIoService, _, _>(config, |local| async move {
        // Zero-config default construction routes through `build_service` so the
        // in-process host (da#538 Phase C) and the binary share one default path
        // and cannot drift. `--block-path` / `--block-file` layer extra deny-list
        // entries on top of the built-in defaults.
        if local.block_paths.is_empty() && local.block_file.is_none() {
            Ok(fileio_mcp::build_service())
        } else {
            let guard = PathGuard::new(&local.block_paths, local.block_file.as_deref());
            Ok(FileIoService::with_guard(guard))
        }
    })
    .await
}
