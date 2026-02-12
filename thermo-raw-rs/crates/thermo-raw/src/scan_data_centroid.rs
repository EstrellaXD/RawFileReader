//! Centroid mode scan data decoding.
//!
//! Peak list format:
//! - count (u32): number of peaks
//! - For each peak: mz (f32), intensity (f32) -- interleaved pairs

use crate::io_utils::BinaryReader;
use crate::RawError;

/// Decode centroid data from a scan data packet.
///
/// Returns (mz_array, intensity_array).
/// Uses batch slice read for peak data to minimize per-element overhead.
pub fn decode_centroid(data: &[u8], offset: usize) -> Result<(Vec<f64>, Vec<f64>), RawError> {
    let mut reader = BinaryReader::at_offset(data, offset as u64);

    let count = reader.read_u32()?;

    // Sanity check
    if count > 10_000_000 {
        return Err(RawError::ScanDecodeError {
            offset,
            reason: format!("centroid data has unreasonable peak count: {}", count),
        });
    }

    if count == 0 {
        return Ok((vec![], vec![]));
    }

    let mut mz_values = Vec::with_capacity(count as usize);
    let mut intensities = Vec::with_capacity(count as usize);

    // Batch read: get all peak data as a single slice (8 bytes per peak: f32 mz + f32 int)
    let total_bytes = count as usize * 8;
    let raw_slice = reader.slice(total_bytes)?;

    for i in 0..count as usize {
        let base = i * 8;
        let mz_bytes = [
            raw_slice[base],
            raw_slice[base + 1],
            raw_slice[base + 2],
            raw_slice[base + 3],
        ];
        let int_bytes = [
            raw_slice[base + 4],
            raw_slice[base + 5],
            raw_slice[base + 6],
            raw_slice[base + 7],
        ];
        mz_values.push(f32::from_le_bytes(mz_bytes) as f64);
        intensities.push(f32::from_le_bytes(int_bytes) as f64);
    }

    Ok((mz_values, intensities))
}
