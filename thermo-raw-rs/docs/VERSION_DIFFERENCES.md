# RAW File Version Differences

> **Source**: Decompiled ThermoFisher.CommonCore.RawFileReader DLL (version dispatch logic
> in ScanIndices, ScanEvent, Reaction, RawFileInfo, RunHeader, Filter classes).

This document details every version-dependent structural change in the Thermo RAW file
format. The version number is stored in the `FileHeader` at offset 54 (u32).

Supported range: **v57 – v66** (this library). The decompiled DLL handles v1–v66.

---

## Quick Reference: Version Boundaries

| Version | Key Changes |
|---------|-------------|
| v < 7   | RawFileInfoStruct1 (no computer name string) |
| v7–24   | RawFileInfoStruct2 (adds computer name) |
| v25–63  | RawFileInfoStruct3 (OldVirtualControllerInfo[64]) |
| v < 31  | MsReactionStruct1 (24 bytes), ScanEventInfoStruct2/3 |
| v31–47  | MsReactionStruct2 (32 bytes), ScanEventInfoStruct3 |
| v48–50  | ScanEventInfoStruct50 |
| v49     | RunHeaderStruct4 (extended 64-bit offsets) |
| v51–53  | ScanEventInfoStruct51 |
| v54–61  | ScanEventInfoStruct54 (80 bytes) |
| v57     | **Minimum version supported by this library** |
| v62     | ScanEventInfoStruct62 (120 bytes) — adds PulsedQ, ETD, HCD |
| v63     | ScanEventInfoStruct63 (128 bytes) — adds SupplementalActivation, CompensationVoltage |
| v64     | 64-bit addresses throughout; VirtualControllerInfoStruct[64]; ScanIndexStruct2 (80 bytes); RawFileInfoStruct4; RunHeaderStruct5 |
| v65     | ScanIndexStruct (88 bytes) with CycleNumber + DataSize; ScanEventInfoStruct (132 bytes); MsReactionStruct3 (48 bytes); RawFileInfoStruct with BlobOffset/BlobSize; ScanEvent gains Name field |
| v66     | MsReactionStruct (56 bytes) with IsolationWidthOffset; RunHeader gains InstrumentType field |

---

## 1. FileHeader

The FileHeader is the same across all supported versions:

| Offset | Size | Field |
|--------|------|-------|
| 0      | 2    | Magic (0xA101 LE) |
| 2      | 36   | Signature ("Finnigan" in UTF-16LE, 18 chars) |
| 38     | 16   | Unknown / reserved |
| 54     | 4    | Version (u32) |

Total: **58 bytes** (all versions).

---

## 2. ScanIndexEntry

Three struct variants exist. The version dispatch is in `ScanIndices.GetSizeOfScanIndexStructByFileVersion`.

### v < 64: ScanIndexStruct1 (72 bytes)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0      | 4    | i32  | DataOffset32Bit (used as scan data offset) |
| 4      | 4    | i32  | TrailerOffset |
| 8      | 4    | i32  | ScanNumber |
| 12     | 4    | u32  | PacketType |
| 16     | 4    | i32  | NumberPackets |
| 20     | 4    | i32  | Unknown1 |
| 24     | 4    | u32  | Unknown2 |
| 28     | 4    | u32  | ScanEvent |
| 32     | 4    | u32  | ScanSegment |
| 36     | 4    | u32  | Unknown3 |
| 40     | 8    | f64  | StartTime (RT in minutes) |
| 48     | 8    | f64  | TotalIonCurrent |
| 56     | 8    | f64  | BasePeakIntensity |
| 64     | 4    | f32  | LowMZ (since v49; v<49 may differ) |
| 68     | 4    | f32  | HighMZ (since v49) |

### v64: ScanIndexStruct2 (80 bytes)

Same as v<64, plus an 8-byte 64-bit data offset at the end:

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0–71   | 72   | —    | (same as ScanIndexStruct1) |
| 72     | 8    | i64  | DataOffset64 (replaces DataOffset32Bit for addressing) |

### v65+: ScanIndexStruct (88 bytes)

