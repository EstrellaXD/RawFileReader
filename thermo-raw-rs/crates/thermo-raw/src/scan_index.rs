//! ScanIndex parsing (offset table).
//!
//! The scan index maps scan numbers to their byte offsets in the scan data
//! stream, along with lightweight per-scan metadata (RT, TIC, etc.).
//!
//! Entry sizes: v57-63 = 72 bytes, v64 = 80 bytes, v66 = 88 bytes.

use crate::io_utils::BinaryReader;
use crate::version;
use crate::RawError;

/// A single entry in the scan index.
#[derive(Debug, Clone)]
pub struct ScanIndexEntry {
    /// Byte offset into the scan data stream.
    pub offset: u64,
    /// Scan number / index.
    pub index: u32,
    /// Scan event index.
    pub scan_event: u16,
    /// Scan segment number.
    pub scan_segment: u16,
    /// Scan data size in bytes.
    pub data_size: u32,
    /// Retention time in minutes.
    pub rt: f64,
    /// Total ion current.
    pub tic: f64,
    /// Base peak intensity.
    pub base_peak_intensity: f64,
    /// Base peak m/z.
    pub base_peak_mz: f64,
    /// Scan low m/z.
    pub low_mz: f64,
    /// Scan high m/z.
    pub high_mz: f64,
}

/// Parse the entire scan index from the data stream.
///
/// `data` is the full file data. `offset` is the absolute address of the scan index.
/// Returns one `ScanIndexEntry` per scan.
pub fn parse_scan_index(
    data: &[u8],
    offset: u64,
    version: u32,
    n_scans: u32,
) -> Result<Vec<ScanIndexEntry>, RawError> {
    let entry_size = version::scan_index_entry_size(version);
    let mut reader = BinaryReader::at_offset(data, offset);
    let mut entries = Vec::with_capacity(n_scans as usize);

    for _ in 0..n_scans {
        let entry_start = reader.position();

        // Common fields (72 bytes)
        let offset_32 = reader.read_u32()?;
        let index = reader.read_u32()?;
        let scan_event = reader.read_u16()?;
        let scan_segment = reader.read_u16()?;
        let _next = reader.read_u32()?;
        let _unknown = reader.read_u32()?;
        let data_size = reader.read_u32()?;
        let rt = reader.read_f64()?;
        let tic = reader.read_f64()?;
        let base_peak_intensity = reader.read_f64()?;
        let base_peak_mz = reader.read_f64()?;
        let low_mz = reader.read_f64()?;
        let high_mz = reader.read_f64()?;

        // Version-dependent extra fields
        let scan_offset = if version >= 64 {
            // v64+: 64-bit offset at byte 72
            let offset_64 = reader.read_u64()?;
            if version >= 66 {
                // v66: two additional unknown u32s
                let _unknown1 = reader.read_u32()?;
                let _unknown2 = reader.read_u32()?;
            }
            offset_64
        } else {
            offset_32 as u64
        };

        // Ensure we consumed exactly entry_size bytes
        let expected_end = entry_start + entry_size as u64;
        if reader.position() != expected_end {
            reader.set_position(expected_end);
        }

        entries.push(ScanIndexEntry {
            offset: scan_offset,
            index,
            scan_event,
            scan_segment,
            data_size,
            rt,
            tic,
            base_peak_intensity,
            base_peak_mz,
            low_mz,
            high_mz,
        });
    }

    Ok(entries)
}
