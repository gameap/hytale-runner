use anyhow::Result;

/// Execute the version command
pub async fn execute() -> Result<()> {
    println!("hytale-runner {}", env!("CARGO_PKG_VERSION"));
    println!();
    println!("A cross-platform CLI tool for running Hytale dedicated servers.");
    println!();
    println!("Repository: {}", env!("CARGO_PKG_REPOSITORY"));
    println!("License: {}", env!("CARGO_PKG_LICENSE"));

    Ok(())
}
