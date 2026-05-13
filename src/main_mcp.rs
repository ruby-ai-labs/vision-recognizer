//! Entry point for the `vision-recognizer-mcp` binary.
//!
//! Initialises tracing to stderr (stdout is JSON-RPC) and starts the MCP stdio server.

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    if let Err(err) = vision_recognizer::mcp::run().await {
        eprintln!("Error: {err:#}");
        std::process::exit(1);
    }
}
