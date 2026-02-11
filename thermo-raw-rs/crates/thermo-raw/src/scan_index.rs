//! ScanIndex parsing (offset table).
//!
//! The scan index maps scan numbers to their byte offsets in the scan data
//! stream, along with lightweight per-scan metadata (RT, TIC, etc.).

/// A single entry in the scan index.
#[derive(Debug, Clone)]
pub struct ScanIndexEntry {
    /// Byte offset into the scan data stream.
    pub offset: u64,
    /// Scan data length in bytes.
    pub length: u32,
    /// Retention time in minutes.
    pub rt: f64,
    /// Total ion current.
    pub tic: f64,
    /// Base peak m/z.
    pub base_peak_mz: f64,
    /// Base peak intensity.
    pub base_peak_intensity: f64,
}

/// Parse the entire scan index from its raw byte stream.
///
/// Returns one `ScanIndexEntry` per scan in the file.
pub fn parse_scan_index(
    _data: &[u8],
    _version: u32,
    _n_scans: u32,
) -> Result<Vec<ScanIndexEntry>, crate::RawError> {
    todo!("Implement based on FORMAT_SPEC.md (Phase 2 output)")
}
