use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use tracing::{info, warn};

use crate::cli::{Cli, RunArgs};
use crate::config::AppConfig;
use crate::java::detector::JavaDetector;
use crate::java::installer::JavaInstaller;
use crate::server::downloader::ServerDownloader;
use crate::server::runner::ServerRunner;
use crate::server::updater::ServerUpdater;

/// Execute the run command
pub async fn execute(args: &RunArgs, cli: &Cli) -> Result<()> {
    let server_dir = cli
        .dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    info!("Server directory: {}", server_dir.display());

    // Load configuration
    let config = AppConfig::load(cli.config.as_ref(), &server_dir)?;

    // Ensure Java is available
    let java_path = ensure_java(args, &config).await?;
    info!("Using Java: {}", java_path.display());

    // Ensure server files are available
    ensure_server_files(&server_dir, &config).await?;

    // Run the server with auto-update loop
    run_server_loop(args, &config, &server_dir, &java_path).await
}

/// Ensure Java is available, installing if necessary
async fn ensure_java(args: &RunArgs, config: &AppConfig) -> Result<PathBuf> {
    // Check if custom Java path is specified
    if let Some(ref path) = args.java_path {
        if path.exists() {
            return Ok(path.clone());
        }
        warn!("Specified Java path does not exist: {}", path.display());
    }

    if let Some(ref path) = config.java.path {
        if path.exists() {
            return Ok(path.clone());
        }
        warn!("Configured Java path does not exist: {}", path.display());
    }

    // Try to detect installed Java 25
    let detector = JavaDetector::new();
    if let Some(java) = detector.find_java(25)? {
        return Ok(java.path);
    }

    // Auto-install if enabled
    if config.java.auto_install {
        info!("Java 25 not found, installing via Adoptium...");
        let installer = JavaInstaller::new();
        let java = installer.install("25").await?;
        return Ok(java);
    }

    anyhow::bail!(
        "Java 25 not found. Run 'hytale-runner install java' or set java.path in config."
    );
}

/// Ensure server files are available, downloading if necessary
async fn ensure_server_files(server_dir: &Path, config: &AppConfig) -> Result<()> {
    let jar_path = server_dir.join("HytaleServer.jar");
    let assets_path = server_dir.join("Assets.zip");

    if jar_path.exists() && assets_path.exists() {
        info!("Server files found");
        return Ok(());
    }

    info!("Server files not found, downloading...");
    let downloader = ServerDownloader::new(config)?;
    downloader.download_all(server_dir, false).await?;

    Ok(())
}

/// Run the server with auto-update loop
async fn run_server_loop(
    args: &RunArgs,
    config: &AppConfig,
    server_dir: &Path,
    java_path: &Path,
) -> Result<()> {
    let updater = ServerUpdater::new(server_dir.to_path_buf(), config);
    let runner = ServerRunner::new(
        config.clone(),
        server_dir.to_path_buf(),
        java_path.to_path_buf(),
    );

    loop {
        // Apply staged update if present
        if updater.has_staged_update() {
            info!("Applying staged update...");
            updater
                .apply_staged_update()
                .context("Failed to apply staged update")?;
            info!("Update applied successfully");
        }

        // Build JVM arguments
        let memory_max = args
            .memory
            .clone()
            .unwrap_or_else(|| config.defaults.memory_max.clone());
        let memory_min = args
            .min_memory
            .clone()
            .unwrap_or_else(|| config.defaults.memory_min.clone());

        // Determine AOT setting
        let aot_enabled = if args.no_aot {
            false
        } else if args.aot {
            true
        } else {
            config.defaults.aot_enabled
        };

        // Determine assets path
        let assets_path = args
            .assets
            .clone()
            .unwrap_or_else(|| server_dir.join("Assets.zip"));

        // Run the server
        let exit_code = runner
            .run(
                &memory_min,
                &memory_max,
                aot_enabled,
                args.port,
                &assets_path,
                args.jvm_args.as_deref(),
            )
            .await?;

        // Exit code 8 = restart for update
        if exit_code == 8 {
            if args.no_restart {
                info!("Server exited with code 8 (update requested), but --no-restart is set");
                return Ok(());
            }
            info!("Server exited with code 8, restarting to apply update...");
            continue;
        }

        // Any other exit code means stop
        if exit_code != 0 {
            anyhow::bail!("Server exited with code {}", exit_code);
        }

        return Ok(());
    }
}
