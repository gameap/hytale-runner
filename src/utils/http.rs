use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use reqwest::Client;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;
use tracing::debug;

/// HTTP client with retry and progress bar support
pub struct HttpClient {
    client: Client,
    max_retries: u32,
}

impl HttpClient {
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(300))
            .connect_timeout(Duration::from_secs(30))
            .build()
            .expect("Failed to create HTTP client");

        Self {
            client,
            max_retries: 3,
        }
    }

    /// Download a file with progress bar and retry logic
    pub async fn download_file(
        &self,
        url: &str,
        dest: &Path,
        expected_size: Option<u64>,
    ) -> Result<()> {
        let mut last_error = None;

        for attempt in 1..=self.max_retries {
            match self.download_file_attempt(url, dest, expected_size).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    debug!("Download attempt {} failed: {}", attempt, e);
                    last_error = Some(e);

                    if attempt < self.max_retries {
                        let delay = Duration::from_secs(2u64.pow(attempt));
                        debug!("Retrying in {:?}...", delay);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }

        Err(last_error.unwrap())
    }

    async fn download_file_attempt(
        &self,
        url: &str,
        dest: &Path,
        expected_size: Option<u64>,
    ) -> Result<()> {
        debug!("Downloading {} to {}", url, dest.display());

        // Create parent directory if needed
        if let Some(parent) = dest.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create destination directory")?;
        }

        // Download to temp file first
        let temp_path = dest.with_extension("tmp");

        let response = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to send download request")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status: {}", response.status());
        }

        let total_size = response.content_length().or(expected_size);

        // Create progress bar
        let pb = if let Some(size) = total_size {
            let pb = ProgressBar::new(size);
            pb.set_style(
                ProgressStyle::default_bar()
                    .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({bytes_per_sec})")
                    .unwrap()
                    .progress_chars("=>-"),
            );
            pb
        } else {
            let pb = ProgressBar::new_spinner();
            pb.set_style(
                ProgressStyle::default_spinner()
                    .template("{spinner:.green} [{elapsed_precise}] {bytes} ({bytes_per_sec})")
                    .unwrap(),
            );
            pb
        };

        // Stream response to file
        let mut file = File::create(&temp_path)
            .await
            .context("Failed to create temp file")?;

        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Failed to read response chunk")?;
            file.write_all(&chunk)
                .await
                .context("Failed to write to temp file")?;

            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        file.flush().await.context("Failed to flush temp file")?;
        drop(file);

        pb.finish_with_message("Download complete");

        // Atomic rename
        tokio::fs::rename(&temp_path, dest)
            .await
            .context("Failed to rename temp file to destination")?;

        Ok(())
    }
}

impl Default for HttpClient {
    fn default() -> Self {
        Self::new()
    }
}
