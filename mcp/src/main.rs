mod serialize;
mod server;
mod tools;

use rmcp::{ServiceExt, transport::stdio};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_ansi(false)
        .init();

    let service = server::PlutoMcp::new()
        .serve(stdio())
        .await?;

    service.waiting().await?;
    Ok(())
}
