//! File metadata extraction.
//!
//! Combines information from FileHeader, RawFileInfo, and RunHeader.

use crate::file_header::{filetime_to_string, FileHeader};
use crate::raw_file_info::RawFileInfo;
use crate::run_header::RunHeader;
use crate::types::FileMetadata;

/// Build FileMetadata from the parsed structures.
pub fn build_metadata(
    header: &FileHeader,
    info: &RawFileInfo,
    run_header: &RunHeader,
) -> FileMetadata {
    // Use RawFileInfo date if available, otherwise fall back to FileHeader creation time
    let creation_date = if info.year > 0 {
        info.acquisition_date()
    } else {
        filetime_to_string(header.creation_time)
    };

    FileMetadata {
        creation_date,
        instrument_model: run_header.model.clone(),
        instrument_name: run_header.device_name.clone(),
        serial_number: run_header.serial_number.clone(),
        software_version: run_header.software_version.clone(),
        sample_name: run_header.sample_tag1.clone(),
        comment: run_header.sample_tag3.clone(),
    }
}
