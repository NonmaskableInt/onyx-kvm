use std::io;

mod https;

fn main() -> io::Result<()> {
    if https::serve_from_environment()? {
        return Ok(());
    }
    // MCP protocol output is the only data written to stdout. Any future
    // diagnostics must use stderr so they cannot corrupt stdio framing.
    cleankvm_mcp::serve(io::stdin().lock(), io::stdout().lock())
}
