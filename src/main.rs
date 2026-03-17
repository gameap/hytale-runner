mod api;
mod cli;
mod config;
mod java;
mod server;
mod utils;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use cli::{Cli, Commands};

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose {
        EnvFilter::new("debug")
    } else {
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
    };

    let is_tty = std::io::IsTerminal::is_terminal(&std::io::stderr());
    if !is_tty {
        // SAFETY: Called early in main before any threads are spawned
        unsafe { std::env::set_var("NO_COLOR", "1") };
    }

    tracing_subscriber::registry()
        .with(fmt::layer().with_target(false).with_ansi(is_tty))
        .with(filter)
        .init();

    // Execute the appropriate command
    match &cli.command {
        Commands::Run(args) => cli::run::execute(args, &cli).await,
        Commands::Download(args) => cli::download::execute(args, &cli).await,
        Commands::Install(args) => cli::install::execute(args, &cli).await,
        Commands::Update(args) => cli::update::execute(args, &cli).await,
        Commands::Version => cli::version::execute().await,
    }
}
