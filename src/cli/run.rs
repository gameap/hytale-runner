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

/// Execute the run command. Without `color`, ANSI escape sequences are removed from the server output.
pub async fn execute(args: &RunArgs, cli: &Cli, color: bool) -> Result<()> {
    let server_dir = cli
        .dir
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap());

    info!("Server directory: {}", server_dir.display());

    // Calculate jar and aot paths with defaults
    let jar_path = args
        .jar_path
        .clone()
        .unwrap_or_else(|| server_dir.join("Server/HytaleServer.jar"));

    let aot_path = args
        .aot_path
        .clone()
        .unwrap_or_else(|| server_dir.join("Server/HytaleServer.aot"));

    // Load configuration
    let config = AppConfig::load(cli.config.as_ref(), &server_dir)?;

    // Ensure Java is available
    let java_path = ensure_java(args, &config).await?;
    info!("Using Java: {}", java_path.display());

    let patchline = args.patchline.as_deref().or(config.patchline.as_deref());
    let server_version = args
        .server_version
        .as_deref()
        .or(config.server_version.as_deref());

    // Ensure server files are available
    ensure_server_files(&server_dir, &jar_path, &config, patchline, server_version).await?;

    if let Some(ref version) = args.server_version {
        if let Err(e) = AppConfig::update_server_version(&server_dir, version) {
            warn!("Failed to update server_version in config: {:#}", e);
        }
    }

    // Run the server with auto-update loop
    run_server_loop(
        args,
        &config,
        &server_dir,
        &java_path,
        &jar_path,
        &aot_path,
        server_version,
        color,
    )
    .await
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
async fn ensure_server_files(
    server_dir: &Path,
    jar_path: &Path,
    config: &AppConfig,
    patchline: Option<&str>,
    server_version: Option<&str>,
) -> Result<()> {
    let assets_path = server_dir.join("Assets.zip");
    let installed = jar_path.exists() && assets_path.exists();

    if let Some(version) = server_version {
        if installed && ServerDownloader::get_local_version(server_dir).as_deref() == Some(version)
        {
            info!("Server files found (version {})", version);
            return Ok(());
        }

        info!(
            "Server version {} is not installed, downloading...",
            version
        );
        let downloader = ServerDownloader::new(config, patchline)?;
        return downloader
            .download_all(server_dir, true, Some(version))
            .await;
    }

    if installed {
        match patchline {
            Some(patchline) if config.update.enabled => {
                info!(
                    "Server files found, checking for updates (patchline: {})",
                    patchline
                );
                // The installed server still starts if the check or the download fails
                if let Err(e) = update_server_files(server_dir, config, patchline).await {
                    warn!(
                        "Failed to update server files, starting the installed version: {:#}",
                        e
                    );
                }
            }
            _ => info!("Server files found"),
        }
        return Ok(());
    }

    info!("Server files not found, downloading...");
    let downloader = ServerDownloader::new(config, patchline)?;
    downloader.download_all(server_dir, false, None).await?;

    Ok(())
}

/// Download the latest version of the patchline if it differs from the installed one
async fn update_server_files(server_dir: &Path, config: &AppConfig, patchline: &str) -> Result<()> {
    ServerDownloader::new(config, Some(patchline))?
        .download_all(server_dir, false, None)
        .await
}

