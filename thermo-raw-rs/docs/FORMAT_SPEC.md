# Thermo RAW Binary Format Specification

> **Sources**: Decompiled ThermoFisher.CommonCore.RawFileReader (v6.8.0.0),
> unfinnigan (Gene Selkov), unthermo (Pieter Kelchtermans), Finnigan CPAN distribution.
> Covers RAW file versions 57-66 (modern instruments: LTQ, Orbitrap, Q Exactive, Exploris).

## 1. Container Layer: OLE2/CFBF

Thermo RAW files use the Microsoft Compound Binary File Format (OLE2) as their outer container.

**OLE2 Magic**: `D0 CF 11 E0 A1 B1 1A E1` at offset 0.

The Finnigan data structures are stored within OLE2 streams. All addresses in the
Finnigan structures are absolute byte offsets within the main data stream (the root
entry stream of the OLE2 container).

### Known OLE2 Storages/Streams

| Path Pattern               | Purpose                                    |
|----------------------------|--------------------------------------------|
| (root entry stream)        | Main Finnigan data (all structures below)  |
| `<InstrumentName>/Data`    | Binary instrument method                   |
| `<InstrumentName>/Text`    | Human-readable method text                 |
| `<InstrumentName>/Header`  | Metadata header (mass analyzer only)       |

Instrument names vary: `LTQ`, `EksigentNanoLcCom_DLL`, `NanoLC-AS1 Autosampler`, etc.

## 2. String Encoding: PascalStringWin32

All strings in the format are length-prefixed UTF-16LE:

| Offset | Type   | Size           | Field  |
|--------|--------|----------------|--------|
| 0      | i32    | 4 bytes        | length (number of UTF-16 code units) |
| 4      | u16[]  | 2 * length     | UTF-16LE text |

## 3. FileHeader

The first structure in the main data stream. Identifies the file and its version.

From decompiled `FileHeader.cs` and `FileHeaderStruct`:

| Offset | Type       | Size       | Field              | Notes                           |
|--------|------------|------------|--------------------|---------------------------------|
| 0      | u16        | 2          | magic              | Must be `0xA101` (FinnID)       |
| 2      | UTF16LE    | 36 (18ch)  | signature          | Must be `"Finnigan"` (FinnSig)  |
| 38     | u32        | 4          | unknown1           |                                 |
| 42     | u32        | 4          | unknown2           |                                 |
| 46     | u32        | 4          | unknown3           |                                 |
| 50     | u32        | 4          | unknown4           |                                 |
| 54     | u32        | 4          | **version** (FileRev) | Key field: e.g. 57,60,62,63,64,65,66 |
| 58     | AuditTag   | 112        | audit_start (Created)  | Creation timestamp + user       |
| 170    | AuditTag   | 112        | audit_end (Changed)    | Last modification               |
| 282    | u32        | 4          | unknown5           |                                 |
| 286    | bytes      | 60         | unknown_area       |                                 |
| 346    | UTF16LE    | 2056 (1028ch) | FileDescription  | File description text           |

**Total FileHeader size**: ~2402 bytes (fixed).

The current DLL writes version 66 by default. Files with version > 66 are considered
"newer revision" by the DLL's `IsNewerRevision()` method.

### AuditTag (112 bytes)

| Offset | Type       | Size       | Field         |
|--------|------------|------------|---------------|
| 0      | u64        | 8          | time (Windows FILETIME) |
| 8      | UTF16LE    | 100 (50ch) | tag1 (user)   |
| 108    | u32        | 4          | unknown (possibly CRC) |

### Checksum

For version >= 57, the file header uses Adler32 checksum:
- Seed: computed from the header struct bytes
- Data: up to 10 MB of file data after the header
- Stored in `FileHeaderStruct.CheckSum`

## 4. RawFileInfoPreamble

Located after the FileHeader. Contains acquisition date and pointers to RunHeaders.

From decompiled `RawFileInfo.cs` and `RawFileInfoStruct*`:

### Common preamble (all versions v25+):

