//! RunHeader stream parsing.
//!
//! The RunHeader contains top-level information about the acquisition:
//! scan range, retention time range, mass range, etc.

/// Parsed RunHeader data.
#[derive(Debug, Clone)]
pub struct RunHeader {
    pub first_scan: u32,
    pub last_scan: u32,
    pub start_time: f64,
    pub end_time: f64,
    pub low_mass: f64,
    pub high_mass: f64,
    pub mass_resolution: f64,
}

impl RunHeader {
    /// Parse RunHeader from a raw byte stream.
    ///
    /// The exact offsets and field layout depend on the RAW file version
    /// and will be determined during the reverse engineering phase (Phase 2).
    pub fn parse(_data: &[u8], _version: u32) -> Result<Self, crate::RawError> {
        todo!("Implement based on FORMAT_SPEC.md (Phase 2 output)")
    }
}
