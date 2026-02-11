//! Centroid mode scan data decoding.
//!
//! Centroid scans store discrete (m/z, intensity) pairs, often with
//! additional per-peak metadata (noise, resolution, charge, etc.).

use crate::RawError;

/// Decode centroid data from a scan data packet.
///
/// Returns (mz_array, intensity_array).
pub fn decode_centroid(
    _packet: &[u8],
    _offset: usize,
) -> Result<(Vec<f64>, Vec<f64>), RawError> {
    todo!("Implement based on FORMAT_SPEC.md / SCAN_DATA_ENCODING.md")
}
