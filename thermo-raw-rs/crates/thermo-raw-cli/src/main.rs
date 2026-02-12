use clap::{Parser, Subcommand};
use std::path::PathBuf;
use thermo_raw::RawFile;

#[derive(Parser)]
#[command(name = "thermo-raw", about = "Thermo RAW file reader CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Show basic RAW file information.
    Info { file: PathBuf },

    /// List OLE2 container streams.
    Streams { file: PathBuf },

    /// Export a single scan as JSON.
    Scan {
        file: PathBuf,
        #[arg(short, long)]
        number: u32,
    },

    /// Export TIC as CSV.
    Tic {
        file: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Export XIC as CSV.
    Xic {
        file: PathBuf,
        #[arg(short, long)]
        mz: f64,
        #[arg(short, long, default_value = "5.0")]
        ppm: f64,
        #[arg(short, long)]
        output: Option<PathBuf>,
    },

    /// Show trailer extra data for a scan.
    Trailer {
        file: PathBuf,
        #[arg(short, long)]
        number: u32,
    },

    /// Validate against ground truth data.
    Validate {
        file: PathBuf,
        #[arg(short, long)]
        truth_dir: PathBuf,
    },

    /// Benchmark: read all scans (performance test).
    Benchmark {
        file: PathBuf,
        #[arg(long)]
        parallel: bool,
        #[arg(long)]
        mmap: bool,
    },
}

fn ms_level_str(level: &thermo_raw::MsLevel) -> &'static str {
    match level {
        thermo_raw::MsLevel::Ms1 => "MS1",
        thermo_raw::MsLevel::Ms2 => "MS2",
        thermo_raw::MsLevel::Ms3 => "MS3",
        thermo_raw::MsLevel::Other(_) => "Other",
    }
}