The offset 0 field changes meaning, and CycleNumber is added:

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0      | 4    | i32  | **DataSize** (byte size of scan data packet; replaces DataOffset32Bit) |
| 4      | 4    | i32  | TrailerOffset |
| 8      | 4    | i32  | ScanNumber |
| 12     | 4    | u32  | PacketType |
| 16     | 4    | i32  | NumberPackets |
| 20     | 4    | i32  | Unknown1 |
| 24     | 4    | u32  | Unknown2 |
| 28     | 4    | u32  | ScanEvent |
| 32     | 4    | u32  | ScanSegment |
| 36     | 4    | u32  | Unknown3 |
| 40     | 8    | f64  | StartTime |
| 48     | 8    | f64  | TotalIonCurrent |
| 56     | 8    | f64  | BasePeakIntensity |
| 64     | 4    | f32  | LowMZ |
| 68     | 4    | f32  | HighMZ |
| 72     | 8    | i64  | DataOffset64 |
| 80     | 4    | i32  | CycleNumber |
| 84     | 4    | —    | Padding (struct alignment to 8-byte boundary) |

**Key change at v65**: Offset 0 switches from being a 32-bit data offset (used for data
addressing in v<64) to a data size field. Data addressing uses `DataOffset64` exclusively.

---

## 3. ScanEvent / ScanEventInfoStruct (Preamble)

The ScanEvent preamble is a fixed-size struct whose size depends on version. The version
dispatch is in `ScanEvent.ReadStructure`.

### Preamble Size by Version

| Version Range | Struct Name | Size (bytes) |
|---------------|-------------|--------------|
| v < 31        | ScanEventInfoStruct2 | ~41 |
| v31–47        | ScanEventInfoStruct3 | ~41 |
| v48–50        | ScanEventInfoStruct50 | varies |
| v51–53        | ScanEventInfoStruct51 | varies |
| v54–61        | ScanEventInfoStruct54 | 80 |
| v62           | ScanEventInfoStruct62 | 120 |
| v63–64        | ScanEventInfoStruct63 | 128 |
| v65+          | ScanEventInfoStruct | 132 |

### ScanEventInfoStruct54 Layout (80 bytes, v54–v61)

Key fields in the preamble (byte offsets from preamble start):

| Byte | Field | Description |
|------|-------|-------------|
| 0    | MSOrder | 0=MS1, 1=MS2, 2=MS3, etc. |
| 1    | Polarity | 0=Positive, 1=Negative |
| 2    | ScanType | 0=Full, 1=Zoom, 2=SIM, 3=SRM, 4=CRM, 5=Q1MS, 6=Q3MS |
| 3    | ScanMode | (internal Thermo enum) |
| 4    | DependentScanType | |
| 5    | SourceFragmentationType | ActivationType enum for source fragmentation |
| 6    | MassAnalyzer | 0=ITMS, 1=TQMS, 2=SQMS, 3=TOFMS, 4=FTMS, 5=Sector |
| 7    | SourceType | (DetectorType) |
| ...  | ...  | (additional instrument configuration fields) |

### v62 additions (120 bytes)
- PulsedQDissociationType, ETDDissociationType, HCDDissociationType fields
- Extended collision energy parameters

### v63 additions (128 bytes)
- SupplementalActivation (u32)
- CompensationVoltage (f64)

### v65 additions (132 bytes)
- UpperFilterTextFlags (u16)
- LowerFilterTextFlags (u16)
- Additional filter parameter bytes

### Full ScanEvent Stream Layout

After the preamble, additional data is read in sequence by `ScanEvent.Load`:

```
[Preamble]                          — version-dependent fixed size
[NumReactions: u32]
[Reaction[NumReactions]]            — each reaction is version-dependent size
[NumMassRanges: u32]
[MassRange[NumMassRanges]]          — each is 2 x f64 = 16 bytes (low, high)
[NumMassCalibrators: u32]
[MassCalibrators[NumMassCalibrators]] — each is f64 = 8 bytes
[NumSourceFragments: u32]
[SourceFragmentation[NumSourceFragments]] — each is f64 = 8 bytes
[NumSourceFragMassRanges: u32]
[SourceFragMassRange[NumSourceFragMassRanges]] — each is 2 x f64 = 16 bytes
[Name: PascalStringWin32]           — v65+ only
```