| Offset | Type   | Size | Field                   | Notes |
|--------|--------|------|-------------------------|-------|
| 0      | i32    | 4    | IsExpMethodPresent      | bool marshalled as i32 |
| 4      | u16x8  | 16   | SystemTimeStruct        | Win32 SYSTEMTIME (year,month,dow,day,hour,min,sec,ms) |
| 20     | i32    | 4    | IsInAcquisition         | bool as i32 |
| 24     | u32    | 4    | VirtualDataOffset32     | 32-bit data offset |
| 28     | i32    | 4    | NumberOfVirtualControllers |  |
| 32     | i32    | 4    | NextAvailableControllerIndex | |

### OldVirtualControllerInfo[64] (all versions v25+):

| Offset | Type   | Size | Field                   |
|--------|--------|------|-------------------------|
| 36     | struct | 768  | 64 entries x 12 bytes each |

Each OldVirtualControllerInfo (12 bytes):

| Offset | Type | Size | Field |
|--------|------|------|-------|
| 0      | i32  | 4    | VirtualDeviceType (enum) |
| 4      | i32  | 4    | VirtualDeviceIndex |
| 8      | i32  | 4    | Offset (32-bit file pointer) |

### Version 64+ additional fields (after OldVirtualControllerInfo[64]):

| Offset  | Type   | Size | Field                   |
|---------|--------|------|-------------------------|
| 804     | i64    | 8    | VirtualDataOffset (64-bit) |
| 812     | struct | 1024 | VirtualControllerInfoStruct[64] |

Each VirtualControllerInfoStruct (16 bytes):

| Offset | Type | Size | Field |
|--------|------|------|-------|
| 0      | i32  | 4    | VirtualDeviceType (enum) |
| 4      | i32  | 4    | VirtualDeviceIndex |
| 8      | i64  | 8    | Offset (64-bit file pointer) |

### Version 65+ additional fields (after VirtualControllerInfoStruct[64]):

| Offset  | Type | Size | Field       |
|---------|------|------|-------------|
| 1836    | i64  | 8    | BlobOffset  |
| 1844    | u32  | 4    | BlobSize    |

### After the struct: variable-length strings

| Order | Type              | Field         |
|-------|-------------------|---------------|
| 1-5   | PascalStringWin32 | UserLabels[5] |
| 6     | PascalStringWin32 | ComputerName (v7+) |

## 5. RunHeader

The primary index structure for each instrument. Located at the address from
the first VirtualControllerInfo's Offset field.

From decompiled `RunHeader.cs` and `RunHeaderStruct*`:

### SampleInfo (nested at start of RunHeader)

| Offset | Type   | Size | Field                   | Notes |
|--------|--------|------|-------------------------|-------|
| 0      | i16    | 2    | Revision                | Software revision level |
| 2      | i32    | 4    | DataSetID               |  |
| 6      | i32    | 4    | **FirstSpectrum**       |  |
| 10     | i32    | 4    | **LastSpectrum**        |  |
| 14     | i32    | 4    | NumStatusLog            |  |
| 18     | i32    | 4    | NumErrorLog             |  |
| 22     | i32    | 4    | FileFlag                |  |
| 26     | i32    | 4    | SpectPos32Bit           | Scan index offset (32-bit) |
| 30     | i32    | 4    | PacketPos32Bit          | Data stream offset (32-bit) |
| 34     | i32    | 4    | StatusLogPos32Bit       |  |
| 38     | i32    | 4    | ErrorLogPos32Bit        |  |
| 42     | i16    | 2    | MaxPacket               |  |
| 44     | f64    | 8    | MaxIntegIntensity       |  |
| 52     | f64    | 8    | **LowMass**             |  |
| 60     | f64    | 8    | **HighMass**            |  |
| 68     | f64    | 8    | **StartTime** (minutes) |  |
| 76     | f64    | 8    | **EndTime** (minutes)   |  |

Followed by instrument metadata, 13 filename strings (each 260 UTF-16 chars = 520 bytes),
and trailer/tune data positions (see RunHeaderStruct source for full layout).

### Version 49+ additional fields (RunHeaderStruct4):

| Field              | Type | Size |
|--------------------|------|------|
| ToleranceUnit      | i32  | 4    |
| FilterMassPrecision| i32  | 4    |

### Version 64+ additional fields (RunHeaderStruct5):

