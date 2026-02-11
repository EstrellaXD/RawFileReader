use clap::{Parser, Subcommand};
use std::fs;
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "hex-analyzer", about = "Thermo RAW file reverse engineering helper")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for an f64 value (IEEE 754 little-endian) in a file.
    SearchF64 {
        file: PathBuf,
        value: f64,
        #[arg(long, default_value = "1e-9")]
        tolerance: f64,
        /// Limit search range: offset:length
        #[arg(long)]
        range: Option<String>,
    },

    /// Search for an f32 value in a file.
    SearchF32 {
        file: PathBuf,
        value: f32,
        #[arg(long, default_value = "1e-5")]
        tolerance: f32,
    },

    /// Search for a u32 value in a file.
    SearchU32 { file: PathBuf, value: u32 },

    /// Search for a UTF-16LE string in a file.
    SearchUtf16 { file: PathBuf, pattern: String },

    /// Hex dump a region of a file.
    Dump {
        file: PathBuf,
        #[arg(long)]
        offset: usize,
        #[arg(long, default_value = "256")]
        length: usize,
        /// Also show type interpretation for each line.
        #[arg(long)]
        interpret: bool,
    },

    /// Detect a repeating structure stride from known f64 values.
    DetectStride {
        file: PathBuf,
        /// Comma-separated f64 values, e.g. "0.0032,0.0098,0.0164"
        values: String,
        #[arg(long, default_value = "1e-9")]
        tolerance: f64,
    },

    /// Binary diff two files (highlight difference regions).
    Diff {
        file_a: PathBuf,
        file_b: PathBuf,
        /// Maximum number of diff regions to display.
        #[arg(long, default_value = "50")]
        max_diffs: usize,
    },

    /// Given ground truth JSON, auto-locate fields in the binary.
    AutoLocate {
        /// RAW file path.
        raw_file: PathBuf,
        /// Ground truth directory (with scan_index.json, metadata.json, etc.).
        truth_dir: PathBuf,
    },
}

fn search_f64(data: &[u8], target: f64, tolerance: f64) -> Vec<(usize, f64)> {
    let mut hits = Vec::new();
    if data.len() < 8 {
        return hits;
    }
    for i in 0..=data.len() - 8 {
        let val = f64::from_le_bytes(data[i..i + 8].try_into().unwrap());
        if val.is_finite() && (val - target).abs() <= tolerance.max(target.abs() * tolerance) {
            hits.push((i, val));
        }
    }
    hits
}

fn search_f32(data: &[u8], target: f32, tolerance: f32) -> Vec<(usize, f32)> {
    let mut hits = Vec::new();
    if data.len() < 4 {
        return hits;
    }
    for i in 0..=data.len() - 4 {
        let val = f32::from_le_bytes(data[i..i + 4].try_into().unwrap());
        if val.is_finite() && (val - target).abs() <= tolerance.max(target.abs() * tolerance) {
            hits.push((i, val));
        }
    }
    hits
}

fn search_u32(data: &[u8], target: u32) -> Vec<usize> {
    let target_bytes = target.to_le_bytes();
    let mut hits = Vec::new();
    if data.len() < 4 {
        return hits;
    }
    for i in 0..=data.len() - 4 {
        if data[i..i + 4] == target_bytes {
            hits.push(i);
        }
    }
    hits
}

fn search_utf16le(data: &[u8], pattern: &str) -> Vec<usize> {
    let encoded: Vec<u8> = pattern
        .encode_utf16()
        .flat_map(|c| c.to_le_bytes())
        .collect();
    let mut hits = Vec::new();
    if data.len() < encoded.len() {
        return hits;
    }
    for i in 0..=data.len() - encoded.len() {
        if data[i..i + encoded.len()] == encoded[..] {
            hits.push(i);
        }
    }
    hits
}

