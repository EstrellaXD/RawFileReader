//! TrailerExtra parsing (scan-level metadata).
//!
//! The trailer extra uses a self-describing format:
//! 1. GenericDataHeader: count + array of GenericDataDescriptor
//! 2. GenericRecord[n_scans]: one per scan, fields match the header descriptors

use std::collections::HashMap;

use crate::io_utils::BinaryReader;
use crate::RawError;

/// Parsed trailer extra data for a single scan.
pub type TrailerExtra = HashMap<String, String>;

/// A field descriptor in the GenericDataHeader.
#[derive(Debug, Clone)]
pub struct GenericDataDescriptor {
    /// Data type code (see FORMAT_SPEC.md Section 9).
    pub type_code: u32,
    /// Field length in bytes.
    pub length: u32,
    /// Human-readable label.
    pub label: String,
}

/// Parsed GenericDataHeader (template for all trailer records).
#[derive(Debug, Clone)]
pub struct GenericDataHeader {
    pub descriptors: Vec<GenericDataDescriptor>,
    /// Byte offset after the header (where records begin).
    pub records_offset: u64,
}

/// Type codes for GenericDataDescriptor.
pub mod type_codes {
    pub const BOOL: u32 = 0x1;
    pub const I8: u32 = 0x2;
    pub const I16: u32 = 0x3;
    pub const I32: u32 = 0x4;
    pub const F32: u32 = 0x5;
    pub const F64: u32 = 0x6;
    pub const U8: u32 = 0x7;
    pub const U16: u32 = 0x8;
    pub const U32: u32 = 0x9;
    pub const F32_ALT: u32 = 0xA;
    pub const F64_ALT: u32 = 0xB;
    pub const ASCII: u32 = 0xC;
    pub const WIDE_STRING: u32 = 0xD;
}

/// Get the byte size of a field based on its type code and declared length.
fn field_byte_size(desc: &GenericDataDescriptor) -> usize {
    match desc.type_code {
        type_codes::BOOL | type_codes::I8 | type_codes::U8 => 1,
        type_codes::I16 | type_codes::U16 => 2,
        type_codes::I32 | type_codes::U32 | type_codes::F32 | type_codes::F32_ALT => 4,
        type_codes::F64 | type_codes::F64_ALT => 8,
        type_codes::ASCII => desc.length as usize,
        type_codes::WIDE_STRING => desc.length as usize,
        _ => desc.length as usize,
    }
}

/// Calculate the total byte size of one GenericRecord.
fn record_byte_size(header: &GenericDataHeader) -> usize {
    header
        .descriptors
        .iter()
        .map(|d| field_byte_size(d))
        .sum()
}

/// Parse the GenericDataHeader at the given offset.
pub fn parse_generic_data_header(data: &[u8], offset: u64) -> Result<GenericDataHeader, RawError> {
    let mut reader = BinaryReader::at_offset(data, offset);

    let n_fields = reader.read_u32()?;
    if n_fields > 10_000 {
        return Err(RawError::CorruptedData(format!(
            "GenericDataHeader has unreasonable field count: {}",
            n_fields
        )));
    }

    let mut descriptors = Vec::with_capacity(n_fields as usize);
    for _ in 0..n_fields {
        let type_code = reader.read_u32()?;
        let length = reader.read_u32()?;
        let label = reader.read_pascal_string()?;
        descriptors.push(GenericDataDescriptor {
            type_code,
            length,
            label,
        });
    }

    Ok(GenericDataHeader {
        descriptors,
        records_offset: reader.position(),
    })
}

/// Parse trailer extra data for a specific scan.
///
/// `trailer_addr` is the absolute offset of the GenericDataHeader.
/// `scan_index` is 0-based (scan_number - first_scan).
pub fn parse_trailer_extra(
    data: &[u8],
    header: &GenericDataHeader,
    scan_index: u32,
) -> Result<TrailerExtra, RawError> {
    let rec_size = record_byte_size(header);
    let rec_offset = header.records_offset + (scan_index as u64) * (rec_size as u64);

    let mut reader = BinaryReader::at_offset(data, rec_offset);
    let mut result = HashMap::new();

    for desc in &header.descriptors {
        let label = desc.label.trim_end_matches(':').trim().to_string();
        let value = read_field_as_string(&mut reader, desc)?;
        result.insert(label, value);
    }

    Ok(result)
}

/// Read a single field value as a string representation.
fn read_field_as_string(
    reader: &mut BinaryReader,
    desc: &GenericDataDescriptor,
) -> Result<String, RawError> {
    match desc.type_code {
        type_codes::BOOL => {
            let v = reader.read_u8()?;
            Ok(if v != 0 { "true" } else { "false" }.to_string())
        }
        type_codes::I8 => {
            let v = reader.read_u8()? as i8;
            Ok(v.to_string())
        }
        type_codes::I16 => {
            let v = reader.read_u16()? as i16;
            Ok(v.to_string())
        }
        type_codes::I32 => {
            let v = reader.read_i32()?;
            Ok(v.to_string())
        }
        type_codes::F32 | type_codes::F32_ALT => {
            let v = reader.read_f32()?;
            Ok(format!("{}", v))
        }
        type_codes::F64 | type_codes::F64_ALT => {
            let v = reader.read_f64()?;
            Ok(format!("{}", v))
        }
        type_codes::U8 => {
            let v = reader.read_u8()?;
            Ok(v.to_string())
        }
        type_codes::U16 => {
            let v = reader.read_u16()?;
            Ok(v.to_string())
        }
        type_codes::U32 => {
            let v = reader.read_u32()?;
            Ok(v.to_string())
        }
        type_codes::ASCII => {
            let bytes = reader.read_bytes(desc.length as usize)?;
            let s = String::from_utf8_lossy(&bytes)
                .trim_end_matches('\0')
                .to_string();
            Ok(s)
        }
        type_codes::WIDE_STRING => {
            let s = reader.read_utf16_fixed(desc.length as usize)?;
            Ok(s)
        }
        _ => {
            // Unknown type: skip bytes
            reader.skip(desc.length as usize)?;
            Ok(String::new())
        }
    }
}

/// Get the list of trailer extra field labels.
pub fn parse_trailer_fields(data: &[u8], trailer_addr: u64) -> Result<Vec<String>, RawError> {
    let header = parse_generic_data_header(data, trailer_addr)?;
    Ok(header
        .descriptors
        .iter()
        .map(|d| d.label.trim_end_matches(':').trim().to_string())
        .collect())
}
