//! Ground truth validation framework.
//!
//! Loads JSON exported by C# GroundTruthExporter and compares against
//! Rust parser output.

use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundTruthScanIndex {
    pub scan_number: u32,
    pub rt: f64,
    pub ms_level: u8,
    pub polarity: String,
    pub tic: f64,
    pub base_peak_mz: f64,
    pub base_peak_intensity: f64,
    pub filter_string: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroundTruthScanData {
    pub scan_number: u32,
    pub centroid_count: usize,
    pub centroid_mz: Option<Vec<f64>>,
    pub centroid_intensity: Option<Vec<f64>>,
    pub profile_count: usize,
    pub profile_mz: Option<Vec<f64>>,
    pub profile_intensity: Option<Vec<f64>>,
}

#[derive(Debug)]
pub struct ValidationResult {
    pub scan_number: u32,
    pub passed: bool,
    pub mz_max_error_ppm: f64,
    pub mz_mean_error_ppm: f64,
    pub intensity_max_relative_error: f64,
    pub rt_error_seconds: f64,
    pub peak_count_match: bool,
    pub errors: Vec<String>,
}

#[derive(Debug)]
pub struct FileValidationReport {
    pub total_scans: u32,
    pub passed_scans: u32,
    pub failed_scans: u32,
    pub pass_rate: f64,
    pub worst_mz_error_ppm: f64,
    pub worst_intensity_error: f64,
    pub failures: Vec<ValidationResult>,
}

/// Acceptance criteria thresholds.
pub struct ValidationCriteria {
    /// Maximum allowed m/z error in ppm (default: 0.1).
    pub mz_tolerance_ppm: f64,
    /// Maximum allowed relative intensity error (default: 1e-6).
    pub intensity_rel_tolerance: f64,
    /// Maximum allowed RT error in minutes (default: 0.001).
    pub rt_tolerance_minutes: f64,
}

impl Default for ValidationCriteria {
    fn default() -> Self {
        Self {
            mz_tolerance_ppm: 0.1,
            intensity_rel_tolerance: 1e-6,
            rt_tolerance_minutes: 0.001,
        }
    }
}

/// Load the scan index ground truth from a directory.
pub fn load_scan_index(truth_dir: &Path) -> Vec<GroundTruthScanIndex> {
    let path = truth_dir.join("scan_index.json");
    let data = std::fs::read_to_string(path).expect("Failed to read scan_index.json");
    serde_json::from_str(&data).expect("Failed to parse scan_index.json")
}

/// Load per-scan ground truth data.
pub fn load_scan_data(truth_dir: &Path, scan_number: u32) -> GroundTruthScanData {
    let path = truth_dir
        .join("scans")
        .join(format!("scan_{:05}.json", scan_number));
    let data = std::fs::read_to_string(path).expect("Failed to read scan data");
    serde_json::from_str(&data).expect("Failed to parse scan data")
}

/// Compare two m/z arrays and return (max_error_ppm, mean_error_ppm, error_messages).
pub fn validate_mz_arrays(
    parsed: &[f64],
    truth: &[f64],
    tolerance_ppm: f64,
) -> (f64, f64, Vec<String>) {
    let mut max_error = 0.0_f64;
    let mut sum_error = 0.0_f64;
    let mut errors = Vec::new();

    if parsed.len() != truth.len() {
        errors.push(format!(
            "Peak count mismatch: parsed={} truth={}",
            parsed.len(),
            truth.len()
        ));
        return (f64::INFINITY, f64::INFINITY, errors);
    }

    for (i, (p, t)) in parsed.iter().zip(truth.iter()).enumerate() {
        let error_ppm = if *t != 0.0 {
            ((p - t) / t).abs() * 1e6
        } else {
            0.0
        };
        max_error = max_error.max(error_ppm);
        sum_error += error_ppm;
        if error_ppm > tolerance_ppm {
            errors.push(format!(
                "Peak {}: mz parsed={:.8} truth={:.8} error={:.4} ppm",
                i, p, t, error_ppm
            ));
        }
    }

    let mean_error = if !truth.is_empty() {
        sum_error / truth.len() as f64
    } else {
        0.0
    };
    (max_error, mean_error, errors)
}
