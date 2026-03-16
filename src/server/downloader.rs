use std::path::Path;

use anyhow::{Context, Result};
use tracing::info;

use crate::api::hytale::HytaleClient;
use crate::config::AppConfig;
use crate::utils::http::HttpClient;

/// Downloads Hytale server files via OAuth2 device flow
pub struct ServerDownloader {
    hytale_client: HytaleClient,
    http_client: HttpClient,
}

impl ServerDownloader {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let hytale_client = HytaleClient::new(config)?;

        Ok(Self {
            hytale_client,
            http_client: HttpClient::new(),
        })
    }

    /// Download all server files
    pub async fn download_all(&self, server_dir: &Path, force: bool) -> Result<()> {
        std::fs::create_dir_all(server_dir).context("Failed to create server directory")?;

        let jar_path = server_dir.join("HytaleServer.jar");
        let assets_path = server_dir.join("Assets.zip");

        // Download HytaleServer.jar
        if force || !jar_path.exists() {
            self.download_server_jar(&jar_path).await?;
        } else {
            info!("HytaleServer.jar already exists, skipping (use --force to re-download)");
        }

        // Download Assets.zip
        if force || !assets_path.exists() {
            self.download_assets(&assets_path).await?;
        } else {
            info!("Assets.zip already exists, skipping (use --force to re-download)");
        }

        Ok(())
    }

    /// Download the server JAR file
    async fn download_server_jar(&self, dest: &Path) -> Result<()> {
        info!("Authenticating with Hytale...");

        // Get download URL (will trigger OAuth2 flow if needed)
        let download_info = self
            .hytale_client
            .get_server_download_url()
            .await
            .context("Failed to get server download URL")?;

        info!("Downloading HytaleServer.jar...");
        self.http_client
            .download_file(&download_info.server_url, dest, None)
            .await
            .context("Failed to download HytaleServer.jar")?;

        info!("HytaleServer.jar downloaded successfully");
        Ok(())
    }

    /// Download the assets file
    async fn download_assets(&self, dest: &Path) -> Result<()> {
        info!("Authenticating with Hytale...");

        // Get download URL (will trigger OAuth2 flow if needed)
        let download_info = self
            .hytale_client
            .get_server_download_url()
            .await
            .context("Failed to get assets download URL")?;

        info!("Downloading Assets.zip...");
        self.http_client
            .download_file(&download_info.assets_url, dest, None)
            .await
            .context("Failed to download Assets.zip")?;

        info!("Assets.zip downloaded successfully");
        Ok(())
    }
}