Multiple ScanEvents are stored back-to-back. The count is in the GenericDataHeader
that precedes the scan_params stream. Each event's end position determines the start
of the next.

---

## 4. Reaction / MsReactionStruct

Four struct variants. Version dispatch is in `Reaction.Load`.

### v < 31: MsReactionStruct1 (24 bytes)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0      | 8    | f64  | PrecursorMass |
| 8      | 8    | f64  | IsolationWidth |
| 16     | 8    | f64  | CollisionEnergy |

### v31–64: MsReactionStruct2 (32 bytes)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0      | 8    | f64  | PrecursorMass |
| 8      | 8    | f64  | IsolationWidth |
| 16     | 8    | f64  | CollisionEnergy |
| 24     | 4    | u32  | CollisionEnergyValid |
| 28     | 4    | —    | Padding (struct alignment) |

`CollisionEnergyValid` is a bitfield:
- Bit 0: collision energy validity flag
- Bits 1–8: `ActivationType` enum value (CID=0, MPD=1, ECD=2, PQD=3, ETD=4, HCD=5, AnyType=6, SA=7, PTR=8, NETD=9, NPTR=10, UVPD=11, ETHCD=12, ETCID=13)

### v65: MsReactionStruct3 (48 bytes)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0      | 8    | f64  | PrecursorMass |
| 8      | 8    | f64  | IsolationWidth |
| 16     | 8    | f64  | CollisionEnergy |
| 24     | 4    | u32  | CollisionEnergyValid |
| 28     | 4    | bool | PrecursorRangeIsValid (i32 marshalled as bool) |
| 32     | 8    | f64  | FirstPrecursorMass |
| 40     | 8    | f64  | LastPrecursorMass |

### v66: MsReactionStruct (56 bytes)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0      | 8    | f64  | PrecursorMass |
| 8      | 8    | f64  | IsolationWidth |
| 16     | 8    | f64  | CollisionEnergy |
| 24     | 4    | u32  | CollisionEnergyValid |
| 28     | 4    | bool | PrecursorRangeIsValid |
| 32     | 8    | f64  | FirstPrecursorMass |
| 40     | 8    | f64  | LastPrecursorMass |
| 48     | 8    | f64  | IsolationWidthOffset |

---

## 5. RawFileInfo

Version dispatch is in `RawFileInfo.Load`. The preamble is common to all versions.

### Common Preamble (all versions)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0      | 4    | i32  | IsExpMethodPresent (bool as i32) |
| 4      | 16   | struct | SYSTEMTIME (8 x u16: Year, Month, DayOfWeek, Day, Hour, Minute, Second, Milliseconds) |
| 20     | 4    | i32  | IsInAcquisition (bool as i32) |
| 24     | 4    | u32  | VirtualDataOffset32 |
| 28     | 4    | i32  | NumberOfVirtualControllers |
| 32     | 4    | i32  | NextAvailableControllerIndex |

### v25–63: RawFileInfoStruct3

After preamble:
- **OldVirtualControllerInfo[64]** (12 bytes each = 768 bytes total)
  - VirtualDeviceType (i32) + VirtualDeviceIndex (i32) + Offset (i32)
  - First controller's Offset → 32-bit RunHeader address

### v64: RawFileInfoStruct4

After preamble:
- OldVirtualControllerInfo[64] (768 bytes) — same as v25–63
- **VirtualDataOffset** (i64) — 64-bit version of VirtualDataOffset32
- **VirtualControllerInfoStruct[64]** (16 bytes each = 1024 bytes total)
  - VirtualDeviceType (i32) + VirtualDeviceIndex (i32) + Offset (i64)
  - First controller's Offset → 64-bit RunHeader address

### v65+: RawFileInfoStruct (current)

After preamble:
- OldVirtualControllerInfo[64] (768 bytes)
- VirtualDataOffset (i64)
- VirtualControllerInfoStruct[64] (1024 bytes)
- **BlobOffset** (i64) — offset to embedded blob data, -1 if none
- **BlobSize** (u32) — blob size in bytes, 0 if none

### After the struct: Heading strings

