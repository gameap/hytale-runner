# Hytale Runner

A cross-platform CLI tool for running Hytale dedicated servers, written in Rust.

## Features

- **Java 25 auto-install** via Adoptium API
- **Server file download** via OAuth2 device flow (like official hytale-downloader)
- **Auto-update support** with exit code 8 handling and staged update application
- **Interactive console** with stdin/stdout passthrough
- **Cross-platform builds** for Linux, Windows, macOS (x86_64, arm64)

## Installation

### Download Pre-built Binary

Download the latest release for your platform from the [Releases](https://github.com/gameap/hytale-runner/releases) page.

### Build from Source

```bash
# Clone the repository
git clone https://github.com/gameap/hytale-runner.git
cd hytale-runner

# Build release binary
cargo build --release

# The binary will be at target/release/hytale-runner
```

## Usage

### Quick Start

```bash
# Download and run the Hytale server
hytale-runner run

# This will:
# 1. Install Java 25 if not found
# 2. Authenticate with Hytale (opens browser for OAuth2)
# 3. Download HytaleServer.jar and Assets.zip
# 4. Start the server
```

### Commands

```
hytale-runner [OPTIONS] <COMMAND>

Commands:
  run        Download (if needed) and run the Hytale server
  download   Download server files without running
  install    Install Java 25 via Adoptium
  update     Manage server updates
  version    Show version info

Options:
  -d, --dir <PATH>     Server directory (default: current)
  -c, --config <PATH>  Config file path
  -V, --verbose        Enable verbose output
  -h, --help           Print help
```

### Run Command

```bash
# Run with default settings
hytale-runner run

# Run with custom port and memory
hytale-runner run --port 5521 --memory 8G --min-memory 2G

# Run with custom Java path
hytale-runner run --java-path /path/to/java

# Run without auto-restart on updates
hytale-runner run --no-restart

# Run the latest version of the pre-release patchline
hytale-runner run --patchline pre-release

# Pin the server to a specific version
hytale-runner run --server-version 0.7.2

# Allow op commands and enable backups every 30 minutes
hytale-runner run --allow-op --backup --backup-dir backups --backup-frequency 30
```

With `--patchline` (or `patchline` in config), every start compares the installed version (`version.txt`)
with the latest version of the patchline and downloads the latest one if they differ, so switching the patchline
also switches the installed server. If the check or the download fails, the installed version is started.
The check is skipped when the version is pinned with `--server-version` or when `update.enabled` is `false`.
Without a patchline, server files are only downloaded if they are missing.

`--server-version` keeps the server on the given version: if it is not installed, the runner downloads it,
and staged updates are not applied. `hytale-downloader` can only download the latest version of a patchline,
so a version that is no longer the latest can only be used if it is already installed.
If `.hytale-runner.yaml` pins a different `server_version`, it is updated to the version from the flag
once that version is installed (the same applies to `download --server-version`).

`--accept-early-plugins`, `--allow-op` and `--backup` also take an explicit value: `--allow-op=true`
or `--allow-op=false`. An explicit value overrides the config file.

An empty argument and an option with an empty value (`--server-version=`) are ignored, as if they were not
passed. This lets panels like GameAP put every option into the start command and leave unset ones empty:

```bash
hytale-runner run --patchline=release --server-version= --allow-op=false --backup=true --jvm-args=
```

### Download Command

```bash
# Download server files
hytale-runner download

# Force re-download
hytale-runner download --force

# Download pre-release version
hytale-runner download --patchline pre-release

# Download only if the latest version matches
hytale-runner download --server-version 0.7.2
```

### Install Command

```bash
# Install Java 25
hytale-runner install java

# Install specific version
hytale-runner install java --version 21

# List installed Java versions
hytale-runner install java --list
```

### Update Command

```bash
# Check for updates
hytale-runner update check

# Apply staged update
hytale-runner update apply

# Show update status
hytale-runner update status
```

## Configuration

### Global Config

Location: `~/.config/hytale-runner/config.yaml` (Linux/macOS) or `%APPDATA%\hytale-runner\config.yaml` (Windows)

```yaml
defaults:
  memory_max: "4G"
  memory_min: "1G"
  port: 5520
  aot_enabled: true

java:
  auto_install: true
  # path: /path/to/custom/java

jvm:
  args: []

# OAuth2 tokens (auto-saved after authentication)
auth:
  access_token: ""
  refresh_token: ""
  expires_at: ""
```

### Per-Server Config

Location: `.hytale-runner.yaml` in the server directory

```yaml
port: 5520
memory_max: "8G"
assets: Assets.zip
patchline: release
# server_version: 0.7.2

java:
  path: ""

update:
  enabled: true # check for patchline updates on start
  auto_apply: false
```

## Hytale Server Requirements

| Requirement | Value |
|-------------|-------|
| Java Version | 25 (Adoptium recommended) |
| Default Port | 5520 (QUIC/UDP) |
| Required Files | `HytaleServer.jar`, `Assets.zip` |
| AOT Cache | `HytaleServer.aot` (optional, faster startup) |
| Config Files | `config.json`, `permissions.json`, `whitelist.json`, `bans.json` |

## Auto-Update Behavior

The Hytale server uses exit code 8 to signal that an update is ready. When this happens:

1. The runner detects exit code 8
2. It looks for staged files in `updater/staging/`
3. It copies the updated files to the server directory
4. It restarts the server automatically

Use `--no-restart` to disable this behavior and exit instead.

When the server version is pinned (`--server-version` or `server_version` in config), staged updates are skipped
and left in `updater/staging/`. Downloading server files removes `updater/staging/`, since a staged update
is older than the downloaded version.

## Building for Multiple Platforms

```bash
# Linux x86_64 (native)
cargo build --release --target x86_64-unknown-linux-gnu

# Linux ARM64 (requires cross)
cross build --release --target aarch64-unknown-linux-gnu

# Windows (native on Windows, or cross-compile)
cargo build --release --target x86_64-pc-windows-msvc

# macOS (native)
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin
```

## License

MIT License

## Related Projects

- [Hytale Server Manual](https://support.hytale.com/hc/en-us/articles/45326769420827-Hytale-Server-Manual)
- [minecraft-runner](https://github.com/gameap/minecraft-runner) - Similar tool for Minecraft servers
