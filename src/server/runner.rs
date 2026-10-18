use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use anstream::adapter::StripBytes;
use anyhow::{Context, Result};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::process::Command;
use tokio::signal;
use tokio::task::JoinHandle;
use tracing::{debug, info};

use crate::config::AppConfig;

/// How long to wait for the rest of the server output after the server exits
const OUTPUT_DRAIN_TIMEOUT: Duration = Duration::from_secs(5);

/// Runs the Hytale server process
pub struct ServerRunner {
    config: AppConfig,
    server_dir: PathBuf,
    java_path: PathBuf,
    jar_path: PathBuf,
    aot_path: PathBuf,
    color: bool,
}

impl ServerRunner {
    pub fn new(
        config: AppConfig,
        server_dir: PathBuf,
        java_path: PathBuf,
        jar_path: PathBuf,
        aot_path: PathBuf,
        color: bool,
    ) -> Self {
        Self {
            config,
            server_dir,
            java_path,
            jar_path,
            aot_path,
            color,
        }
    }

    /// Run the server and return the exit code
    #[allow(clippy::too_many_arguments)]
    pub async fn run(
        &self,
        memory_min: &str,
        memory_max: &str,
        aot_enabled: bool,
        port: u16,
        ip: &str,
        assets_path: &PathBuf,
        jvm_args: Option<&str>,
        accept_early_plugins: bool,
        allow_op: bool,
        backup_enabled: bool,
        backup_dir: Option<&PathBuf>,
        backup_frequency: Option<u16>,
    ) -> Result<i32> {
        if !self.jar_path.exists() {
            anyhow::bail!("HytaleServer.jar not found at {}", self.jar_path.display());
        }

        if !assets_path.exists() {
            anyhow::bail!("Assets.zip not found at {}", assets_path.display());
        }

        let mut cmd = Command::new(&self.java_path);
        cmd.current_dir(&self.server_dir);

        // JVM memory arguments
        cmd.arg(format!("-Xms{}", memory_min));
        cmd.arg(format!("-Xmx{}", memory_max));

        // AOT cache if enabled and exists
        if aot_enabled && self.aot_path.exists() {
            info!("Using AOT cache: {}", self.aot_path.display());
            cmd.arg(format!("-XX:AOTCache={}", self.aot_path.display()));
        }

        // Additional JVM arguments from config
        for arg in &self.config.jvm.args {
            cmd.arg(arg);
        }

        // Additional JVM arguments from command line
        if let Some(args) = jvm_args {
            for arg in args.split_whitespace() {
                cmd.arg(arg);
            }
        }

        // Server arguments
        cmd.arg("-jar").arg(&self.jar_path);
        cmd.arg("--assets").arg(assets_path);
        cmd.arg("--bind").arg(format!("{}:{}", ip, port));

        // Early plugins acceptance
        if accept_early_plugins {
            cmd.arg("--accept-early-plugins");
        }

        // Allow op
        if allow_op {
            cmd.arg("--allow-op");
        }

        // Backup settings
        if backup_enabled {
            cmd.arg("--backup");
            if let Some(dir) = backup_dir {
                cmd.arg("--backup-dir").arg(dir);
            }
            if let Some(freq) = backup_frequency {
                cmd.arg("--backup-frequency").arg(freq.to_string());
            }
        }

        // Interactive console - stdin is always inherited. Without colors the output goes
        // through the runner, which removes the ANSI escape sequences the server prints anyway.
        cmd.stdin(Stdio::inherit());
        if self.color {
            cmd.stdout(Stdio::inherit());
            cmd.stderr(Stdio::inherit());
        } else {
            cmd.stdout(Stdio::piped());
            cmd.stderr(Stdio::piped());
        }

        debug!("Starting server with command: {:?}", cmd);
        info!("Starting Hytale server on {}:{}...", ip, port);

        let mut child = cmd.spawn().context("Failed to start server process")?;

        let mut output = Vec::new();
        if let Some(stdout) = child.stdout.take() {
            output.push(forward_stripped(stdout, tokio::io::stdout()));
        }
        if let Some(stderr) = child.stderr.take() {
            output.push(forward_stripped(stderr, tokio::io::stderr()));
        }

        // Handle Ctrl+C gracefully
        let exit_code = tokio::select! {
            status = child.wait() => {
                match status {
                    Ok(status) => status.code().unwrap_or(1),
                    Err(e) => {
                        tracing::error!("Failed to wait for server process: {}", e);
                        1
                    }
                }
            }
            _ = signal::ctrl_c() => {
                info!("Received Ctrl+C, shutting down server...");

                // Try to kill the process gracefully
                #[cfg(unix)]
                {
                    use tokio::process::Command as TokioCommand;
                    if let Some(pid) = child.id() {
                        // Send SIGTERM first
                        let _ = TokioCommand::new("kill")
                            .arg("-TERM")
                            .arg(pid.to_string())
                            .output()
                            .await;

                        // Wait a bit for graceful shutdown
                        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                    }
                }

                // Force kill if still running
                let _ = child.kill().await;
                0
            }
        };

        // A process left running by the server can keep its output pipes open
        let _ = tokio::time::timeout(OUTPUT_DRAIN_TIMEOUT, futures_util::future::join_all(output))
            .await;

        info!("Server exited with code {}", exit_code);
        Ok(exit_code)
    }
}

