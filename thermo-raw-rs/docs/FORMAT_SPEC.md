# Thermo RAW Binary Format Specification

> **Sources**: unfinnigan (Gene Selkov), unthermo (Pieter Kelchtermans), Finnigan CPAN distribution.
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

| Offset | Type       | Size       | Field              | Notes                           |
|--------|------------|------------|--------------------|---------------------------------|
| 0      | u16        | 2          | magic              | Must be `0xA101`                |
| 2      | UTF16LE    | 36 (18ch)  | signature          | Format signature string         |
| 38     | u32        | 4          | unknown1           |                                 |
| 42     | u32        | 4          | unknown2           |                                 |
| 46     | u32        | 4          | unknown3           |                                 |
| 50     | u32        | 4          | unknown4           |                                 |
| 54     | u32        | 4          | **version**        | Key field: 8,47,57,60,62,63,64,66 |
| 58     | AuditTag   | 112        | audit_start        | Creation timestamp + user       |
| 170    | AuditTag   | 112        | audit_end          | Last modification               |
| 282    | u32        | 4          | unknown5           |                                 |
| 286    | bytes      | 60         | unknown_area       |                                 |
| 346    | UTF16LE    | 2056 (1028ch) | tag             | File description                |

**Total FileHeader size**: ~2402 bytes (fixed).

### AuditTag (112 bytes)

| Offset | Type       | Size       | Field         |
|--------|------------|------------|---------------|
| 0      | u64        | 8          | time (Windows FILETIME) |
| 8      | UTF16LE    | 100 (50ch) | tag1 (user)   |
| 108    | u32        | 4          | unknown (possibly CRC) |

## 4. RawFileInfoPreamble

Located after the FileHeader. Contains acquisition date and pointers to RunHeaders.

| Offset | Type   | Size | Field                   |
|--------|--------|------|-------------------------|
| 0      | u32    | 4    | method_file_present     |
| 4      | u16    | 2    | year                    |
| 6      | u16    | 2    | month                   |
| 8      | u16    | 2    | day_of_week             |
| 10     | u16    | 2    | day                     |
| 12     | u16    | 2    | hour                    |
| 14     | u16    | 2    | minute                  |
| 16     | u16    | 2    | second                  |
| 18     | u16    | 2    | millisecond             |

### Version 57-63 additional fields (after preamble):

| Offset | Type   | Size | Field                   |
|--------|--------|------|-------------------------|
| 20     | u32    | 4    | unknown1                |
| 24     | u32    | 4    | data_addr_32            |
| 28     | u32    | 4    | n_controllers_1         |
| 32     | u32    | 4    | n_controllers_2         |
| 36     | u32    | 16   | unknown2..5             |
| 52     | u32    | 4    | run_header_addr_32      |
| 56     | u32    | 4    | run_header_addr2_32     |
| 60     | bytes  | 744  | unknown_area            |

### Version 64-66 additional fields:

Same 32-bit fields as above, plus 64-bit addresses:

| Offset | Type   | Size | Field                   |
|--------|--------|------|-------------------------|
| ...    | u64    | 8    | data_addr_64            |
| ...    | u64    | 8    | unknown_addr_64         |
| ...    | u64[]  | 8 ea | run_header_addrs_64     |
| ...    | bytes  | 992-1032 | unknown_area (version-dependent) |

After the preamble, 5 heading strings follow as PascalStringWin32.

## 5. RunHeader

The primary index structure for each instrument. Located at the address from RawFileInfo.

### SampleInfo (nested at start of RunHeader)

