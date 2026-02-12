//! RAW file version detection and handling.
//!
//! Thermo RAW files have version numbers typically in the range v57-v66.
//! The version determines the exact layout of internal structures.
//!
//! Key version boundaries (from decompiled ThermoFisher.CommonCore.RawFileReader):
//! - v64: 64-bit addresses, VirtualControllerInfoStruct, RunHeader extended offsets
//! - v65: ScanIndexEntry gains CycleNumber (84→88 bytes with padding),
//!        ScanEventInfoStruct gains new filter flags (128→132 bytes),
//!        RawFileInfo gains BlobOffset/BlobSize,
//!        ScanEvent gains Name field, Reaction gains precursor mass range
//! - v66: RunHeader gains InstrumentType field,
//!        Reaction gains IsolationWidthOffset (48→56 bytes)

/// Minimum supported RAW file version.
pub const MIN_SUPPORTED_VERSION: u32 = 57;
/// Maximum supported RAW file version.
pub const MAX_SUPPORTED_VERSION: u32 = 66;

/// Finnigan file header magic number.
pub const FINNIGAN_MAGIC: u16 = 0xA101;

/// Check whether a RAW file version is supported.
pub fn is_supported(version: u32) -> bool {
    version >= MIN_SUPPORTED_VERSION && version <= MAX_SUPPORTED_VERSION
}

/// Size of a ScanIndexEntry for a given version.
///
/// From decompiled ScanIndices.GetSizeOfScanIndexStructByFileVersion:
/// - v65+: ScanIndexStruct (84 bytes + 4 padding = 88), has CycleNumber
/// - v64:  ScanIndexStruct2 (80 bytes), has 64-bit DataOffset
/// - v<64: ScanIndexStruct1 (72 bytes), 32-bit DataOffset only
pub fn scan_index_entry_size(version: u32) -> usize {
    if version >= 65 {
        88
    } else if version >= 64 {
        80
    } else {
        72
    }
}

/// Whether the version uses 64-bit addresses.
pub fn uses_64bit_addresses(version: u32) -> bool {
    version >= 64
}

/// Size of ScanEventPreamble (ScanEventInfoStruct) for a given version.
///
/// From decompiled ScanEvent.ReadStructure version dispatch:
/// - v65+: ScanEventInfoStruct (132 bytes) - adds UpperFlags, LowerFlags, filter param bytes
/// - v63-64: ScanEventInfoStruct63 (128 bytes) - adds SupplementalActivation, CompensationVoltage
/// - v62: ScanEventInfoStruct62 (120 bytes) - adds PulsedQ, ETD, HCD dissociation
/// - v54-61: ScanEventInfoStruct54 (80 bytes) - adds MassAnalyzer, ECD, MPD, etc.
/// - v51-53: ScanEventInfoStruct51
/// - v48-50: ScanEventInfoStruct50
/// - v31-47: ScanEventInfoStruct3
/// - v<30: ScanEventInfoStruct2
pub fn scan_event_preamble_size(version: u32) -> usize {
    if version >= 65 {
        132
    } else if version >= 63 {
        128
    } else if version >= 62 {
        120
    } else if version >= 57 {
        80
    } else {
        41
    }
}

/// Size of a Reaction (MsReactionStruct) for a given version.
///
/// From decompiled Reaction.Load version dispatch:
/// - v66:   MsReactionStruct  (56 bytes) - adds IsolationWidthOffset
/// - v65:   MsReactionStruct3 (48 bytes) - adds RangeIsValid, First/LastPrecursorMass
/// - v31-64: MsReactionStruct2 (32 bytes) - adds CollisionEnergyValid (with struct padding)
/// - v<31:  MsReactionStruct1 (24 bytes) - PrecursorMass, IsolationWidth, CollisionEnergy only
pub fn reaction_size(version: u32) -> usize {
    if version >= 66 {
        56
    } else if version >= 65 {
        48
    } else if version >= 31 {
        32
    } else {
        24
    }
}
