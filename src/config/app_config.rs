use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{debug, info};

/// Global configuration file name
const GLOBAL_CONFIG_FILE: &str = "config.yaml";

/// Per-server configuration file name
const SERVER_CONFIG_FILE: &str = ".hytale-runner.yaml";

/// Main application configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub defaults: DefaultsConfig,
    pub java: JavaConfig,
    pub jvm: JvmConfig,
    pub auth: AuthConfig,
    pub update: UpdateConfig,
    /// Path to hytale-downloader binary
    pub downloader_path: Option<String>,
    /// Patchline to download from (release, pre-release)
    pub patchline: Option<String>,
    /// Pinned server version
    pub server_version: Option<String>,
}

/// Default server settings
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct DefaultsConfig {
    pub memory_max: String,
    pub memory_min: String,
    pub port: u16,
    pub ip: String,
    pub aot_enabled: bool,
    pub accept_early_plugins: bool,
    pub allow_op: bool,
    pub backup: BackupConfig,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            memory_max: "4G".to_string(),
            memory_min: "1G".to_string(),
            port: 5520,
            ip: "0.0.0.0".to_string(),
            aot_enabled: true,
            accept_early_plugins: false,
            allow_op: false,
            backup: BackupConfig::default(),
        }
    }
}

/// Java configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct JavaConfig {
    pub auto_install: bool,
    pub path: Option<PathBuf>,
}

impl Default for JavaConfig {
    fn default() -> Self {
        Self {
            auto_install: true,
            path: None,
        }
    }
}

/// JVM configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct JvmConfig {
    pub args: Vec<String>,
}

/// OAuth2 authentication configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub access_token: Option<String>,
    pub refresh_token: Option<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

/// Update configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct UpdateConfig {
    pub enabled: bool,
    pub auto_apply: bool,
}

/// Backup configuration
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(default)]
pub struct BackupConfig {
    pub enabled: bool,
    pub path: Option<PathBuf>,
    pub frequency: u16,
}

impl Default for UpdateConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_apply: false,
        }
    }
}

impl AppConfig {
    /// Load configuration from files
    pub fn load(config_path: Option<&PathBuf>, server_dir: &Path) -> Result<Self> {
        let mut config = AppConfig::default();

        // Load global config
        let global_config_path = if let Some(path) = config_path {
            path.clone()
        } else {
            global_config_dir()
                .map(|d| d.join(GLOBAL_CONFIG_FILE))
                .unwrap_or_default()
        };

        if global_config_path.exists() {
            debug!(
                "Loading global config from: {}",
                global_config_path.display()
            );
            let global_config = load_config_file(&global_config_path)?;
            config = merge_configs(config, global_config);
        }

        // Load per-server config
        let server_config_path = server_dir.join(SERVER_CONFIG_FILE);
        if server_config_path.exists() {
            debug!(
                "Loading server config from: {}",
                server_config_path.display()
            );
            let server_config = load_config_file(&server_config_path)?;
            config = merge_configs(config, server_config);
        }

        Ok(config)
    }

    /// Replace `server_version` in the per-server config if it is set there to a different version.
    /// The file is edited line by line to keep comments and formatting.
    pub fn update_server_version(server_dir: &Path, version: &str) -> Result<()> {
        let path = server_dir.join(SERVER_CONFIG_FILE);
        if !path.exists() {
            return Ok(());
        }

        let current = match load_config_file(&path)?.server_version {
            Some(current) if current != version => current,
            _ => return Ok(()),
        };

        let content = std::fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let updated: String = content
            .split_inclusive('\n')
            .map(|line| {
                if line.starts_with("server_version:") {
                    let eol = &line[line.trim_end_matches(['\r', '\n']).len()..];
                    format!("server_version: \"{}\"{}", version, eol)
                } else {
                    line.to_string()
                }
            })
            .collect();

        if updated == content {
            anyhow::bail!(
                "Failed to find top-level 'server_version' in {}",
                path.display()
            );
        }

        std::fs::write(&path, updated)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        info!(
            "Updated server_version in {}: {} -> {}",
            path.display(),
            current,
            version
        );
        Ok(())
    }
}

/// Get the global configuration directory
fn global_config_dir() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("hytale-runner"))
}

