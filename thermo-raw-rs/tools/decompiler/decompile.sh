#!/bin/bash
# Decompile Thermo CommonCore DLLs for format specification extraction.
# Prerequisites: dotnet tool install -g ilspycmd

set -euo pipefail

DLL_DIR="${1:?Usage: decompile.sh <dll_directory> [output_dir]}"
OUT_DIR="${2:-./decompiled}"

mkdir -p "$OUT_DIR"

# Core DLLs listed by analysis priority.
DLLS=(
    "ThermoFisher.CommonCore.RawFileReader.dll"            # Core read logic
    "ThermoFisher.CommonCore.Data.dll"                     # Data model / struct definitions
    "ThermoFisher.CommonCore.MassPrecisionEstimator.dll"   # Precision-related
    "ThermoFisher.CommonCore.BackgroundSubtraction.dll"    # Post-processing
)

for dll in "${DLLS[@]}"; do
    dllpath="$DLL_DIR/$dll"
    if [ -f "$dllpath" ]; then
        name="${dll%.dll}"
        echo "Decompiling $dll -> $OUT_DIR/$name/"
        ilspycmd "$dllpath" -p -o "$OUT_DIR/$name/" 2>/dev/null || \
            echo "  Warning: ilspycmd failed for $dll, trying with --nested-directories"
        echo "  Done. $(find "$OUT_DIR/$name/" -name '*.cs' | wc -l) files"
    else
        echo "Skipping $dll (not found in $DLL_DIR)"
    fi
done

echo ""
echo "=== Decompilation complete ==="
echo "Key files to examine (by reverse engineering priority):"
echo "  1. $OUT_DIR/ThermoFisher.CommonCore.RawFileReader/  -> File read core logic"
echo "  2. $OUT_DIR/ThermoFisher.CommonCore.Data/           -> Struct definitions"
echo ""
echo "Search for key classes/methods:"
echo "  grep -rn 'ReadScanData\|ScanDataPacket\|ProfileChunk' $OUT_DIR/"
echo "  grep -rn 'RunHeader\|ScanIndex\|TrailerExtra' $OUT_DIR/"
echo "  grep -rn 'OleStream\|CompoundFile\|ReadStream' $OUT_DIR/"
