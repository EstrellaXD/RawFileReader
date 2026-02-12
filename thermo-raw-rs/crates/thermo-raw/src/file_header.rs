//! FileHeader parsing -- the first structure in the Finnigan data stream.
//!
//! Layout (FORMAT_SPEC.md Section 3):
//! - 2 bytes: magic (0xA101)
//! - 36 bytes: signature (UTF-16LE, 18 chars)
//! - 16 bytes: 4 unknown u32
//! - 4 bytes: version
//! - 112 bytes: audit_start (AuditTag)
//! - 112 bytes: audit_end (AuditTag)
//! - 4 bytes: unknown
//! - 60 bytes: unknown area
//! - 2056 bytes: tag (UTF-16LE, 1028 chars)

use crate::io_utils::BinaryReader;
use crate::version::FINNIGAN_MAGIC;
use crate::RawError;

/// Parsed Finnigan file header.
#[derive(Debug, Clone)]
pub struct FileHeader {
    pub magic: u16,
    pub signature: String,
    pub version: u32,
    pub creation_time: u64,
    pub creation_user: String,
    pub modification_time: u64,
    pub tag: String,
}

/// Audit tag: timestamp + user info (112 bytes total).
#[derive(Debug, Clone)]
struct AuditTag {
    time: u64,
    tag1: String,
    _unknown: u32,
}

impl AuditTag {
    fn parse(reader: &mut BinaryReader) -> Result<Self, RawError> {
        let time = reader.read_u64()?;
        let tag1 = reader.read_utf16_fixed(100)?; // 50 UTF-16 chars
        let unknown = reader.read_u32()?;
        Ok(Self {
            time,
            tag1,
            _unknown: unknown,
        })
    }
}

impl FileHeader {
    /// Parse the FileHeader from the beginning of the data stream.
    pub fn parse(data: &[u8]) -> Result<Self, RawError> {
        let mut reader = BinaryReader::new(data);

        let magic = reader.read_u16()?;
        if magic != FINNIGAN_MAGIC {
            return Err(RawError::NotRawFile);
        }

        let signature = reader.read_utf16_fixed(36)?; // 18 UTF-16 chars
        let _unknown1 = reader.read_u32()?;
        let _unknown2 = reader.read_u32()?;
        let _unknown3 = reader.read_u32()?;
        let _unknown4 = reader.read_u32()?;
        let version = reader.read_u32()?;

        let audit_start = AuditTag::parse(&mut reader)?;
        let audit_end = AuditTag::parse(&mut reader)?;

        let _unknown5 = reader.read_u32()?;
        reader.skip(60)?; // unknown area

        let tag = reader.read_utf16_fixed(2056)?; // 1028 UTF-16 chars

        Ok(Self {
            magic,
            signature,
            version,
            creation_time: audit_start.time,
            creation_user: audit_start.tag1,
            modification_time: audit_end.time,
            tag,
        })
    }

    /// Size of the FileHeader in bytes.
    pub fn size() -> usize {
        2 + 36 + 16 + 4 + 112 + 112 + 4 + 60 + 2056
    }
}

/// Convert Windows FILETIME (100-nanosecond intervals since 1601-01-01) to
/// an ISO 8601 date string.
pub fn filetime_to_string(filetime: u64) -> String {
    if filetime == 0 {
        return "unknown".to_string();
    }
    const FILETIME_UNIX_DIFF: u64 = 116_444_736_000_000_000;
    if filetime < FILETIME_UNIX_DIFF {
        return "unknown".to_string();
    }
    let unix_100ns = filetime - FILETIME_UNIX_DIFF;
    let unix_secs = unix_100ns / 10_000_000;

    let days = unix_secs / 86400;
    let remaining = unix_secs % 86400;
    let hours = remaining / 3600;
    let minutes = (remaining % 3600) / 60;
    let seconds = remaining % 60;

    let (year, month, day) = days_to_ymd(days);

    format!(
        "{:04}-{:02}-{:02}T{:02}:{:02}:{:02}Z",
        year, month, day, hours, minutes, seconds
    )
}

fn days_to_ymd(mut days: u64) -> (u64, u64, u64) {
    let mut year = 1970;
    loop {
        let days_in_year = if is_leap_year(year) { 366 } else { 365 };
        if days < days_in_year {
            break;
        }
        days -= days_in_year;
        year += 1;
    }
    let leap = is_leap_year(year);
    let month_days: [u64; 12] = if leap {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };
    let mut month = 1;
    for &md in &month_days {
        if days < md {
            break;
        }
        days -= md;
        month += 1;
    }
    (year, month, days + 1)
}

fn is_leap_year(year: u64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || (year % 400 == 0)
}
