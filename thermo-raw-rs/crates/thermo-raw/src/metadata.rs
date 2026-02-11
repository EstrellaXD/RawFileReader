//! File metadata parsing (sample info, instrument info).

use crate::types::FileMetadata;
use crate::RawError;

/// Parse file metadata from the relevant OLE2 streams.
pub fn parse_metadata(_data: &[u8], _version: u32) -> Result<FileMetadata, RawError> {
    todo!("Implement based on FORMAT_SPEC.md (Phase 2 output)")
}
