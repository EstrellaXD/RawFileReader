using System;
using System.Collections.Generic;
using System.IO;
using System.Linq;
using System.Text.Json;
using ThermoFisher.CommonCore.Data;
using ThermoFisher.CommonCore.Data.Business;
using ThermoFisher.CommonCore.Data.FilterEnums;
using ThermoFisher.CommonCore.Data.Interfaces;
using ThermoFisher.CommonCore.RawFileReader;

class Program
{
    static readonly JsonSerializerOptions JsonOpts = new()
    {
        WriteIndented = true,
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
    };

    static void Main(string[] args)
    {
        if (args.Length < 1)
        {
            Console.Error.WriteLine("Usage: GroundTruthExporter <input.raw> [output_dir]");
            return;
        }

        string rawPath = args[0];
        string outDir = args.Length > 1
            ? args[1]
            : Path.Combine(Path.GetDirectoryName(rawPath)!, "ground_truth");
        Directory.CreateDirectory(outDir);

        using var raw = RawFileReaderAdapter.FileFactory(rawPath);
        raw.SelectInstrument(Device.MS, 1);

        var header = raw.RunHeaderEx;
        int firstScan = header.FirstSpectrum;
        int lastScan = header.LastSpectrum;

        // 1. File metadata
        var metadata = new
        {
            FileName = Path.GetFileName(rawPath),
            CreationDate = raw.FileHeader.CreationDate.ToString("o"),
            InstrumentModel = raw.GetInstrumentData().Model,
            InstrumentName = raw.GetInstrumentData().Name,
            SerialNumber = raw.GetInstrumentData().SerialNumber,
            SoftwareVersion = raw.GetInstrumentData().SoftwareVersion,
            SampleName = raw.SampleInformation.SampleName,
            Comment = raw.SampleInformation.Comment,
            FirstScan = firstScan,
            LastScan = lastScan,
            StartTime = header.StartTime,
            EndTime = header.EndTime,
            LowMass = header.LowMass,
            HighMass = header.HighMass,
            MassResolution = header.MassResolution,
        };
        File.WriteAllText(
            Path.Combine(outDir, "metadata.json"),
            JsonSerializer.Serialize(metadata, JsonOpts));

        // 2. Scan index (lightweight: RT + basic info per scan)
        var scanIndex = new List<object>();
        for (int scan = firstScan; scan <= lastScan; scan++)
        {
            var filter = raw.GetFilterForScanNumber(scan);
            var stats = raw.GetScanStatsForScanNumber(scan);
            scanIndex.Add(new
            {
                ScanNumber = scan,
                Rt = Math.Round(stats.StartTime, 6),
                MsLevel = (int)filter.MSOrder,
                Polarity = filter.Polarity.ToString(),
                ScanMode = filter.ScanMode.ToString(),
                MassAnalyzer = filter.MassAnalyzer.ToString(),
                TIC = stats.TIC,
                BasePeakMz = stats.BasePeakMass,
                BasePeakIntensity = stats.BasePeakIntensity,
                LowMass = stats.LowMass,
                HighMass = stats.HighMass,
                FilterString = filter.ToString(),
            });
        }
        File.WriteAllText(
            Path.Combine(outDir, "scan_index.json"),
            JsonSerializer.Serialize(scanIndex, JsonOpts));

        // 3. Per-scan centroid data (one file per scan)
        string scansDir = Path.Combine(outDir, "scans");
        Directory.CreateDirectory(scansDir);

        for (int scan = firstScan; scan <= lastScan; scan++)
        {
            // Centroid
            var centroid = raw.GetCentroidStream(scan, false);
            double[]? centroidMz = null, centroidIntensity = null;
            if (centroid != null && centroid.Length > 0)
            {
                centroidMz = centroid.Masses;
                centroidIntensity = centroid.Intensities;
            }

            // Profile (if available)
            var segScan = raw.GetSegmentedScanFromScanNumber(scan, null);
            double[]? profileMz = null, profileIntensity = null;
            if (segScan != null && segScan.Positions != null && segScan.Positions.Length > 0)
            {
                profileMz = segScan.Positions;
                profileIntensity = segScan.Intensities;
            }

            var scanData = new
            {
                ScanNumber = scan,
                CentroidCount = centroidMz?.Length ?? 0,
                CentroidMz = centroidMz,
                CentroidIntensity = centroidIntensity,
                ProfileCount = profileMz?.Length ?? 0,
                ProfileMz = profileMz,
                ProfileIntensity = profileIntensity,
            };

            string scanFile = Path.Combine(scansDir, $"scan_{scan:D5}.json");
            File.WriteAllText(scanFile, JsonSerializer.Serialize(scanData, JsonOpts));

            if (scan % 500 == 0)
                Console.Error.WriteLine($"  Exported scan {scan}/{lastScan}");
        }

        // 4. Trailer Extra (scan-level metadata)
        string trailerDir = Path.Combine(outDir, "trailer_extra");
        Directory.CreateDirectory(trailerDir);

        // Get trailer extra header labels
        var trailerFields = raw.GetTrailerExtraHeaderInformation();
        var fieldLabels = trailerFields.Select(f => f.Label.Trim(':', ' ')).ToArray();
        File.WriteAllText(
            Path.Combine(outDir, "trailer_fields.json"),
            JsonSerializer.Serialize(fieldLabels, JsonOpts));

        for (int scan = firstScan; scan <= lastScan; scan++)
        {
            var trailer = raw.GetTrailerExtraInformation(scan);
            var dict = new Dictionary<string, string>();
            for (int i = 0; i < trailer.Labels.Length; i++)
            {
                dict[trailer.Labels[i].Trim(':', ' ')] = trailer.Values[i]?.Trim() ?? "";
            }
            string trailerFile = Path.Combine(trailerDir, $"scan_{scan:D5}.json");
            File.WriteAllText(trailerFile, JsonSerializer.Serialize(dict, JsonOpts));
        }

        // 5. Chromatograms
        string chromDir = Path.Combine(outDir, "chromatograms");
        Directory.CreateDirectory(chromDir);

        // TIC
        var settings = new ChromatogramTraceSettings(TraceType.TIC);
        var ticData = raw.GetChromatogramData(new[] { settings }, firstScan, lastScan);
        if (ticData != null)
        {
            var tic = ChromatogramSignal.FromChromatogramData(ticData);
            if (tic != null && tic.Length > 0)
            {
                File.WriteAllText(Path.Combine(chromDir, "tic.json"), JsonSerializer.Serialize(new
                {
                    Rt = tic[0].Times,
                    Intensity = tic[0].Intensities,
                }, JsonOpts));
            }
        }

        // Base Peak
        var bpSettings = new ChromatogramTraceSettings(TraceType.BasePeak);
        var bpData = raw.GetChromatogramData(new[] { bpSettings }, firstScan, lastScan);
        if (bpData != null)
        {
            var bp = ChromatogramSignal.FromChromatogramData(bpData);
            if (bp != null && bp.Length > 0)
            {
                File.WriteAllText(Path.Combine(chromDir, "bpc.json"), JsonSerializer.Serialize(new
                {
                    Rt = bp[0].Times,
                    Intensity = bp[0].Intensities,
                }, JsonOpts));
            }
        }

        Console.Error.WriteLine($"Ground truth exported to {outDir}");
        Console.Error.WriteLine($"  Scans: {firstScan}-{lastScan} ({lastScan - firstScan + 1} total)");
    }
}
