//! Profile mode scan data decoding.
//!
//! Profile scans store continuous m/z-intensity traces. The encoding
//! typically involves segmented chunks with per-segment metadata.

use crate::RawError;

/// Decode profile data from a scan data packet.
///
/// Returns (mz_array, intensity_array).
pub fn decode_profile(
    _packet: &[u8],
    _offset: usize,
) -> Result<(Vec<f64>, Vec<f64>), RawError> {
    todo!("Implement based on FORMAT_SPEC.md / SCAN_DATA_ENCODING.md")
}
