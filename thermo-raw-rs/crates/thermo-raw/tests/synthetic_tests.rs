//! Integration tests using synthetic binary data.
//!
//! These tests construct minimal binary buffers that mimic real RAW file structures
//! and verify that the parsing pipeline handles them correctly.

use thermo_raw::scan_data_centroid;
use thermo_raw::scan_data_profile;
use thermo_raw::scan_event::{
    frequency_to_mz, ActivationType, AnalyzerType, IonizationType, ScanMode, ScanType,
};
use thermo_raw::scan_filter;
use thermo_raw::validation;

/// Build a minimal centroid data buffer: count + (f32 mz, f32 intensity) pairs.
fn build_centroid_data(peaks: &[(f32, f32)]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(peaks.len() as u32).to_le_bytes());
    for (mz, int) in peaks {
        buf.extend_from_slice(&mz.to_le_bytes());
        buf.extend_from_slice(&int.to_le_bytes());
    }
    buf
}

/// Build a minimal profile data buffer with a single chunk, layout=0.
fn build_profile_data(first_value: f64, step: f64, first_bin: u32, signals: &[f32]) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&first_value.to_le_bytes());
    buf.extend_from_slice(&step.to_le_bytes());
    buf.extend_from_slice(&1u32.to_le_bytes()); // peak_count = 1
    buf.extend_from_slice(&(signals.len() as u32).to_le_bytes()); // nbins_total
    // Chunk: first_bin, nbins, signals
    buf.extend_from_slice(&first_bin.to_le_bytes());
    buf.extend_from_slice(&(signals.len() as u32).to_le_bytes());
    for s in signals {
        buf.extend_from_slice(&s.to_le_bytes());
    }
    buf
}

#[test]
fn test_centroid_decode_roundtrip() {
    let peaks = vec![
        (100.5f32, 1000.0f32),
        (200.75, 2500.0),
        (300.25, 500.0),
        (500.123, 12345.6),
    ];
    let data = build_centroid_data(&peaks);
    let (mz, intensity) = scan_data_centroid::decode_centroid(&data, 0).unwrap();

    assert_eq!(mz.len(), 4);
    assert_eq!(intensity.len(), 4);
    for (i, (expected_mz, expected_int)) in peaks.iter().enumerate() {
        assert!(
            (mz[i] - *expected_mz as f64).abs() < 1e-3,
            "mz[{}]: got {}, expected {}",
            i,
            mz[i],
            expected_mz
        );
        assert!(
            (intensity[i] - *expected_int as f64).abs() < 1e-1,
            "intensity[{}]: got {}, expected {}",
            i,
            intensity[i],
            expected_int
        );
    }
}

#[test]
fn test_centroid_decode_empty() {
    let data = 0u32.to_le_bytes().to_vec();
    let (mz, intensity) = scan_data_centroid::decode_centroid(&data, 0).unwrap();
    assert!(mz.is_empty());
    assert!(intensity.is_empty());
}

#[test]
fn test_profile_decode_roundtrip() {
    let first_value = 200.0;
    let step = 0.01;
    let signals: Vec<f32> = (0..100).map(|i| (i as f32) * 10.0).collect();
    let data = build_profile_data(first_value, step, 0, &signals);

    let (mz, intensity) = scan_data_profile::decode_profile(&data, 0, 0).unwrap();

    assert_eq!(mz.len(), 100);
    assert_eq!(intensity.len(), 100);

    // Verify m/z reconstruction: mz[i] = first_value + i * step
    for i in 0..100 {
        let expected_mz = first_value + (i as f64) * step;
        assert!(
            (mz[i] - expected_mz).abs() < 1e-10,
            "mz[{}]: got {}, expected {}",
            i,
            mz[i],
            expected_mz
        );
        assert!(
            (intensity[i] - signals[i] as f64).abs() < 1e-3,
            "intensity[{}]: got {}, expected {}",
            i,
            intensity[i],
            signals[i]
        );
    }
}

#[test]
fn test_profile_decode_with_fudge() {
    // Layout > 0 means each chunk has an extra f32 fudge factor
    let first_value: f64 = 400.0;
    let step: f64 = 0.005;
    let signals = vec![100.0f32, 200.0, 300.0];

    let mut data = Vec::new();
    data.extend_from_slice(&first_value.to_le_bytes());
    data.extend_from_slice(&step.to_le_bytes());
    data.extend_from_slice(&1u32.to_le_bytes()); // peak_count
    data.extend_from_slice(&3u32.to_le_bytes()); // nbins_total
    // Chunk with fudge
    data.extend_from_slice(&10u32.to_le_bytes()); // first_bin = 10
    data.extend_from_slice(&3u32.to_le_bytes()); // nbins = 3
    data.extend_from_slice(&0.001f32.to_le_bytes()); // fudge
    for s in &signals {
        data.extend_from_slice(&s.to_le_bytes());
    }

    let (mz, intensity) = scan_data_profile::decode_profile(&data, 0, 1).unwrap();
    assert_eq!(mz.len(), 3);

    // mz[0] = 400.0 + 10 * 0.005 = 400.05
    assert!((mz[0] - 400.05).abs() < 1e-10);
    // mz[1] = 400.0 + 11 * 0.005 = 400.055
    assert!((mz[1] - 400.055).abs() < 1e-10);
    assert!((intensity[0] - 100.0).abs() < 1e-3);
}

