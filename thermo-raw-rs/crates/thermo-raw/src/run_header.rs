//! RunHeader stream parsing.
//!
//! The RunHeader is the primary index structure for each instrument.
//! It contains SampleInfo (scan range, time/mass range) and addresses
//! to ScanIndex, DataStream, TrailerExtra, etc.
//!
//! From decompiled RunHeader.Load version dispatch:
//! - v66+: RunHeaderStruct  (current, has InstrumentType at end)
//! - v64-65: RunHeaderStruct5 (64-bit offsets, Extra0-5, no InstrumentType)
//! - v49-63: RunHeaderStruct4 (ends at FilterMassPrecision, 32-bit offsets only)
//! - v40-48: RunHeaderStruct3
//! - v25-39: RunHeaderStruct2
//! - v<25: RunHeaderStruct1
//!
//! For v<=63, 32-bit offsets are promoted to 64-bit via ConvertFrom32Bit.

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
    /// Instrument type identifier (v66+ only, 0 for older versions).
    /// Avoids hardcoded instrument name checks in code.
    pub instrument_type: i32,
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

        // === Version 64+ extra fields (64-bit addresses) ===
        // From decompiled RunHeaderStruct5/RunHeaderStruct (v64+):
        //   SpectPos (i64), PacketPos (i64), StatusLogPos (i64),
        //   ErrorLogPos (i64), RunHeaderPos (i64),
        //   TrailerScanEventsPos (i64), TrailerExtraPos (i64),
        //   VirtualControllerInfoStruct (16 bytes),
        //   Extra0..5 Pos/Count pairs (12 bytes each = 72 bytes)
        let mut scan_index_addr_64 = None;
        let mut data_addr_64 = None;
        let mut scan_trailer_addr_64 = None;
        let mut scan_params_addr_64 = None;
        let mut instrument_type = 0i32;

        if version >= 64 {
            scan_index_addr_64 = Some(reader.read_u64()?);   // SpectPos
            data_addr_64 = Some(reader.read_u64()?);          // PacketPos
            let _inst_log_addr_64 = reader.read_u64()?;       // StatusLogPos
            let _error_log_addr_64 = reader.read_u64()?;      // ErrorLogPos
            let _run_header_addr_64 = reader.read_u64()?;     // RunHeaderPos
            scan_trailer_addr_64 = Some(reader.read_u64()?);  // TrailerScanEventsPos
            scan_params_addr_64 = Some(reader.read_u64()?);   // TrailerExtraPos
            // VirtualControllerInfoStruct: VirtualDeviceType(4) + VirtualDeviceIndex(4) + Offset(8) = 16
            reader.skip(16)?;
            // Extra0..5: each is Pos(i64) + Count(i32) = 12 bytes, 6 pairs = 72 bytes
            reader.skip(72)?;

            // v66+: InstrumentType field (i32)
            if version >= 66 {
                instrument_type = reader.read_i32()?;
            }
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
            instrument_type,
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
