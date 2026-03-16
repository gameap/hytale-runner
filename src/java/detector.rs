use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result};
use tracing::{debug, trace};

use super::platform::{get_java_search_paths, java_executable_name};
use super::JavaInfo;

/// Detects installed Java versions
pub struct JavaDetector {
    search_paths: Vec<PathBuf>,
}

impl JavaDetector {
    pub fn new() -> Self {
        Self {
            search_paths: get_java_search_paths(),
        }
    }

    /// Find a specific Java version
    pub fn find_java(&self, required_version: u32) -> Result<Option<JavaInfo>> {
        let all_versions = self.find_all()?;

        // First, try to find an exact major version match
        for java in &all_versions {
            if let Some(major) = parse_major_version(&java.version) {
                if major == required_version {
                    return Ok(Some(java.clone()));
                }
            }
        }

        Ok(None)
    }

    /// Find all installed Java versions
    pub fn find_all(&self) -> Result<Vec<JavaInfo>> {
        let mut found = Vec::new();

        // Check JAVA_HOME first
        if let Ok(java_home) = std::env::var("JAVA_HOME") {
            let path = PathBuf::from(&java_home);
            if let Some(info) = self.check_java_home(&path)? {
                found.push(info);
            }
        }

        // Search platform-specific paths
        for search_path in &self.search_paths {
            if !search_path.exists() {
                continue;
            }

            trace!("Searching for Java in: {}", search_path.display());

            if let Ok(entries) = std::fs::read_dir(search_path) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        if let Some(info) = self.check_java_home(&path)? {
                            // Avoid duplicates
                            if !found.iter().any(|j: &JavaInfo| j.path == info.path) {
                                found.push(info);
                            }
                        }
                    }
                }
            }
        }

        // Sort by version (newest first)
        found.sort_by(|a, b| {
            let a_major = parse_major_version(&a.version).unwrap_or(0);
            let b_major = parse_major_version(&b.version).unwrap_or(0);
            b_major.cmp(&a_major)
        });

        Ok(found)
    }

    /// Check if a directory contains a valid Java installation
    fn check_java_home(&self, java_home: &Path) -> Result<Option<JavaInfo>> {
        // Try different possible locations for the Java executable
        let possible_bins = [
            java_home.join("bin").join(java_executable_name()),
            java_home
                .join("Contents")
                .join("Home")
                .join("bin")
                .join(java_executable_name()), // macOS bundle
        ];

        for java_bin in &possible_bins {
            if java_bin.exists() {
                if let Some(info) = self.get_java_info(java_bin)? {
                    return Ok(Some(info));
                }
            }
        }

        Ok(None)
    }

    /// Get Java version info by running `java -version`
    fn get_java_info(&self, java_path: &Path) -> Result<Option<JavaInfo>> {
        debug!("Checking Java at: {}", java_path.display());

        let output = Command::new(java_path)
            .arg("-version")
            .output()
            .context("Failed to execute java -version")?;

        // Java outputs version info to stderr
        let version_output = String::from_utf8_lossy(&output.stderr);

        if let Some(info) = parse_java_version_output(&version_output, java_path.to_path_buf()) {
            return Ok(Some(info));
        }

        Ok(None)
    }
}

impl Default for JavaDetector {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse the output of `java -version`
fn parse_java_version_output(output: &str, path: PathBuf) -> Option<JavaInfo> {
    let lines: Vec<&str> = output.lines().collect();
    if lines.is_empty() {
        return None;
    }

    // First line typically contains version info
    // Examples:
    // openjdk version "21.0.1" 2023-10-17
    // java version "1.8.0_381"
    // openjdk version "25" 2025-03-18

    let first_line = lines[0];

    // Extract version string between quotes
    let version = if let Some(start) = first_line.find('"') {
        let rest = &first_line[start + 1..];
        if let Some(end) = rest.find('"') {
            rest[..end].to_string()
        } else {
            return None;
        }
    } else {
        return None;
    };

    // Try to extract vendor from second line
    let vendor = lines.get(1).and_then(|line| {
        if line.contains("Eclipse Adoptium") || line.contains("Temurin") {
            Some("Eclipse Adoptium".to_string())
        } else if line.contains("Oracle") {
            Some("Oracle".to_string())
        } else if line.contains("OpenJDK") {
            Some("OpenJDK".to_string())
        } else if line.contains("Azul") || line.contains("Zulu") {
            Some("Azul Zulu".to_string())
        } else if line.contains("Amazon") || line.contains("Corretto") {
            Some("Amazon Corretto".to_string())
        } else if line.contains("Microsoft") {
            Some("Microsoft".to_string())
        } else {
            None
        }
    });

    // Determine if JDK or JRE (JDKs have javac)
    let is_jdk = path
        .parent()
        .map(|bin| {
            let javac = bin.join(if cfg!(windows) { "javac.exe" } else { "javac" });
            javac.exists()
        })
        .unwrap_or(false);

    Some(JavaInfo {
        path,
        version,
        vendor,
        is_jdk,
    })
}

/// Parse major version from version string
fn parse_major_version(version: &str) -> Option<u32> {
    // Handle old format (1.8.0_xxx -> 8)
    if version.starts_with("1.") {
        let parts: Vec<&str> = version.split('.').collect();
        if parts.len() >= 2 {
            return parts[1].parse().ok();
        }
    }

    // Handle new format (21.0.1 -> 21, 25 -> 25)
    let parts: Vec<&str> = version.split('.').collect();
    if !parts.is_empty() {
        return parts[0].parse().ok();
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_major_version() {
        assert_eq!(parse_major_version("25"), Some(25));
        assert_eq!(parse_major_version("21.0.1"), Some(21));
        assert_eq!(parse_major_version("1.8.0_381"), Some(8));
        assert_eq!(parse_major_version("17.0.2"), Some(17));
    }
}