/// Load a configuration file
fn load_config_file(path: &Path) -> Result<AppConfig> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read config file: {}", path.display()))?;

    let config: AppConfig = serde_yaml::from_str(&content)
        .with_context(|| format!("Failed to parse config file: {}", path.display()))?;

    Ok(config)
}

/// Merge two configurations (second overrides first)
fn merge_configs(base: AppConfig, overlay: AppConfig) -> AppConfig {
    AppConfig {
        defaults: DefaultsConfig {
            memory_max: if overlay.defaults.memory_max != DefaultsConfig::default().memory_max {
                overlay.defaults.memory_max
            } else {
                base.defaults.memory_max
            },
            memory_min: if overlay.defaults.memory_min != DefaultsConfig::default().memory_min {
                overlay.defaults.memory_min
            } else {
                base.defaults.memory_min
            },
            port: if overlay.defaults.port != DefaultsConfig::default().port {
                overlay.defaults.port
            } else {
                base.defaults.port
            },
            ip: if overlay.defaults.ip != DefaultsConfig::default().ip {
                overlay.defaults.ip
            } else {
                base.defaults.ip
            },
            accept_early_plugins: if overlay.defaults.accept_early_plugins
                != DefaultsConfig::default().accept_early_plugins
            {
                overlay.defaults.accept_early_plugins
            } else {
                base.defaults.accept_early_plugins
            },
            allow_op: if overlay.defaults.allow_op != DefaultsConfig::default().allow_op {
                overlay.defaults.allow_op
            } else {
                base.defaults.allow_op
            },
            backup: BackupConfig {
                enabled: overlay.defaults.backup.enabled || base.defaults.backup.enabled,
                path: overlay
                    .defaults
                    .backup
                    .path
                    .clone()
                    .or(base.defaults.backup.path.clone()),
                frequency: if overlay.defaults.backup.frequency != 0 {
                    overlay.defaults.backup.frequency
                } else {
                    base.defaults.backup.frequency
                },
            },
            aot_enabled: overlay.defaults.aot_enabled,
        },
        java: JavaConfig {
            auto_install: overlay.java.auto_install,
            path: overlay.java.path.or(base.java.path),
        },
        jvm: JvmConfig {
            args: if !overlay.jvm.args.is_empty() {
                overlay.jvm.args
            } else {
                base.jvm.args
            },
        },
        auth: if overlay.auth.access_token.is_some() {
            overlay.auth
        } else {
            base.auth
        },
        update: overlay.update,
        downloader_path: overlay.downloader_path.or(base.downloader_path),
        patchline: overlay.patchline.or(base.patchline),
        server_version: overlay.server_version.or(base.server_version),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_server_config(dir: &TempDir, content: &str) -> PathBuf {
        let path = dir.path().join(SERVER_CONFIG_FILE);
        std::fs::write(&path, content).unwrap();
        path
    }

    #[test]
    fn test_update_server_version_replaces_value() {
        let dir = TempDir::new().unwrap();
        let path = write_server_config(
            &dir,
            "# pinned\r\npatchline: release\r\nserver_version: 0.7.2\r\ndownloader_path: /bin/dl\r\n",
        );

        AppConfig::update_server_version(dir.path(), "0.7.3").unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "# pinned\r\npatchline: release\r\nserver_version: \"0.7.3\"\r\ndownloader_path: /bin/dl\r\n"
        );
        assert_eq!(
            load_config_file(&path).unwrap().server_version.as_deref(),
            Some("0.7.3")
        );
    }

    #[test]
    fn test_update_server_version_same_version() {
        let dir = TempDir::new().unwrap();
        let path = write_server_config(&dir, "server_version: 0.7.3 # keep\n");

        AppConfig::update_server_version(dir.path(), "0.7.3").unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "server_version: 0.7.3 # keep\n"
        );
    }

    #[test]
    fn test_update_server_version_not_pinned() {
        let dir = TempDir::new().unwrap();
        let path = write_server_config(&dir, "patchline: release\n# server_version: 0.7.2\n");

        AppConfig::update_server_version(dir.path(), "0.7.3").unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            "patchline: release\n# server_version: 0.7.2\n"
        );
    }

    #[test]
    fn test_update_server_version_no_config() {
        let dir = TempDir::new().unwrap();

        AppConfig::update_server_version(dir.path(), "0.7.3").unwrap();

        assert!(!dir.path().join(SERVER_CONFIG_FILE).exists());
    }
}
