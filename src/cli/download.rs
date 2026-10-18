use anyhow::Result;
use tracing::{info, warn};

use crate::cli::{Cli, DownloadArgs};
use crate::config::AppConfig;
use crate::server::downloader::ServerDownloader;

/// Execute the download command
pub async fn execute(args: &DownloadArgs, cli: &Cli) -> Result<()> {
    let server_dir = cli
        .dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    info!("Server directory: {}", server_dir.display());

    // Load configuration
    let config = AppConfig::load(cli.config.as_ref(), &server_dir)?;

    let patchline = args.patchline.as_deref().or(config.patchline.as_deref());
    let server_version = args
        .server_version
        .as_deref()
        .or(config.server_version.as_deref());

    // Create downloader
    let downloader = ServerDownloader::new(&config, patchline)?;

    // Download server files
    info!(
        "Downloading Hytale server files (patchline: {})...",
        patchline.unwrap_or("default")
    );
    downloader
        .download_all(&server_dir, args.force, server_version)
        .await?;

    if let Some(ref version) = args.server_version {
        if let Err(e) = AppConfig::update_server_version(&server_dir, version) {
            warn!("Failed to update server_version in config: {:#}", e);
        }
    }

    info!("Download complete!");
    Ok(())
}
