//! RawFileInfoPreamble parsing.
//!
//! Contains acquisition date and pointers to RunHeaders.
//!
//! From decompiled RawFileInfo.Load version dispatch:
//! - v65+: RawFileInfoStruct  (current, with VirtualControllerInfoStruct[64] + BlobOffset + BlobSize)
//! - v64:  RawFileInfoStruct4 (with VirtualControllerInfoStruct[64], no blob)
//! - v25-63: RawFileInfoStruct3 (OldVirtualControllerInfo[64] only, no 64-bit fields)
//! - v7-24: RawFileInfoStruct2
//! - v<7:  RawFileInfoStruct1
//!
//! Common preamble fields (all versions):
//!   IsExpMethodPresent (bool/u32, 4 bytes), SystemTimeStruct (16 bytes),
//!   IsInAcquisition (bool/u32, 4 bytes), VirtualDataOffset32 (u32, 4 bytes),
//!   NumberOfVirtualControllers (i32, 4 bytes), NextAvailableControllerIndex (i32, 4 bytes)
//!
//! VirtualControllerInfo arrays:
//!   - OldVirtualControllerInfo (12 bytes each): VirtualDeviceType(i32) + VirtualDeviceIndex(i32) + Offset(i32)
//!   - VirtualControllerInfoStruct (16 bytes each): VirtualDeviceType(i32) + VirtualDeviceIndex(i32) + Offset(i64)
//!
//! After the struct: 5 user label strings (PascalStringWin32), then ComputerName (v7+).

use crate::io_utils::BinaryReader;
use crate::RawError;

/// Parsed RawFileInfo with addresses to key data structures.
#[derive(Debug, Clone)]
pub struct RawFileInfo {
    pub year: u16,
    pub month: u16,
    pub day: u16,
    pub hour: u16,
    pub minute: u16,
    pub second: u16,
    pub millisecond: u16,
    /// 32-bit RunHeader address (used for v < 64).
    pub run_header_addr_32: u32,
    /// 64-bit RunHeader address (used for v >= 64), if available.
    pub run_header_addr_64: Option<u64>,
    /// Number of data controllers.
    pub n_controllers: u32,
    /// Heading strings (user labels + computer name).
    pub headings: Vec<String>,
    /// Blob offset (v65+ only, -1 if no blob).
    pub blob_offset: i64,
    /// Blob size in bytes (v65+ only, 0 if no blob).
    pub blob_size: u32,
    /// Byte offset after parsing (where the next structure begins).
    pub end_offset: u64,
}

impl RawFileInfo {
    /// Parse RawFileInfo starting at the given offset in the data stream.
    ///
    /// Handles all supported versions (v57-v66) following the decompiled
    /// RawFileInfo.Load version dispatch logic.
    pub fn parse(data: &[u8], offset: u64, version: u32) -> Result<Self, RawError> {
        let mut reader = BinaryReader::at_offset(data, offset);

        // IsExpMethodPresent (bool marshalled as i32 = 4 bytes)
        let _method_file_present = reader.read_u32()?;

        // SystemTimeStruct (8 x u16 = 16 bytes)
        let year = reader.read_u16()?;
        let month = reader.read_u16()?;
        let _day_of_week = reader.read_u16()?;
        let day = reader.read_u16()?;
        let hour = reader.read_u16()?;
        let minute = reader.read_u16()?;
        let second = reader.read_u16()?;
        let millisecond = reader.read_u16()?;

        // IsInAcquisition (bool as i32 = 4 bytes)
        let _is_in_acquisition = reader.read_u32()?;

        // VirtualDataOffset32 (u32)
        let _data_addr_32 = reader.read_u32()?;

        // NumberOfVirtualControllers (i32)
        let n_controllers = reader.read_u32()?;

        // NextAvailableControllerIndex (i32)
        let _n_controllers_2 = reader.read_u32()?;

        // OldVirtualControllerInfo[64] (12 bytes each = 768 bytes)
        // Each: VirtualDeviceType(i32) + VirtualDeviceIndex(i32) + Offset(i32)
        // The first controller's Offset gives us the 32-bit run header address
        let _old_ctl_array_pos = reader.position();
        // Read first controller: skip type(4) + index(4), read offset(4)
        reader.skip(8)?;
        let run_header_addr_32 = reader.read_u32()?;
        // Skip remaining bytes of the OldVirtualControllerInfo[64] array
        // We already read 12 bytes of the first entry, skip the remaining 63 entries
        reader.skip(63 * 12)?;

        // Version-dependent extended fields
        let mut run_header_addr_64 = None;
        let mut blob_offset = -1i64;
        let mut blob_size = 0u32;

        if version >= 64 {
            // VirtualDataOffset (i64) - 64-bit version of VirtualDataOffset32
            let _data_addr_64 = reader.read_u64()?;

            // VirtualControllerInfoStruct[64] (16 bytes each = 1024 bytes)
            // Each: VirtualDeviceType(i32) + VirtualDeviceIndex(i32) + Offset(i64)
            // The first controller's Offset gives us the 64-bit run header address
            // Skip type(4) + index(4), read offset(8)
            reader.skip(8)?;
            run_header_addr_64 = Some(reader.read_u64()?);
            // Skip remaining: we read 16 bytes of first entry, skip 63 entries
            reader.skip(63 * 16)?;

            if version >= 65 {
                // BlobOffset (i64) + BlobSize (u32)
                blob_offset = reader.read_u64()? as i64;
                blob_size = reader.read_u32()?;
            }
        }

        // Read heading strings: 5 user labels (PascalStringWin32)
        let mut headings = Vec::new();
        for _ in 0..5 {
            match reader.read_pascal_string() {
                Ok(s) => headings.push(s),
                Err(_) => break,
            }
        }

        // Computer name (v7+)
        if version >= 7 {
            match reader.read_pascal_string() {
                Ok(s) => headings.push(s),
                Err(_) => {}
            }
        }

        Ok(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
            millisecond,
            run_header_addr_32,
            run_header_addr_64,
            n_controllers,
            headings,
            blob_offset,
            blob_size,
            end_offset: reader.position(),
        })
    }

    /// Get the best available RunHeader address.
    pub fn run_header_addr(&self) -> u64 {
        self.run_header_addr_64
            .unwrap_or(self.run_header_addr_32 as u64)
    }

    /// Format the acquisition date as ISO 8601.
    pub fn acquisition_date(&self) -> String {
        format!(
            "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}
