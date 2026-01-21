//! Convert DTrace output files to SPAA format.
//!
//! This binary reads DTrace output (aggregated stack traces) and converts
//! them to the SPAA (Stack Profile for Agentic Analysis) format.
//!
//! # Usage
//!
//! ```bash
//! dtrace_to_spaa input.txt -o output.spaa
//! dtrace_to_spaa input.txt --event syscall::read:entry --frequency 0
//! dtrace_to_spaa input.txt  # outputs to input.spaa
//! ```

use clap::{Parser, ValueEnum};
use dtrace_to_spaa::{ConverterConfig, DtraceConverter, InputFormat};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Format {
    /// Aggregated stack output: @[ustack()] = count();
    Aggregated,
    /// Split user/kernel stacks (not yet supported)
    Split,
    /// Per-probe output (not yet supported)
    PerProbe,
}

impl From<Format> for InputFormat {
    fn from(f: Format) -> Self {
        match f {
            Format::Aggregated => InputFormat::AggregatedStack,
            Format::Split => InputFormat::SplitStacks,
            Format::PerProbe => InputFormat::PerProbe,
        }
    }
}

#[derive(Parser, Debug)]
#[command(name = "dtrace_to_spaa")]
#[command(about = "Convert DTrace output to SPAA format")]
#[command(version)]
struct Args {
    /// Input DTrace output file
    input: PathBuf,

    /// Output SPAA file (defaults to input filename with .spaa extension)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Input format
    #[arg(short, long, value_enum, default_value = "aggregated")]
    format: Format,

    /// Event name for the SPAA output
    #[arg(short, long, default_value = "profile-997")]
    event: String,

    /// Sampling frequency in Hz (set to 0 for event/probe-based tracing)
    #[arg(short = 'z', long)]
    frequency: Option<u64>,
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    // Determine output path
    let output_path = args.output.unwrap_or_else(|| {
        let mut path = args.input.clone();
        path.set_extension("spaa");
        path
    });

    // Build config
    let frequency_hz = match args.frequency {
        Some(0) => None,
        Some(f) => Some(f),
        None => {
            // Infer from event name if it matches profile-N pattern
            if let Some(freq_str) = args.event.strip_prefix("profile-") {
                freq_str.parse().ok()
            } else {
                None
            }
        }
    };

    let config = ConverterConfig {
        event_name: args.event,
        frequency_hz,
    };

    // Open input
    let input_file = File::open(&args.input).map_err(|e| {
        format!(
            "Failed to open input file '{}': {}",
            args.input.display(),
            e
        )
    })?;
    let reader = BufReader::new(input_file);

    // Parse
    let mut converter = DtraceConverter::with_config(args.format.into(), config);
    converter.parse(reader)?;

    // Create output
    let output_file = File::create(&output_path).map_err(|e| {
        format!(
            "Failed to create output file '{}': {}",
            output_path.display(),
            e
        )
    })?;
    let mut writer = BufWriter::new(output_file);

    // Write SPAA
    converter.write_spaa(&mut writer)?;
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
