//! RAW file version detection and handling.
//!
//! Thermo RAW files have version numbers typically in the range v57-v66+.
//! The version determines the exact layout of internal structures.

/// Minimum supported RAW file version.
pub const MIN_SUPPORTED_VERSION: u32 = 57;
/// Maximum supported RAW file version.
pub const MAX_SUPPORTED_VERSION: u32 = 66;

/// Finnigan file header magic number.
pub const FINNIGAN_MAGIC: u16 = 0xA101;

/// Check whether a RAW file version is supported.
pub fn is_supported(version: u32) -> bool {
    version >= MIN_SUPPORTED_VERSION && version <= MAX_SUPPORTED_VERSION
}

/// Size of a ScanIndexEntry for a given version.
pub fn scan_index_entry_size(version: u32) -> usize {
    if version >= 66 {
        88
    } else if version >= 64 {
        80
    } else {
        72
    }
}

/// Whether the version uses 64-bit addresses.
pub fn uses_64bit_addresses(version: u32) -> bool {
    version >= 64
}

/// Size of ScanEventPreamble for a given version.
pub fn scan_event_preamble_size(version: u32) -> usize {
    if version >= 66 {
        132
    } else if version >= 63 {
        128
    } else if version >= 62 {
        120
    } else if version >= 57 {
        80
    } else {
        41
    }
}