/// Forward server output to `writer` without ANSI escape sequences
fn forward_stripped<R, W>(reader: R, writer: W) -> JoinHandle<()>
where
    R: AsyncRead + Unpin + Send + 'static,
    W: AsyncWrite + Unpin + Send + 'static,
{
    tokio::spawn(async move {
        if let Err(e) = copy_stripped(reader, writer).await {
            debug!("Failed to forward server output: {}", e);
        }
    })
}

/// Copy `reader` to `writer` without ANSI escape sequences. The reader is read to the end
/// even after a write error, so the server never blocks on a full output pipe.
async fn copy_stripped<R, W>(mut reader: R, mut writer: W) -> std::io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut stripper = StripBytes::new();
    let mut buf = [0u8; 8192];
    let mut stripped = Vec::with_capacity(buf.len());
    let mut write_error = None;

    loop {
        let n = reader.read(&mut buf).await?;
        if n == 0 {
            return write_error.map_or(Ok(()), Err);
        }
        if write_error.is_some() {
            continue;
        }

        // One write per read keeps lines whole when stdout and stderr share a terminal
        stripped.clear();
        for printable in stripper.strip_next(&buf[..n]) {
            stripped.extend_from_slice(printable);
        }

        let result = async {
            writer.write_all(&stripped).await?;
            writer.flush().await
        }
        .await;
        write_error = result.err();
    }
}

#[cfg(test)]
mod tests {
    use std::io;
    use std::pin::Pin;
    use std::task::{Context, Poll};

    use super::*;

    async fn strip(chunks: &[&[u8]]) -> Vec<u8> {
        let mut reader: Box<dyn AsyncRead + Unpin> = Box::new(&b""[..]);
        for chunk in chunks.iter().rev() {
            reader = Box::new(chunk.chain(reader));
        }

        let mut output = Vec::new();
        copy_stripped(reader, &mut output).await.unwrap();
        output
    }

    #[tokio::test]
    async fn test_strips_colors() {
        let output = strip(&[b"[2026/10/18 17:29:19 \x1b[33m  WARN\x1b[m] Unused key\r\n"]).await;
        assert_eq!(output, b"[2026/10/18 17:29:19   WARN] Unused key\r\n");
    }

    #[tokio::test]
    async fn test_strips_sequences_split_between_reads() {
        let output = strip(&[b"\x1b[3", b"3mWARN\x1b[", b"0m\tdone\n"]).await;
        assert_eq!(output, b"WARN\tdone\n");
    }

    #[tokio::test]
    async fn test_keeps_utf8_split_between_reads() {
        let text = "Привет, мир\n".as_bytes();
        let output = strip(&[&text[..3], &text[3..]]).await;
        assert_eq!(output, text);
    }

    struct BrokenWriter;

    impl AsyncWrite for BrokenWriter {
        fn poll_write(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: &[u8],
        ) -> Poll<io::Result<usize>> {
            Poll::Ready(Err(io::ErrorKind::BrokenPipe.into()))
        }

        fn poll_flush(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<io::Result<()>> {
            Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn test_reads_to_end_after_write_error() {
        let data = vec![b'x'; 100_000];
        let mut reader = &data[..];

        let result = copy_stripped(&mut reader, BrokenWriter).await;

        assert_eq!(result.unwrap_err().kind(), io::ErrorKind::BrokenPipe);
        assert!(reader.is_empty());
    }
}
