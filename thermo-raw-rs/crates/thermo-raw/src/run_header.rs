//! RunHeader stream parsing.
//!
//! The RunHeader is the primary index structure for each instrument.
//! It contains SampleInfo (scan range, time/mass range) and addresses
//! to ScanIndex, DataStream, TrailerExtra, etc.

use crate::io_utils::BinaryReader;
use crate::RawError;

/// Parsed RunHeader data.
#[derive(Debug, Clone)]
pub struct RunHeader {
    pub first_scan: u32,
    pub last_scan: u32,
    pub start_time: f64,
    pub end_time: f64,
    pub low_mass: f64,
    pub high_mass: f64,
    pub max_ion_current: f64,
    /// 32-bit addresses (for v < 64).
    pub scan_index_addr_32: u32,
    pub data_addr_32: u32,
    pub scan_trailer_addr_32: u32,
    pub scan_params_addr_32: u32,
    /// 64-bit addresses (for v >= 64).
    pub scan_index_addr_64: Option<u64>,
    pub data_addr_64: Option<u64>,
    pub scan_trailer_addr_64: Option<u64>,
    pub scan_params_addr_64: Option<u64>,
    /// Instrument info strings (from PascalStringWin32 at end of RunHeader).
    pub device_name: String,
    pub model: String,
    pub serial_number: String,
    pub software_version: String,
    /// Sample info tags.
    pub sample_tag1: String,
    pub sample_tag2: String,
    pub sample_tag3: String,
    /// Byte offset after parsing.
    pub end_offset: u64,
}

impl RunHeader {
    /// Parse RunHeader from the data stream at the given absolute offset.
    pub fn parse(data: &[u8], offset: u64, version: u32) -> Result<Self, RawError> {
        let mut reader = BinaryReader::at_offset(data, offset);

        // === SampleInfo (nested at start) ===
        let _unknown1 = reader.read_u32()?;
        let _unknown2 = reader.read_u32()?;
        let first_scan = reader.read_u32()?;
        let last_scan = reader.read_u32()?;
        let _inst_log_length = reader.read_u32()?;
        let _error_log_length = reader.read_u32()?;
        let _unknown3 = reader.read_u32()?;

        let scan_index_addr_32 = reader.read_u32()?;
        let data_addr_32 = reader.read_u32()?;
        let _inst_log_addr_32 = reader.read_u32()?;
        let _error_log_addr_32 = reader.read_u32()?;
        let _unknown4 = reader.read_u32()?;

        let max_ion_current = reader.read_f64()?;
        let low_mass = reader.read_f64()?;
        let high_mass = reader.read_f64()?;
        let start_time = reader.read_f64()?;
        let end_time = reader.read_f64()?;

        reader.skip(56)?; // unknown_area

        // Sample info tags (fixed-size UTF-16)
        let sample_tag1 = reader.read_utf16_fixed(88)?; // 44 chars
        let sample_tag2 = reader.read_utf16_fixed(40)?; // 20 chars
        let sample_tag3 = reader.read_utf16_fixed(320)?; // 160 chars

        // === RunHeader fields after SampleInfo ===

        // 13 filename strings (each 260 UTF-16 chars = 520 bytes)
        for _ in 0..13 {
            reader.skip(520)?;
        }

        let _unknown_double1 = reader.read_f64()?;
        let _unknown_double2 = reader.read_f64()?;

        let scan_trailer_addr_32 = reader.read_u32()?;
        let scan_params_addr_32 = reader.read_u32()?;
        reader.skip(8)?; // unknown_lengths
        let _n_segments = reader.read_u32()?;
        reader.skip(16)?; // unknown4..7
        let _own_addr_32 = reader.read_u32()?;

        // === Version 64-66 extra fields ===
        let mut scan_index_addr_64 = None;
        let mut data_addr_64 = None;
        let mut scan_trailer_addr_64 = None;
        let mut scan_params_addr_64 = None;

        if version >= 64 {
            scan_index_addr_64 = Some(reader.read_u64()?);
            data_addr_64 = Some(reader.read_u64()?);
            let _inst_log_addr_64 = reader.read_u64()?;
            let _error_log_addr_64 = reader.read_u64()?;
            let _unknown_addr1_64 = reader.read_u64()?;
            scan_trailer_addr_64 = Some(reader.read_u64()?);
            scan_params_addr_64 = Some(reader.read_u64()?);
            reader.skip(8)?; // unknown5..6
            let _own_addr_64 = reader.read_u64()?;
            reader.skip(96)?; // unknown7..30 (24 u32s)
        }

        // PascalStringWin32 strings: device name, model, serial, software, tags
        let device_name = reader.read_pascal_string().unwrap_or_default();
        let model = reader.read_pascal_string().unwrap_or_default();
        let serial_number = reader.read_pascal_string().unwrap_or_default();
        let software_version = reader.read_pascal_string().unwrap_or_default();
        // Read remaining tag strings (4 more)
        for _ in 0..4 {
            let _ = reader.read_pascal_string();
        }

        Ok(Self {
            first_scan,
            last_scan,
            start_time,
            end_time,
            low_mass,
            high_mass,
            max_ion_current,
            scan_index_addr_32,
            data_addr_32,
            scan_trailer_addr_32,
            scan_params_addr_32,
            scan_index_addr_64,
            data_addr_64,
            scan_trailer_addr_64,
            scan_params_addr_64,
            device_name,
            model,
            serial_number,
            software_version,
            sample_tag1,
            sample_tag2,
            sample_tag3,
            end_offset: reader.position(),
        })
    }

    /// Get the best available scan index address.
    pub fn scan_index_addr(&self) -> u64 {
        self.scan_index_addr_64
            .unwrap_or(self.scan_index_addr_32 as u64)
    }

    /// Get the best available data stream address.
    pub fn data_addr(&self) -> u64 {
        self.data_addr_64.unwrap_or(self.data_addr_32 as u64)
    }

    /// Get the best available trailer extra address.
    pub fn scan_trailer_addr(&self) -> u64 {
        self.scan_trailer_addr_64
            .unwrap_or(self.scan_trailer_addr_32 as u64)
    }

    /// Get the best available scan params address.
    pub fn scan_params_addr(&self) -> u64 {
        self.scan_params_addr_64
            .unwrap_or(self.scan_params_addr_32 as u64)
    }

    /// Number of scans.
    pub fn n_scans(&self) -> u32 {
        if self.last_scan >= self.first_scan {
            self.last_scan - self.first_scan + 1
        } else {
            0
        }
    }
}
