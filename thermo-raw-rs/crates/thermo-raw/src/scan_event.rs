//! ScanEvent / ScanEventPreamble parsing.
//!
//! ScanEvents describe acquisition parameters for each scan type.
//! The scan event stream at `scan_params_addr` contains unique event templates,
//! each with a version-dependent preamble and variable-length reaction/conversion data.
//!
//! From decompiled ScanEvent.Load, the per-event layout is:
//!   1. ScanEventInfoStruct (preamble, version-dependent fixed size)
//!   2. Reactions array: u32 count + count * Reaction (version-dependent size per entry)
//!   3. MassRanges: u32 count + count * (f64 low, f64 high)
//!   4. MassCalibrators: u32 count + count * f64 (conversion params)
//!   5. SourceFragmentations: u32 count + count * f64
//!   6. SourceFragmentationMassRanges: u32 count + count * (f64, f64)
//!   7. Name: PascalStringWin32 (v65+ only)
//!
//! Reaction sizes (from decompiled Reaction.Load):
//!   - v66:    56 bytes (MsReactionStruct: adds IsolationWidthOffset)
//!   - v65:    48 bytes (MsReactionStruct3: adds RangeIsValid, First/LastPrecursorMass)
//!   - v31-64: 32 bytes (MsReactionStruct2: PrecursorMass, IsolationWidth, CollisionEnergy, CollisionEnergyValid)
//!   - v<31:   24 bytes (MsReactionStruct1: PrecursorMass, IsolationWidth, CollisionEnergy)

use crate::io_utils::BinaryReader;
use crate::types::{MsLevel, Polarity};
use crate::version;
use crate::RawError;
use serde::{Deserialize, Serialize};

/// Parsed ScanEvent preamble fields.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanEventPreamble {
    /// Polarity (0=negative, 1=positive, 2=undefined).
    pub polarity: Polarity,
    /// Scan mode (0=centroid, 1=profile, 2=undefined).
    pub scan_mode: ScanMode,
    /// MS power level (1=MS1, 2=MS2, ..., 8=MS8; 0=undefined).
    pub ms_level: MsLevel,
    /// Scan type (Full, Zoom, SIM, SRM, CRM).
    pub scan_type: ScanType,
    /// Whether this is a dependent (DDA) scan.
    pub dependent: bool,
    /// Ionization type.
    pub ionization: IonizationType,
    /// Activation type (HCD, CID, etc.).
    pub activation: ActivationType,
    /// Analyzer type (ITMS, FTMS, etc.).
    pub analyzer: AnalyzerType,
}

/// Scan acquisition mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanMode {
    Centroid,
    Profile,
    Unknown,
}

/// Scan type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScanType {
    Full,
    Zoom,
    Sim,
    Srm,
    Crm,
    Q1Ms,
    Q3Ms,
    Unknown(u8),
}

/// Ionization type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum IonizationType {
    Ei,
    Ci,
    Fab,
    Esi,
    Apci,
    Nsi,
    Tsi,
    Fdi,
    Maldi,
    Gd,
    Unknown(u8),
}

/// Activation type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActivationType {
    Cid,
    Hcd,
    Etd,
    Ecd,
    Unknown(u8),
}

/// Analyzer type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AnalyzerType {
    Itms,
    Tqms,
    Sqms,
    Tofms,
    Ftms,
    Sector,
    Unknown(u8),
}

/// A complete ScanEvent with preamble + reactions + conversion parameters.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanEvent {
    pub preamble: ScanEventPreamble,
    pub reactions: Vec<Reaction>,
    pub conversion_params: Vec<f64>,
}

