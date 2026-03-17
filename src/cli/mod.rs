pub mod download;
pub mod install;
pub mod run;
pub mod update;
pub mod version;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Hytale Runner - Cross-platform CLI for running Hytale dedicated servers
#[derive(Parser, Debug)]
#[command(name = "hytale-runner")]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Server directory (default: current directory)
    #[arg(short = 'd', long, global = true)]
    pub dir: Option<PathBuf>,

    /// Config file path
    #[arg(short = 'c', long, global = true)]
    pub config: Option<PathBuf>,

    /// Enable verbose output
    #[arg(short = 'V', long, global = true)]
    pub verbose: bool,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Download (if needed) and run the Hytale server
    Run(RunArgs),

    /// Download server files without running
    Download(DownloadArgs),

    /// Install Java 25 via Adoptium
    Install(InstallArgs),

    /// Manage server updates
    Update(UpdateArgs),

    /// Show version info
    Version,
}

#[derive(Parser, Debug)]
pub struct RunArgs {
    /// Path to Assets.zip (default: auto-detect)
    #[arg(long)]
    pub assets: Option<PathBuf>,

    /// Server port (default: 5520)
    #[arg(long, default_value = "5520")]
    pub port: u16,

    /// IP address to bind to (default: 0.0.0.0)
    #[arg(long, default_value = "0.0.0.0")]
    pub ip: String,

    /// Max memory, e.g., 4G
    #[arg(long)]
    pub memory: Option<String>,

    /// Min memory, e.g., 1G
    #[arg(long)]
    pub min_memory: Option<String>,

    /// Enable AOT cache (default: true if exists)
    #[arg(long)]
    pub aot: bool,

    /// Disable AOT cache
    #[arg(long)]
    pub no_aot: bool,

    /// Custom Java binary path
    #[arg(long)]
    pub java_path: Option<PathBuf>,

    /// Path to HytaleServer.jar (default: Server/HytaleServer.jar)
    #[arg(long)]
    pub jar_path: Option<PathBuf>,

    /// Path to HytaleServer.aot (default: Server/HytaleServer.aot)
    #[arg(long)]
    pub aot_path: Option<PathBuf>,

    /// Additional JVM arguments
    #[arg(long)]
    pub jvm_args: Option<String>,

    /// Exit on code 8 instead of applying updates
    #[arg(long)]
    pub no_restart: bool,
}

#[derive(Parser, Debug)]
pub struct DownloadArgs {
    /// Patchline: release (default), pre-release
    #[arg(long, default_value = "release")]
    pub patchline: String,

    /// Re-download even if files exist
    #[arg(long)]
    pub force: bool,
}

#[derive(Parser, Debug)]
pub struct InstallArgs {
    #[command(subcommand)]
    pub command: InstallCommands,
}

#[derive(Subcommand, Debug)]
pub enum InstallCommands {
    /// Install Java from Adoptium
    Java {
        /// Java version (default: 25)
        #[arg(long, default_value = "25")]
        version: String,

        /// List installed Java versions
        #[arg(long)]
        list: bool,
    },
}

#[derive(Parser, Debug)]
pub struct UpdateArgs {
    #[command(subcommand)]
    pub command: Option<UpdateCommands>,
}

#[derive(Subcommand, Debug)]
pub enum UpdateCommands {
    /// Check for available updates
    Check,

    /// Apply staged update
    Apply,

    /// Show current version and update status
    Status,
}
