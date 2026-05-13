//! CLI entry point for `vision-recognizer`.
//!
//! Current commands: `mcp` — start the MCP stdio server.

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "vision-recognizer", about = "OpenAI Vision API tools")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP stdio server.
    Mcp,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .init();

    let cli = Cli::parse();
    match cli.command {
        Commands::Mcp => vision_recognizer::mcp::run().await,
    }
}