/// Reaction (precursor fragmentation info).
///
/// From decompiled MsReactionStruct layout:
/// - PrecursorMass (f64, offset 0)
/// - IsolationWidth (f64, offset 8)
/// - CollisionEnergy (f64, offset 16)
/// - CollisionEnergyValid (u32, offset 24) - bit 0: valid flag, bits 1-8: ActivationType enum
/// - RangeIsValid (i32, offset 28) - v65+ only
/// - FirstPrecursorMass (f64, offset 32) - v65+ only
/// - LastPrecursorMass (f64, offset 40) - v65+ only
/// - IsolationWidthOffset (f64, offset 48) - v66+ only
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reaction {
    pub precursor_mz: f64,
    pub isolation_width: f64,
    pub collision_energy: f64,
    /// Collision energy valid flag (bit 0), activation type enum (bits 1-8).
    pub collision_energy_valid: u32,
    /// Whether first/last precursor mass range is valid (v65+ only).
    pub precursor_range_valid: bool,
    /// First precursor mass of isolation range (v65+ only).
    pub first_precursor_mass: f64,
    /// Last precursor mass of isolation range (v65+ only).
    pub last_precursor_mass: f64,
    /// Isolation width offset (v66+ only).
    pub isolation_width_offset: f64,
}

impl Reaction {
    /// Derive the activation type from the CollisionEnergyValid field.
    pub fn activation_type(&self) -> ActivationType {
        if self.collision_energy_valid == 0 {
            return ActivationType::Cid; // default
        }
        let type_bits = ((self.collision_energy_valid >> 1) & 0xFF) as u8;
        match type_bits {
            0 => ActivationType::Cid,
            1 => ActivationType::Hcd,
            2 => ActivationType::Etd,
            3 => ActivationType::Ecd,
            n => ActivationType::Unknown(n),
        }
    }
}

/// Parse the ScanEventPreamble from raw bytes.
///
/// The preamble is a fixed-size block at the start of each ScanEvent.
/// Key fields are at well-known byte positions within the preamble.
///
/// From decompiled ScanEventInfoStruct field layout (all versions):
/// - byte 4:  Polarity
/// - byte 5:  ScanDataType (centroid/profile)
/// - byte 6:  MSOrder
/// - byte 7:  ScanType
/// - byte 10: DependentData
/// - byte 11: IonizationMode
/// - byte 40: MassAnalyzerType (v54+, offset 40 for all versions due to field padding)
fn parse_preamble(data: &[u8]) -> ScanEventPreamble {
    let polarity = if data.len() > 4 {
        match data[4] {
            0 => Polarity::Negative,
            1 => Polarity::Positive,
            _ => Polarity::Unknown,
        }
    } else {
        Polarity::Unknown
    };

    let scan_mode = if data.len() > 5 {
        match data[5] {
            0 => ScanMode::Centroid,
            1 => ScanMode::Profile,
            _ => ScanMode::Unknown,
        }
    } else {
        ScanMode::Unknown
    };

    let ms_level = if data.len() > 6 {
        match data[6] {
            1 => MsLevel::Ms1,
            2 => MsLevel::Ms2,
            3 => MsLevel::Ms3,
            n if n > 3 && n <= 8 => MsLevel::Other(n),
            _ => MsLevel::Ms1,
        }
    } else {
        MsLevel::Ms1
    };

    let scan_type = if data.len() > 7 {
        match data[7] {
            0 => ScanType::Full,
            1 => ScanType::Zoom,
            2 => ScanType::Sim,
            3 => ScanType::Srm,
            4 => ScanType::Crm,
            7 => ScanType::Q1Ms,
            8 => ScanType::Q3Ms,
            n => ScanType::Unknown(n),
        }
    } else {
        ScanType::Full
    };

    let dependent = data.len() > 10 && data[10] == 1;

    let ionization = if data.len() > 11 {
        match data[11] {
            0 => IonizationType::Ei,
            1 => IonizationType::Ci,
            2 => IonizationType::Fab,
            3 => IonizationType::Esi,
            4 => IonizationType::Apci,
            5 => IonizationType::Nsi,
            6 => IonizationType::Tsi,
            7 => IonizationType::Fdi,
            8 => IonizationType::Maldi,
            9 => IonizationType::Gd,
            n => IonizationType::Unknown(n),
        }
    } else {
        IonizationType::Unknown(255)
    };

    // Activation type: derived from reactions (CollisionEnergyValid field).
    // Byte 24 in the preamble is SourceFragmentationType (source CID type), not
    // the MS/MS activation type. We set a default here and let the caller
    // override from Reaction data when available.
    let activation = ActivationType::Unknown(255);

    let analyzer = if data.len() > 40 {
        match data[40] {
            0 => AnalyzerType::Itms,
            1 => AnalyzerType::Tqms,
            2 => AnalyzerType::Sqms,
            3 => AnalyzerType::Tofms,
            4 => AnalyzerType::Ftms,
            5 => AnalyzerType::Sector,
            n => AnalyzerType::Unknown(n),
        }
    } else {
        AnalyzerType::Unknown(255)
    };

    ScanEventPreamble {
        polarity,
        scan_mode,
        ms_level,
        scan_type,
        dependent,
        ionization,
        activation,
        analyzer,
    }
}