fn hex_dump(data: &[u8], offset: usize, length: usize, interpret: bool) {
    let end = (offset + length).min(data.len());
    let slice = &data[offset..end];

    for (i, chunk) in slice.chunks(16).enumerate() {
        let addr = offset + i * 16;
        print!("{:08X}  ", addr);
        for (j, byte) in chunk.iter().enumerate() {
            print!("{:02X} ", byte);
            if j == 7 {
                print!(" ");
            }
        }
        // Padding for incomplete lines.
        for j in chunk.len()..16 {
            print!("   ");
            if j == 7 {
                print!(" ");
            }
        }
        print!(" |");
        for byte in chunk {
            let c = if *byte >= 0x20 && *byte < 0x7F {
                *byte as char
            } else {
                '.'
            };
            print!("{}", c);
        }
        println!("|");

        if interpret && chunk.len() >= 8 {
            let pos = offset + i * 16;
            if pos + 8 <= data.len() {
                let f64_val = f64::from_le_bytes(data[pos..pos + 8].try_into().unwrap());
                let f32_val = f32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                let u32_val = u32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                let i32_val = i32::from_le_bytes(data[pos..pos + 4].try_into().unwrap());
                if f64_val.is_finite() && f64_val.abs() < 1e15 && f64_val.abs() > 1e-15 {
                    println!("          -> f64: {:.10}", f64_val);
                }
                if f32_val.is_finite() && f32_val.abs() < 1e10 && f32_val.abs() > 1e-10 {
                    println!("          -> f32: {:.6}", f32_val);
                }
                println!("          -> u32: {}  i32: {}", u32_val, i32_val);
            }
        }
    }
}

