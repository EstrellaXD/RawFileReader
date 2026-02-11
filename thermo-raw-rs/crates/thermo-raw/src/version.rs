//! RAW file version detection.
//!
//! Thermo RAW files have version numbers typically in the range v57-v66+.
//! The version determines the exact layout of internal structures.

/// Supported RAW file version range.
pub const MIN_SUPPORTED_VERSION: u32 = 57;
pub const MAX_SUPPORTED_VERSION: u32 = 66;

/// Check whether a RAW file version is supported.
pub fn is_supported(version: u32) -> bool {
    version >= MIN_SUPPORTED_VERSION && version <= MAX_SUPPORTED_VERSION
}
