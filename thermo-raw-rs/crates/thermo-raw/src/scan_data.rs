//! ScanData decoding -- the core difficulty of the project.
//!
//! Each scan's raw data is stored as a ScanDataPacket at an offset in the
//! data stream. The packet has a 40-byte header followed by profile data,
//! peak list (centroids), peak descriptors, and additional streams.

use crate::io_utils::BinaryReader;
use crate::scan_data_centroid;
use crate::scan_data_profile;
use crate::scan_index::ScanIndexEntry;
use crate::types::{MsLevel, Polarity, Scan};
use crate::RawError;

/// Parsed ScanDataPacket header (40 bytes).
#[derive(Debug, Clone)]
pub struct PacketHeader {
    pub unknown1: u32,
    /// Number of 4-byte words of profile data.
    pub profile_size: u32,
    /// Number of 4-byte words of peak list data.
    pub peak_list_size: u32,
    /// Layout flag: 0 = no fudge, >0 = with fudge per profile chunk.
    pub layout: u32,
    /// Number of peak descriptor entries.
    pub descriptor_list_size: u32,
    /// Size of unknown stream.
    pub unknown_stream_size: u32,
    /// Size of triplet stream.
    pub triplet_stream_size: u32,
    pub unknown2: u32,
    pub low_mz: f32,
    pub high_mz: f32,
}

impl PacketHeader {
    pub fn parse(reader: &mut BinaryReader) -> Result<Self, RawError> {
        Ok(Self {
            unknown1: reader.read_u32()?,
            profile_size: reader.read_u32()?,
            peak_list_size: reader.read_u32()?,
            layout: reader.read_u32()?,
            descriptor_list_size: reader.read_u32()?,
            unknown_stream_size: reader.read_u32()?,
            triplet_stream_size: reader.read_u32()?,
            unknown2: reader.read_u32()?,
            low_mz: reader.read_f32()?,
            high_mz: reader.read_f32()?,
        })
    }

    pub const SIZE: usize = 40;
}

/// Decode a single scan from the data stream.
///
/// `data` is the full file data. `data_addr` is the base address of the data
/// stream. `entry` provides the scan's offset and data size.
pub fn decode_scan(
    data: &[u8],
    data_addr: usize,
    entry: &ScanIndexEntry,
    scan_number: u32,
) -> Result<Scan, RawError> {
    let abs_offset = data_addr as u64 + entry.offset;
    // Bounds check: for v65+ we have DataSize; for v<65 just verify offset is valid
    if entry.data_size > 0 {
        if abs_offset as usize + entry.data_size as usize > data.len() {
            return Err(RawError::ScanDecodeError {
                offset: abs_offset as usize,
                reason: format!(
                    "scan {} data extends beyond file (offset={}, size={}, file_len={})",
                    scan_number,
                    abs_offset,
                    entry.data_size,
                    data.len()
                ),
            });
        }
    } else if abs_offset as usize >= data.len() {
        return Err(RawError::ScanDecodeError {
            offset: abs_offset as usize,
            reason: format!(
                "scan {} data offset beyond file (offset={}, file_len={})",
                scan_number,
                abs_offset,
                data.len()
            ),
        });
    }

    // Empty scan: no packets and no data size means nothing to decode
    if entry.number_packets == 0 && entry.data_size == 0 {
        return Ok(Scan {
            scan_number,
            rt: entry.rt,
            ms_level: MsLevel::Ms1,
            polarity: Polarity::Unknown,
            tic: entry.tic,
            base_peak_mz: entry.base_peak_mz,
            base_peak_intensity: entry.base_peak_intensity,
            centroid_mz: vec![],
            centroid_intensity: vec![],
            profile_mz: None,
            profile_intensity: None,
            precursor: None,
            filter_string: None,
        });
    }

    let mut reader = BinaryReader::at_offset(data, abs_offset);
    let header = PacketHeader::parse(&mut reader)?;

    // Read profile data
    let (profile_mz, profile_intensity) = if header.profile_size > 0 {
        let profile_bytes = header.profile_size as usize * 4;
        let profile_start = reader.position();
        let result = scan_data_profile::decode_profile(data, profile_start as usize, header.layout);
        reader.set_position(profile_start + profile_bytes as u64);
        match result {
            Ok((mz, int)) => (Some(mz), Some(int)),
            Err(_) => {
                // Profile decoding can fail due to unknown encoding; skip gracefully
                (None, None)
            }
        }
    } else {
        (None, None)
    };

    // Read peak list (centroid data)
    let (centroid_mz, centroid_intensity) = if header.peak_list_size > 0 {
        let peak_bytes = header.peak_list_size as usize * 4;
        let peak_start = reader.position();
        let result = scan_data_centroid::decode_centroid(data, peak_start as usize);
        reader.set_position(peak_start + peak_bytes as u64);
        match result {
            Ok((mz, int)) => (mz, int),
            Err(_) => (vec![], vec![]),
        }
    } else {
        (vec![], vec![])
    };

    // Skip descriptors, unknown stream, and triplet stream
    // (we don't need them for basic m/z + intensity extraction)

    Ok(Scan {
        scan_number,
        rt: entry.rt,
        ms_level: MsLevel::Ms1, // Will be refined when ScanEvent parsing is added
        polarity: Polarity::Unknown,
        tic: entry.tic,
        base_peak_mz: entry.base_peak_mz,
        base_peak_intensity: entry.base_peak_intensity,
        centroid_mz,
        centroid_intensity,
        profile_mz,
        profile_intensity,
        precursor: None,
        filter_string: None,
    })
}
