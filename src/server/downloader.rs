use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;
use tracing::{debug, info};

use crate::config::AppConfig;

const HYTALE_DOWNLOADER_BIN: &str = "hytale-downloader";

/// Downloads Hytale server files using hytale-downloader
pub struct ServerDownloader {
    downloader_path: PathBuf,
}

impl ServerDownloader {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let downloader_path = find_hytale_downloader(config)?;

        Ok(Self { downloader_path })
    }

    /// Download all server files using hytale-downloader
    pub async fn download_all(&self, server_dir: &Path, force: bool) -> Result<()> {
        std::fs::create_dir_all(server_dir).context("Failed to create server directory")?;

        // Check if download is needed
        if !force {
            if let Some(local_version) = Self::get_local_version(server_dir) {
                // Only check remote version if we have a local version to compare
                let remote_version = self.get_remote_version().await?;
                if local_version == remote_version {
                    info!("Server files are up to date (version {})", local_version);
                    return Ok(());
                }
                info!("Update available: {} -> {}", local_version, remote_version);
            }
            // No local version = first download, proceed without version check
        }

        info!("Downloading server files...");

        // Download to temp zip file
        let zip_path = server_dir.join("server-download.zip");

        let mut cmd = Command::new(&self.downloader_path);
        cmd.arg("-download-path")
            .arg(&zip_path)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());

        debug!("Executing: {:?}", cmd);

        let status = cmd
            .status()
            .await
            .context("Failed to execute hytale-downloader")?;

        if !status.success() {
            let code = status.code().unwrap_or(-1);
            anyhow::bail!("hytale-downloader exited with code {}", code);
        }

        // Extract zip to server directory
        info!("Extracting server files...");
        crate::utils::archive::extract_zip(&zip_path, server_dir)?;

        // Get version and save it
        let version = self.get_remote_version().await?;
        Self::save_version(server_dir, &version)?;

        // Clean up zip file
        std::fs::remove_file(&zip_path).ok();

        info!("Server files downloaded successfully (version {})", version);
        Ok(())
    }

    /// Check if server files need updating
    pub async fn check_updates(&self, server_dir: &Path) -> Result<bool> {
        info!("Checking for server updates...");

        let remote_version = self.get_remote_version().await?;

        if let Some(local_version) = Self::get_local_version(server_dir) {
            let has_update = local_version != remote_version;
            if has_update {
                info!("Update available: {} -> {}", local_version, remote_version);
            } else {
                info!("Server is up to date (version {})", local_version);
            }
            Ok(has_update)
        } else {
            // No local version = needs download
            Ok(true)
        }
    }

    /// Get the remote version from hytale-downloader
    async fn get_remote_version(&self) -> Result<String> {
        let output = Command::new(&self.downloader_path)
            .arg("-print-version")
            .output()
            .await
            .context("Failed to get version from hytale-downloader")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("hytale-downloader -print-version failed: {}", stderr);
        }

        let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(version)
    }

    /// Get the local installed version from version.txt
    fn get_local_version(server_dir: &Path) -> Option<String> {
        let version_file = server_dir.join("version.txt");
        std::fs::read_to_string(version_file)
            .ok()
            .map(|s| s.trim().to_string())
    }

    /// Save version to version.txt
    fn save_version(server_dir: &Path, version: &str) -> Result<()> {
        let version_file = server_dir.join("version.txt");
        std::fs::write(version_file, version).context("Failed to save version file")
    }
}

/// Find hytale-downloader binary
fn find_hytale_downloader(config: &AppConfig) -> Result<PathBuf> {
    // 1. Check if configured path exists
    if let Some(ref path) = config.downloader_path {
        let path = PathBuf::from(path);
        if path.exists() {
            debug!("Using configured hytale-downloader: {:?}", path);
            return Ok(path);
        }
    }

    // 2. Check in same directory as hytale-runner executable
    if let Ok(exe_path) = std::env::current_exe() {
        if let Some(exe_dir) = exe_path.parent() {
            let downloader_path = exe_dir.join(HYTALE_DOWNLOADER_BIN);
            if downloader_path.exists() {
                debug!(
                    "Found hytale-downloader next to executable: {:?}",
                    downloader_path
                );
                return Ok(downloader_path);
            }

            // Also check with .exe extension on Windows
            #[cfg(windows)]
            {
                let downloader_path = exe_dir.join(format!("{}.exe", HYTALE_DOWNLOADER_BIN));
                if downloader_path.exists() {
                    debug!(
                        "Found hytale-downloader.exe next to executable: {:?}",
                        downloader_path
                    );
                    return Ok(downloader_path);
                }
            }
        }
    }

    // 3. Check in PATH
    if let Ok(path) = which::which(HYTALE_DOWNLOADER_BIN) {
        debug!("Found hytale-downloader in PATH: {:?}", path);
        return Ok(path);
    }

    // 4. Check in current directory
    let current_dir_path = PathBuf::from(HYTALE_DOWNLOADER_BIN);
    if current_dir_path.exists() {
        debug!("Found hytale-downloader in current directory");
        return Ok(current_dir_path);
    }

    #[cfg(windows)]
    {
        let current_dir_path = PathBuf::from(format!("{}.exe", HYTALE_DOWNLOADER_BIN));
        if current_dir_path.exists() {
            debug!("Found hytale-downloader.exe in current directory");
            return Ok(current_dir_path);
        }
    }

    anyhow::bail!(
        "hytale-downloader not found. Please install it or set 'downloader_path' in config.\n\
         Expected locations:\n\
         - In PATH\n\
         - Next to hytale-runner executable\n\
         - In current directory\n\
         - Configured via 'downloader_path' in config file"
    )
}
