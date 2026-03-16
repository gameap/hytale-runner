use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use tracing::debug;

/// Calculate SHA256 hash of a file
pub fn calculate_sha256(path: &Path) -> Result<String> {
    debug!("Calculating SHA256 for {}", path.display());

    let file = File::open(path)
        .with_context(|| format!("Failed to open file for hashing: {}", path.display()))?;

    let mut reader = BufReader::new(file);
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let bytes_read = reader
            .read(&mut buffer)
            .with_context(|| format!("Failed to read file: {}", path.display()))?;

        if bytes_read == 0 {
            break;
        }

        hasher.update(&buffer[..bytes_read]);
    }

    let hash = hasher.finalize();
    let hex = hex::encode(hash);

    debug!("SHA256: {}", hex);
    Ok(hex)
}

/// Verify SHA256 hash of a file
pub fn verify_sha256(path: &Path, expected: &str) -> Result<()> {
    let actual = calculate_sha256(path)?;

    // Normalize to lowercase for comparison
    let expected_lower = expected.to_lowercase();
    let actual_lower = actual.to_lowercase();

    if expected_lower != actual_lower {
        anyhow::bail!(
            "Checksum mismatch for {}\n  Expected: {}\n  Actual:   {}",
            path.display(),
            expected_lower,
            actual_lower
        );
    }

    debug!("Checksum verified for {}", path.display());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_sha256_calculation() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"Hello, World!").unwrap();
        file.flush().unwrap();

        let hash = calculate_sha256(file.path()).unwrap();

        // Known SHA256 of "Hello, World!"
        assert_eq!(
            hash.to_lowercase(),
            "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f"
        );
    }

    #[test]
    fn test_verify_sha256_success() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"Hello, World!").unwrap();
        file.flush().unwrap();

        let result = verify_sha256(
            file.path(),
            "dffd6021bb2bd5b0af676290809ec3a53191dd81c7f70a4b28688a362182986f",
        );

        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_sha256_failure() {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(b"Hello, World!").unwrap();
        file.flush().unwrap();

        let result = verify_sha256(
            file.path(),
            "0000000000000000000000000000000000000000000000000000000000000000",
        );

        assert!(result.is_err());
    }
}
