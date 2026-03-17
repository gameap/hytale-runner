use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::debug;

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
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            memory_max: "4G".to_string(),
            memory_min: "1G".to_string(),
            port: 5520,
            ip: "0.0.0.0".to_string(),
            aot_enabled: true,
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
    }
}
