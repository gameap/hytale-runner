use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

use crate::config::AppConfig;

// OAuth2 endpoints (based on Hytale's OAuth2 device flow)
const DEVICE_AUTH_URL: &str = "https://accounts.hytale.com/oauth2/device";
const TOKEN_URL: &str = "https://accounts.hytale.com/oauth2/token";
const CLIENT_ID: &str = "hytale-server-downloader";

// Download URLs
const SERVER_DOWNLOAD_URL: &str = "https://download.hytale.com/server/HytaleServer.jar";
const ASSETS_DOWNLOAD_URL: &str = "https://download.hytale.com/assets/Assets.zip";

/// Device code response from OAuth2 device authorization
#[derive(Debug, Deserialize)]
struct DeviceCodeResponse {
    device_code: String,
    user_code: String,
    verification_uri: String,
    #[serde(default = "default_expires_in")]
    expires_in: u64,
    #[serde(default = "default_interval")]
    interval: u64,
}

fn default_expires_in() -> u64 {
    600
}

fn default_interval() -> u64 {
    5
}

/// Token response from OAuth2 token endpoint
#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenResponse {
    access_token: String,
    token_type: String,
    expires_in: u64,
    refresh_token: Option<String>,
}

/// Error response from OAuth2 token endpoint
#[derive(Debug, Deserialize)]
struct TokenErrorResponse {
    error: String,
    error_description: Option<String>,
}

/// Download information
pub struct DownloadInfo {
    pub server_url: String,
    pub assets_url: String,
}

/// Hytale API client with OAuth2 device flow support
pub struct HytaleClient {
    client: Client,
    config: AppConfig,
}

impl HytaleClient {
    pub fn new(config: &AppConfig) -> Result<Self> {
        let client = Client::builder()
            .user_agent(format!(
                "{}/{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            client,
            config: config.clone(),
        })
    }

    /// Get server download URLs, authenticating if needed
    pub async fn get_server_download_url(&self) -> Result<DownloadInfo> {
        // Check if we have a valid access token
        if let Some(ref token) = self.config.auth.access_token {
            if let Some(expires_at) = self.config.auth.expires_at {
                if expires_at > Utc::now() {
                    debug!("Using cached access token");
                    return Ok(DownloadInfo {
                        server_url: add_auth_header(SERVER_DOWNLOAD_URL, token),
                        assets_url: add_auth_header(ASSETS_DOWNLOAD_URL, token),
                    });
                }
            }

            // Try to refresh if we have a refresh token
            if let Some(ref refresh_token) = self.config.auth.refresh_token {
                if let Ok(token_response) = self.refresh_token(refresh_token).await {
                    let access_token = token_response.access_token.clone();
                    self.save_token(&token_response).await?;
                    return Ok(DownloadInfo {
                        server_url: add_auth_header(SERVER_DOWNLOAD_URL, &access_token),
                        assets_url: add_auth_header(ASSETS_DOWNLOAD_URL, &access_token),
                    });
                }
            }
        }

        // Need to authenticate via device flow
        let token_response = self.authenticate_device_flow().await?;
        let access_token = token_response.access_token.clone();
        self.save_token(&token_response).await?;

        Ok(DownloadInfo {
            server_url: add_auth_header(SERVER_DOWNLOAD_URL, &access_token),
            assets_url: add_auth_header(ASSETS_DOWNLOAD_URL, &access_token),
        })
    }

    /// Perform OAuth2 device authorization flow
    async fn authenticate_device_flow(&self) -> Result<TokenResponse> {
        // Request device code
        let device_code_response = self.request_device_code().await?;

        // Display instructions to user
        println!();
        println!("=== Authentication Required ===");
        println!();
        println!("To download Hytale server files, please authenticate:");
        println!();
        println!("  1. Visit: {}", device_code_response.verification_uri);
        println!("  2. Enter code: {}", device_code_response.user_code);
        println!();
        println!("Waiting for authorization...");

        // Try to open browser automatically
        if let Err(e) = open::that(&device_code_response.verification_uri) {
            debug!("Failed to open browser: {}", e);
        }

        // Poll for authorization
        let token = self
            .poll_for_token(
                &device_code_response.device_code,
                device_code_response.interval,
                device_code_response.expires_in,
            )
            .await?;

        println!("Authentication successful!");
        println!();

        Ok(token)
    }

