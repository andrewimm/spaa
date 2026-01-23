//! Compare two Chrome heap snapshots to find memory leaks.
//!
//! This tool computes the diff between two heap snapshots and outputs
//! an agent-friendly format showing:
//! - Object types that grew (count and size deltas)
//! - Retention paths for new objects (what's keeping them alive)
//!
//! # Usage
//!
//! ```bash
//! heapdiff baseline.heapsnapshot target.heapsnapshot -o diff.ndjson
//! ```

use clap::Parser;
use spaa::heapdiff::{HeapDiff, ParsedSnapshot};
use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::PathBuf;
use std::process::ExitCode;

#[derive(Parser, Debug)]
#[command(name = "heapdiff")]
#[command(about = "Compare heap snapshots to find memory leaks")]
#[command(version)]
struct Args {
    /// Baseline heap snapshot (before the leak)
    baseline: PathBuf,

    /// Target heap snapshot (after the leak)
    target: PathBuf,

    /// Output file (defaults to stdout)
    #[arg(short, long)]
    output: Option<PathBuf>,

    /// Maximum number of retained objects to analyze
    #[arg(short = 'n', long, default_value = "100")]
    max_retained: usize,
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    eprintln!("Loading baseline: {}", args.baseline.display());
    let baseline_file = File::open(&args.baseline)?;
    let baseline = ParsedSnapshot::parse(BufReader::new(baseline_file))?;
    eprintln!(
        "  {} nodes, {} edges",
        baseline.nodes.len(),
        baseline.edges.len()
    );

    eprintln!("Loading target: {}", args.target.display());
    let target_file = File::open(&args.target)?;
    let target = ParsedSnapshot::parse(BufReader::new(target_file))?;
    eprintln!(
        "  {} nodes, {} edges",
        target.nodes.len(),
        target.edges.len()
    );

    eprintln!("Computing diff...");
    let diff = HeapDiff::compute(
        &baseline,
        &target,
        args.baseline.to_str().unwrap_or("baseline"),
        args.target.to_str().unwrap_or("target"),
        args.max_retained,
    );

    eprintln!(
        "Found {} growing types, {} retained objects",
        diff.type_growth.len(),
        diff.retained_objects.len()
    );

    // Write output
    match args.output {
        Some(path) => {
            let file = File::create(&path)?;
            let writer = BufWriter::new(file);
            diff.write_ndjson(writer)?;
            eprintln!("Wrote diff to {}", path.display());
        }
        None => {
            diff.write_ndjson(std::io::stdout())?;
        }
    }

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
