use anyhow::{Context, Result, bail};
use path_clean::PathClean;
use serde::{Serialize, de::DeserializeOwned};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub fn absolute(path: impl AsRef<Path>) -> Result<PathBuf> {
    let path = path.as_ref();
    Ok(if path.is_absolute() {
        path.to_owned()
    } else {
        std::env::current_dir()?.join(path)
    }
    .clean())
}

pub fn same(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    } else {
        a == b
    }
}

pub fn exists(path: &Path) -> Result<bool> {
    match fs::symlink_metadata(path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("Cannot inspect {}", path.display())),
    }
}

pub fn directory(path: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(path)
        .with_context(|| format!("Cannot read directory {}", path.display()))?;
    if !metadata.file_type().is_dir() {
        bail!("Use a regular directory, not a link: {}", path.display());
    }
    Ok(())
}

pub fn read_json<T: DeserializeOwned>(path: &Path) -> Result<T> {
    let metadata =
        fs::symlink_metadata(path).with_context(|| format!("Cannot read {}", path.display()))?;
    if !metadata.file_type().is_file() || metadata.len() > 128 * 1024 {
        bail!("Invalid configuration file: {}", path.display());
    }
    let bytes = fs::read(path)?;
    let bytes = bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(&bytes);
    serde_json::from_slice(bytes).with_context(|| format!("Invalid JSON in {}", path.display()))
}

pub fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    if exists(path)? && !fs::symlink_metadata(path)?.file_type().is_file() {
        bail!("Refusing to replace a non-regular file: {}", path.display());
    }
    let parent = path.parent().context("File has no parent directory")?;
    let mut temp = tempfile::NamedTempFile::new_in(parent)?;
    serde_json::to_writer_pretty(temp.as_file_mut(), value)?;
    temp.write_all(b"\n")?;
    temp.as_file().sync_all()?;
    temp.persist(path)
        .with_context(|| format!("Cannot save {}", path.display()))?;
    Ok(())
}

pub fn portable_name(value: &str) -> bool {
    let first = value
        .as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric);
    let chars = value
        .bytes()
        .all(|c| c.is_ascii_alphanumeric() || b"._-".contains(&c));
    let stem = value.split('.').next().unwrap_or("").to_ascii_lowercase();
    let reserved = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || ["com", "lpt"].iter().any(|prefix| {
            stem.strip_prefix(prefix)
                .is_some_and(|n| n.len() == 1 && matches!(n.as_bytes()[0], b'1'..=b'9'))
        });
    first && chars && !value.ends_with('.') && !reserved
}
