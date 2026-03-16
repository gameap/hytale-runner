use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use tracing::{debug, info};

use crate::config::AppConfig;
use crate::server::downloader::ServerDownloader;

/// Information about an available update
pub struct UpdateInfo {
    pub current_version: String,
    pub new_version: String,
}

/// Status of the server installation
pub struct ServerStatus {
    pub is_installed: bool,
    pub current_version: Option<String>,
    pub has_staged_update: bool,
    pub has_remote_update: bool,
}

/// Handles server updates
pub struct ServerUpdater {
    server_dir: PathBuf,
    downloader: Option<ServerDownloader>,
}

impl ServerUpdater {
    pub fn new(server_dir: PathBuf, config: &AppConfig) -> Self {
        let downloader = ServerDownloader::new(config).ok();
        Self {
            server_dir,
            downloader,
        }
    }

    /// Check if there's a staged update waiting to be applied
    pub fn has_staged_update(&self) -> bool {
        let staging_jar = self
            .server_dir
            .join("updater")
            .join("staging")
            .join("Server")
            .join("HytaleServer.jar");

        staging_jar.exists()
    }

    /// Apply a staged update
    pub fn apply_staged_update(&self) -> Result<()> {
        let staging_dir = self.server_dir.join("updater").join("staging");

        if !staging_dir.exists() {
            anyhow::bail!("No staged update found");
        }

        // Copy server files
        let staged_jar = staging_dir.join("Server").join("HytaleServer.jar");
        if staged_jar.exists() {
            let dest_jar = self.server_dir.join("HytaleServer.jar");
            debug!("Copying {} to {}", staged_jar.display(), dest_jar.display());

            // Create backup of current jar
            if dest_jar.exists() {
                let backup_jar = self.server_dir.join("HytaleServer.jar.bak");
                fs::copy(&dest_jar, &backup_jar)
                    .context("Failed to create backup of current server")?;
            }

            fs::copy(&staged_jar, &dest_jar).context("Failed to copy updated server JAR")?;
        }

        // Copy AOT cache if present
        let staged_aot = staging_dir.join("Server").join("HytaleServer.aot");
        if staged_aot.exists() {
            let dest_aot = self.server_dir.join("HytaleServer.aot");
            debug!("Copying {} to {}", staged_aot.display(), dest_aot.display());
            fs::copy(&staged_aot, &dest_aot).context("Failed to copy updated AOT cache")?;
        }

        // Copy assets if present
        let staged_assets = staging_dir.join("Assets.zip");
        if staged_assets.exists() {
            let dest_assets = self.server_dir.join("Assets.zip");
            debug!(
                "Copying {} to {}",
                staged_assets.display(),
                dest_assets.display()
            );
            fs::copy(&staged_assets, &dest_assets).context("Failed to copy updated assets")?;
        }

        // Remove staging directory
        debug!("Removing staging directory: {}", staging_dir.display());
        fs::remove_dir_all(&staging_dir).context("Failed to remove staging directory")?;

        info!("Staged update applied successfully");
        Ok(())
    }

    /// Check for available updates
    pub async fn check_for_updates(&self) -> Result<Option<UpdateInfo>> {
        // Check if there's a staged update
        if self.has_staged_update() {
            return Ok(Some(UpdateInfo {
                current_version: self
                    .get_current_version()?
                    .unwrap_or_else(|| "unknown".to_string()),
                new_version: "staged".to_string(),
            }));
        }

        // Check for remote updates via hytale-downloader
        if let Some(ref downloader) = self.downloader {
            if downloader.check_updates(&self.server_dir).await? {
                return Ok(Some(UpdateInfo {
                    current_version: self
                        .get_current_version()?
                        .unwrap_or_else(|| "unknown".to_string()),
                    new_version: "available".to_string(),
                }));
            }
        }

        Ok(None)
    }

    /// Get the current server status
    pub async fn get_status(&self) -> Result<ServerStatus> {
        let jar_path = self.server_dir.join("HytaleServer.jar");

        let has_remote_update = if let Some(ref downloader) = self.downloader {
            downloader
                .check_updates(&self.server_dir)
                .await
                .unwrap_or(false)
        } else {
            false
        };

        Ok(ServerStatus {
            is_installed: jar_path.exists(),
            current_version: self.get_current_version()?,
            has_staged_update: self.has_staged_update(),
            has_remote_update,
        })
    }

    /// Get the current server version
    fn get_current_version(&self) -> Result<Option<String>> {
        let version_file = self.server_dir.join("version.txt");

        if version_file.exists() {
            let version = fs::read_to_string(&version_file)
                .context("Failed to read version file")?
                .trim()
                .to_string();
            return Ok(Some(version));
        }

        // Try to read from JAR manifest
        // For now, return None if no version file exists
        Ok(None)
    }
}