/// Read a "doubles array": u32 count followed by count f64 values.
/// Matches the decompiled ReadDoublesExt pattern.
fn read_doubles_array(reader: &mut BinaryReader) -> Result<Vec<f64>, RawError> {
    let count = reader.read_u32()?;
    if count > 10_000 {
        return Err(RawError::CorruptedData(format!(
            "Unreasonable doubles array count: {}",
            count
        )));
    }
    reader.read_f64_array(count as usize)
}

/// Read a "mass range array": u32 count followed by count * (f64, f64) pairs.
/// Matches the decompiled MassRangeStruct.LoadArray pattern.
fn read_mass_range_array(reader: &mut BinaryReader) -> Result<Vec<(f64, f64)>, RawError> {
    let count = reader.read_u32()?;
    if count > 10_000 {
        return Err(RawError::CorruptedData(format!(
            "Unreasonable mass range count: {}",
            count
        )));
    }
    let mut ranges = Vec::with_capacity(count as usize);
    for _ in 0..count {
        let low = reader.read_f64()?;
        let high = reader.read_f64()?;
        ranges.push((low, high));
    }
    Ok(ranges)
}

/// Parse a single Reaction from the data stream.
///
/// Reads version-dependent number of bytes per the decompiled Reaction.Load:
/// - v66:    56 bytes (full MsReactionStruct)
/// - v65:    48 bytes (MsReactionStruct3)
/// - v31-64: 32 bytes (MsReactionStruct2)
/// - v<31:   24 bytes (MsReactionStruct1)
fn parse_reaction(reader: &mut BinaryReader, ver: u32) -> Result<Reaction, RawError> {
    let rxn_size = version::reaction_size(ver);
    let start = reader.position();

    // Common fields (all versions): PrecursorMass, IsolationWidth, CollisionEnergy
    let precursor_mz = reader.read_f64()?;
    let isolation_width = reader.read_f64()?;
    let collision_energy = reader.read_f64()?;

    // CollisionEnergyValid (v31+)
    let collision_energy_valid = if ver >= 31 {
        reader.read_u32()?
    } else {
        1 // default: valid, CID
    };

    // RangeIsValid + First/LastPrecursorMass (v65+)
    let (precursor_range_valid, first_precursor_mass, last_precursor_mass) = if ver >= 65 {
        let range_valid = reader.read_i32()? > 0;
        let first = reader.read_f64()?;
        let last = reader.read_f64()?;
        (range_valid, first, last)
    } else {
        (false, 0.0, 0.0)
    };

    // IsolationWidthOffset (v66+)
    let isolation_width_offset = if ver >= 66 {
        reader.read_f64()?
    } else {
        0.0
    };

    // Ensure we advanced exactly rxn_size bytes (handles struct padding)
    let expected_end = start + rxn_size as u64;
    if reader.position() != expected_end {
        reader.set_position(expected_end);
    }

    Ok(Reaction {
        precursor_mz,
        isolation_width,
        collision_energy,
        collision_energy_valid,
        precursor_range_valid,
        first_precursor_mass,
        last_precursor_mass,
        isolation_width_offset,
    })
}

