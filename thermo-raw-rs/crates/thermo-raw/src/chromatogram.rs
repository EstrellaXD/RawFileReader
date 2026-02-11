//! Chromatogram extraction (TIC, BPC, XIC).

use crate::types::Chromatogram;

/// Build a TIC chromatogram from scan index entries.
pub fn build_tic(entries: &[crate::scan_index::ScanIndexEntry]) -> Chromatogram {
    Chromatogram {
        rt: entries.iter().map(|e| e.rt).collect(),
        intensity: entries.iter().map(|e| e.tic).collect(),
    }
}

/// Build a base peak chromatogram from scan index entries.
pub fn build_bpc(entries: &[crate::scan_index::ScanIndexEntry]) -> Chromatogram {
    Chromatogram {
        rt: entries.iter().map(|e| e.rt).collect(),
        intensity: entries.iter().map(|e| e.base_peak_intensity).collect(),
    }
}