64-bit offset versions of all position fields:

| Field                 | Type | Size |
|-----------------------|------|------|
| SpectPos              | i64  | 8    |
| PacketPos             | i64  | 8    |
| StatusLogPos          | i64  | 8    |
| ErrorLogPos           | i64  | 8    |
| RunHeaderPos          | i64  | 8    |
| TrailerScanEventsPos  | i64  | 8    |
| TrailerExtraPos       | i64  | 8    |
| ControllerInfo        | VirtualControllerInfoStruct | 16 |
| Extra0..5 Pos/Count   | (i64 + i32) x 6 | 72 |

### Version 66+ additional field (RunHeaderStruct):

| Field          | Type | Size | Notes |
|----------------|------|------|-------|
| InstrumentType | i32  | 4    | Numeric instrument type identifier |

After the fixed fields: PascalStringWin32 strings (device name, model, serial number,
software version, tag1-tag4).

For v<=63, 32-bit offsets are promoted to 64-bit via StructureConversion.ConvertFrom32Bit.

## 6. ScanIndex

Array of `ScanIndexEntry`, one per scan. Located at `SpectPos`/`SpectPos32Bit`.

From decompiled `ScanIndices.cs` and `ScanIndexStruct*`:

### ScanIndexEntry v<64: ScanIndexStruct1 (72 bytes)

| Offset | Type  | Size | Field                |
|--------|-------|------|----------------------|
| 0      | u32   | 4    | DataOffset32Bit      |
| 4      | i32   | 4    | TrailerOffset        |
| 8      | i32   | 4    | ScanTypeIndex (HIWORD=segment, LOWORD=scan type) |
| 12     | i32   | 4    | ScanNumber           |
| 16     | u32   | 4    | PacketType (HIWORD=SIScanData, LOWORD=scan type) |
| 20     | i32   | 4    | NumberPackets        |
| 24     | f64   | 8    | **StartTime** (RT)   |
| 32     | f64   | 8    | **TIC**              |
| 40     | f64   | 8    | **BasePeakIntensity**|
| 48     | f64   | 8    | **BasePeakMass**     |
| 56     | f64   | 8    | **LowMass**          |
| 64     | f64   | 8    | **HighMass**         |

### ScanIndexEntry v64: ScanIndexStruct2 (80 bytes)

Same as v<64, plus:

| Offset | Type  | Size | Field                |
|--------|-------|------|----------------------|
| 72     | i64   | 8    | **DataOffset** (64-bit) |

Note: `DataOffset32Bit` at offset 0 exists but is ignored; `DataSize` is set to 0.

### ScanIndexEntry v65+: ScanIndexStruct (88 bytes)

| Offset | Type  | Size | Field                |
|--------|-------|------|----------------------|
| 0      | u32   | 4    | **DataSize** (replaces DataOffset32Bit) |
| 4      | i32   | 4    | TrailerOffset        |
| 8      | i32   | 4    | ScanTypeIndex        |
| 12     | i32   | 4    | ScanNumber           |
| 16     | u32   | 4    | PacketType           |
| 20     | i32   | 4    | NumberPackets        |
| 24     | f64   | 8    | StartTime            |
| 32     | f64   | 8    | TIC                  |
| 40     | f64   | 8    | BasePeakIntensity    |
| 48     | f64   | 8    | BasePeakMass         |
| 56     | f64   | 8    | LowMass              |
| 64     | f64   | 8    | HighMass             |
| 72     | i64   | 8    | **DataOffset** (64-bit) |
| 80     | i32   | 4    | **CycleNumber** (scan event cycle association) |
| 84     | --    | 4    | (struct alignment padding) |

## 7. ScanDataPacket

Located in the data stream at offset from ScanIndexEntry. Contains spectral data.

### PacketHeader (40 bytes)

| Offset | Type  | Size | Field                      |
|--------|-------|------|----------------------------|
| 0      | u32   | 4    | unknown1                   |
| 4      | u32   | 4    | **profile_size** (in u32 units) |
| 8      | u32   | 4    | **peak_list_size** (in u32 units) |
| 12     | u32   | 4    | **layout** (profile chunk format flag) |
| 16     | u32   | 4    | descriptor_list_size       |
| 20     | u32   | 4    | unknown_stream_size        |
| 24     | u32   | 4    | triplet_stream_size        |
| 28     | u32   | 4    | unknown2                   |
| 32     | f32   | 4    | low_mz                     |
| 36     | f32   | 4    | high_mz                    |

