use anyhow::Result;
use tracing::info;

use crate::cli::{Cli, DownloadArgs};
use crate::config::AppConfig;
use crate::server::downloader::ServerDownloader;

/// Execute the download command
pub async fn execute(args: &DownloadArgs, cli: &Cli) -> Result<()> {
    let server_dir = cli
        .dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap().join("Server"));

    info!("Server directory: {}", server_dir.display());

    // Load configuration
    let config = AppConfig::load(cli.config.as_ref(), &server_dir)?;

    // Create downloader
    let downloader = ServerDownloader::new(&config)?;

    // Download server files
    info!(
        "Downloading Hytale server files (patchline: {})...",
        args.patchline
    );
    downloader.download_all(&server_dir, args.force).await?;

    info!("Download complete!");
    Ok(())
}
