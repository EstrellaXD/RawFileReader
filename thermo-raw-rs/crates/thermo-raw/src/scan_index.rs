//! ScanIndex parsing (offset table).
//!
//! The scan index maps scan numbers to their byte offsets in the scan data
//! stream, along with lightweight per-scan metadata (RT, TIC, etc.).
//!
//! From decompiled ScanIndices.GetSizeOfScanIndexStructByFileVersion:
//! - v<64:  ScanIndexStruct1 (72 bytes) - 32-bit DataOffset at offset 0
//! - v64:   ScanIndexStruct2 (80 bytes) - 32-bit DataOffset at 0, 64-bit DataOffset at 72
//! - v65+:  ScanIndexStruct  (88 bytes) - DataSize at 0, 64-bit DataOffset at 72, CycleNumber at 80
//!
//! Field layout (from decompiled ReadScanIndexStruct):
//! | Offset | Type   | Field                              | v<64   | v64   | v65+     |
//! |--------|--------|------------------------------------|--------|-------|----------|
//! | 0      | u32    | DataOffset32Bit / DataSize         | offset | (ign) | DataSize |
//! | 4      | i32    | TrailerOffset                      | ✓      | ✓     | ✓        |
//! | 8      | i32    | ScanTypeIndex (HIWORD=seg,LOWORD=type) | ✓  | ✓     | ✓        |
//! | 12     | i32    | ScanNumber                         | ✓      | ✓     | ✓        |
//! | 16     | u32    | PacketType                         | ✓      | ✓     | ✓        |
//! | 20     | i32    | NumberPackets                      | ✓      | ✓     | ✓        |
//! | 24     | f64    | StartTime (RT in minutes)          | ✓      | ✓     | ✓        |
//! | 32     | f64    | TIC                                | ✓      | ✓     | ✓        |
//! | 40     | f64    | BasePeakIntensity                  | ✓      | ✓     | ✓        |
//! | 48     | f64    | BasePeakMass                       | ✓      | ✓     | ✓        |
//! | 56     | f64    | LowMass                            | ✓      | ✓     | ✓        |
//! | 64     | f64    | HighMass                           | ✓      | ✓     | ✓        |
//! | 72     | i64    | DataOffset (64-bit)                | --     | ✓     | ✓        |
//! | 80     | i32    | CycleNumber                        | --     | --    | ✓        |
//! | 84     | --     | (4 bytes struct alignment padding)  | --     | --    | ✓        |

use crate::io_utils::BinaryReader;
use crate::version;
use crate::RawError;

/// A single entry in the scan index.
#[derive(Debug, Clone)]
pub struct ScanIndexEntry {
    /// Byte offset into the scan data stream.
    pub offset: u64,
    /// Trailer data offset (relative, for locating per-scan trailer data).
    pub trailer_offset: i32,
    /// Scan event index (LOWORD of ScanTypeIndex).
    pub scan_event: u16,
    /// Scan segment number (HIWORD of ScanTypeIndex).
    pub scan_segment: u16,
    /// Scan number as stored in the index.
    pub scan_number: i32,
    /// Packet type (LOWORD=scan type, HIWORD=SIScanData flag).
    pub packet_type: u32,
    /// Number of data packets.
    pub number_packets: i32,
    /// Scan data size in bytes (v65+ only, from offset 0; 0 for older versions).
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
    /// Cycle number for associating events within a scan event cycle (v65+ only).
    pub cycle_number: i32,
}

// Keep backward-compatible field alias
impl ScanIndexEntry {
    /// Backward-compatible alias for scan_event field.
    pub fn index(&self) -> u32 {
        self.scan_number as u32
    }
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

        // Offset 0: DataOffset32Bit (v<65) or DataSize (v65+)
        let offset_or_size = reader.read_u32()?;
        // Offset 4: TrailerOffset
        let trailer_offset = reader.read_i32()?;
        // Offset 8: ScanTypeIndex (HIWORD=segment, LOWORD=scan type)
        let scan_event = reader.read_u16()?;
        let scan_segment = reader.read_u16()?;
        // Offset 12: ScanNumber
        let scan_number = reader.read_i32()?;
        // Offset 16: PacketType
        let packet_type = reader.read_u32()?;
        // Offset 20: NumberPackets
        let number_packets = reader.read_i32()?;
        // Offset 24: StartTime (RT)
        let rt = reader.read_f64()?;
        // Offset 32: TIC
        let tic = reader.read_f64()?;
        // Offset 40: BasePeakIntensity
        let base_peak_intensity = reader.read_f64()?;
        // Offset 48: BasePeakMass
        let base_peak_mz = reader.read_f64()?;
        // Offset 56: LowMass
        let low_mz = reader.read_f64()?;
        // Offset 64: HighMass
        let high_mz = reader.read_f64()?;

        // Version-dependent fields after the common 72 bytes
        let (scan_offset, data_size, cycle_number) = if version >= 64 {
            // Offset 72: DataOffset (64-bit)
            let offset_64 = reader.read_u64()?;

            if version >= 65 {
                // Offset 80: CycleNumber (i32)
                let cycle = reader.read_i32()?;
                // Offset 84: struct alignment padding (4 bytes)
                let _padding = reader.read_u32()?;
                // For v65+, offset 0 contains DataSize (not DataOffset32Bit)
                (offset_64, offset_or_size, cycle)
            } else {
                // v64: offset 0 is DataOffset32Bit (unused since we have 64-bit offset)
                (offset_64, 0u32, 0i32)
            }
        } else {
            // v<64: offset 0 is DataOffset32Bit, used as the scan offset
            (offset_or_size as u64, 0u32, 0i32)
        };

        // Ensure we consumed exactly entry_size bytes
        let expected_end = entry_start + entry_size as u64;
        if reader.position() != expected_end {
            reader.set_position(expected_end);
        }

        entries.push(ScanIndexEntry {
            offset: scan_offset,
            trailer_offset,
            scan_event,
            scan_segment,
            scan_number,
            packet_type,
            number_packets,
            data_size,
            rt,
            tic,
            base_peak_intensity,
            base_peak_mz,
            low_mz,
            high_mz,
            cycle_number,
        });
    }

    Ok(entries)
}
