//! Convert DTrace output to SPAA format.
//!
//! This module parses DTrace output and converts it to the SPAA
//! (Stack Profile for Agentic Analysis) format.
//!
//! # Supported Formats
//!
//! Currently supported:
//! - Aggregated stacks: `@[ustack()] = count();` or `@[stack()] = count();`
//!
//! Planned:
//! - Split user/kernel stacks: `@[ustack(), stack()] = count();`
//! - Per-probe output with timestamps
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

use serde::Serialize;
use spaa_parse::{
    EventDef, EventKind, ExclusiveWeights, FrameKind, FrameOrder, Header, Sampling, SamplingMode,
    StackContext, StackIdMode, StackType, Weight,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{BufRead, BufReader, Read, Write};
use thiserror::Error;

/// Errors that can occur during DTrace output parsing.
#[derive(Error, Debug)]
pub enum ConvertError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("no stacks found in input")]
    NoStacks,

    #[error("unsupported input format for this operation")]
    UnsupportedFormat,
}

pub type Result<T> = std::result::Result<T, ConvertError>;

/// Input format type for DTrace output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputFormat {
    /// Aggregated stack output: `@[ustack()] = count();`
    /// Stacks are listed with frames followed by a count.
    AggregatedStack,

    /// Split user and kernel stacks: `@[ustack(), stack()] = count();`
    /// User stack followed by kernel stack, then count.
    /// (Not yet implemented)
    SplitStacks,

    /// Per-probe output with optional timestamps.
    /// (Not yet implemented)
    PerProbe,
}

/// Type of stack being parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StackKind {
    User,
    Kernel,
    Unknown,
}

/// A parsed stack from DTrace output.
#[derive(Debug, Clone)]
struct DtraceStack {
    frames: Vec<DtraceFrame>,
    count: u64,
    kind: StackKind,
    /// For split stacks, the related stack (user/kernel pair).
    /// Reserved for future SplitStacks format support.
    #[allow(dead_code)]
    related: Option<Box<DtraceStack>>,
}

/// A parsed stack frame from DTrace output.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct DtraceFrame {
    module: String,
    symbol: String,
    offset: Option<String>,
}

/// Configuration for the converter.
#[derive(Debug, Clone)]
pub struct ConverterConfig {
    /// Event name to use in SPAA output.
    pub event_name: String,
    /// Sampling frequency if known (for profile-N provider).
    pub frequency_hz: Option<u64>,
}

impl Default for ConverterConfig {
    fn default() -> Self {
        Self {
            event_name: "profile-997".to_string(),
            frequency_hz: Some(997),
        }
    }
}

/// Converter from DTrace output to SPAA format.
pub struct DtraceConverter {
    format: InputFormat,
    config: ConverterConfig,
    stacks: Vec<DtraceStack>,
}

impl DtraceConverter {
    /// Create a new converter for the specified input format.
    pub fn new(format: InputFormat) -> Self {
        Self {
            format,
            config: ConverterConfig::default(),
            stacks: Vec::new(),
        }
    }

    /// Create a new converter with custom configuration.
    pub fn with_config(format: InputFormat, config: ConverterConfig) -> Self {
        Self {
            format,
            config,
            stacks: Vec::new(),
        }
    }

    /// Parse DTrace output from a reader.
    pub fn parse<R: Read>(&mut self, reader: R) -> Result<()> {
        match self.format {
            InputFormat::AggregatedStack => self.parse_aggregated(reader),
            InputFormat::SplitStacks => Err(ConvertError::UnsupportedFormat),
            InputFormat::PerProbe => Err(ConvertError::UnsupportedFormat),
        }
    }