/// Run the server with auto-update loop
#[allow(clippy::too_many_arguments)]
async fn run_server_loop(
    args: &RunArgs,
    config: &AppConfig,
    server_dir: &Path,
    java_path: &Path,
    jar_path: &Path,
    aot_path: &Path,
    server_version: Option<&str>,
    color: bool,
) -> Result<()> {
    let updater = ServerUpdater::new(server_dir.to_path_buf(), config);
    let runner = ServerRunner::new(
        config.clone(),
        server_dir.to_path_buf(),
        java_path.to_path_buf(),
        jar_path.to_path_buf(),
        aot_path.to_path_buf(),
        color,
    );

    loop {
        // Apply staged update if present
        if updater.has_staged_update() {
            if let Some(version) = server_version {
                warn!(
                    "Skipping staged update: server version is pinned to {}",
                    version
                );
            } else {
                info!("Applying staged update...");
                updater
                    .apply_staged_update()
                    .context("Failed to apply staged update")?;
                info!("Update applied successfully");
            }
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

        // Determine IP address
        let ip = if args.ip != "0.0.0.0" {
            args.ip.clone()
        } else {
            config.defaults.ip.clone()
        };

        // Determine accept_early_plugins (CLI overrides config)
        let accept_early_plugins = args
            .accept_early_plugins
            .unwrap_or(config.defaults.accept_early_plugins);

        // Determine allow_op (CLI overrides config)
        let allow_op = args.allow_op.unwrap_or(config.defaults.allow_op);

        // Determine backup settings (CLI overrides config)
        let backup_enabled = args.backup.unwrap_or(config.defaults.backup.enabled);
        let backup_dir: Option<PathBuf> =
            args.backup_dir
                .clone()
                .or(config.defaults.backup.path.clone());
        let backup_frequency =
            args.backup_frequency
                .or((config.defaults.backup.frequency != 0)
                    .then_some(config.defaults.backup.frequency));

        // Run the server
        let exit_code = runner
            .run(
                &memory_min,
                &memory_max,
                aot_enabled,
                args.port,
                &ip,
                &assets_path,
                args.jvm_args.as_deref(),
                accept_early_plugins,
                allow_op,
                backup_enabled,
                backup_dir.as_ref(),
                backup_frequency,
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

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::io::Write;
    use std::os::unix::fs::PermissionsExt;

    use tempfile::TempDir;

    use super::*;

    /// Fake hytale-downloader: the latest version of a patchline is read from
    /// `version-<patchline>`, a download copies `server.zip`
    const FAKE_DOWNLOADER: &str = r#"#!/bin/sh
dir=$(dirname "$0")
patchline=release
while [ $# -gt 0 ]; do
    case "$1" in
        -patchline) patchline=$2; shift 2 ;;
        -print-version) cat "$dir/version-$patchline"; exit ;;
        -download-path) cp "$dir/server.zip" "$2"; exit ;;
        *) shift ;;
    esac
done
"#;

    struct Fixture {
        tools: TempDir,
        server: TempDir,
        config: AppConfig,
    }

    impl Fixture {
        fn new(installed_version: &str) -> Self {
            let tools = TempDir::new().unwrap();
            let downloader = tools.path().join("hytale-downloader");
            fs::write(&downloader, FAKE_DOWNLOADER).unwrap();
            fs::set_permissions(&downloader, fs::Permissions::from_mode(0o755)).unwrap();

            let server_zip = fs::File::create(tools.path().join("server.zip")).unwrap();
            let mut zip = zip::ZipWriter::new(server_zip);
            for (name, content) in [
                ("Server/HytaleServer.jar", "new jar"),
                ("Assets.zip", "new assets"),
            ] {
                zip.start_file(name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                zip.write_all(content.as_bytes()).unwrap();
            }
            zip.finish().unwrap();

            let server = TempDir::new().unwrap();
            fs::create_dir_all(server.path().join("Server")).unwrap();
            fs::write(server.path().join("Server/HytaleServer.jar"), "old jar").unwrap();
            fs::write(server.path().join("Assets.zip"), "old assets").unwrap();
            fs::write(server.path().join("version.txt"), installed_version).unwrap();

            let config = AppConfig {
                downloader_path: Some(downloader.to_string_lossy().into_owned()),
                ..AppConfig::default()
            };

            Self {
                tools,
                server,
                config,
            }
        }

        fn set_latest_version(&self, patchline: &str, version: &str) {
            let path = self.tools.path().join(format!("version-{}", patchline));
            fs::write(path, version).unwrap();
        }

        async fn ensure_server_files(&self, patchline: Option<&str>) -> Result<()> {
            let server_dir = self.server.path();
            let jar_path = server_dir.join("Server/HytaleServer.jar");
            ensure_server_files(server_dir, &jar_path, &self.config, patchline, None).await
        }

        fn installed_jar(&self) -> String {
            fs::read_to_string(self.server.path().join("Server/HytaleServer.jar")).unwrap()
        }

        fn installed_version(&self) -> Option<String> {
            ServerDownloader::get_local_version(self.server.path())
        }
    }

    #[tokio::test]
    async fn test_patchline_update_is_installed_on_start() {
        let fixture = Fixture::new("1.0");
        fixture.set_latest_version("pre-release", "2.0");

        fixture
            .ensure_server_files(Some("pre-release"))
            .await
            .unwrap();

        assert_eq!(fixture.installed_jar(), "new jar");
        assert_eq!(fixture.installed_version().as_deref(), Some("2.0"));
    }

    #[tokio::test]
    async fn test_patchline_up_to_date_is_not_downloaded() {
        let fixture = Fixture::new("1.0");
        fixture.set_latest_version("pre-release", "1.0");

        fixture
            .ensure_server_files(Some("pre-release"))
            .await
            .unwrap();

        assert_eq!(fixture.installed_jar(), "old jar");
    }

    #[tokio::test]
    async fn test_failed_update_check_starts_installed_version() {
        let fixture = Fixture::new("1.0");

        fixture
            .ensure_server_files(Some("pre-release"))
            .await
            .unwrap();

        assert_eq!(fixture.installed_jar(), "old jar");
        assert_eq!(fixture.installed_version().as_deref(), Some("1.0"));
    }

    #[tokio::test]
    async fn test_no_update_check_without_patchline() {
        let fixture = Fixture::new("1.0");
        fixture.set_latest_version("release", "2.0");

        fixture.ensure_server_files(None).await.unwrap();

        assert_eq!(fixture.installed_jar(), "old jar");
    }

    #[tokio::test]
    async fn test_no_update_check_when_updates_disabled() {
        let mut fixture = Fixture::new("1.0");
        fixture.config.update.enabled = false;
        fixture.set_latest_version("release", "2.0");

        fixture.ensure_server_files(Some("release")).await.unwrap();

        assert_eq!(fixture.installed_jar(), "old jar");
    }
}