Read in order after the fixed struct:
1. 5 x User Label strings (PascalStringWin32)
2. Computer Name string (v7+ only, PascalStringWin32)

---

## 6. RunHeader

Version dispatch is in `RunHeader.Load`. The RunHeader contains scan range metadata
and pointers to major data streams.

### Common Fields (all versions)

These fields appear in all RunHeader variants (field names from decompiled source):

| Field | Type | Description |
|-------|------|-------------|
| SampleInfoStruct | struct | Sample metadata (embedded) |
| FileName1, FileName2, etc. | PascalStringWin32 | Various path strings |
| NScans (ScanCount) | i32 | Number of scans |
| StartTime, EndTime | f64 | Acquisition time range (minutes) |
| LowMass, HighMass | f64 | Mass range |
| MaxIntegratedIntensity | f64 | Max TIC value |
| FirstScanNumber | i32 | First scan number (usually 1) |
| LastScanNumber | i32 | Last scan number |

### v49+ (RunHeaderStruct4 / RunHeaderStruct5): Extended Offsets

These versions add 64-bit stream addresses after the main struct:

| Field | Type | Description |
|-------|------|-------------|
| SpectPos | i64 | Spectrum data stream offset |
| PacketPos | i64 | Packet data stream offset |
| StatusLogPos | i64 | Status log stream offset |
| ErrorLogPos | i64 | Error log stream offset |
| RunHeaderPos | i64 | RunHeader position (self-reference) |
| TrailerScanEventsPos | i64 | Scan events (scan_params) stream offset |
| TrailerExtraPos | i64 | Trailer extra data stream offset |

### v64+ (RunHeaderStruct5): Additional Fields

After the 7 stream offsets:
- VirtualControllerInfoStruct (16 bytes, skipped)
- Extra0Pos through Extra5Pos (6 x i64) + Extra0Count through Extra5Count (6 x i32)
  - Total: 72 bytes of extra stream pointers

### v66+: InstrumentType

After the extra stream pointers:
- **InstrumentType** (i32) — identifies the instrument model/type

---

## 7. ScanDataPacket

The scan data packet format does not change between versions (the version affects how
you find the packet via the ScanIndex, but the packet layout itself is stable).

### Packet Header (40 bytes)

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0      | 4    | u32  | Unknown1 |
| 4      | 4    | u32  | ProfileSize (in 4-byte words) |
| 8      | 4    | u32  | PeakListSize (in 4-byte words) |
| 12     | 4    | u32  | Layout (0=no fudge, >0=with fudge per chunk) |
| 16     | 4    | u32  | DescriptorListSize |
| 20     | 4    | u32  | UnknownStreamSize |
| 24     | 4    | u32  | TripletStreamSize |
| 28     | 4    | u32  | Unknown2 |
| 32     | 4    | f32  | LowMZ |
| 36     | 4    | f32  | HighMZ |

After the header:
1. Profile data (ProfileSize * 4 bytes)
2. Peak list / centroid data (PeakListSize * 4 bytes)
3. Peak descriptors (DescriptorListSize * entries)
4. Unknown stream (UnknownStreamSize * entries)
5. Triplet stream (TripletStreamSize * entries)

---

## 8. Trailer Extra Data

The trailer extra stream uses a GenericDataHeader followed by per-scan records. The
header defines field names, types, and offsets. This is version-independent (the header
is self-describing).

### GenericDataHeader

| Field | Type | Description |
|-------|------|-------------|
| NumEntries | u32 | Number of scan records |
| NumFields | u32 | Number of fields per record |
| RecordSize | u32 | Size of each record in bytes |
| Fields[NumFields] | struct | Field descriptors |

Each field descriptor:
| Field | Type | Description |
|-------|------|-------------|
| Label | PascalStringWin32 | Field name (e.g., "Monoisotopic M/Z") |
| DataType | u16 | Type code (8=f64, 10=i32, 12=string, etc.) |
| Offset | u16 | Byte offset within record |
| Size | u16 | Field size in bytes |

### Important Trailer Fields