fn detect_stride(data: &[u8], values: &[f64], tolerance: f64) -> Vec<(usize, usize, usize)> {
    let first_hits = search_f64(data, values[0], tolerance);
    let mut results = Vec::new();

    for (hit_offset, _) in &first_hits {
        for stride in (8..=256).step_by(4) {
            let mut all_match = true;
            for (vi, &val) in values.iter().enumerate().skip(1) {
                let expected_offset = hit_offset + vi * stride;
                if expected_offset + 8 > data.len() {
                    all_match = false;
                    break;
                }
                let found = f64::from_le_bytes(
                    data[expected_offset..expected_offset + 8].try_into().unwrap(),
                );
                if (found - val).abs() > tolerance.max(val.abs() * tolerance) {
                    all_match = false;
                    break;
                }
            }
            if all_match {
                results.push((*hit_offset, stride, values.len()));
            }
        }
    }
    results
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::SearchF64 {
            file,
            value,
            tolerance,
            range,
        } => {
            let data = fs::read(&file).expect("Failed to read file");
            let (search_data, base_offset) = if let Some(r) = range {
                let parts: Vec<usize> = r.split(':').map(|s| s.parse().unwrap()).collect();
                let start = parts[0];
                let len = parts.get(1).copied().unwrap_or(data.len() - start);
                let end = (start + len).min(data.len());
                (&data[start..end], start)
            } else {
                (&data[..], 0)
            };
            let hits = search_f64(search_data, value, tolerance);
            println!(
                "Searching for f64 {:.10} (+/-{}) in {}:",
                value,
                tolerance,
                file.display()
            );
            println!("Found {} hits:", hits.len());
            for (offset, val) in &hits {
                println!(
                    "  offset 0x{:08X} ({:>10}): {:.15}",
                    offset + base_offset,
                    offset + base_offset,
                    val
                );
            }
        }

        Commands::SearchF32 {
            file,
            value,
            tolerance,
        } => {
            let data = fs::read(&file).expect("Failed to read file");
            let hits = search_f32(&data, value, tolerance);
            println!("Found {} hits for f32 {:.6}:", hits.len(), value);
            for (offset, val) in &hits {
                println!("  offset 0x{:08X}: {:.10}", offset, val);
            }
        }

        Commands::SearchU32 { file, value } => {
            let data = fs::read(&file).expect("Failed to read file");
            let hits = search_u32(&data, value);
            println!("Found {} hits for u32 {}:", hits.len(), value);
            for offset in &hits {
                println!("  offset 0x{:08X} ({:>10})", offset, offset);
            }
        }

        Commands::SearchUtf16 { file, pattern } => {
            let data = fs::read(&file).expect("Failed to read file");
            let hits = search_utf16le(&data, &pattern);
            println!("Found {} hits for UTF-16LE \"{}\":", hits.len(), pattern);
            for offset in &hits {
                println!("  offset 0x{:08X}", offset);
            }
        }

        Commands::Dump {
            file,
            offset,
            length,
            interpret,
        } => {
            let data = fs::read(&file).expect("Failed to read file");
            hex_dump(&data, offset, length, interpret);
        }

        Commands::DetectStride {
            file,
            values,
            tolerance,
        } => {
            let data = fs::read(&file).expect("Failed to read file");
            let vals: Vec<f64> = values
                .split(',')
                .map(|s| s.trim().parse().unwrap())
                .collect();
            println!("Detecting stride for {} values: {:?}", vals.len(), vals);
            let results = detect_stride(&data, &vals, tolerance);
            if results.is_empty() {
                println!("No stride pattern found.");
            } else {
                println!("Found {} candidate(s):", results.len());
                for (offset, stride, matched) in &results {
                    println!(
                        "  base_offset=0x{:08X}  stride={} bytes  ({} values matched)",
                        offset, stride, matched
                    );
                }
            }
        }

        Commands::Diff {
            file_a,
            file_b,
            max_diffs,
        } => {
            let a = fs::read(&file_a).expect("Failed to read file A");
            let b = fs::read(&file_b).expect("Failed to read file B");
            let min_len = a.len().min(b.len());
            println!("File A: {} ({} bytes)", file_a.display(), a.len());
            println!("File B: {} ({} bytes)", file_b.display(), b.len());

            let mut diff_count = 0;
            let mut in_diff = false;
            let mut diff_start = 0;

            for i in 0..min_len {
                if a[i] != b[i] {
                    if !in_diff {
                        diff_start = i;
                        in_diff = true;
                    }
                } else if in_diff {
                    let diff_len = i - diff_start;
                    println!(
                        "  DIFF at 0x{:08X}-0x{:08X} ({} bytes)",
                        diff_start,
                        i - 1,
                        diff_len
                    );
                    diff_count += 1;
                    in_diff = false;
                    if diff_count >= max_diffs {
                        println!("  ... (max diffs reached)");
                        break;
                    }
                }
            }
            if in_diff {
                println!(
                    "  DIFF at 0x{:08X}-0x{:08X} ({} bytes)",
                    diff_start,
                    min_len - 1,
                    min_len - diff_start
                );
            }
            if a.len() != b.len() {
                println!(
                    "  SIZE DIFF: A={} B={} (delta={})",
                    a.len(),
                    b.len(),
                    (a.len() as i64 - b.len() as i64).abs()
                );
            }
        }

        Commands::AutoLocate {
            raw_file,
            truth_dir,
        } => {
            let data = fs::read(&raw_file).expect("Failed to read RAW file");

            // Load scan_index.json
            let index_path = truth_dir.join("scan_index.json");
            let index_str =
                fs::read_to_string(&index_path).expect("Failed to read scan_index.json");
            let index: serde_json::Value = serde_json::from_str(&index_str).unwrap();
            let scans = index.as_array().unwrap();

            // Extract first 10 RTs
            let rts: Vec<f64> = scans
                .iter()
                .take(10)
                .filter_map(|s| s["rt"].as_f64())
                .collect();
            println!(
                "=== Auto-locating fields in {} ===",
                raw_file.display()
            );
            println!(
                "Using first {} RT values: {:?}",
                rts.len(),
                &rts[..3.min(rts.len())]
            );

            // 1. RT stride detection
            println!("\n--- Retention Time Stride Detection ---");
            let stride_results = detect_stride(&data, &rts, 1e-9);
            for (offset, stride, matched) in &stride_results {
                println!(
                    "  RT field at offset 0x{:08X}, stride {} bytes, {} matched",
                    offset, stride, matched
                );
            }

            // 2. TIC values
            let first_tic = scans.first().and_then(|s| s["tic"].as_f64());
            if let Some(tic) = first_tic {
                println!("\n--- TIC Search (first scan: {:.2}) ---", tic);
                let hits = search_f64(&data, tic, tic * 1e-6);
                for (offset, val) in hits.iter().take(10) {
                    println!("  hit at 0x{:08X}: {:.6}", offset, val);
                }
            }

            // 3. Scan count
            let n_scans = scans.len() as u32;
            println!("\n--- Scan Count ({}) ---", n_scans);
            let hits = search_u32(&data, n_scans);
            for offset in hits.iter().take(20) {
                println!("  hit at 0x{:08X}", offset);
            }

            // 4. Metadata strings
            let meta_path = truth_dir.join("metadata.json");
            if let Ok(meta_str) = fs::read_to_string(&meta_path) {
                let meta: serde_json::Value = serde_json::from_str(&meta_str).unwrap();
                for key in &["instrumentModel", "sampleName", "serialNumber"] {
                    if let Some(val) = meta[key].as_str() {
                        if !val.is_empty() {
                            println!("\n--- UTF-16LE search: {} = \"{}\" ---", key, val);
                            let hits = search_utf16le(&data, val);
                            for offset in hits.iter().take(5) {
                                println!("  hit at 0x{:08X}", offset);
                            }
                        }
                    }
                }
            }
        }
    }
}
