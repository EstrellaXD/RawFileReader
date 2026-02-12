//! RawFileInfoPreamble parsing.
//!
//! Contains acquisition date and pointers to RunHeaders.

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
    /// Heading strings.
    pub headings: Vec<String>,
    /// Byte offset after parsing (where the next structure begins).
    pub end_offset: u64,
}

impl RawFileInfo {
    /// Parse RawFileInfo starting at the given offset in the data stream.
    pub fn parse(data: &[u8], offset: u64, version: u32) -> Result<Self, RawError> {
        let mut reader = BinaryReader::at_offset(data, offset);

        let _method_file_present = reader.read_u32()?;
        let year = reader.read_u16()?;
        let month = reader.read_u16()?;
        let _day_of_week = reader.read_u16()?;
        let day = reader.read_u16()?;
        let hour = reader.read_u16()?;
        let minute = reader.read_u16()?;
        let second = reader.read_u16()?;
        let millisecond = reader.read_u16()?;

        let _unknown1 = reader.read_u32()?;
        let _data_addr_32 = reader.read_u32()?;
        let n_controllers = reader.read_u32()?;
        let _n_controllers_2 = reader.read_u32()?;

        reader.skip(16)?; // unknown2..5

        let run_header_addr_32 = reader.read_u32()?;
        let _run_header_addr2_32 = reader.read_u32()?;

        let mut run_header_addr_64 = None;

        if version >= 64 {
            reader.skip(744)?;
            let _data_addr_64 = reader.read_u64()?;
            let _unknown_addr_64 = reader.read_u64()?;
            run_header_addr_64 = Some(reader.read_u64()?);
            if n_controllers > 1 {
                for _ in 1..n_controllers.min(8) {
                    let _ = reader.read_u64()?;
                }
            }
            let skip_size = if version >= 66 { 256 } else { 248 };
            reader.skip(skip_size)?;
        } else {
            reader.skip(744)?;
        }

        // Read heading strings (PascalStringWin32)
        let mut headings = Vec::new();
        for _ in 0..6 {
            match reader.read_pascal_string() {
                Ok(s) => headings.push(s),
                Err(_) => break,
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