### Reading sequence after PacketHeader:

1. **Profile data** (4 bytes x profile_size)
2. **Peak list / centroids** (4 bytes x peak_list_size)
3. **Peak descriptors** (descriptor_list_size entries)
4. **Unknown stream** (unknown_stream_size entries)
5. **Triplet stream** (triplet_stream_size entries)

### Profile Structure

| Offset | Type  | Size | Field                |
|--------|-------|------|----------------------|
| 0      | f64   | 8    | first_value          |
| 8      | f64   | 8    | step                 |
| 16     | u32   | 4    | peak_count (# chunks)|
| 20     | u32   | 4    | nbins (total)        |

Followed by `peak_count` ProfileChunk structures.

### ProfileChunk (layout == 0)

| Offset | Type    | Size         | Field           |
|--------|---------|--------------|-----------------|
| 0      | u32     | 4            | first_bin       |
| 4      | u32     | 4            | nbins           |
| 8      | f32[]   | 4 * nbins    | signal (intensities) |

### ProfileChunk (layout > 0)

| Offset | Type    | Size         | Field           |
|--------|---------|--------------|-----------------|
| 0      | u32     | 4            | first_bin       |
| 4      | u32     | 4            | nbins           |
| 8      | f32     | 4            | fudge (drift correction) |
| 12     | f32[]   | 4 * nbins    | signal (intensities) |

### m/z Reconstruction from Profile

For each bin `i` in a chunk:
```
frequency = first_value + (first_bin + i) * step
m/z = convert(frequency)  // using ScanEvent conversion params
```

For LTQ-FT (4 params): `m/z = A / (frequency + B)`
For Orbitrap (7 params): more complex polynomial.

### PeakList (Centroid Data)

| Offset | Type  | Size | Field           |
|--------|-------|------|-----------------|
| 0      | u32   | 4    | count           |
| 4+     | f32   | 4    | mz per peak     |
| 4+     | f32   | 4    | intensity per peak |

Peaks stored as interleaved (mz, intensity) pairs, each f32.

### PeakDescriptor (4 bytes each)

| Offset | Type  | Size | Field      |
|--------|-------|------|------------|
| 0      | u16   | 2    | index      |
| 2      | u8    | 1    | flags      |
| 3      | u8    | 1    | charge     |

## 8. ScanEvent / ScanEventPreamble

Describes acquisition parameters for each scan type.

From decompiled `ScanEvent.cs` and `ScanEventInfoStruct*`:

### ScanEvent stream layout

The scan event stream (at `TrailerExtraPos`) starts with a u32 count,
followed by that many ScanEvent structures. Each ScanEvent is:

1. **ScanEventInfoStruct** (preamble, version-dependent fixed size)
2. **Reactions array**: u32 count + count * MsReactionStruct
3. **MassRanges**: u32 count + count * (f64 low, f64 high)
4. **MassCalibrators**: u32 count + count * f64 (Hz-to-m/z coefficients)
5. **SourceFragmentations**: u32 count + count * f64
6. **SourceFragmentationMassRanges**: u32 count + count * (f64, f64)
7. **Name**: PascalStringWin32 (v65+ only)

### ScanEventPreamble size by version:

From decompiled `ScanEvent.ReadStructure`:

| Version  | Struct                | Size (bytes) |
|----------|----------------------|-------------|
| v<30     | ScanEventInfoStruct2 | ~20 (custom read) |
| v30-47   | ScanEventInfoStruct3 | ~25 |
| v48-50   | ScanEventInfoStruct50| ~28 |
| v51-53   | ScanEventInfoStruct51| ~32 |
| v54-61   | ScanEventInfoStruct54| 80  |
| v62      | ScanEventInfoStruct62| 120 |
| v63-64   | ScanEventInfoStruct63| 128 |
| v65+     | ScanEventInfoStruct  | 132 |

### Key byte positions in ScanEventPreamble (all versions):

| Byte | Field          | Values                                              |
|------|----------------|-----------------------------------------------------|
| 0    | IsValid        | Validity flag                                       |
| 1    | IsCustom       | Custom scan flag                                    |
| 2    | Corona         | Corona discharge state                              |
| 3    | Detector       | Detector type                                       |
| 4    | Polarity       | 0=negative, 1=positive, 2=undefined                 |
| 5    | ScanDataType   | 0=centroid, 1=profile, 2=undefined                  |
| 6    | MSOrder        | 0=undef, 1=MS1, 2=MS2, ..., 8=MS8 (sbyte in v65+)  |
| 7    | ScanType       | 0=Full, 1=Zoom, 2=SIM, 3=SRM, 4=CRM                |
| 8    | SourceFrag     | Source fragmentation state                          |
| 9    | TurboScan      | Turbo scan state                                    |
| 10   | DependentData  | 0=primary, 1=dependent (DDA)                        |
| 11   | IonizationMode | 0=EI,1=CI,2=FAB,3=ESI,4=APCI,5=NSI,6=TSI,7=FDI,8=MALDI |
| 24   | SourceFragType | Source fragmentation type (NOT activation type)     |
| 40   | MassAnalyzer   | 0=ITMS,1=TQMS,2=SQMS,3=TOFMS,4=FTMS,5=Sector (v54+) |

### v65+ new preamble fields:

- `UpperCaseFilterFlags` (at offset 12, within existing padding before DetectorValue)
- `LowerFlags` (ushort, at offset 25-26, within existing padding before ScanTypeIndex)
- `Multiplex`, `ParamA`, `ParamB`, `ParamF`, `SpsMultiNotch`, `ParamR`, `ParamV` (bytes, at end)

### Reaction (MsReactionStruct)

From decompiled `Reaction.cs` and `MsReactionStruct*`:

#### MsReactionStruct1 (24 bytes, v<31):

| Offset | Type  | Size | Field              |
|--------|-------|------|--------------------|
| 0      | f64   | 8    | PrecursorMass      |
| 8      | f64   | 8    | IsolationWidth     |
| 16     | f64   | 8    | CollisionEnergy    |

#### MsReactionStruct2 (32 bytes with padding, v31-64):

| Offset | Type  | Size | Field              |
|--------|-------|------|--------------------|
| 0      | f64   | 8    | PrecursorMass      |
| 8      | f64   | 8    | IsolationWidth     |
| 16     | f64   | 8    | CollisionEnergy    |
| 24     | u32   | 4    | CollisionEnergyValid |
| 28     | --    | 4    | (struct alignment padding) |

`CollisionEnergyValid` encoding:
- Bit 0: valid flag
- Bits 1-8 (0xFFE >> 1): ActivationType enum (0=CID, 1=HCD, 2=ETD, 3=ECD, ...)
- Bit 12 (0x1000): multiple activation flag

#### MsReactionStruct3 (48 bytes, v65):

| Offset | Type  | Size | Field              |
|--------|-------|------|--------------------|
| 0      | f64   | 8    | PrecursorMass      |
| 8      | f64   | 8    | IsolationWidth     |
| 16     | f64   | 8    | CollisionEnergy    |
| 24     | u32   | 4    | CollisionEnergyValid |
| 28     | i32   | 4    | RangeIsValid (bool)|
| 32     | f64   | 8    | FirstPrecursorMass |
| 40     | f64   | 8    | LastPrecursorMass  |

#### MsReactionStruct (56 bytes, v66+):

| Offset | Type  | Size | Field              |
|--------|-------|------|--------------------|
| 0-47   | ...   | 48   | (same as v65)      |
| 48     | f64   | 8    | IsolationWidthOffset |

## 9. Filter (scan filter)

From decompiled `Filter.cs`:

Filter is read after preamble (via `Utilities.ReadStructure`) and contains:
1. **FilterInfoStruct** (version-dependent fixed-size struct)
2. **Masses**: u32 count + count * f64
3. **MassRanges**: u32 count + count * (f64, f64)
4. **SourceFragmentations** (v25+): u32 count + count * f64
5. **SourceFragmentationMassRanges** (v25+): u32 count + count * (f64, f64)
6. **PrecursorEnergies** (v31+): u32 count + count * f64
7. **PrecursorEnergiesValid** (v31+): u32 count + count * u32
8. **SourceFragmentationInfoValid** (v31+): u32 count + count * i32
9. **Name** (v65+): PascalStringWin32
10. **PrecursorMassRanges** (v65+): u32 count + count * (f64, f64)
11. **PrecursorMassRangesValid** (v65+): u32 count + count * u32

## 10. Trailer Extra (Self-Describing Records)

Located at `TrailerScanEventsPos`/`TrailerScanEventsPos32Bit`. Uses a metadata-driven approach.

### GenericDataHeader

| Offset | Type              | Size     | Field            |
|--------|-------------------|----------|------------------|
| 0      | u32               | 4        | n_fields         |
| 4+     | GenericDescriptor | variable | field definitions |

### GenericDataDescriptor

| Offset | Type              | Size     | Field    |
|--------|-------------------|----------|----------|
| 0      | u32               | 4        | type_code|
| 4      | u32               | 4        | length   |
| 8      | PascalStringWin32 | variable | label    |

### Type Codes

| Code | Type                    |
|------|-------------------------|
| 0x1  | bool (1 byte)           |
| 0x2  | i8                      |
| 0x3  | i16                     |
| 0x4  | i32                     |
| 0x5  | f32                     |
| 0x6  | f64                     |
| 0x7  | u8                      |
| 0x8  | u16                     |
| 0x9  | u32                     |
| 0xC  | null-terminated ASCII   |
| 0xD  | UTF-16LE wide string    |

### GenericRecord

After the header, each scan has a record with fields matching the header descriptors.
Common labels include: `"Charge State:"`, `"Monoisotopic M/Z:"`,
`"Ion Injection Time (ms):"`, `"Elapsed Scan Time (sec):"`, etc.

## 11. Overall File Layout (Reading Sequence)

```
[OLE2 Header: 512 bytes, magic d0 cf 11 e0 a1 b1 1a e1]
[OLE2 FAT / Directory structures]

Within the main data stream:
  [0]              FileHeader (magic 0xA101, version, ~2402 bytes)
  [after header]   SequencerRow (optional)
  [...]            AutoSamplerInfo (optional)
  [...]            RawFileInfoPreamble + heading strings
                     -> VirtualControllerInfo[0].Offset = run_header_addr
  [run_header_addr] RunHeader
                     -> SampleInfo (first/last scan, time range, mass range)
                     -> SpectPos, PacketPos, TrailerScanEventsPos, TrailerExtraPos
  [SpectPos]       ScanIndexEntry[first_scan..last_scan]
                     -> per-scan: offset, RT, TIC, base peak, data size
  [PacketPos+off]  ScanDataPacket (per scan)
                     -> PacketHeader -> Profile -> PeakList -> Descriptors
  [TrailerScanEventsPos] GenericDataHeader (template)
                         GenericRecord[first_scan..last_scan]
  [TrailerExtraPos]      ScanEvent[n_events] (unique event templates)
```

## 12. Version Differences Summary

| Feature              | v57-63   | v64       | v65       | v66       |
|----------------------|----------|-----------|-----------|-----------|
| Address width        | 32-bit   | **64-bit**| 64-bit    | 64-bit    |
| ScanIndexEntry size  | 72 bytes | 80 bytes  | **88 bytes** | 88 bytes |
| ScanIndex CycleNumber| No       | No        | **Yes**   | Yes       |
| ScanIndex DataSize   | No       | No        | **Yes**   | Yes       |
| Preamble size        | 80 bytes | 128 bytes | **132 bytes** | 132 bytes |
| Reaction size        | 32 bytes | 32 bytes  | **48 bytes** | **56 bytes** |
| ScanEvent Name       | No       | No        | **Yes**   | Yes       |
| RunHeader InstrType  | No       | No        | No        | **Yes**   |
| RawFileInfo Blob     | No       | No        | **Yes**   | Yes       |
| VirtualCtlInfoStruct | No       | **Yes**   | Yes       | Yes       |
