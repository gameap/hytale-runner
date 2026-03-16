pub mod detector;
pub mod installer;
pub mod platform;

use std::path::PathBuf;

/// Information about an installed Java version
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct JavaInfo {
    /// Path to the Java executable
    pub path: PathBuf,
    /// Java version (e.g., "25", "21.0.1")
    pub version: String,
    /// Java vendor (e.g., "Eclipse Adoptium", "Oracle")
    pub vendor: Option<String>,
    /// Whether this is a JRE or JDK
    pub is_jdk: bool,
}
