//! Convert Chrome profiling data to SPAA format.
//!
//! This binary reads Chrome DevTools profiling data and converts it to the
//! SPAA (Stack Profile for Agentic Analysis) format.
//!
//! Supported input formats:
//! - Chrome Performance traces (`.json`) from the Performance panel
//! - Standalone cpuprofile files (`.cpuprofile`)
//! - Chrome heap snapshots (`.heapsnapshot`) from the Memory panel
//! - Chrome heap timelines (`.heaptimeline`) from the Memory panel
//!
//! # Usage
//!
//! ```bash
//! chrome_to_spaa trace.json -o output.spaa
//! chrome_to_spaa profile.cpuprofile
//! chrome_to_spaa Heap.heapsnapshot -o heap.spaa
//! chrome_to_spaa timeline.heaptimeline -o timeline.spaa
//! ```

use clap::Parser;
use spaa::chrome::{CpuProfileConverter, HeapSnapshotConverter, ProfileType, detect_profile_type};
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "chrome_to_spaa")]
#[command(about = "Convert Chrome profiling data to SPAA format")]
#[command(version)]
struct Args {
    /// Input file (Performance trace, cpuprofile, heap snapshot, or heap timeline)
    input: PathBuf,

    /// Output SPAA file (defaults to input filename with .spaa extension)
    #[arg(short, long)]
    output: Option<PathBuf>,
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Determine output path
    let output_path = args.output.unwrap_or_else(|| {
        let mut path = args.input.clone();
        path.set_extension("spaa");
        path
    });

    // Read input file
    let input_file = File::open(&args.input).map_err(|e| {
        format!(
            "Failed to open input file '{}': {}",
            args.input.display(),
            e
        )
    })?;
    let mut reader = BufReader::new(input_file);
    let mut contents = String::new();
    reader.read_to_string(&mut contents)?;

    // Detect profile type
    let profile_type = detect_profile_type(&contents)?;

    // Create output file
    let output_file = File::create(&output_path).map_err(|e| {
        format!(
            "Failed to create output file '{}': {}",
            output_path.display(),
            e
        )
    })?;
    let mut writer = BufWriter::new(output_file);

    // Convert based on type
    match profile_type {
        ProfileType::HeapSnapshot | ProfileType::HeapTimeline => {
            let type_name = match profile_type {
                ProfileType::HeapSnapshot => "Chrome heap snapshot",
                ProfileType::HeapTimeline => "Chrome heap timeline",
                _ => unreachable!(),
            };
            eprintln!("Detected: {}", type_name);
            let mut converter = HeapSnapshotConverter::new();
            converter.parse(std::io::Cursor::new(&contents))?;
            converter.write_spaa(&mut writer)?;
        }
        ProfileType::PerformanceTrace | ProfileType::CpuProfile => {
            let type_name = match profile_type {
                ProfileType::PerformanceTrace => "Chrome Performance trace",
                ProfileType::CpuProfile => "V8 cpuprofile",
                _ => unreachable!(),
            };
            eprintln!("Detected: {}", type_name);
            let mut converter = CpuProfileConverter::new();
            converter.parse(std::io::Cursor::new(&contents))?;
            converter.write_spaa(&mut writer)?;
        }
    }

    writer.flush()?;

    eprintln!(
        "Converted '{}' -> '{}'",
        args.input.display(),
        output_path.display()
    );

    Ok(())
}

fn main() -> ExitCode {
    let args = Args::parse();

    match run(args) {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("Error: {}", e);
            ExitCode::FAILURE
        }
    }
}