    /// Parse aggregated stack format.
    fn parse_aggregated<R: Read>(&mut self, reader: R) -> Result<()> {
        let buf_reader = BufReader::new(reader);
        let mut current_frames: Vec<DtraceFrame> = Vec::new();

        for line_result in buf_reader.lines() {
            let line = line_result?;
            let trimmed = line.trim();

            // Skip empty lines - they separate stacks
            if trimmed.is_empty() {
                // If we have frames but no count yet, wait for count
                continue;
            }

            // Check if this is a count line (just a number)
            if let Ok(count) = trimmed.parse::<u64>() {
                if !current_frames.is_empty() {
                    // Determine stack kind from frames
                    let kind = Self::infer_stack_kind(&current_frames);

                    self.stacks.push(DtraceStack {
                        frames: std::mem::take(&mut current_frames),
                        count,
                        kind,
                        related: None,
                    });
                }
                continue;
            }

            // Skip header/metadata lines (dtrace output sometimes has these)
            if trimmed.starts_with("dtrace:")
                || trimmed.starts_with("CPU")
                || trimmed.starts_with("ID")
                || trimmed.contains("FUNCTION:NAME")
            {
                continue;
            }

            // Parse as a frame
            if let Some(frame) = Self::parse_frame(trimmed) {
                current_frames.push(frame);
            }
        }

        // Handle case where last stack doesn't have trailing newline
        if !current_frames.is_empty() {
            // No count found - this shouldn't happen in well-formed output
            // but we'll skip it rather than error
        }

        Ok(())
    }

    /// Parse a single frame line.
    /// Format: `module`symbol+offset` or `module`symbol` or just `symbol+offset`
    fn parse_frame(line: &str) -> Option<DtraceFrame> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        // DTrace uses backtick to separate module from symbol
        let (module, rest) = if let Some(tick_pos) = line.find('`') {
            (line[..tick_pos].to_string(), &line[tick_pos + 1..])
        } else {
            // No module, just symbol (unusual but possible)
            ("unknown".to_string(), line)
        };

        // Split symbol and offset
        let (symbol, offset) = if let Some(plus_pos) = rest.rfind('+') {
            let sym = rest[..plus_pos].to_string();
            let off = rest[plus_pos + 1..].to_string();
            (sym, Some(off))
        } else {
            (rest.to_string(), None)
        };

        // Skip empty symbols
        if symbol.is_empty() || symbol == "0x0" {
            return None;
        }

