//! Thin wrapper around the `cfb` crate for Thermo RAW OLE2 container access.

use cfb::CompoundFile;
use std::io::{self, Read, Seek};
use std::path::Path;

pub struct Ole2Container<F: Read + Seek> {
    cf: CompoundFile<F>,
}

impl Ole2Container<std::fs::File> {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = std::fs::File::open(path)?;
        let cf = CompoundFile::open(file)?;
        Ok(Self { cf })
    }
}

impl<F: Read + Seek> Ole2Container<F> {
    /// Create from an existing Read+Seek source.
    pub fn from_reader(reader: F) -> io::Result<Self> {
        let cf = CompoundFile::open(reader)?;
        Ok(Self { cf })
    }

    /// List all stream paths in the container.
    pub fn list_streams(&self) -> Vec<String> {
        self.cf
            .walk()
            .filter(|e| !e.is_storage())
            .map(|e| e.path().to_string_lossy().into_owned())
            .collect()
    }

    /// Read the entire contents of a stream.
    pub fn read_stream(&mut self, path: &str) -> io::Result<Vec<u8>> {
        let mut stream = self.cf.open_stream(path)?;
        let mut buf = Vec::new();
        stream.read_to_end(&mut buf)?;
        Ok(buf)
    }

    /// Get the size of a stream in bytes.
    pub fn stream_len(&self, path: &str) -> Option<u64> {
        self.cf.entry(path).ok().map(|e| e.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_api_compiles() {
        // Smoke test: ensure the API compiles.
        // Real tests require actual RAW files.
        let _ = std::mem::size_of::<Ole2Container<std::fs::File>>();
    }
}
