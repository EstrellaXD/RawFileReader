//! TrailerExtra parsing (scan-level metadata).
//!
//! The trailer extra stream stores per-scan key-value metadata such as
//! injection time, AGC targets, charge state, monoisotopic m/z, etc.

use std::collections::HashMap;

use crate::RawError;

/// Parsed trailer extra data for a single scan.
pub type TrailerExtra = HashMap<String, String>;

/// Parse trailer extra data for a specific scan.
pub fn parse_trailer_extra(
    _data: &[u8],
    _version: u32,
    _scan_number: u32,
) -> Result<TrailerExtra, RawError> {
    todo!("Implement based on FORMAT_SPEC.md (Phase 2 output)")
}

/// Get the list of trailer extra field labels.
pub fn parse_trailer_fields(_data: &[u8], _version: u32) -> Result<Vec<String>, RawError> {
    todo!("Implement based on FORMAT_SPEC.md (Phase 2 output)")
}
