use anyhow::{Context, Result};
use reqwest::Client;
use serde::Deserialize;
use tracing::debug;

const ADOPTIUM_API_BASE: &str = "https://api.adoptium.net/v3";

/// Adoptium API v3 client
pub struct AdoptiumClient {
    client: Client,
}

/// Binary package information
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct BinaryPackage {
    /// Download URL
    pub link: String,
    /// File size in bytes
    pub size: u64,
    /// SHA256 checksum
    pub checksum: Option<String>,
    /// File name
    pub name: String,
}

/// Binary information
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
pub struct Binary {
    pub package: BinaryPackage,
    /// Image type (jre, jdk)
    pub image_type: String,
    /// OS (linux, windows, mac)
    pub os: String,
    /// Architecture (x64, aarch64)
    pub architecture: String,
}

/// Wrapper for version data in latest response
#[derive(Debug, Deserialize)]
pub struct VersionDataWrapper {
    pub semver: String,
}

/// Latest release response from Adoptium API
#[derive(Debug, Deserialize)]
pub struct LatestReleaseResponse {
    pub binary: Binary,
    pub version: VersionDataWrapper,
}

/// Simplified asset structure for our use
pub struct AdoptiumAsset {
    pub binary: Binary,
    pub version_data: VersionDataWrapper,
}

impl AdoptiumClient {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    /// Get the latest release for a specific version, OS, and architecture
    pub async fn get_latest_release(
        &self,
        version: &str,
        os: &str,
        arch: &str,
    ) -> Result<AdoptiumAsset> {
        // Use JRE for smaller download size
        let url = format!(
            "{}/assets/latest/{}/hotspot?os={}&architecture={}&image_type=jre",
            ADOPTIUM_API_BASE, version, os, arch
        );

        debug!("Fetching Adoptium release info from: {}", url);

        let response = self
            .client
            .get(&url)
            .header("Accept", "application/json")
            .send()
            .await
            .context("Failed to fetch release info from Adoptium")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Adoptium API returned {}: {}", status, body);
        }

        let releases: Vec<LatestReleaseResponse> = response
            .json()
            .await
            .context("Failed to parse Adoptium API response")?;

        if releases.is_empty() {
            anyhow::bail!(
                "No Java {} release found for {} {}",
                version,
                os,
                arch
            );
        }

        let release = releases.into_iter().next().unwrap();

        Ok(AdoptiumAsset {
            binary: release.binary,
            version_data: release.version,
        })
    }

}

impl Default for AdoptiumClient {
    fn default() -> Self {
        Self::new()
    }
}