fn polarity_str(p: &thermo_raw::Polarity) -> &'static str {
    match p {
        thermo_raw::Polarity::Positive => "Positive",
        thermo_raw::Polarity::Negative => "Negative",
        thermo_raw::Polarity::Unknown => "Unknown",
    }
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match cli.command {
        Commands::Info { file } => {
            let raw = RawFile::open(&file)?;
            let meta = raw.metadata();
            println!("File:        {}", file.display());
            println!("Version:     {}", raw.version());
            println!("Instrument:  {}", meta.instrument_model);
            println!("Serial:      {}", meta.serial_number);
            println!("Software:    {}", meta.software_version);
            println!("Sample:      {}", meta.sample_name);
            println!("Created:     {}", meta.creation_date);
            println!(
                "Scans:       {}-{} ({} total)",
                raw.first_scan(),
                raw.last_scan(),
                raw.n_scans()
            );
            println!(
                "RT range:    {:.4}-{:.4} min",
                raw.start_time(),
                raw.end_time()
            );
            println!(
                "Mass range:  {:.2}-{:.2} Da",
                raw.low_mass(),
                raw.high_mass()
            );

            // Show trailer field names
            let fields = raw.trailer_fields();
            if !fields.is_empty() {
                println!("Trailer fields ({}):", fields.len());
                for f in &fields {
                    println!("  {}", f);
                }
            }
        }

        Commands::Streams { file } => {
            let container = cfb_reader::Ole2Container::open(&file)?;
            let streams = container.list_streams();
            println!("OLE2 streams in {}:", file.display());
            for s in &streams {
                println!("  {}", s);
            }
        }

        Commands::Scan { file, number } => {
            let raw = RawFile::open(&file)?;
            let scan = raw.scan(number)?;

            let precursor_json = if let Some(ref p) = scan.precursor {
                serde_json::json!({
                    "mz": p.mz,
                    "charge": p.charge,
                    "isolationWidth": p.isolation_width,
                    "activationType": p.activation_type,
                    "collisionEnergy": p.collision_energy,
                })
            } else {
                serde_json::Value::Null
            };

            let json = serde_json::json!({
                "scanNumber": scan.scan_number,
                "rt": scan.rt,
                "msLevel": ms_level_str(&scan.ms_level),
                "polarity": polarity_str(&scan.polarity),
                "filterString": scan.filter_string,
                "tic": scan.tic,
                "basePeakMz": scan.base_peak_mz,
                "basePeakIntensity": scan.base_peak_intensity,
                "precursor": precursor_json,
                "centroidCount": scan.centroid_mz.len(),
                "centroidMz": scan.centroid_mz,
                "centroidIntensity": scan.centroid_intensity,
            });
            println!("{}", serde_json::to_string_pretty(&json)?);
        }

        Commands::Tic { file, output } => {
            let raw = RawFile::open(&file)?;
            let chrom = raw.tic();
            let mut writer: Box<dyn std::io::Write> = if let Some(path) = output {
                Box::new(std::fs::File::create(path)?)
            } else {
                Box::new(std::io::stdout())
            };
            writeln!(writer, "rt,intensity")?;
            for (rt, int) in chrom.rt.iter().zip(chrom.intensity.iter()) {
                writeln!(writer, "{:.6},{:.2}", rt, int)?;
            }
        }

        Commands::Xic {
            file,
            mz,
            ppm,
            output,
        } => {
            let raw = RawFile::open(&file)?;
            let chrom = raw.xic(mz, ppm)?;
            let mut writer: Box<dyn std::io::Write> = if let Some(path) = output {
                Box::new(std::fs::File::create(path)?)
            } else {
                Box::new(std::io::stdout())
            };
            writeln!(writer, "rt,intensity")?;
            for (rt, int) in chrom.rt.iter().zip(chrom.intensity.iter()) {
                writeln!(writer, "{:.6},{:.2}", rt, int)?;
            }
        }

        Commands::Trailer { file, number } => {
            let raw = RawFile::open(&file)?;
            let trailer = raw.trailer_extra(number)?;
            println!("Trailer extra for scan {}:", number);
            let mut keys: Vec<_> = trailer.keys().collect();
            keys.sort();
            for key in keys {
                println!("  {}: {}", key, trailer[key]);
            }
        }

        Commands::Validate { file, truth_dir } => {
            let raw = RawFile::open(&file)?;
            let criteria = thermo_raw::validation::ValidationCriteria::default();
            let report = thermo_raw::validation::validate_file(&raw, &truth_dir, &criteria)?;

            println!("Validation Report");
            println!("=================");
            println!(
                "Total scans:  {}",
                report.total_scans
            );
            println!(
                "Passed:       {} ({:.1}%)",
                report.passed_scans,
                report.pass_rate * 100.0
            );
            println!(
                "Failed:       {}",
                report.failed_scans
            );
            println!(
                "Worst m/z error: {:.4} ppm",
                report.worst_mz_error_ppm
            );
            println!(
                "Worst intensity error: {:.2e}",
                report.worst_intensity_error
            );

            if !report.failures.is_empty() {
                println!("\nFailed scans:");
                for fail in &report.failures {
                    println!(
                        "  Scan {}: mz_err={:.4}ppm rt_err={:.4}s peaks_match={}",
                        fail.scan_number,
                        fail.mz_max_error_ppm,
                        fail.rt_error_seconds,
                        fail.peak_count_match
                    );
                    for err in &fail.errors {
                        println!("    {}", err);
                    }
                }
            }
        }

        Commands::Benchmark {
            file,
            parallel,
            mmap,
        } => {
            let raw = if mmap {
                RawFile::open_mmap(&file)?
            } else {
                RawFile::open(&file)?
            };
            let mode = if mmap { "mmap" } else { "read" };

            let start = std::time::Instant::now();
            if parallel {
                let scans = raw.scans_parallel(raw.first_scan()..raw.last_scan() + 1)?;
                let elapsed = start.elapsed();
                println!(
                    "{} scans read in {:.1}ms ({:.1} scans/sec) [parallel, {}]",
                    scans.len(),
                    elapsed.as_secs_f64() * 1000.0,
                    scans.len() as f64 / elapsed.as_secs_f64(),
                    mode
                );
            } else {
                let mut count = 0u32;
                for i in raw.first_scan()..=raw.last_scan() {
                    let _ = raw.scan(i)?;
                    count += 1;
                }
                let elapsed = start.elapsed();
                println!(
                    "{} scans read in {:.1}ms ({:.1} scans/sec) [sequential, {}]",
                    count,
                    elapsed.as_secs_f64() * 1000.0,
                    count as f64 / elapsed.as_secs_f64(),
                    mode
                );
            }
        }
    }
    Ok(())
}