| Offset | Type   | Size | Field                   |
|--------|--------|------|-------------------------|
| 0      | u32    | 4    | unknown1                |
| 4      | u32    | 4    | unknown2                |
| 8      | u32    | 4    | **first_scan_number**   |
| 12     | u32    | 4    | **last_scan_number**    |
| 16     | u32    | 4    | inst_log_length         |
| 20     | u32    | 4    | error_log_length        |
| 24     | u32    | 4    | unknown3                |
| 28     | u32    | 4    | scan_index_addr_32      |
| 32     | u32    | 4    | data_addr_32            |
| 36     | u32    | 4    | inst_log_addr_32        |
| 40     | u32    | 4    | error_log_addr_32       |
| 44     | u32    | 4    | unknown4                |
| 48     | f64    | 8    | max_ion_current         |
| 56     | f64    | 8    | **low_mz**              |
| 64     | f64    | 8    | **high_mz**             |
| 72     | f64    | 8    | **start_time** (minutes)|
| 80     | f64    | 8    | **end_time** (minutes)  |
| 88     | bytes  | 56   | unknown_area            |
| 144    | UTF16  | 88 (44ch) | tag1 (sample text) |
| 232    | UTF16  | 40 (20ch) | tag2              |
| 272    | UTF16  | 320 (160ch) | tag3            |

**SampleInfo total**: ~592 bytes.

### RunHeader fields (after SampleInfo)

13 filename strings (each 520 bytes = 260 UTF-16 chars), followed by:

| Offset (relative) | Type | Size | Field                    |
|--------------------|------|------|--------------------------|
| after filenames    | f64  | 8    | unknown_double1          |
| +8                 | f64  | 8    | unknown_double2          |
| +16                | u32  | 4    | scan_trailer_addr_32     |
| +20                | u32  | 4    | scan_params_addr_32      |
| +24                | u32  | 8    | unknown_lengths          |
| +32                | u32  | 4    | n_segments               |
| +36                | u32  | 16   | unknown4..7              |
| +52                | u32  | 4    | own_addr_32              |

### Version 64-66 extra fields (64-bit addresses):

| Offset (relative) | Type | Size | Field                    |
|--------------------|------|------|--------------------------|
| ...                | u64  | 8    | **scan_index_addr_64**   |
| ...                | u64  | 8    | **data_addr_64**         |
| ...                | u64  | 8    | inst_log_addr_64         |
| ...                | u64  | 8    | error_log_addr_64        |
| ...                | u64  | 8    | unknown_addr1_64         |
| ...                | u64  | 8    | **scan_trailer_addr_64** |
| ...                | u64  | 8    | **scan_params_addr_64**  |
| ...                | u32  | 8    | unknown5..6              |
| ...                | u64  | 8    | own_addr_64              |
| ...                | u32  | 96   | unknown7..30 (24 u32s)   |

After fixed fields, PascalStringWin32 strings: device name, model, serial number,
software version, tag1-tag4.

## 6. ScanIndex

Array of `ScanIndexEntry`, one per scan. Located at `scan_index_addr`.

### ScanIndexEntry v57-63 (72 bytes)

| Offset | Type  | Size | Field                |
|--------|-------|------|----------------------|
| 0      | u32   | 4    | offset_32            |
| 4      | u32   | 4    | index (scan number)  |
| 8      | u16   | 2    | scan_event           |
| 10     | u16   | 2    | scan_segment         |
| 12     | u32   | 4    | next                 |
| 16     | u32   | 4    | unknown              |
| 20     | u32   | 4    | **data_size**        |
| 24     | f64   | 8    | **start_time** (RT)  |
| 32     | f64   | 8    | **total_current** (TIC) |
| 40     | f64   | 8    | **base_intensity**   |
| 48     | f64   | 8    | **base_mz**          |
| 56     | f64   | 8    | **low_mz**           |
| 64     | f64   | 8    | **high_mz**          |

### ScanIndexEntry v64 (80 bytes)

Same as v57-63, plus:

| Offset | Type  | Size | Field                |
|--------|-------|------|----------------------|
| 72     | u64   | 8    | **offset_64**        |

### ScanIndexEntry v66 (88 bytes)

Same as v64, plus:

| Offset | Type  | Size | Field                |
|--------|-------|------|----------------------|
| 80     | u32   | 4    | unknown1             |
| 84     | u32   | 4    | unknown2             |

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

