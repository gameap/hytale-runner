use anyhow::Result;
use tracing::info;

use crate::cli::{Cli, InstallArgs, InstallCommands};
use crate::java::detector::JavaDetector;
use crate::java::installer::JavaInstaller;

/// Execute the install command
pub async fn execute(args: &InstallArgs, _cli: &Cli) -> Result<()> {
    match &args.command {
        InstallCommands::Java { version, list } => {
            if *list {
                list_java_versions()?;
            } else {
                install_java(version).await?;
            }
        }
    }

    Ok(())
}

/// List installed Java versions
fn list_java_versions() -> Result<()> {
    let detector = JavaDetector::new();
    let versions = detector.find_all()?;

    if versions.is_empty() {
        println!("No Java installations found.");
        return Ok(());
    }

    println!("Installed Java versions:");
    println!();

    for java in versions {
        println!(
            "  Java {} ({}) - {}",
            java.version,
            java.vendor.as_deref().unwrap_or("unknown"),
            java.path.display()
        );
    }

    Ok(())
}

/// Install Java via Adoptium
async fn install_java(version: &str) -> Result<()> {
    info!("Installing Java {} via Adoptium...", version);

    let installer = JavaInstaller::new();

    // Check if already installed
    let detector = JavaDetector::new();
    if let Some(java) = detector.find_java(version.parse().unwrap_or(25))? {
        println!(
            "Java {} is already installed at: {}",
            version,
            java.path.display()
        );
        return Ok(());
    }

    let java_path = installer.install(version).await?;

    println!("Java {} installed successfully!", version);
    println!("Location: {}", java_path.display());

    Ok(())
}
