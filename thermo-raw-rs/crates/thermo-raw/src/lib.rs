//! Pure Rust Thermo RAW file reader.
//!
//! This crate provides zero-dependency (no Thermo DLLs) reading of Thermo
//! Scientific RAW mass spectrometry data files. It supports:
//!
//! - File metadata extraction
//! - Scan data reading (centroid and profile)
//! - Chromatogram extraction (TIC, BPC, XIC)
//! - Parallel scan decoding via rayon
//!
//! # Example
//!
//! ```no_run
//! use thermo_raw::RawFile;
//!
//! let raw = RawFile::open("sample.raw").unwrap();
//! println!("Scans: {}", raw.n_scans());
//!
//! let scan = raw.scan(1).unwrap();
//! println!("m/z values: {:?}", &scan.centroid_mz[..5]);
//! ```

pub mod chromatogram;
pub mod error;
pub mod file_header;
pub mod io_utils;
pub mod metadata;
pub mod raw_file;
pub mod raw_file_info;
pub mod run_header;
pub mod scan_data;
pub mod scan_data_centroid;
pub mod scan_data_profile;
pub mod scan_event;
pub mod scan_filter;
pub mod scan_index;
pub mod trailer;
pub mod types;
pub mod version;

pub mod validation;

pub use error::RawError;
pub use raw_file::RawFile;
pub use types::*;
