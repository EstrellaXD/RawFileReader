//! Top-level entry point: open and read Thermo RAW files.

use crate::chromatogram;
use crate::file_header::FileHeader;
use crate::metadata;
use crate::raw_file_info::RawFileInfo;
use crate::run_header::RunHeader;
use crate::scan_data;
use crate::scan_index::{self, ScanIndexEntry};
use crate::trailer;
use crate::types::{Chromatogram, FileMetadata, MsLevel, Polarity, Scan};
use crate::version;
use crate::RawError;
use std::collections::HashMap;
use std::path::Path;

/// A Thermo RAW file opened for reading.
pub struct RawFile {
    /// Raw file bytes (memory-mapped or read into memory).
    data: Vec<u8>,
    /// RAW file format version.
    version: u32,
    /// File-level metadata.
    file_metadata: FileMetadata,
    /// Parsed run header.
    run_header: RunHeader,
    /// Scan index (one entry per scan).
    scan_index: Vec<ScanIndexEntry>,
    /// Base address of the data stream.
    data_addr: u64,
    /// Trailer extra header (lazy-parsed on first use).
    trailer_header: Option<trailer::GenericDataHeader>,
    /// Trailer extra address.
    trailer_addr: u64,
}

impl RawFile {
    /// Open a Thermo RAW file.
    ///
    /// Parses the Finnigan file header, RawFileInfo, RunHeader, and ScanIndex.
    /// Scan data is decoded lazily on demand.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RawError> {
        let data = std::fs::read(path.as_ref())?;

        // Thermo RAW files are OLE2 containers. The Finnigan data starts
        // within the main data stream. For now we parse the raw bytes directly,
        // treating the entire file content as the data stream.
        // The Finnigan magic (0xA101) should be found at or near the beginning
        // of the file data.

        // Find the Finnigan magic in the first 64KB of the file
        let finnigan_offset = find_finnigan_magic(&data).ok_or(RawError::NotRawFile)?;

        // Parse FileHeader
        let file_header = FileHeader::parse(&data[finnigan_offset..])?;
        let ver = file_header.version;

        if !version::is_supported(ver) {
            return Err(RawError::UnsupportedVersion(ver));
        }

        // Parse RawFileInfo (immediately after FileHeader)
        let info_offset = finnigan_offset as u64 + FileHeader::size() as u64;
        let raw_file_info = RawFileInfo::parse(&data, info_offset, ver)?;

        // Parse RunHeader at the address from RawFileInfo
        let rh_addr = raw_file_info.run_header_addr();
        let run_header = RunHeader::parse(&data, rh_addr, ver)?;

        // Parse ScanIndex
        let n_scans = run_header.n_scans();
        let si_addr = run_header.scan_index_addr();
        let scan_index_entries = scan_index::parse_scan_index(&data, si_addr, ver, n_scans)?;

        let data_addr = run_header.data_addr();
        let trailer_addr = run_header.scan_trailer_addr();

        // Build metadata
        let file_metadata = metadata::build_metadata(&file_header, &raw_file_info, &run_header);