| Label | Type | Used For |
|-------|------|----------|
| "Filter Text" or "Master Scan Filter" | string | Scan filter string (MS level, polarity, etc.) |
| "Monoisotopic M/Z" | f64 | Accurate precursor m/z |
| "Charge State" | i32 | Precursor charge |
| "MS2 Isolation Width" | f64 | Isolation window width |
| "Master Index" | i32 | Master scan reference |
| "Ion Injection Time (ms)" | f64 | Injection time |
| "Elapsed Scan Time (sec)" | f64 | Scan duration |

---

## 9. PascalStringWin32

All strings in RAW files use the PascalStringWin32 encoding:

| Offset | Size | Type | Field |
|--------|------|------|-------|
| 0      | 4    | i32  | Length (number of UTF-16LE code units, NOT bytes) |
| 4      | Length*2 | u8[] | UTF-16LE encoded characters |

If length is 0 or negative, the string is empty (no bytes follow).

---

## 10. Version Progression Summary

### What each major version adds

**v31**: MsReactionStruct2 — CollisionEnergyValid (with ActivationType encoding)

**v49**: RunHeaderStruct4 — 64-bit stream addresses (SpectPos, PacketPos, etc.)

**v54**: ScanEventInfoStruct54 (80 bytes) — MassAnalyzer, ECD, MPD fields

**v62**: ScanEventInfoStruct62 (120 bytes) — PulsedQ, ETD, HCD dissociation types

**v63**: ScanEventInfoStruct63 (128 bytes) — SupplementalActivation, CompensationVoltage

**v64** (major upgrade):
- 64-bit addressing throughout (ScanIndex, RawFileInfo, RunHeader)
- ScanIndexStruct2 (80 bytes): adds DataOffset64 field
- RawFileInfoStruct4: adds VirtualControllerInfoStruct[64] (16-byte entries with i64 offset)
- RunHeaderStruct5: adds VirtualControllerInfoStruct + Extra0–5 stream pointers

**v65** (major upgrade):
- ScanIndexStruct (88 bytes): offset 0 becomes DataSize (not offset), adds CycleNumber
- ScanEventInfoStruct (132 bytes): adds UpperFlags, LowerFlags, filter parameter bytes
- MsReactionStruct3 (48 bytes): adds PrecursorRangeIsValid, First/LastPrecursorMass
- RawFileInfoStruct: adds BlobOffset (i64) + BlobSize (u32)
- ScanEvent gains Name field (PascalStringWin32 after source frag mass ranges)

**v66**:
- MsReactionStruct (56 bytes): adds IsolationWidthOffset (f64)
- RunHeader: adds InstrumentType (i32) field

---

## 11. Struct Size Summary Table

| Structure | v < 31 | v31–53 | v54–61 | v62 | v63–64 | v65 | v66 |
|-----------|--------|--------|--------|-----|--------|-----|-----|
| ScanIndexEntry | 72 | 72 | 72 | 72 | 72/80* | 88 | 88 |
| ScanEventPreamble | ~41 | ~41 | 80 | 120 | 128 | 132 | 132 |
| Reaction | 24 | 32 | 32 | 32 | 32 | 48 | 56 |
| ScanDataPacket header | 40 | 40 | 40 | 40 | 40 | 40 | 40 |

\* v64 ScanIndex is 80 bytes; v<64 is 72 bytes.

---

## 12. ActivationType Enum

Encoded in bits 1–8 of MsReactionStruct's `CollisionEnergyValid` field:

| Value | Name | Description |
|-------|------|-------------|
| 0     | CID  | Collision-Induced Dissociation |
| 1     | MPD  | Multi-Photon Dissociation |
| 2     | ECD  | Electron Capture Dissociation |
| 3     | PQD  | Pulsed-Q Dissociation |
| 4     | ETD  | Electron Transfer Dissociation |
| 5     | HCD  | Higher-energy Collisional Dissociation |
| 6     | AnyType | Any activation type |
| 7     | SA   | Supplemental Activation |
| 8     | PTR  | Proton Transfer Reaction |
| 9     | NETD | Negative ETD |
| 10    | NPTR | Negative PTR |
| 11    | UVPD | Ultraviolet Photodissociation |
| 12    | ETHCD | ETD + HCD |
| 13    | ETCID | ETD + CID |

Extraction: `activation_type = (collision_energy_valid >> 1) & 0xFF`
