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

    let mut mz_values = Vec::with_capacity(count as usize);
    let mut intensities = Vec::with_capacity(count as usize);

    for _ in 0..count {
        let mz = reader.read_f32()? as f64;
        let intensity = reader.read_f32()? as f64;
        mz_values.push(mz);
        intensities.push(intensity);
    }

    Ok((mz_values, intensities))
}