    /// Request a device code
    async fn request_device_code(&self) -> Result<DeviceCodeResponse> {
        #[derive(Serialize)]
        struct DeviceCodeRequest {
            client_id: &'static str,
            scope: &'static str,
        }

        let response = self
            .client
            .post(DEVICE_AUTH_URL)
            .form(&DeviceCodeRequest {
                client_id: CLIENT_ID,
                scope: "download",
            })
            .send()
            .await
            .context("Failed to request device code")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Device authorization request failed: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse device code response")
    }

    /// Poll for token after user authorizes
    async fn poll_for_token(
        &self,
        device_code: &str,
        interval: u64,
        expires_in: u64,
    ) -> Result<TokenResponse> {
        let start = std::time::Instant::now();
        let timeout = Duration::from_secs(expires_in);

        loop {
            if start.elapsed() > timeout {
                anyhow::bail!("Authorization timed out. Please try again.");
            }

            tokio::time::sleep(Duration::from_secs(interval)).await;

            match self.request_token(device_code).await {
                Ok(token) => return Ok(token),
                Err(e) => {
                    let err_str = e.to_string();
                    if err_str.contains("authorization_pending") {
                        // Still waiting for user to authorize
                        continue;
                    } else if err_str.contains("slow_down") {
                        // We're polling too fast, add extra delay
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        continue;
                    } else if err_str.contains("access_denied") {
                        anyhow::bail!("Authorization was denied by user");
                    } else if err_str.contains("expired_token") {
                        anyhow::bail!("Device code expired. Please try again.");
                    } else {
                        return Err(e);
                    }
                }
            }
        }
    }

    /// Request access token using device code
    async fn request_token(&self, device_code: &str) -> Result<TokenResponse> {
        #[derive(Serialize)]
        struct TokenRequest<'a> {
            grant_type: &'static str,
            device_code: &'a str,
            client_id: &'static str,
        }

        let response = self
            .client
            .post(TOKEN_URL)
            .form(&TokenRequest {
                grant_type: "urn:ietf:params:oauth:grant-type:device_code",
                device_code,
                client_id: CLIENT_ID,
            })
            .send()
            .await
            .context("Failed to request token")?;

        if response.status().is_success() {
            return response
                .json()
                .await
                .context("Failed to parse token response");
        }

        // Parse error response
        let error: TokenErrorResponse = response
            .json()
            .await
            .context("Failed to parse error response")?;

        anyhow::bail!(
            "{}: {}",
            error.error,
            error.error_description.unwrap_or_default()
        )
    }

    /// Refresh access token
    async fn refresh_token(&self, refresh_token: &str) -> Result<TokenResponse> {
        #[derive(Serialize)]
        struct RefreshRequest<'a> {
            grant_type: &'static str,
            refresh_token: &'a str,
            client_id: &'static str,
        }

        let response = self
            .client
            .post(TOKEN_URL)
            .form(&RefreshRequest {
                grant_type: "refresh_token",
                refresh_token,
                client_id: CLIENT_ID,
            })
            .send()
            .await
            .context("Failed to refresh token")?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().await.unwrap_or_default();
            anyhow::bail!("Token refresh failed: {} - {}", status, body);
        }

        response
            .json()
            .await
            .context("Failed to parse refresh token response")
    }

    /// Save token to configuration
    async fn save_token(&self, token: &TokenResponse) -> Result<()> {
        let mut config = self.config.clone();
        config.auth.access_token = Some(token.access_token.clone());
        config.auth.refresh_token = token.refresh_token.clone();
        config.auth.expires_at =
            Some(Utc::now() + chrono::Duration::seconds(token.expires_in as i64));

        config
            .save_auth()
            .context("Failed to save authentication tokens")?;

        info!("Authentication tokens saved");
        Ok(())
    }
}

/// Add authentication header to URL
fn add_auth_header(url: &str, token: &str) -> String {
    // For URLs that need auth, we'll pass the token via query param or header
    // This depends on how Hytale's download API expects auth
    // For now, assuming Bearer token via URL
    format!("{}?access_token={}", url, token)
}