#[test]
fn test_scan_filter_complex_ms2() {
    let filter =
        scan_filter::parse_filter("FTMS + c NSI d Full ms2 524.2648@hcd28.00 [100.0000-1060.0000]");
    assert!(matches!(filter.ms_level, thermo_raw::MsLevel::Ms2));
    assert_eq!(filter.polarity, thermo_raw::Polarity::Positive);
    assert_eq!(filter.analyzer, "FTMS");

    let p = filter.precursor.unwrap();
    assert!((p.mz - 524.2648).abs() < 1e-4);
    assert_eq!(p.activation, "hcd");
    assert!((p.collision_energy - 28.0).abs() < 0.01);

    let (low, high) = filter.mass_range.unwrap();
    assert!((low - 100.0).abs() < 0.01);
    assert!((high - 1060.0).abs() < 0.01);
}

#[test]
fn test_scan_event_preamble_all_fields() {
    // Build a preamble buffer with known byte positions
    let mut preamble_data = vec![0u8; 80];
    preamble_data[4] = 1; // Positive
    preamble_data[5] = 0; // Centroid
    preamble_data[6] = 2; // MS2
    preamble_data[7] = 2; // SIM
    preamble_data[10] = 1; // Dependent
    preamble_data[11] = 3; // ESI
    preamble_data[24] = 1; // HCD
    preamble_data[40] = 4; // FTMS

    // Build a full scan event buffer matching decompiled ScanEvent.Load layout:
    //   preamble + reactions + mass_ranges + calibrators + source_frags + source_frag_ranges
    let mut data = preamble_data;
    // n_precursors = 1
    data.extend_from_slice(&1u32.to_le_bytes());
    // Reaction (32 bytes for v57: MsReactionStruct2):
    //   PrecursorMass, IsolationWidth, CollisionEnergy, CollisionEnergyValid, (padding)
    data.extend_from_slice(&524.2648f64.to_le_bytes());
    data.extend_from_slice(&1.5f64.to_le_bytes());
    data.extend_from_slice(&28.0f64.to_le_bytes());
    // CollisionEnergyValid: bit 0 = valid (1), bits 1-8 = HCD (1) → value = 1 | (1 << 1) = 3
    data.extend_from_slice(&3u32.to_le_bytes());
    // struct padding (4 bytes to reach 32-byte reaction size)
    data.extend_from_slice(&0u32.to_le_bytes());
    // n_mass_ranges = 1
    data.extend_from_slice(&1u32.to_le_bytes());
    // mass range: (100.0, 1060.0)
    data.extend_from_slice(&100.0f64.to_le_bytes());
    data.extend_from_slice(&1060.0f64.to_le_bytes());
    // n_conversion_params (mass calibrators) = 0
    data.extend_from_slice(&0u32.to_le_bytes());
    // n_source_fragmentations = 0
    data.extend_from_slice(&0u32.to_le_bytes());
    // n_source_frag_mass_ranges = 0
    data.extend_from_slice(&0u32.to_le_bytes());

    let (event, _end_pos) =
        thermo_raw::scan_event::parse_scan_event(&data, 0, 57).unwrap();

    assert_eq!(event.preamble.polarity, thermo_raw::Polarity::Positive);
    assert_eq!(event.preamble.scan_mode, ScanMode::Centroid);
    assert!(matches!(event.preamble.ms_level, thermo_raw::MsLevel::Ms2));
    assert_eq!(event.preamble.scan_type, ScanType::Sim);
    assert!(event.preamble.dependent);
    assert_eq!(event.preamble.ionization, IonizationType::Esi);
    // Activation derived from Reaction's CollisionEnergyValid field (bits 1-8 = 1 = HCD)
    assert_eq!(event.preamble.activation, ActivationType::Hcd);
    assert_eq!(event.preamble.analyzer, AnalyzerType::Ftms);
    assert_eq!(event.reactions.len(), 1);
    assert!((event.reactions[0].precursor_mz - 524.2648).abs() < 1e-4);
    assert!((event.reactions[0].isolation_width - 1.5).abs() < 1e-6);
    assert!((event.reactions[0].collision_energy - 28.0).abs() < 1e-6);
    assert_eq!(event.reactions[0].collision_energy_valid, 3); // valid + HCD
    assert!(event.conversion_params.is_empty());
}

