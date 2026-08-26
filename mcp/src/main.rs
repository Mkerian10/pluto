mod docs;
mod serialize;
mod server;
mod supervisor;
mod tools;

use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    // Default mode is the hot-reload supervisor (#213): it spawns this same
    // binary with --serve as a child, proxies stdio, and restarts the child
    // (replaying the MCP handshake) when the binary on disk changes.
    // `--serve` runs the actual server; PLUTO_MCP_NO_SUPERVISOR=1 also
    // bypasses the supervisor for environments that manage reloads themselves.
    let serve_directly = std::env::args().any(|a| a == "--serve")
        || std::env::var("PLUTO_MCP_NO_SUPERVISOR").is_ok_and(|v| v == "1");

    if serve_directly {
        let service = server::PlutoMcp::new()
            .serve(stdio())
            .await?;
        service.waiting().await?;
        Ok(())
    } else {
        supervisor::run().await
    }
}
