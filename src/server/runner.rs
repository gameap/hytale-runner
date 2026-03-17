use std::path::PathBuf;
use std::process::Stdio;

use anyhow::{Context, Result};
use tokio::process::Command;
use tokio::signal;
use tracing::{debug, info};

use crate::config::AppConfig;

/// Runs the Hytale server process
pub struct ServerRunner {
    config: AppConfig,
    server_dir: PathBuf,
    java_path: PathBuf,
}

impl ServerRunner {
    pub fn new(config: AppConfig, server_dir: PathBuf, java_path: PathBuf) -> Self {
        Self {
            config,
            server_dir,
            java_path,
        }
    }

    /// Run the server and return the exit code
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        &self,
        memory_min: &str,
        memory_max: &str,
        aot_enabled: bool,
        port: u16,
        ip: &str,
        assets_path: &PathBuf,
        jvm_args: Option<&str>,
    ) -> Result<i32> {
        let jar_path = self.server_dir.join("HytaleServer.jar");
        let aot_path = self.server_dir.join("HytaleServer.aot");

        if !jar_path.exists() {
            anyhow::bail!(
                "HytaleServer.jar not found in {}",
                self.server_dir.display()
            );
        }

        if !assets_path.exists() {
            anyhow::bail!("Assets.zip not found at {}", assets_path.display());
        }

        let mut cmd = Command::new(&self.java_path);
        cmd.current_dir(&self.server_dir);

        // JVM memory arguments
        cmd.arg(format!("-Xms{}", memory_min));
        cmd.arg(format!("-Xmx{}", memory_max));

        // AOT cache if enabled and exists
        if aot_enabled && aot_path.exists() {
            info!("Using AOT cache: {}", aot_path.display());
            cmd.arg(format!("-XX:AOTCache={}", aot_path.display()));
        }

        // Additional JVM arguments from config
        for arg in &self.config.jvm.args {
            cmd.arg(arg);
        }

        // Additional JVM arguments from command line
        if let Some(args) = jvm_args {
            for arg in args.split_whitespace() {
                cmd.arg(arg);
            }
        }

        // Server arguments
        cmd.arg("-jar").arg("HytaleServer.jar");
        cmd.arg("--assets").arg(assets_path);
        cmd.arg("--bind").arg(format!("{}:{}", ip, port));

        // Interactive console - inherit stdin/stdout/stderr
        cmd.stdin(Stdio::inherit());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        debug!("Starting server with command: {:?}", cmd);
        info!("Starting Hytale server on {}:{}...", ip, port);

        let mut child = cmd.spawn().context("Failed to start server process")?;

        // Handle Ctrl+C gracefully
        let exit_code = tokio::select! {
            status = child.wait() => {
                match status {
                    Ok(status) => status.code().unwrap_or(1),
                    Err(e) => {
                        tracing::error!("Failed to wait for server process: {}", e);
                        1
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C, shutting down server...");

                // Try to kill the process gracefully
                #[cfg(unix)]
                {
                    use tokio::process::Command as TokioCommand;
                    if let Some(pid) = child.id() {
                        // Send SIGTERM first
                        let _ = TokioCommand::new("kill")
                            .arg("-TERM")
                            .arg(pid.to_string())
                            .output()
                            .await;

                        // Wait a bit for graceful shutdown
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }

                // Force kill if still running
                let _ = child.kill().await;
                0
            }
        };

        info!("Server exited with code {}", exit_code);
        Ok(exit_code)
    }
}
