//! Ground truth validation framework.
//!
//! Loads JSON exported by C# GroundTruthExporter and compares against
//! Rust parser output.

use crate::raw_file::RawFile;
use crate::RawError;
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
pub fn load_scan_index(truth_dir: &Path) -> Result<Vec<GroundTruthScanIndex>, RawError> {
    let path = truth_dir.join("scan_index.json");
    let data = std::fs::read_to_string(&path).map_err(|e| {
        RawError::CorruptedData(format!("Failed to read {}: {}", path.display(), e))
    })?;
    serde_json::from_str(&data).map_err(|e| {
        RawError::CorruptedData(format!("Failed to parse {}: {}", path.display(), e))
    })
}

/// Load per-scan ground truth data.
pub fn load_scan_data(truth_dir: &Path, scan_number: u32) -> Result<GroundTruthScanData, RawError> {
    let path = truth_dir
        .join("scans")
        .join(format!("scan_{:05}.json", scan_number));
    let data = std::fs::read_to_string(&path).map_err(|e| {
        RawError::CorruptedData(format!("Failed to read {}: {}", path.display(), e))
    })?;
    serde_json::from_str(&data).map_err(|e| {
        RawError::CorruptedData(format!("Failed to parse {}: {}", path.display(), e))
    })
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

/// Compare two intensity arrays and return (max_relative_error, error_messages).
pub fn validate_intensity_arrays(
    parsed: &[f64],
    truth: &[f64],
    tolerance: f64,
) -> (f64, Vec<String>) {
    let mut max_error = 0.0_f64;
    let mut errors = Vec::new();

    if parsed.len() != truth.len() {
        errors.push(format!(
            "Intensity count mismatch: parsed={} truth={}",
            parsed.len(),
            truth.len()
        ));
        return (f64::INFINITY, errors);
    }

    for (i, (p, t)) in parsed.iter().zip(truth.iter()).enumerate() {
        let rel_error = if *t != 0.0 {
            ((p - t) / t).abs()
        } else if *p != 0.0 {
            f64::INFINITY
        } else {
            0.0
        };
        max_error = max_error.max(rel_error);
        if rel_error > tolerance {
            errors.push(format!(
                "Peak {}: intensity parsed={:.6} truth={:.6} rel_error={:.2e}",
                i, p, t, rel_error
            ));
        }
    }

    (max_error, errors)
}

/// Validate a single scan against ground truth.
fn validate_single_scan(
    raw: &RawFile,
    truth_index: &GroundTruthScanIndex,
    truth_dir: &Path,
    criteria: &ValidationCriteria,
) -> ValidationResult {
    let scan_number = truth_index.scan_number;
    let mut errors = Vec::new();

    // Read the scan
    let scan = match raw.scan(scan_number) {
        Ok(s) => s,
        Err(e) => {
            return ValidationResult {
                scan_number,
                passed: false,
                mz_max_error_ppm: f64::INFINITY,
                mz_mean_error_ppm: f64::INFINITY,
                intensity_max_relative_error: f64::INFINITY,
                rt_error_seconds: f64::INFINITY,
                peak_count_match: false,
                errors: vec![format!("Failed to read scan: {}", e)],
            };
        }
    };

    // Validate RT
    let rt_error_min = (scan.rt - truth_index.rt).abs();
    let rt_error_seconds = rt_error_min * 60.0;
    if rt_error_min > criteria.rt_tolerance_minutes {
        errors.push(format!(
            "RT error: parsed={:.6} truth={:.6} diff={:.6} min",
            scan.rt, truth_index.rt, rt_error_min
        ));
    }

    // Default values for m/z and intensity validation
    let mut mz_max_error_ppm = 0.0;
    let mut mz_mean_error_ppm = 0.0;
    let mut intensity_max_error = 0.0;
    let mut peak_count_match = true;

    // Load per-scan truth data if available
    let scan_truth = load_scan_data(truth_dir, scan_number).ok();
    if let Some(ref truth_data) = scan_truth {
        // Validate centroid m/z
        if let Some(ref truth_mz) = truth_data.centroid_mz {
            peak_count_match = scan.centroid_mz.len() == truth_mz.len();
            let (max_ppm, mean_ppm, mz_errors) =
                validate_mz_arrays(&scan.centroid_mz, truth_mz, criteria.mz_tolerance_ppm);
            mz_max_error_ppm = max_ppm;
            mz_mean_error_ppm = mean_ppm;
            errors.extend(mz_errors);
        }

        // Validate centroid intensity
        if let Some(ref truth_int) = truth_data.centroid_intensity {
            let (max_int_err, int_errors) =
                validate_intensity_arrays(&scan.centroid_intensity, truth_int, criteria.intensity_rel_tolerance);
            intensity_max_error = max_int_err;
            errors.extend(int_errors);
        }
    }

    let passed = errors.is_empty();

    ValidationResult {
        scan_number,
        passed,
        mz_max_error_ppm,
        mz_mean_error_ppm,
        intensity_max_relative_error: intensity_max_error,
        rt_error_seconds,
        peak_count_match,
        errors,
    }
}

/// Validate an entire RAW file against ground truth data.
///
/// Returns a FileValidationReport with per-scan results and aggregate statistics.
pub fn validate_file(
    raw: &RawFile,
    truth_dir: &Path,
    criteria: &ValidationCriteria,
) -> Result<FileValidationReport, RawError> {
    let truth_index = load_scan_index(truth_dir)?;

    let mut total_scans = 0u32;
    let mut passed_scans = 0u32;
    let mut failed_scans = 0u32;
    let mut worst_mz = 0.0f64;
    let mut worst_intensity = 0.0f64;
    let mut failures = Vec::new();

    for truth in &truth_index {
        total_scans += 1;
        let result = validate_single_scan(raw, truth, truth_dir, criteria);

        worst_mz = worst_mz.max(result.mz_max_error_ppm);
        worst_intensity = worst_intensity.max(result.intensity_max_relative_error);

        if result.passed {
            passed_scans += 1;
        } else {
            failed_scans += 1;
            failures.push(result);
        }
    }

    let pass_rate = if total_scans > 0 {
        passed_scans as f64 / total_scans as f64
    } else {
        1.0
    };

    Ok(FileValidationReport {
        total_scans,
        passed_scans,
        failed_scans,
        pass_rate,
        worst_mz_error_ppm: worst_mz,
        worst_intensity_error: worst_intensity,
        failures,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_mz_arrays_perfect_match() {
        let parsed = vec![100.0, 200.0, 300.0];
        let truth = vec![100.0, 200.0, 300.0];
        let (max_err, mean_err, errors) = validate_mz_arrays(&parsed, &truth, 0.1);
        assert_eq!(max_err, 0.0);
        assert_eq!(mean_err, 0.0);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_mz_arrays_small_error() {
        let parsed = vec![100.000005, 200.00001, 300.000015];
        let truth = vec![100.0, 200.0, 300.0];
        let (max_err, _mean_err, errors) = validate_mz_arrays(&parsed, &truth, 0.1);
        // Error should be ~0.05 ppm for 100.000005 vs 100.0 = 5e-8/100 * 1e6 = 0.0005 ppm
        assert!(max_err < 0.1);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_mz_arrays_large_error() {
        let parsed = vec![100.001];
        let truth = vec![100.0];
        let (max_err, _, errors) = validate_mz_arrays(&parsed, &truth, 0.1);
        // Error: 0.001/100 * 1e6 = 10 ppm
        assert!(max_err > 1.0);
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_validate_mz_arrays_length_mismatch() {
        let parsed = vec![100.0, 200.0];
        let truth = vec![100.0];
        let (max_err, _, errors) = validate_mz_arrays(&parsed, &truth, 0.1);
        assert!(max_err.is_infinite());
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_validate_intensity_arrays_perfect() {
        let parsed = vec![1000.0, 2000.0];
        let truth = vec![1000.0, 2000.0];
        let (max_err, errors) = validate_intensity_arrays(&parsed, &truth, 1e-6);
        assert_eq!(max_err, 0.0);
        assert!(errors.is_empty());
    }

    #[test]
    fn test_validate_intensity_arrays_with_error() {
        let parsed = vec![1000.01];
        let truth = vec![1000.0];
        let (max_err, _) = validate_intensity_arrays(&parsed, &truth, 1e-6);
        // relative error = 0.01/1000 = 1e-5
        assert!(max_err > 1e-6);
    }
}
