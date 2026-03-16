use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{debug, info};

use crate::api::adoptium::AdoptiumClient;
use crate::utils::archive;
use crate::utils::http::HttpClient;

use super::platform::{adoptium_arch, adoptium_os, archive_extension, runner_java_dir};

/// Installs Java from Adoptium
pub struct JavaInstaller {
    client: AdoptiumClient,
    http: HttpClient,
}

impl JavaInstaller {
    pub fn new() -> Self {
        Self {
            client: AdoptiumClient::new(),
            http: HttpClient::new(),
        }
    }

    /// Install a specific Java version
    pub async fn install(&self, version: &str) -> Result<PathBuf> {
        // Get download info from Adoptium API
        let asset = self
            .client
            .get_latest_release(version, adoptium_os(), adoptium_arch())
            .await
            .context("Failed to get Java release info from Adoptium")?;

        info!(
            "Downloading Java {} from Adoptium...",
            asset.version_data.semver
        );
        debug!("Download URL: {}", asset.binary.package.link);

        // Prepare installation directory
        let install_dir = runner_java_dir()
            .context("Could not determine Java installation directory")?
            .join(format!("jdk-{}", version));

        std::fs::create_dir_all(&install_dir)
            .context("Failed to create Java installation directory")?;

        // Download the archive
        let archive_path = install_dir.join(format!("java.{}", archive_extension()));
        self.http
            .download_file(
                &asset.binary.package.link,
                &archive_path,
                Some(asset.binary.package.size),
            )
            .await
            .context("Failed to download Java")?;

        // Verify checksum if available
        if let Some(ref checksum) = asset.binary.package.checksum {
            info!("Verifying checksum...");
            crate::utils::hash::verify_sha256(&archive_path, checksum)
                .context("Checksum verification failed")?;
        }

        // Extract the archive
        info!("Extracting Java...");
        let extracted_dir = extract_java_archive(&archive_path, &install_dir)?;

        // Clean up archive
        std::fs::remove_file(&archive_path).ok();

        // Find the java executable
        let java_bin = find_java_executable(&extracted_dir)?;

        info!("Java {} installed successfully", version);

        Ok(java_bin)
    }
}

impl Default for JavaInstaller {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract the Java archive and return the extracted directory
fn extract_java_archive(archive_path: &Path, dest_dir: &Path) -> Result<PathBuf> {
    let extension = archive_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    if extension == "zip" {
        archive::extract_zip(archive_path, dest_dir)?;
    } else {
        // Assume tar.gz
        archive::extract_tar_gz(archive_path, dest_dir)?;
    }

    // Find the extracted directory (typically jdk-25+xx or similar)
    let entries: Vec<_> = std::fs::read_dir(dest_dir)?
        .filter_map(|e| e.ok())
        .filter(|e| e.path().is_dir())
        .filter(|e| {
            e.file_name()
                .to_str()
                .map(|n| n.starts_with("jdk") || n.starts_with("jre"))
                .unwrap_or(false)
        })
        .collect();

    if entries.is_empty() {
        anyhow::bail!("No JDK/JRE directory found in extracted archive");
    }

    Ok(entries[0].path())
}

/// Find the Java executable in the extracted directory
fn find_java_executable(java_home: &Path) -> Result<PathBuf> {
    let java_name = if cfg!(windows) {
        "java.exe"
    } else {
        "java"
    };

    // Try standard location
    let java_bin = java_home.join("bin").join(java_name);
    if java_bin.exists() {
        return Ok(java_bin);
    }

    // Try macOS bundle location
    let java_bin_macos = java_home
        .join("Contents")
        .join("Home")
        .join("bin")
        .join(java_name);
    if java_bin_macos.exists() {
        return Ok(java_bin_macos);
    }

    anyhow::bail!("Could not find Java executable in extracted archive")
}
