//! Top-level entry point: open and read Thermo RAW files.

use crate::chromatogram;
use crate::run_header::RunHeader;
use crate::scan_data;
use crate::scan_index::ScanIndexEntry;
use crate::types::{Chromatogram, FileMetadata, MsLevel, Polarity, Scan};
use crate::RawError;
use std::path::Path;

/// A Thermo RAW file opened for reading.
pub struct RawFile {
    /// Memory-mapped file data.
    _data: Vec<u8>,
    /// RAW file format version.
    version: u32,
    /// File-level metadata.
    metadata: FileMetadata,
    /// Parsed run header.
    run_header: RunHeader,
    /// Scan index (one entry per scan).
    scan_index: Vec<ScanIndexEntry>,
    /// Byte offset of the scan data stream within the file.
    _scan_data_offset: usize,
    /// Length of the scan data stream.
    _scan_data_len: usize,
}

impl RawFile {
    /// Open a Thermo RAW file.
    ///
    /// This parses the OLE2 container, run header, scan index, and metadata.
    /// Scan data is decoded lazily on demand.
    pub fn open(_path: impl AsRef<Path>) -> Result<Self, RawError> {
        // Implementation roadmap:
        // 1. Memory-map the file
        // 2. Parse OLE2 container, locate internal streams
        // 3. Parse RunHeader stream
        // 4. Parse ScanIndex stream
        // 5. Cache metadata
        todo!("Implement based on FORMAT_SPEC.md (Phase 2 output)")
    }

    /// RAW file format version.
    pub fn version(&self) -> u32 {
        self.version
    }

    /// File-level metadata.
    pub fn metadata(&self) -> &FileMetadata {
        &self.metadata
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

    /// Read a single scan by scan number.
    pub fn scan(&self, scan_number: u32) -> Result<Scan, RawError> {
        let idx = (scan_number - self.run_header.first_scan) as usize;
        let entry = self
            .scan_index
            .get(idx)
            .ok_or(RawError::ScanOutOfRange(scan_number))?;
        scan_data::decode_scan(&self._data, self._scan_data_offset, entry, scan_number)
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
                scan_data::decode_scan(&self._data, self._scan_data_offset, entry, *scan_num)
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
}
