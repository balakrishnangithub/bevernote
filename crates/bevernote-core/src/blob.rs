use std::{
    fs::{self, File},
    io::{BufReader, BufWriter, Read, Write},
    path::{Path, PathBuf},
};

use md5::Context as Md5;
use sha2::{Digest as _, Sha256};
use uuid::Uuid;

use crate::{db::Library, error::Result, BeverError};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredBlob {
    pub sha256: String,
    pub md5: String,
    pub size: u64,
}

struct BlobWriter {
    file: BufWriter<File>,
    sha256: Sha256,
    md5: Md5,
    size: u64,
}

impl Write for BlobWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let written = self.file.write(buf)?;
        self.sha256.update(&buf[..written]);
        self.md5.consume(&buf[..written]);
        self.size += written as u64;
        Ok(written)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

impl Library {
    pub fn store_blob(&self, bytes: &[u8]) -> Result<StoredBlob> {
        self.store_blob_reader(&mut BufReader::new(bytes))
    }

    pub fn store_blob_file(&self, path: impl AsRef<Path>) -> Result<StoredBlob> {
        let mut reader = BufReader::new(File::open(path)?);
        self.store_blob_reader(&mut reader)
    }

    pub fn store_blob_reader(&self, reader: &mut impl Read) -> Result<StoredBlob> {
        let staging = self.blob_dir.join(format!(".{}", Uuid::new_v4()));
        let file = File::create(&staging)?;
        let mut writer = BlobWriter {
            file: BufWriter::new(file),
            sha256: Sha256::new(),
            md5: Md5::new(),
            size: 0,
        };
        std::io::copy(reader, &mut writer)?;
        writer.flush()?;
        let stored = StoredBlob {
            sha256: format!("{:x}", writer.sha256.finalize()),
            md5: format!("{:x}", writer.md5.compute()),
            size: writer.size,
        };
        let destination = self.blob_path(&stored.sha256)?;
        if let Some(parent) = destination.parent() {
            fs::create_dir_all(parent)?;
        }
        if destination.exists() {
            fs::remove_file(&staging)?;
        } else if let Err(error) = fs::rename(&staging, &destination) {
            if destination.exists() {
                fs::remove_file(&staging)?;
            } else {
                return Err(error.into());
            }
        }
        Ok(stored)
    }

    pub fn blob_path(&self, sha256: &str) -> Result<PathBuf> {
        if sha256.len() != 64 || !sha256.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Err(BeverError::Invalid("invalid blob SHA-256".into()));
        }
        Ok(self
            .blob_dir
            .join(&sha256[0..2])
            .join(&sha256[2..4])
            .join(sha256))
    }

    pub fn read_blob(&self, sha256: &str) -> Result<Vec<u8>> {
        Ok(fs::read(self.blob_path(sha256)?)?)
    }

    pub fn open_blob(&self, sha256: &str) -> Result<File> {
        Ok(File::open(self.blob_path(sha256)?)?)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn blobs_are_verifiable_and_deduplicated() {
        let directory = tempdir().unwrap();
        let library = Library::open_at(
            directory.path().join("db.sqlite"),
            directory.path().join("blobs"),
        )
        .unwrap();
        let first = library.store_blob(b"bevernote").unwrap();
        let second = library.store_blob(b"bevernote").unwrap();
        assert_eq!(first, second);
        assert_eq!(library.read_blob(&first.sha256).unwrap(), b"bevernote");
        assert_eq!(first.md5, "71038a65e6a98786929c915cf10fc72d");
    }
}