        Ok(Self {
            data,
            version: ver,
            file_metadata,
            run_header,
            scan_index: scan_index_entries,
            data_addr,
            trailer_header: None,
            trailer_addr,
        })
    }

    /// RAW file format version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// File-level metadata.
    pub fn metadata(&self) -> &FileMetadata {
        &self.file_metadata
    }

    /// Total number of scans.
    pub fn n_scans(&self) -> u32 {
        self.scan_index.len() as u32
    }

    /// First scan number.
    pub fn first_scan(&self) -> u32 {
        self.run_header.first_scan
    }

    /// Last scan number.
    pub fn last_scan(&self) -> u32 {
        self.run_header.last_scan
    }

    /// Acquisition start time in minutes.
    pub fn start_time(&self) -> f64 {
        self.run_header.start_time
    }

    /// Acquisition end time in minutes.
    pub fn end_time(&self) -> f64 {
        self.run_header.end_time
    }

    /// Low mass range.
    pub fn low_mass(&self) -> f64 {
        self.run_header.low_mass
    }

    /// High mass range.
    pub fn high_mass(&self) -> f64 {
        self.run_header.high_mass
    }

    /// Read a single scan by scan number.
    pub fn scan(&self, scan_number: u32) -> Result<Scan, RawError> {
        let idx = scan_number
            .checked_sub(self.run_header.first_scan)
            .ok_or(RawError::ScanOutOfRange(scan_number))? as usize;
        let entry = self
            .scan_index
            .get(idx)
            .ok_or(RawError::ScanOutOfRange(scan_number))?;
        scan_data::decode_scan(&self.data, self.data_addr as usize, entry, scan_number)
    }

    /// Read multiple scans in parallel using rayon.
    pub fn scans_parallel(&self, range: std::ops::Range<u32>) -> Result<Vec<Scan>, RawError> {
        use rayon::prelude::*;
        let first = self.run_header.first_scan;
        let entries: Vec<_> = range
            .map(|n| ((n - first) as usize, n))
            .filter_map(|(idx, n)| self.scan_index.get(idx).map(|e| (e, n)))
            .collect();

        entries
            .par_iter()
            .map(|(entry, scan_num)| {
                scan_data::decode_scan(&self.data, self.data_addr as usize, entry, *scan_num)
            })
            .collect()
    }

    /// TIC chromatogram (fast: extracted from scan index, no scan data decoding).
    pub fn tic(&self) -> Chromatogram {
        chromatogram::build_tic(&self.scan_index)
    }

    /// Base peak chromatogram.
    pub fn bpc(&self) -> Chromatogram {
        chromatogram::build_bpc(&self.scan_index)
    }

    /// Extracted ion chromatogram for a target m/z with tolerance in ppm.
    pub fn xic(&self, target_mz: f64, tolerance_ppm: f64) -> Result<Chromatogram, RawError> {
        use rayon::prelude::*;
        let half_width = target_mz * tolerance_ppm * 1e-6;
        let low = target_mz - half_width;
        let high = target_mz + half_width;

        let results: Vec<(f64, f64)> = self
            .scan_index
            .par_iter()
            .enumerate()
            .map(|(idx, entry)| {
                let scan_num = self.run_header.first_scan + idx as u32;
                let scan = self.scan(scan_num).unwrap_or_else(|_| Scan {
                    scan_number: scan_num,
                    rt: entry.rt,
                    ms_level: MsLevel::Ms1,
                    polarity: Polarity::Unknown,
                    tic: 0.0,
                    base_peak_mz: 0.0,
                    base_peak_intensity: 0.0,
                    centroid_mz: vec![],
                    centroid_intensity: vec![],
                    profile_mz: None,
                    profile_intensity: None,
                    precursor: None,
                    filter_string: None,
                });
                let intensity: f64 = scan
                    .centroid_mz
                    .iter()
                    .zip(scan.centroid_intensity.iter())
                    .filter(|(&mz, _)| mz >= low && mz <= high)
                    .map(|(_, &int)| int)
                    .sum();
                (entry.rt, intensity)
            })
            .collect();

        Ok(Chromatogram {
            rt: results.iter().map(|(rt, _)| *rt).collect(),
            intensity: results.iter().map(|(_, int)| *int).collect(),
        })
    }

    /// Get trailer extra data for a specific scan.
    pub fn trailer_extra(
        &mut self,
        scan_number: u32,
    ) -> Result<HashMap<String, String>, RawError> {
        // Lazy-parse trailer header on first access
        if self.trailer_header.is_none() && self.trailer_addr > 0 {
            self.trailer_header =
                Some(trailer::parse_generic_data_header(&self.data, self.trailer_addr)?);
        }

        let header = self
            .trailer_header
            .as_ref()
            .ok_or_else(|| RawError::StreamNotFound("trailer extra".to_string()))?;

        let scan_idx = scan_number
            .checked_sub(self.run_header.first_scan)
            .ok_or(RawError::ScanOutOfRange(scan_number))?;

        trailer::parse_trailer_extra(&self.data, header, scan_idx)
    }

    /// Get the list of trailer extra field labels.
    pub fn trailer_fields(&self) -> Result<Vec<String>, RawError> {
        if self.trailer_addr == 0 {
            return Ok(vec![]);
        }
        trailer::parse_trailer_fields(&self.data, self.trailer_addr)
    }

    /// Get the raw scan index entries.
    pub fn scan_index(&self) -> &[ScanIndexEntry] {
        &self.scan_index
    }

    /// List OLE2 streams in the file (uses cfb-reader).
    pub fn list_streams(path: impl AsRef<Path>) -> Result<Vec<String>, RawError> {
        let container = cfb_reader::Ole2Container::open(path)
            .map_err(|e| RawError::CfbError(e.to_string()))?;
        Ok(container.list_streams())
    }
}

/// Search for the Finnigan magic (0xA101) in the file data.
/// Returns the byte offset of the magic, or None if not found.
fn find_finnigan_magic(data: &[u8]) -> Option<usize> {
    let magic_le = 0xA101u16.to_le_bytes();
    let search_limit = data.len().min(65536);

    for i in 0..search_limit.saturating_sub(1) {
        if data[i] == magic_le[0] && data[i + 1] == magic_le[1] {
            // Verify: the signature string should follow at offset +2
            // (18 UTF-16 chars = 36 bytes). Check for reasonable version
            // at offset +54.
            if i + 58 <= data.len() {
                let ver = u32::from_le_bytes(data[i + 54..i + 58].try_into().ok()?);
                if ver > 0 && ver <= 200 {
                    return Some(i);
                }
            }
        }
    }
    None
}
