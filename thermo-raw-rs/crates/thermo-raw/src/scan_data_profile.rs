//! Profile mode scan data decoding.
//!
//! Profile data format (from PacketHeader):
//! - Profile header: first_value (f64), step (f64), peak_count (u32), nbins (u32)
//! - Followed by peak_count ProfileChunk structures
//!
//! Each ProfileChunk:
//!   layout == 0: first_bin (u32), nbins (u32), signal[nbins] (f32[])
//!   layout > 0:  first_bin (u32), nbins (u32), fudge (f32), signal[nbins] (f32[])
//!
//! m/z for bin i = first_value + (chunk.first_bin + i) * step

use crate::io_utils::BinaryReader;
use crate::RawError;

/// Decode profile data from a scan data packet.
///
/// Returns (mz_array, intensity_array) with one entry per bin across all chunks.
/// Uses batch slice reads for signal data to minimize per-element overhead.
pub fn decode_profile(
    data: &[u8],
    offset: usize,
    layout: u32,
) -> Result<(Vec<f64>, Vec<f64>), RawError> {
    let mut reader = BinaryReader::at_offset(data, offset as u64);

    // Profile header
    let first_value = reader.read_f64()?;
    let step = reader.read_f64()?;
    let peak_count = reader.read_u32()?;
    let nbins_total = reader.read_u32()?;

    if peak_count == 0 || nbins_total == 0 {
        return Ok((vec![], vec![]));
    }

    // Sanity check
    if peak_count > 1_000_000 || nbins_total > 100_000_000 {
        return Err(RawError::ScanDecodeError {
            offset,
            reason: format!(
                "profile data has unreasonable dimensions: peak_count={}, nbins={}",
                peak_count, nbins_total
            ),
        });
    }

    let mut mz_values = Vec::with_capacity(nbins_total as usize);
    let mut intensities = Vec::with_capacity(nbins_total as usize);

    for _ in 0..peak_count {
        let first_bin = reader.read_u32()?;
        let chunk_nbins = reader.read_u32()?;

        // Layout > 0 has a fudge factor (instrument drift correction)
        let _fudge = if layout > 0 {
            Some(reader.read_f32()?)
        } else {
            None
        };

        // Batch read: get raw bytes for all signal values at once
        let signal_bytes = chunk_nbins as usize * 4;
        let raw_slice = reader.slice(signal_bytes)?;

        for i in 0..chunk_nbins as usize {
            let bytes = [
                raw_slice[i * 4],
                raw_slice[i * 4 + 1],
                raw_slice[i * 4 + 2],
                raw_slice[i * 4 + 3],
            ];
            let signal = f32::from_le_bytes(bytes);
            let bin_index = first_bin as u64 + i as u64;
            let mz = first_value + (bin_index as f64) * step;
            mz_values.push(mz);
            intensities.push(signal as f64);
        }

        // Advance reader past the signal data
        reader.skip(signal_bytes)?;
    }

    Ok((mz_values, intensities))
}
