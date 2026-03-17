use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::io::{AsyncBufReadExt, BufReader};
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

        info!("Running hytale-downloader to check and download server files...");

        let mut cmd = Command::new(&self.downloader_path);
        cmd.arg("--output")
            .arg(server_dir)
            .stdin(Stdio::inherit())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        if force {
            cmd.arg("--force");
        }

        debug!("Executing: {:?}", cmd);

        let mut child = cmd.spawn().context("Failed to execute hytale-downloader")?;

        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");

        let mut captured_output = String::new();

        // Stream and capture stdout
        let stdout_handle = tokio::spawn(async move {
            let mut output = String::new();
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                println!("{}", line);
                output.push_str(&line);
                output.push('\n');
            }
            output
        });

        // Stream and capture stderr
        let stderr_handle = tokio::spawn(async move {
            let mut output = String::new();
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                eprintln!("{}", line);
                output.push_str(&line);
                output.push('\n');
            }
            output
        });

        let status = child
            .wait()
            .await
            .context("Failed to wait for hytale-downloader")?;

        // Collect captured output
        if let Ok(stdout_output) = stdout_handle.await {
            captured_output.push_str(&stdout_output);
        }
        if let Ok(stderr_output) = stderr_handle.await {
            captured_output.push_str(&stderr_output);
        }

        if !status.success() {
            let code = status.code().unwrap_or(-1);

            // Check for 403 Forbidden error
            if captured_output.contains("403 Forbidden")
                || captured_output.contains("HTTP status: 403")
            {
                anyhow::bail!(
                    "hytale-downloader exited with code {}\n\n\
                     Note: HTTP 403 Forbidden error detected.\n\
                     This usually means the game has not been purchased.\n\
                     Please purchase Hytale at https://hytale.com to download server files.",
                    code
                );
            }

            anyhow::bail!("hytale-downloader exited with code {}", code);
        }

        info!("Server files downloaded successfully");
        Ok(())
    }

    /// Check if server files need updating using hytale-downloader
    pub async fn check_updates(&self, server_dir: &Path) -> Result<bool> {
        info!("Checking for server updates...");

        let output = Command::new(&self.downloader_path)
            .arg("--output")
            .arg(server_dir)
            .arg("--check")
            .output()
            .await
            .context("Failed to execute hytale-downloader for update check")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            debug!("hytale-downloader check failed: {}", stderr);
            return Ok(false);
        }

        // Parse output to determine if updates are available
        let stdout = String::from_utf8_lossy(&output.stdout);
        let has_updates = stdout.contains("update available")
            || stdout.contains("needs update")
            || stdout.contains("outdated");

        Ok(has_updates)
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
