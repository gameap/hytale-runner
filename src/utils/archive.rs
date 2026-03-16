use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use anyhow::{Context, Result};
use flate2::read::GzDecoder;
use tar::Archive;
use tracing::debug;

/// Extract a tar.gz archive to a destination directory
pub fn extract_tar_gz(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    debug!(
        "Extracting {} to {}",
        archive_path.display(),
        dest_dir.display()
    );

    let file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

    let reader = BufReader::new(file);
    let gz_decoder = GzDecoder::new(reader);
    let mut archive = Archive::new(gz_decoder);

    archive
        .unpack(dest_dir)
        .with_context(|| format!("Failed to extract archive to {}", dest_dir.display()))?;

    debug!("Extraction complete");
    Ok(())
}

/// Extract a zip archive to a destination directory
pub fn extract_zip(archive_path: &Path, dest_dir: &Path) -> Result<()> {
    debug!(
        "Extracting {} to {}",
        archive_path.display(),
        dest_dir.display()
    );

    let file = File::open(archive_path)
        .with_context(|| format!("Failed to open archive: {}", archive_path.display()))?;

    let reader = BufReader::new(file);
    let mut archive = zip::ZipArchive::new(reader)
        .with_context(|| format!("Failed to read zip archive: {}", archive_path.display()))?;

    for i in 0..archive.len() {
        let mut file = archive.by_index(i).context("Failed to read zip entry")?;

        let outpath = match file.enclosed_name() {
            Some(path) => dest_dir.join(path),
            None => continue,
        };

        if file.is_dir() {
            std::fs::create_dir_all(&outpath)
                .with_context(|| format!("Failed to create directory: {}", outpath.display()))?;
        } else {
            if let Some(parent) = outpath.parent() {
                if !parent.exists() {
                    std::fs::create_dir_all(parent).with_context(|| {
                        format!("Failed to create parent directory: {}", parent.display())
                    })?;
                }
            }

            let mut outfile = File::create(&outpath)
                .with_context(|| format!("Failed to create file: {}", outpath.display()))?;

            std::io::copy(&mut file, &mut outfile).with_context(|| {
                format!("Failed to extract file: {}", outpath.display())
            })?;
        }

        // Set permissions on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            if let Some(mode) = file.unix_mode() {
                std::fs::set_permissions(&outpath, std::fs::Permissions::from_mode(mode)).ok();
            }
        }
    }

    debug!("Extraction complete");
    Ok(())
}