        Some(DtraceFrame {
            module,
            symbol,
            offset,
        })
    }

    /// Infer the stack kind from the frames.
    fn infer_stack_kind(frames: &[DtraceFrame]) -> StackKind {
        // Heuristics for detecting kernel frames:
        // - Module contains "kernel", "mach_kernel", "genunix", etc.
        // - High addresses (though we don't have raw addresses here)
        for frame in frames {
            let module_lower = frame.module.to_lowercase();
            if module_lower.contains("kernel")
                || module_lower.contains("genunix")
                || module_lower.contains("unix")
                || module_lower == "mach_kernel"
            {
                return StackKind::Kernel;
            }
        }

        // If we see common userspace libraries, it's likely user
        for frame in frames {
            let module_lower = frame.module.to_lowercase();
            if module_lower.contains("libc")
                || module_lower.contains("libsystem")
                || module_lower.contains("dyld")
                || module_lower.contains(".dylib")
                || module_lower.contains(".so")
            {
                return StackKind::User;
            }
        }

        StackKind::Unknown
    }

    /// Write the parsed data as SPAA format to a writer.
    pub fn write_spaa<W: Write>(&self, mut writer: W) -> Result<()> {
        if self.stacks.is_empty() {
            return Err(ConvertError::NoStacks);
        }

        // Build dictionaries
        let mut dso_map: HashMap<&str, u64> = HashMap::new();
        let mut frame_map: HashMap<&DtraceFrame, u64> = HashMap::new();

        // Collect unique DSOs and frames
        for stack in &self.stacks {
            for frame in &stack.frames {
                if !dso_map.contains_key(frame.module.as_str()) {
                    let id = dso_map.len() as u64 + 1;
                    dso_map.insert(&frame.module, id);
                }
                if !frame_map.contains_key(frame) {
                    let id = frame_map.len() as u64 + 1;
                    frame_map.insert(frame, id);
                }
            }
        }

        // Write header
        let header = self.build_header();
        self.write_record(&mut writer, "header", &header)?;

        // Write DSO dictionary
        for (dso_name, dso_id) in &dso_map {
            let is_kernel = Self::is_kernel_module(dso_name);
            let dso = DsoRecord {
                id: *dso_id,
                name: (*dso_name).to_string(),
                build_id: None,
                is_kernel,
            };
            self.write_record(&mut writer, "dso", &dso)?;
        }

        // Write frame dictionary
        for (dtrace_frame, frame_id) in &frame_map {
            let dso_id = dso_map[dtrace_frame.module.as_str()];
            let is_kernel = Self::is_kernel_module(&dtrace_frame.module);
            let frame = FrameRecord {
                id: *frame_id,
                func: dtrace_frame.symbol.clone(),
                func_resolved: !dtrace_frame.symbol.starts_with("0x"),
                dso: dso_id,
                ip: None,
                symoff: dtrace_frame.offset.clone(),
                srcline: None,
                inlined: false,
                kind: if is_kernel {
                    FrameKind::Kernel
                } else {
                    FrameKind::User
                },
            };
            self.write_record(&mut writer, "frame", &frame)?;
        }

        // Write stacks (aggregated - each unique stack becomes one record)
        let aggregated = self.aggregate_stacks(&frame_map);
        for (stack_key, stack_data) in &aggregated {
            let stack_type = match stack_data.kind {
                StackKind::User => StackType::User,
                StackKind::Kernel => StackType::Kernel,
                StackKind::Unknown => StackType::Unified,
            };

            let stack = StackRecord {
                id: stack_key.id.clone(),
                frames: stack_key.frame_ids.clone(),
                stack_type,
                context: StackContext {
                    event: self.config.event_name.clone(),
                    pid: None,
                    tid: None,
                    cpu: None,
                    comm: None,
                    probe: None,
                    execname: None,
                    uid: None,
                    zonename: None,
                    trace_fields: None,
                    extra: HashMap::new(),
                },
                weights: vec![
                    Weight {
                        metric: "samples".to_string(),
                        value: stack_data.total_count,
                        unit: None,
                    },
                    Weight {
                        metric: "count".to_string(),
                        value: stack_data.total_count,
                        unit: None,
                    },
                ],
                exclusive: stack_key.frame_ids.first().map(|&leaf| ExclusiveWeights {
                    frame: leaf,
                    weights: vec![Weight {
                        metric: "count".to_string(),
                        value: stack_data.total_count,
                        unit: None,
                    }],
                }),
                related_stacks: None,
            };
            self.write_record(&mut writer, "stack", &stack)?;
        }

        Ok(())
    }

    fn is_kernel_module(module: &str) -> bool {
        let module_lower = module.to_lowercase();
        module_lower.contains("kernel")
            || module_lower.contains("genunix")
            || module_lower == "unix"
            || module_lower == "mach_kernel"
    }

    fn build_header(&self) -> Header {
        let sampling = if let Some(freq) = self.config.frequency_hz {
            Sampling {
                mode: SamplingMode::Frequency,
                primary_metric: "samples".to_string(),
                sample_period: None,
                frequency_hz: Some(freq),
            }
        } else {
            Sampling {
                mode: SamplingMode::Event,
                primary_metric: "count".to_string(),
                sample_period: None,
                frequency_hz: None,
            }
        };

        let event = EventDef {
            name: self.config.event_name.clone(),
            kind: if self.config.frequency_hz.is_some() {
                EventKind::Timer
            } else {
                EventKind::Probe
            },
            sampling,
            allocation_tracking: None,
        };

        Header {
            format: "spaa".to_string(),
            version: "1.0".to_string(),
            source_tool: "dtrace".to_string(),
            frame_order: FrameOrder::LeafToRoot,
            events: vec![event],
            time_range: None,
            source: Some(spaa_parse::SourceInfo {
                tool: "dtrace".to_string(),
                command: None,
                tool_version: None,
            }),
            stack_id_mode: StackIdMode::ContentAddressable,
        }
    }

    fn aggregate_stacks(
        &self,
        frame_map: &HashMap<&DtraceFrame, u64>,
    ) -> HashMap<StackKey, StackData> {
        let mut aggregated: HashMap<StackKey, StackData> = HashMap::new();

        for stack in &self.stacks {
            let frame_ids: Vec<u64> = stack.frames.iter().map(|f| frame_map[f]).collect();

            if frame_ids.is_empty() {
                continue;
            }

            let stack_id = Self::compute_stack_id(&frame_ids);
            let key = StackKey {
                id: stack_id,
                frame_ids,
            };

            let data = aggregated.entry(key).or_insert(StackData {
                total_count: 0,
                kind: stack.kind,
            });
            data.total_count += stack.count;
        }

        aggregated
    }

    fn compute_stack_id(frame_ids: &[u64]) -> String {
        use std::collections::hash_map::DefaultHasher;
        let mut hasher = DefaultHasher::new();
        frame_ids.hash(&mut hasher);
        format!("0x{:016x}", hasher.finish())
    }

    fn write_record<W: Write, T: Serialize>(
        &self,
        writer: &mut W,
        record_type: &str,
        data: &T,
    ) -> Result<()> {
        let mut map = serde_json::to_value(data)?;
        if let serde_json::Value::Object(ref mut obj) = map {
            obj.insert(
                "type".to_string(),
                serde_json::Value::String(record_type.to_string()),
            );
        }
        writeln!(writer, "{}", serde_json::to_string(&map)?)?;
        Ok(())
    }
}

