#![allow(
    clippy::too_many_lines,
    clippy::too_many_arguments,
    clippy::needless_raw_string_hashes
)]

use std::io;

mod https;
mod web;

fn main() -> io::Result<()> {
    if https::serve_from_environment()? {
        return Ok(());
    }
    // MCP protocol output is the only data written to stdout. Any future
    // diagnostics must use stderr so they cannot corrupt stdio framing.
    onyx_kvm_mcp::serve(io::stdin().lock(), io::stdout().lock())
}