/// Parse a single ScanEvent from the data stream.
///
/// Reads the full ScanEvent structure following the decompiled ScanEvent.Load:
///   preamble → reactions → mass_ranges → mass_calibrators →
///   source_fragmentations → source_fragmentation_mass_ranges → name (v65+)
///
/// Returns the parsed ScanEvent and the final reader position.
pub fn parse_scan_event(
    data: &[u8],
    offset: u64,
    ver: u32,
) -> Result<(ScanEvent, u64), RawError> {
    let preamble_size = version::scan_event_preamble_size(ver);
    let mut reader = BinaryReader::at_offset(data, offset);

    // 1. Read preamble bytes (ScanEventInfoStruct)
    let preamble_bytes = reader.read_bytes(preamble_size)?;
    let mut preamble = parse_preamble(&preamble_bytes);

    // 2. Read reactions array
    let n_precursors = reader.read_u32()?;
    if n_precursors > 100 {
        return Err(RawError::CorruptedData(format!(
            "ScanEvent has unreasonable n_precursors: {}",
            n_precursors
        )));
    }

    let mut reactions = Vec::with_capacity(n_precursors as usize);
    for _ in 0..n_precursors {
        reactions.push(parse_reaction(&mut reader, ver)?);
    }

    // Derive activation type from the last reaction's CollisionEnergyValid
    if let Some(last_rxn) = reactions.last() {
        preamble.activation = last_rxn.activation_type();
    }

    // 3. Read mass ranges: u32 count + count * (f64 low, f64 high)
    let _mass_ranges = read_mass_range_array(&mut reader)?;

    // 4. Read mass calibrators (conversion params): u32 count + count * f64
    let conversion_params = read_doubles_array(&mut reader)?;

    // 5. Read source fragmentations: u32 count + count * f64
    let _source_fragmentations = read_doubles_array(&mut reader)?;

    // 6. Read source fragmentation mass ranges: u32 count + count * (f64, f64)
    let _source_frag_mass_ranges = read_mass_range_array(&mut reader)?;

    // 7. Name string (v65+ only)
    if ver >= 65 {
        let _name = reader.read_pascal_string()?;
    }

    let end_pos = reader.position();

    Ok((
        ScanEvent {
            preamble,
            reactions,
            conversion_params,
        },
        end_pos,
    ))
}

/// Parse all unique scan events from the scan params stream.
///
/// The stream starts with a u32 count followed by that many ScanEvent structures.
/// Returns a Vec indexed by scan_event number (from ScanIndexEntry.scan_event).
pub fn parse_scan_events(
    data: &[u8],
    scan_params_addr: u64,
    ver: u32,
) -> Result<Vec<ScanEvent>, RawError> {
    if scan_params_addr == 0 || scan_params_addr as usize >= data.len() {
        return Ok(vec![]);
    }

    let mut reader = BinaryReader::at_offset(data, scan_params_addr);
    let n_events = reader.read_u32()?;

    if n_events > 10_000 {
        return Err(RawError::CorruptedData(format!(
            "Unreasonable scan event count: {}",
            n_events
        )));
    }

    let mut events = Vec::with_capacity(n_events as usize);
    let mut next_offset = reader.position();

    for _ in 0..n_events {
        let (event, end_pos) = parse_scan_event(data, next_offset, ver)?;
        next_offset = end_pos;
        events.push(event);
    }

    Ok(events)
}