#[test]
fn test_frequency_to_mz_orbitrap_model() {
    // 7-param Orbitrap: polynomial model
    // Test with simple params where most terms are zero
    let params = [1e10, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
    let freq = 1000.0;
    let mz = frequency_to_mz(freq, &params);
    // m/z = params[0] / f^2 = 1e10 / 1e6 = 10000
    assert!((mz - 10000.0).abs() < 1e-6);
}

#[test]
fn test_frequency_to_mz_zero_frequency() {
    let params = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0];
    assert_eq!(frequency_to_mz(0.0, &params), 0.0);
}

#[test]
fn test_validation_mz_within_tolerance() {
    // Perfect match
    let parsed = vec![100.0, 500.0, 1000.0];
    let truth = vec![100.0, 500.0, 1000.0];
    let (max_err, mean_err, errors) = validation::validate_mz_arrays(&parsed, &truth, 0.1);
    assert_eq!(max_err, 0.0);
    assert_eq!(mean_err, 0.0);
    assert!(errors.is_empty());
}

#[test]
fn test_validation_intensity_within_tolerance() {
    let parsed = vec![1000.0, 2000.0, 3000.0];
    let truth = vec![1000.0, 2000.0, 3000.0];
    let (max_err, errors) = validation::validate_intensity_arrays(&parsed, &truth, 1e-6);
    assert_eq!(max_err, 0.0);
    assert!(errors.is_empty());
}

#[test]
fn test_serde_roundtrip_scan() {
    let scan = thermo_raw::Scan {
        scan_number: 1,
        rt: 1.5,
        ms_level: thermo_raw::MsLevel::Ms2,
        polarity: thermo_raw::Polarity::Positive,
        tic: 1e6,
        base_peak_mz: 524.2648,
        base_peak_intensity: 5e5,
        centroid_mz: vec![100.0, 200.0, 300.0],
        centroid_intensity: vec![1000.0, 2000.0, 3000.0],
        profile_mz: None,
        profile_intensity: None,
        precursor: Some(thermo_raw::PrecursorInfo {
            mz: 524.2648,
            charge: Some(2),
            isolation_width: Some(1.5),
            activation_type: Some("HCD".to_string()),
            collision_energy: Some(28.0),
        }),
        filter_string: Some("FTMS + c NSI d Full ms2 524.2648@hcd28.00".to_string()),
    };

    let json = serde_json::to_string(&scan).unwrap();
    let deserialized: thermo_raw::Scan = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.scan_number, scan.scan_number);
    assert_eq!(deserialized.ms_level, scan.ms_level);
    assert_eq!(deserialized.polarity, scan.polarity);
    assert!((deserialized.rt - scan.rt).abs() < 1e-10);
    assert_eq!(deserialized.centroid_mz.len(), 3);
    let p = deserialized.precursor.unwrap();
    assert!((p.mz - 524.2648).abs() < 1e-4);
    assert_eq!(p.charge, Some(2));
}

#[test]
fn test_serde_roundtrip_chromatogram() {
    let chrom = thermo_raw::Chromatogram {
        rt: vec![0.0, 1.0, 2.0],
        intensity: vec![1e5, 2e5, 1.5e5],
    };

    let json = serde_json::to_string(&chrom).unwrap();
    let deserialized: thermo_raw::Chromatogram = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.rt.len(), 3);
    assert!((deserialized.intensity[1] - 2e5).abs() < 1e-6);
}

#[test]
fn test_serde_roundtrip_metadata() {
    let meta = thermo_raw::FileMetadata {
        creation_date: "2024-01-15".to_string(),
        instrument_model: "Q Exactive HF".to_string(),
        instrument_name: "Instrument 1".to_string(),
        serial_number: "SN12345".to_string(),
        software_version: "2.11".to_string(),
        sample_name: "Test Sample".to_string(),
        comment: "DDA experiment".to_string(),
    };

    let json = serde_json::to_string(&meta).unwrap();
    let deserialized: thermo_raw::FileMetadata = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.instrument_model, "Q Exactive HF");
    assert_eq!(deserialized.serial_number, "SN12345");
}

#[test]
fn test_serde_roundtrip_scan_event() {
    let event = thermo_raw::scan_event::ScanEvent {
        preamble: thermo_raw::scan_event::ScanEventPreamble {
            polarity: thermo_raw::Polarity::Positive,
            scan_mode: ScanMode::Profile,
            ms_level: thermo_raw::MsLevel::Ms1,
            scan_type: ScanType::Full,
            dependent: false,
            ionization: IonizationType::Nsi,
            activation: ActivationType::Hcd,
            analyzer: AnalyzerType::Ftms,
        },
        reactions: vec![],
        conversion_params: vec![1.0, 2.0, 3.0, 4.0],
    };

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: thermo_raw::scan_event::ScanEvent =
        serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.preamble.analyzer, AnalyzerType::Ftms);
    assert_eq!(deserialized.conversion_params.len(), 4);
}
