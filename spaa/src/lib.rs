//! SPAA format converters and tools.
//!
//! This crate provides converters for various profiling formats to the SPAA
//! (Stack Profile for Agentic Analysis) format.
//!
//! # Available Converters
//!
//! - [`dtrace`] - Convert DTrace output to SPAA
//! - [`perf`] - Convert Linux `perf script` output to SPAA
//! - [`chrome`] - Convert Chrome DevTools profiles to SPAA
//!
//! # Analysis Tools
//!
//! - [`heapdiff`] - Compare heap snapshots for memory leak analysis
//!
//! # Example
//!
//! ```no_run
//! use spaa::dtrace::{DtraceConverter, InputFormat};
//! use std::fs::File;
//! use std::io::{BufReader, BufWriter};
//!
//! let input = BufReader::new(File::open("dtrace.out").unwrap());
//! let output = BufWriter::new(File::create("profile.spaa").unwrap());
//!
//! let mut converter = DtraceConverter::new(InputFormat::AggregatedStack);
//! converter.parse(input).unwrap();
//! converter.write_spaa(output).unwrap();
//! ```

pub mod chrome;
pub mod dtrace;
pub mod heapdiff;
pub mod perf;

// Re-export spaa_parse for convenience
pub use spaa_parse;