### ScanEventPreamble size by version:

| Version  | Size (bytes) |
|----------|-------------|
| < 57     | 41          |
| 57-60    | 80          |
| 62       | 120         |
| 63-64    | 128         |
| 66       | 132         |

### Key byte positions in ScanEventPreamble:

| Byte | Field          | Values                                              |
|------|----------------|-----------------------------------------------------|
| 4    | polarity       | 0=negative, 1=positive, 2=undefined                 |
| 5    | scan_mode      | 0=centroid, 1=profile, 2=undefined                  |
| 6    | ms_power       | 0=undef, 1=MS1, 2=MS2, ..., 8=MS8                  |
| 7    | scan_type      | 0=Full, 1=Zoom, 2=SIM, 3=SRM, 4=CRM                |
| 10   | dependent      | 0=primary, 1=dependent (DDA)                        |
| 11   | ionization     | 0=EI,1=CI,2=FABI,3=ESI,4=APCI,5=NSI,6=TSI,7=FDI,8=MALDI |
| 24   | activation     | 1=HCD, 4=CID                                       |
| 40   | analyzer       | 0=ITMS, 1=TQMS, 2=SQMS, 3=TOFMS, 4=FTMS, 5=Sector |

### ScanEvent (after preamble):

| Field                  | Type         | Size      | Notes                        |
|------------------------|-------------|-----------|------------------------------|
| n_precursors           | u32         | 4         | Number of Reaction entries   |
| reactions[0..n-1]      | Reaction[]  | 32 each   | Precursor info               |
| unknown1               | u32         | 4         |                              |
| fraction_collector     | (f64, f64)  | 16        | (low_mz, high_mz)           |
| n_conversion_params    | u32         | 4         | 0, 4 (LTQ-FT), or 7 (Orbitrap) |
| conversion_params[]    | f64[]       | 8 each    | Hz-to-m/z coefficients       |

### Reaction (32 bytes)

| Offset | Type  | Size | Field              |
|--------|-------|------|--------------------|
| 0      | f64   | 8    | precursor_mz       |
| 8      | f64   | 8    | unknown             |
| 16     | f64   | 8    | collision_energy    |
| 24     | u32   | 4    | unknown1            |
| 28     | u32   | 4    | unknown2            |

## 9. Trailer Extra (Self-Describing Records)

Located at `scan_trailer_addr`. Uses a metadata-driven approach.

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

## 10. Overall File Layout (Reading Sequence)

```
[OLE2 Header: 512 bytes, magic d0 cf 11 e0 a1 b1 1a e1]
[OLE2 FAT / Directory structures]

Within the main data stream:
  [0]              FileHeader (magic 0xA101, version, ~2402 bytes)
  [after header]   SequencerRow (optional)
  [...]            AutoSamplerInfo (optional)
  [...]            RawFileInfoPreamble + heading strings
                     -> run_header_addr (absolute offset)
  [run_header_addr] RunHeader
                     -> SampleInfo (first/last scan, time range, mass range)
                     -> scan_index_addr, data_addr, scan_trailer_addr, etc.
  [scan_index_addr] ScanIndexEntry[first_scan..last_scan]
                     -> per-scan: offset, RT, TIC, base peak, data size
  [data_addr+off]  ScanDataPacket (per scan)
                     -> PacketHeader -> Profile -> PeakList -> Descriptors
  [trailer_addr]   GenericDataHeader (template)
                   GenericRecord[first_scan..last_scan]
```

## 11. Version Differences Summary

| Feature              | v57-63   | v64       | v66       |
|----------------------|----------|-----------|-----------|
| Preamble size        | 80 bytes | 128 bytes | 132 bytes |
| Address width        | 32-bit   | **64-bit**| **64-bit**|
| ScanIndexEntry size  | 72 bytes | 80 bytes  | 88 bytes  |
| Extra index fields   | None     | offset_64 | offset_64 + 2 unknowns |
