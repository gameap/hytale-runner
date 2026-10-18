pub mod download;
pub mod install;
pub mod run;
pub mod update;
pub mod version;

use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Parser, Subcommand};

/// Drop empty arguments and options with an empty value (`--server-version=`), so they
/// mean "not set". Panels like GameAP substitute unset variables into the command as empty strings.
pub fn skip_empty_args(args: impl IntoIterator<Item = OsString>) -> Vec<OsString> {
    args.into_iter()
        .filter(|arg| {
            let arg = arg.to_string_lossy();
            let empty_option =
                matches!(arg.split_once('='), Some((name, "")) if name.starts_with("--"));
            !arg.is_empty() && !empty_option
        })
        .collect()
}

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
#[allow(clippy::large_enum_variant)]
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

    /// Patchline to download from: release, pre-release (default: release)
    #[arg(long)]
    pub patchline: Option<String>,

    /// Pin the server to a specific version (staged updates are not applied)
    #[arg(long)]
    pub server_version: Option<String>,

    /// Additional JVM arguments
    #[arg(long)]
    pub jvm_args: Option<String>,

    /// Exit on code 8 instead of applying updates
    #[arg(long)]
    pub no_restart: bool,

    /// Acknowledge that loading early plugins is unsupported
    #[arg(long, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
    pub accept_early_plugins: Option<bool>,

    /// Allow op commands
    #[arg(long, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
    pub allow_op: Option<bool>,

    /// Enable automatic backups
    #[arg(long, num_args = 0..=1, require_equals = true, default_missing_value = "true")]
    pub backup: Option<bool>,

    /// Backup directory
    #[arg(long)]
    pub backup_dir: Option<PathBuf>,

    /// Backup frequency in minutes
    #[arg(long)]
    pub backup_frequency: Option<u16>,
}

#[derive(Parser, Debug)]
pub struct DownloadArgs {
    /// Patchline: release (default), pre-release
    #[arg(long)]
    pub patchline: Option<String>,

    /// Download only this server version
    #[arg(long)]
    pub server_version: Option<String>,

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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_run(args: &[&str]) -> RunArgs {
        let args = ["run"].iter().chain(args).map(OsString::from);
        RunArgs::try_parse_from(skip_empty_args(args)).unwrap()
    }

    #[test]
    fn test_skip_empty_args() {
        let args = [
            "hytale-runner",
            "run",
            "",
            "--server-version=",
            "--patchline=release",
            "--jvm-args=-Dfoo=",
            "-Dbar=",
        ]
        .map(OsString::from);

        assert_eq!(
            skip_empty_args(args),
            [
                "hytale-runner",
                "run",
                "--patchline=release",
                "--jvm-args=-Dfoo=",
                "-Dbar="
            ]
            .map(OsString::from)
        );
    }

    #[test]
    fn test_run_empty_values_are_not_set() {
        let args = parse_run(&[
            "--patchline=",
            "--server-version=",
            "--backup-dir=",
            "--backup-frequency=",
            "--jvm-args=",
            "",
        ]);

        assert_eq!(args.patchline, None);
        assert_eq!(args.server_version, None);
        assert_eq!(args.backup_dir, None);
        assert_eq!(args.backup_frequency, None);
        assert_eq!(args.jvm_args, None);
    }

    #[test]
    fn test_run_bool_flags() {
        let args = parse_run(&[
            "--allow-op",
            "--accept-early-plugins=true",
            "--backup=false",
        ]);

        assert_eq!(args.allow_op, Some(true));
        assert_eq!(args.accept_early_plugins, Some(true));
        assert_eq!(args.backup, Some(false));

        let args = parse_run(&[]);

        assert_eq!(args.allow_op, None);
        assert_eq!(args.accept_early_plugins, None);
        assert_eq!(args.backup, None);
    }
}
