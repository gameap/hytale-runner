use anyhow::Result;
use tracing::info;

use crate::cli::{Cli, UpdateArgs, UpdateCommands};
use crate::server::updater::ServerUpdater;

/// Execute the update command
pub async fn execute(args: &UpdateArgs, cli: &Cli) -> Result<()> {
    let server_dir = cli
        .dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    let updater = ServerUpdater::new(server_dir);

    match &args.command {
        Some(UpdateCommands::Check) => check_updates(&updater).await,
        Some(UpdateCommands::Apply) => apply_update(&updater),
        Some(UpdateCommands::Status) | None => show_status(&updater),
    }
}

/// Check for available updates
async fn check_updates(updater: &ServerUpdater) -> Result<()> {
    info!("Checking for updates...");

    let update_info = updater.check_for_updates().await?;

    if let Some(info) = update_info {
        println!("Update available!");
        println!("  Current version: {}", info.current_version);
        println!("  New version: {}", info.new_version);
        println!();
        println!("Run 'hytale-runner update apply' to apply the update.");
    } else {
        println!("Server is up to date.");
    }

    Ok(())
}

/// Apply staged update
fn apply_update(updater: &ServerUpdater) -> Result<()> {
    if !updater.has_staged_update() {
        println!("No staged update found.");
        println!("Run the server to download and stage updates automatically.");
        return Ok(());
    }

    info!("Applying staged update...");
    updater.apply_staged_update()?;
    println!("Update applied successfully!");

    Ok(())
}

/// Show current version and update status
fn show_status(updater: &ServerUpdater) -> Result<()> {
    let status = updater.get_status()?;

    println!("Server Status");
    println!("=============");
    println!();
    println!("Server installed: {}", if status.is_installed { "yes" } else { "no" });

    if let Some(version) = status.current_version {
        println!("Current version: {}", version);
    }

    println!(
        "Staged update: {}",
        if status.has_staged_update {
            "yes"
        } else {
            "no"
        }
    );

    if status.has_staged_update {
        println!();
        println!("Run 'hytale-runner update apply' to apply the staged update.");
    }

    Ok(())
}