// Serialization records
#[derive(Serialize)]
struct DsoRecord {
    id: u64,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    build_id: Option<String>,
    is_kernel: bool,
}

#[derive(Serialize)]
struct FrameRecord {
    id: u64,
    func: String,
    func_resolved: bool,
    dso: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    symoff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    srcline: Option<String>,
    inlined: bool,
    kind: FrameKind,
}

#[derive(Serialize)]
struct StackRecord {
    id: String,
    frames: Vec<u64>,
    stack_type: StackType,
    context: StackContext,
    weights: Vec<Weight>,
    #[serde(skip_serializing_if = "Option::is_none")]
    exclusive: Option<ExclusiveWeights>,
    #[serde(skip_serializing_if = "Option::is_none")]
    related_stacks: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct StackKey {
    id: String,
    frame_ids: Vec<u64>,
}

#[derive(Debug, Clone)]
struct StackData {
    total_count: u64,
    kind: StackKind,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const SAMPLE_DTRACE_OUTPUT: &str = r#"
              libsystem_c.dylib`malloc+0x1a
              myapp`process_data+0x45
              myapp`main+0x89
              123

              libsystem_c.dylib`free+0x10
              myapp`cleanup+0x32
              myapp`main+0x112
              456
"#;

    #[test]
    fn parse_frame_with_module_and_offset() {
        let frame = DtraceConverter::parse_frame("libsystem_c.dylib`malloc+0x1a").unwrap();

        assert_eq!(frame.module, "libsystem_c.dylib");
        assert_eq!(frame.symbol, "malloc");
        assert_eq!(frame.offset, Some("0x1a".to_string()));
    }

    #[test]
    fn parse_frame_without_offset() {
        let frame = DtraceConverter::parse_frame("libsystem_c.dylib`malloc").unwrap();

        assert_eq!(frame.module, "libsystem_c.dylib");
        assert_eq!(frame.symbol, "malloc");
        assert_eq!(frame.offset, None);
    }

    #[test]
    fn parse_frame_kernel() {
        let frame = DtraceConverter::parse_frame("kernel`vm_fault_enter+0x123").unwrap();

        assert_eq!(frame.module, "kernel");
        assert_eq!(frame.symbol, "vm_fault_enter");
        assert_eq!(frame.offset, Some("0x123".to_string()));
    }

    #[test]
    fn parse_aggregated_stacks() {
        let cursor = Cursor::new(SAMPLE_DTRACE_OUTPUT);
        let mut converter = DtraceConverter::new(InputFormat::AggregatedStack);
        converter.parse(cursor).unwrap();

        assert_eq!(converter.stacks.len(), 2);
        assert_eq!(converter.stacks[0].count, 123);
        assert_eq!(converter.stacks[0].frames.len(), 3);
        assert_eq!(converter.stacks[1].count, 456);
        assert_eq!(converter.stacks[1].frames.len(), 3);
    }