/// Apply conversion parameters to convert frequency to m/z.
///
/// For instruments using frequency-domain detection (FTMS/Orbitrap),
/// the profile data stores frequency values that must be converted.
///
/// - 4 params (LTQ-FT): `m/z = A / (frequency / 1e6 + B)`
/// - 7 params (Orbitrap): polynomial conversion
pub fn frequency_to_mz(frequency: f64, params: &[f64]) -> f64 {
    match params.len() {
        0 => frequency, // No conversion: frequency IS m/z
        4 => {
            // LTQ-FT model: m/z = A / (freq/1e6 + B)
            let a = params[0];
            let b = params[1];
            let freq_mhz = frequency / 1e6;
            if freq_mhz + b != 0.0 {
                a / (freq_mhz + b)
            } else {
                frequency
            }
        }
        7 => {
            // Orbitrap model: polynomial
            // m/z = params[0] / (f^2) + params[1] / f + params[2]
            //       + params[3] * f + params[4] * f^2 + params[5] * f^3 + params[6] * f^4
            // where f = frequency
            if frequency == 0.0 {
                return 0.0;
            }
            let f = frequency;
            let f2 = f * f;
            params[0] / f2
                + params[1] / f
                + params[2]
                + params[3] * f
                + params[4] * f2
                + params[5] * f2 * f
                + params[6] * f2 * f2
        }
        _ => frequency,
    }
}

impl std::fmt::Display for AnalyzerType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalyzerType::Itms => write!(f, "ITMS"),
            AnalyzerType::Tqms => write!(f, "TQMS"),
            AnalyzerType::Sqms => write!(f, "SQMS"),
            AnalyzerType::Tofms => write!(f, "TOFMS"),
            AnalyzerType::Ftms => write!(f, "FTMS"),
            AnalyzerType::Sector => write!(f, "Sector"),
            AnalyzerType::Unknown(n) => write!(f, "Unknown({})", n),
        }
    }
}

impl std::fmt::Display for ActivationType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ActivationType::Cid => write!(f, "CID"),
            ActivationType::Hcd => write!(f, "HCD"),
            ActivationType::Etd => write!(f, "ETD"),
            ActivationType::Ecd => write!(f, "ECD"),
            ActivationType::Unknown(n) => write!(f, "Unknown({})", n),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_preamble_ms1_positive() {
        let mut data = vec![0u8; 80];
        data[4] = 1; // positive
        data[5] = 1; // profile
        data[6] = 1; // MS1
        data[7] = 0; // Full
        data[10] = 0; // not dependent
        data[11] = 5; // NSI
        data[40] = 4; // FTMS

        let preamble = parse_preamble(&data);
        assert_eq!(preamble.polarity, Polarity::Positive);
        assert_eq!(preamble.scan_mode, ScanMode::Profile);
        assert!(matches!(preamble.ms_level, MsLevel::Ms1));
        assert_eq!(preamble.scan_type, ScanType::Full);
        assert!(!preamble.dependent);
        assert_eq!(preamble.ionization, IonizationType::Nsi);
        assert_eq!(preamble.analyzer, AnalyzerType::Ftms);
    }

    #[test]
    fn test_parse_preamble_ms2_negative() {
        let mut data = vec![0u8; 80];
        data[4] = 0; // negative
        data[5] = 0; // centroid
        data[6] = 2; // MS2
        data[7] = 0; // Full
        data[10] = 1; // dependent (DDA)
        data[40] = 0; // ITMS

        let preamble = parse_preamble(&data);
        assert_eq!(preamble.polarity, Polarity::Negative);
        assert_eq!(preamble.scan_mode, ScanMode::Centroid);
        assert!(matches!(preamble.ms_level, MsLevel::Ms2));
        assert!(preamble.dependent);
        assert_eq!(preamble.analyzer, AnalyzerType::Itms);
    }

    #[test]
    fn test_frequency_to_mz_no_params() {
        assert_eq!(frequency_to_mz(500.0, &[]), 500.0);
    }

    #[test]
    fn test_frequency_to_mz_ltq_ft() {
        // A = 1e12, B = 0, freq = 1e6 Hz -> m/z = 1e12 / (1e6/1e6 + 0) = 1e12
        // More realistic: A = 1e11, B = 0, freq = 200000 Hz
        // m/z = 1e11 / (200000/1e6 + 0) = 1e11 / 0.2 = 5e11 (unrealistic example)
        // Just test the math works
        let params = [100.0, 0.0, 0.0, 0.0];
        let mz = frequency_to_mz(1e6, &params);
        // m/z = 100.0 / (1e6/1e6 + 0) = 100.0
        assert!((mz - 100.0).abs() < 1e-6);
    }
}
