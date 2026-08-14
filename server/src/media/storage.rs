//! Media storage abstraction: files live under a root directory, addressed
//! by a relative path (e.g. `ab/abcd1234...mp3`). The trait exists so a
//! future S3 backend can slot in without touching callers.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub trait Storage: Send + Sync {
    fn root(&self) -> &Path;
    /// Write `bytes` to `rel` (creating parent directories).
    fn write(&self, rel: &str, bytes: &[u8]) -> io::Result<()>;
    fn read(&self, rel: &str) -> io::Result<Vec<u8>>;
    fn delete(&self, rel: &str) -> io::Result<()>;
    fn exists(&self, rel: &str) -> bool;
    /// Absolute filesystem path for `rel` (used by the streaming endpoint).
    fn abs_path(&self, rel: &str) -> PathBuf;
}

/// Plain local-filesystem storage under a configured root directory.
pub struct LocalStorage {
    root: PathBuf,
}

impl LocalStorage {
    pub fn new(root: PathBuf) -> Self {
        fs::create_dir_all(&root).expect("media root must be creatable");
        Self { root }
    }
}

impl Storage for LocalStorage {
    fn root(&self) -> &Path {
        &self.root
    }

    fn write(&self, rel: &str, bytes: &[u8]) -> io::Result<()> {
        let path = self.root.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, bytes)
    }

    fn read(&self, rel: &str) -> io::Result<Vec<u8>> {
        fs::read(self.root.join(rel))
    }

    fn delete(&self, rel: &str) -> io::Result<()> {
        fs::remove_file(self.root.join(rel))
    }

    fn exists(&self, rel: &str) -> bool {
        self.root.join(rel).is_file()
    }

    fn abs_path(&self, rel: &str) -> PathBuf {
        self.root.join(rel)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_storage_roundtrip() {
        let dir = std::env::temp_dir().join(format!("crabcast-storage-{}", uuid::Uuid::new_v4()));
        let storage = LocalStorage::new(dir.clone());

        assert!(!storage.exists("ab/abcd1234.mp3"));
        storage.write("ab/abcd1234.mp3", b"audio bytes").unwrap();
        assert!(storage.exists("ab/abcd1234.mp3"));
        assert_eq!(storage.read("ab/abcd1234.mp3").unwrap(), b"audio bytes");
        assert_eq!(
            storage.abs_path("ab/abcd1234.mp3"),
            dir.join("ab/abcd1234.mp3")
        );

        storage.delete("ab/abcd1234.mp3").unwrap();
        assert!(!storage.exists("ab/abcd1234.mp3"));

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn nested_paths_create_parent_dirs() {
        let dir = std::env::temp_dir().join(format!("crabcast-storage-{}", uuid::Uuid::new_v4()));
        let storage = LocalStorage::new(dir.clone());
        storage.write("de/fedcba9876.flac", b"x").unwrap();
        assert!(dir.join("de").is_dir());
        let _ = fs::remove_dir_all(dir);
    }
}
