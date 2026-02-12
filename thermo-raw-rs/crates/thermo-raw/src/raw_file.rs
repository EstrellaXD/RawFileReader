//! Top-level entry point: open and read Thermo RAW files.

use crate::chromatogram;
use crate::file_header::FileHeader;
use crate::metadata;
use crate::raw_file_info::RawFileInfo;
use crate::run_header::RunHeader;
use crate::scan_data;
use crate::scan_filter;
use crate::scan_index::{self, ScanIndexEntry};
use crate::trailer::{self, TrailerLayout};
use crate::types::{Chromatogram, FileMetadata, MsLevel, Polarity, PrecursorInfo, Scan};
use crate::version;
use crate::RawError;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;

/// Abstraction over file data sources (owned bytes or memory-mapped).
enum FileData {
    Owned(Vec<u8>),
    Mapped(memmap2::Mmap),
}

impl Deref for FileData {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            FileData::Owned(v) => v,
            FileData::Mapped(m) => m,
        }
    }
}

/// A Thermo RAW file opened for reading.
pub struct RawFile {
    /// Raw file bytes (owned or memory-mapped).
    data: FileData,
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
    /// Pre-computed trailer layout (eagerly parsed on open).
    trailer_layout: Option<TrailerLayout>,
}

impl RawFile {
    /// Open a Thermo RAW file, reading it entirely into memory.
    ///
    /// Parses the Finnigan file header, RawFileInfo, RunHeader, ScanIndex,
    /// and trailer layout. Scan data is decoded lazily on demand.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, RawError> {
        let data = std::fs::read(path.as_ref())?;
        Self::from_data(FileData::Owned(data))
    }

    /// Open a Thermo RAW file using memory-mapping.
    ///
    /// More memory-efficient for large files — the OS pages data on demand.
    ///
    /// # Safety
    /// The file must not be modified while the RawFile is open.
    pub fn open_mmap(path: impl AsRef<Path>) -> Result<Self, RawError> {
        let file = std::fs::File::open(path.as_ref())?;
        let mmap = unsafe { memmap2::Mmap::map(&file)? };
        Self::from_data(FileData::Mapped(mmap))
    }

    /// Parse RAW file structures from raw data.
    fn from_data(data: FileData) -> Result<Self, RawError> {
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

        // Eagerly parse trailer layout (header only, not all records).
        // This enables efficient per-scan trailer field access via precomputed offsets.
        let trailer_layout = if trailer_addr > 0 {
            match trailer::parse_generic_data_header(&data, trailer_addr) {
                Ok(header) => Some(TrailerLayout::from_header(header)),
                Err(_) => None,
            }
        } else {
            None
        };

        Ok(Self {
            data,
            version: ver,
            file_metadata,
            run_header,
            scan_index: scan_index_entries,
            data_addr,
            trailer_layout,
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
    ///
    /// Decodes the scan data packet and enriches with trailer-derived metadata
    /// (filter string, MS level, polarity, precursor info).
    pub fn scan(&self, scan_number: u32) -> Result<Scan, RawError> {
        let idx = scan_number
            .checked_sub(self.run_header.first_scan)
            .ok_or(RawError::ScanOutOfRange(scan_number))? as usize;
        let entry = self
            .scan_index
            .get(idx)
            .ok_or(RawError::ScanOutOfRange(scan_number))?;
        let mut scan =
            scan_data::decode_scan(&self.data, self.data_addr as usize, entry, scan_number)?;

        // Enrich with trailer-derived metadata
        self.enrich_scan(&mut scan, idx as u32);

        Ok(scan)
    }

    /// Read multiple scans in parallel using rayon.
    ///
    /// Each scan is enriched with trailer-derived metadata.
    pub fn scans_parallel(&self, range: std::ops::Range<u32>) -> Result<Vec<Scan>, RawError> {
        use rayon::prelude::*;
        let first = self.run_header.first_scan;
        let entries: Vec<_> = range
            .map(|n| ((n - first) as usize, n))
            .filter_map(|(idx, n)| self.scan_index.get(idx).map(|e| (e, n, idx as u32)))
            .collect();

        entries
            .par_iter()
            .map(|(entry, scan_num, scan_idx)| {
                let mut scan =
                    scan_data::decode_scan(&self.data, self.data_addr as usize, entry, *scan_num)?;
                self.enrich_scan(&mut scan, *scan_idx);
                Ok(scan)
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

    /// Get trailer extra data for a specific scan as a HashMap.
    pub fn trailer_extra(
        &self,
        scan_number: u32,
    ) -> Result<HashMap<String, String>, RawError> {
        let layout = self
            .trailer_layout
            .as_ref()
            .ok_or_else(|| RawError::StreamNotFound("trailer extra".to_string()))?;

        let scan_idx = scan_number
            .checked_sub(self.run_header.first_scan)
            .ok_or(RawError::ScanOutOfRange(scan_number))?;

        trailer::parse_trailer_extra(&self.data, &layout.header, scan_idx)
    }

    /// Get the list of trailer extra field labels.
    pub fn trailer_fields(&self) -> Vec<String> {
        match &self.trailer_layout {
            Some(layout) => layout.field_labels(),
            None => vec![],
        }
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

    /// Enrich a scan with trailer-derived metadata.
    ///
    /// Extracts filter string from trailer extra, parses it for MS level and
    /// polarity, and builds precursor info for MS2+ scans from both the filter
    /// string and dedicated trailer fields (Monoisotopic M/Z, Charge State).
    fn enrich_scan(&self, scan: &mut Scan, scan_idx: u32) {
        let layout = match &self.trailer_layout {
            Some(l) => l,
            None => return,
        };

        // Extract filter string from trailer
        if let Some(fi) = layout.filter_text_idx {
            if let Ok(filter_str) = layout.read_string(&self.data, scan_idx, fi) {
                if !filter_str.is_empty() {
                    let filter = scan_filter::parse_filter(&filter_str);
                    scan.ms_level = filter.ms_level;
                    scan.polarity = filter.polarity;
                    scan.filter_string = Some(filter_str);

                    // Build precursor info for MS2+ scans
                    if !matches!(scan.ms_level, MsLevel::Ms1) {
                        scan.precursor =
                            self.build_precursor_info(layout, scan_idx, &filter);
                    }
                }
            }
        }
    }

    /// Build PrecursorInfo from trailer fields and filter string.
    ///
    /// Prefers trailer-derived monoisotopic m/z (more accurate) over filter m/z.
    fn build_precursor_info(
        &self,
        layout: &TrailerLayout,
        scan_idx: u32,
        filter: &scan_filter::ScanFilter,
    ) -> Option<PrecursorInfo> {
        let filter_precursor = filter.precursor.as_ref();

        // Get monoisotopic m/z from trailer (more accurate than filter string)
        let mono_mz = layout
            .mono_mz_idx
            .and_then(|idx| layout.read_f64(&self.data, scan_idx, idx).ok())
            .filter(|&mz| mz > 0.0);

        // Get charge state from trailer
        let charge = layout
            .charge_state_idx
            .and_then(|idx| layout.read_i32(&self.data, scan_idx, idx).ok())
            .filter(|&c| c != 0);

        // Get isolation width from trailer
        let isolation_width = layout
            .isolation_width_idx
            .and_then(|idx| layout.read_f64(&self.data, scan_idx, idx).ok())
            .filter(|&w| w > 0.0);

        // Prefer monoisotopic m/z from trailer; fall back to filter string
        let mz = mono_mz.or_else(|| filter_precursor.map(|p| p.mz))?;

        Some(PrecursorInfo {
            mz,
            charge,
            isolation_width,
            activation_type: filter_precursor.map(|p| p.activation.clone()),
            collision_energy: filter_precursor.map(|p| p.collision_energy),
        })
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