    #[test]
    fn convert_to_spaa() {
        let cursor = Cursor::new(SAMPLE_DTRACE_OUTPUT);
        let mut converter = DtraceConverter::new(InputFormat::AggregatedStack);
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = output_str.lines().collect();

        assert!(!lines.is_empty());
        assert!(lines[0].contains("\"type\":\"header\""));
        assert!(lines[0].contains("\"source_tool\":\"dtrace\""));
    }

    #[test]
    fn spaa_output_validates() {
        let cursor = Cursor::new(SAMPLE_DTRACE_OUTPUT);
        let mut converter = DtraceConverter::new(InputFormat::AggregatedStack);
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        // Parse with spaa_parse to validate
        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        assert_eq!(spaa.header.source_tool, "dtrace");
        assert_eq!(spaa.header.frame_order, FrameOrder::LeafToRoot);
        assert!(!spaa.dsos.is_empty());
        assert!(!spaa.frames.is_empty());
        assert_eq!(spaa.stacks.len(), 2);
    }

    #[test]
    fn empty_input_returns_error() {
        let cursor = Cursor::new("");
        let mut converter = DtraceConverter::new(InputFormat::AggregatedStack);
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        let result = converter.write_spaa(&mut output);

        assert!(matches!(result, Err(ConvertError::NoStacks)));
    }

    #[test]
    fn stacks_with_same_frames_aggregate() {
        let input = r#"
              libc`func_a+0x10
              100

              libc`func_a+0x10
              200

              libc`func_a+0x10
              300
"#;
        let cursor = Cursor::new(input);
        let mut converter = DtraceConverter::new(InputFormat::AggregatedStack);
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // All 3 should aggregate into 1 stack
        assert_eq!(spaa.stacks.len(), 1);

        let stack = spaa.stacks.values().next().unwrap();
        let count_weight = stack.weights.iter().find(|w| w.metric == "count").unwrap();
        assert_eq!(count_weight.value, 600); // 100 + 200 + 300
    }

    #[test]
    fn infer_user_stack() {
        let frames = vec![DtraceFrame {
            module: "libsystem_c.dylib".to_string(),
            symbol: "malloc".to_string(),
            offset: None,
        }];

        assert_eq!(DtraceConverter::infer_stack_kind(&frames), StackKind::User);
    }

    #[test]
    fn infer_kernel_stack() {
        let frames = vec![DtraceFrame {
            module: "kernel".to_string(),
            symbol: "vm_fault".to_string(),
            offset: None,
        }];

        assert_eq!(
            DtraceConverter::infer_stack_kind(&frames),
            StackKind::Kernel
        );
    }

    #[test]
    fn custom_config() {
        let config = ConverterConfig {
            event_name: "syscall::read:entry".to_string(),
            frequency_hz: None,
        };

        let cursor = Cursor::new(SAMPLE_DTRACE_OUTPUT);
        let mut converter = DtraceConverter::with_config(InputFormat::AggregatedStack, config);
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();
        assert_eq!(spaa.header.events[0].name, "syscall::read:entry");
        assert_eq!(spaa.header.events[0].kind, EventKind::Probe);
    }

    #[test]
    fn skips_dtrace_header_lines() {
        let input = r#"
dtrace: description 'profile-997' matched 1 probe
CPU     ID                    FUNCTION:NAME
  0  12345                     :tick-1s

              libc`malloc+0x10
              50
"#;
        let cursor = Cursor::new(input);
        let mut converter = DtraceConverter::new(InputFormat::AggregatedStack);
        converter.parse(cursor).unwrap();

        assert_eq!(converter.stacks.len(), 1);
        assert_eq!(converter.stacks[0].count, 50);
    }

    #[test]
    fn unsupported_format_returns_error() {
        let cursor = Cursor::new("");
        let mut converter = DtraceConverter::new(InputFormat::SplitStacks);
        let result = converter.parse(cursor);

        assert!(matches!(result, Err(ConvertError::UnsupportedFormat)));
    }
}
