use flate2::read::GzDecoder;
use std::fs::File;
use std::io::{self, BufReader, Seek};
use std::path::Path;
use tar::Archive;

pub fn extract_tar_gz<P: AsRef<Path>>(path: P, output_dir: &Path) -> io::Result<()> {
    let mut reader = BufReader::new(File::open(path)?);
    let mut topmost_dir: Option<String> = None;

    {
        // First pass: Determine the topmost directory
        let mut archive = Archive::new(GzDecoder::new(&mut reader));
        for entry in archive.entries()? {
            let entry = entry?;
            let path = entry.path()?;
            let path_str = path.to_string_lossy().to_string();

            if path_str.ends_with(std::path::MAIN_SEPARATOR_STR) {
                if topmost_dir.is_none() {
                    topmost_dir = Some(path.to_string_lossy().to_string());
                } else {
                    // If there is more than one directory, check that it's a child of the topmost.
                    if path_str.starts_with(topmost_dir.as_ref().unwrap()) {
                        // This directory is a child of the topmost.
                        continue;
                    }
                    topmost_dir = None;
                    break;
                }
            } else {
                // If we encounter a file, we can stop looking for a topmost directory
                if let Some(topmost_dir) = topmost_dir.as_ref() {
                    if path_str.starts_with(topmost_dir) {
                        // This file is a child of the topmost.
                        continue;
                    }
                }
                topmost_dir = None;
                break;
            }
        }
    }

    // Reset reader
    reader.seek(io::SeekFrom::Start(0))?;
    let mut archive = Archive::new(GzDecoder::new(&mut reader));

    // Second pass: Extract entries, stripping the topmost directory if it exists
    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;

        // If we have a topmost directory, strip it from the path
        if let Some(ref top_dir) = topmost_dir {
            if let Ok(stripped_path) = path.strip_prefix(top_dir) {
                let output_path = output_dir.join(stripped_path);
                // Create parent directories if necessary
                if let Some(parent) = output_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                // Extract the file
                entry.unpack(output_path)?;
            }
        } else {
            // If there's no topmost directory, extract normally
            let output_path = output_dir.join(path);
            // Create parent directories if necessary
            if let Some(parent) = output_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            // Extract the file
            entry.unpack(output_path)?;
        }
    }

    Ok(())
}
