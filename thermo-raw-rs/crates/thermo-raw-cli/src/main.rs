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
    },
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
            println!("Sample:      {}", meta.sample_name);
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
            let json = serde_json::json!({
                "scanNumber": scan.scan_number,
                "rt": scan.rt,
                "tic": scan.tic,
                "basePeakMz": scan.base_peak_mz,
                "basePeakIntensity": scan.base_peak_intensity,
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

        Commands::Validate { file: _, truth_dir: _ } => {
            println!("Validation not yet implemented (requires Phase 3 completion)");
        }

        Commands::Benchmark { file, parallel } => {
            let raw = RawFile::open(&file)?;
            let start = std::time::Instant::now();
            if parallel {
                let scans = raw.scans_parallel(raw.first_scan()..raw.last_scan() + 1)?;
                let elapsed = start.elapsed();
                println!(
                    "{} scans read in {:.1}ms ({:.1} scans/sec)",
                    scans.len(),
                    elapsed.as_secs_f64() * 1000.0,
                    scans.len() as f64 / elapsed.as_secs_f64()
                );
            } else {
                let mut count = 0u32;
                for i in raw.first_scan()..=raw.last_scan() {
                    let _ = raw.scan(i)?;
                    count += 1;
                }
                let elapsed = start.elapsed();
                println!(
                    "{} scans read in {:.1}ms ({:.1} scans/sec)",
                    count,
                    elapsed.as_secs_f64() * 1000.0,
                    count as f64 / elapsed.as_secs_f64()
                );
            }
        }
    }
    Ok(())
}
