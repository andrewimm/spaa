//! Convert Linux `perf script` output to SPAA format.
//!
//! This module parses the text output from `perf script` and converts it
//! to the SPAA (Stack Profile for Agentic Analysis) format.
//!
//! # Example
//!
//! ```no_run
//! use spaa::perf::PerfConverter;
//! use std::fs::File;
//! use std::io::{BufReader, BufWriter};
//!
//! let input = BufReader::new(File::open("perf.txt").unwrap());
//! let output = BufWriter::new(File::create("profile.spaa").unwrap());
//!
//! let mut converter = PerfConverter::new();
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

/// Errors that can occur during perf script parsing.
#[derive(Error, Debug)]
pub enum ConvertError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("no samples found in input")]
    NoSamples,
}

pub type Result<T> = std::result::Result<T, ConvertError>;

/// A parsed sample from perf script output.
#[derive(Debug, Clone)]
struct PerfSample {
    comm: String,
    pid: u64,
    tid: u64,
    #[allow(dead_code)] // Preserved for potential future use in per-sample output
    cpu: Option<u32>,
    timestamp: Option<f64>,
    period: u64,
    event: String,
    frames: Vec<PerfFrame>,
}

/// A parsed stack frame from perf script output.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct PerfFrame {
    ip: String,
    symbol: String,
    offset: Option<String>,
    dso: String,
    srcline: Option<String>,
}

/// Converter from perf script output to SPAA format.
pub struct PerfConverter {
    samples: Vec<PerfSample>,
    events: HashMap<String, EventInfo>,
    time_range: Option<(f64, f64)>,
}

#[derive(Debug, Clone)]
struct EventInfo {
    name: String,
    kind: EventKind,
}

impl PerfConverter {
    /// Create a new converter.
    pub fn new() -> Self {
        Self {
            samples: Vec::new(),
            events: HashMap::new(),
            time_range: None,
        }
    }

    /// Parse perf script output from a reader.
    pub fn parse<R: Read>(&mut self, reader: R) -> Result<()> {
        let buf_reader = BufReader::new(reader);
        let mut current_sample: Option<PerfSample> = None;
        let mut line_num = 0;

        for line_result in buf_reader.lines() {
            line_num += 1;
            let line = line_result?;

            // Skip empty lines and comments
            if line.trim().is_empty() || line.starts_with('#') {
                // If we have a current sample and hit empty line, finalize it
                if let Some(sample) = current_sample.take() {
                    if !sample.frames.is_empty() {
                        self.add_sample(sample);
                    }
                }
                continue;
            }

            // Check if this is a sample header line or a stack frame
            if !line.starts_with('\t') && !line.starts_with(' ') {
                // Finalize previous sample
                if let Some(sample) = current_sample.take() {
                    if !sample.frames.is_empty() {
                        self.add_sample(sample);
                    }
                }

                // Parse new sample header
                match Self::parse_sample_header(&line) {
                    Ok(sample) => current_sample = Some(sample),
                    Err(msg) => {
                        // Could be a header line from perf script --header, skip it
                        if !line.contains(':') {
                            continue;
                        }
                        return Err(ConvertError::Parse {
                            line: line_num,
                            message: msg,
                        });
                    }
                }
            } else if let Some(ref mut sample) = current_sample {
                // Parse stack frame
                if let Some(frame) = Self::parse_frame(&line) {
                    sample.frames.push(frame);
                }
            }
        }

        // Finalize last sample
        if let Some(sample) = current_sample {
            if !sample.frames.is_empty() {
                self.add_sample(sample);
            }
        }

        Ok(())
    }

    fn add_sample(&mut self, sample: PerfSample) {
        // Track event types
        if !self.events.contains_key(&sample.event) {
            let kind = Self::classify_event(&sample.event);
            self.events.insert(
                sample.event.clone(),
                EventInfo {
                    name: sample.event.clone(),
                    kind,
                },
            );
        }

        // Track time range
        if let Some(ts) = sample.timestamp {
            match &mut self.time_range {
                None => self.time_range = Some((ts, ts)),
                Some((start, end)) => {
                    if ts < *start {
                        *start = ts;
                    }
                    if ts > *end {
                        *end = ts;
                    }
                }
            }
        }

        self.samples.push(sample);
    }

    /// Parse a sample header line.
    /// Format: `comm pid[/tid] [cpu] timestamp: period event:`
    /// Examples:
    ///   `myapp  1234 [000] 12345.678901:     123456 cycles:`
    ///   `myapp  1234/5678 [000] 12345.678901:     123456 cycles:`
    fn parse_sample_header(line: &str) -> std::result::Result<PerfSample, String> {
        let line = line.trim();

        // Find the colon that separates timestamp from period/event
        let colon_pos = line.find(':').ok_or("no colon found")?;
        let before_colon = &line[..colon_pos];
        let after_colon = &line[colon_pos + 1..];

        // Parse period and event from after the colon
        let after_parts: Vec<&str> = after_colon.split_whitespace().collect();
        if after_parts.is_empty() {
            return Err("no event info after colon".into());
        }

        let (period, event) = if after_parts.len() >= 2 {
            let period = after_parts[0].parse::<u64>().unwrap_or(1);
            let event = after_parts[1].trim_end_matches(':').to_string();
            (period, event)
        } else {
            // Sometimes just the event name
            (1, after_parts[0].trim_end_matches(':').to_string())
        };

        // Parse the part before the colon
        let parts: Vec<&str> = before_colon.split_whitespace().collect();
        if parts.len() < 2 {
            return Err("not enough fields before colon".into());
        }

        // First part is comm (may have spaces, but we take first token)
        let comm = parts[0].to_string();

        // Second part is pid or pid/tid
        let (pid, tid) = Self::parse_pid_tid(parts[1])?;

        // Look for CPU in brackets and timestamp
        let mut cpu = None;
        let mut timestamp = None;

        for part in &parts[2..] {
            if part.starts_with('[') && part.ends_with(']') {
                // CPU number
                let cpu_str = part.trim_start_matches('[').trim_end_matches(']');
                cpu = cpu_str.parse().ok();
            } else if part.contains('.') {
                // Timestamp
                timestamp = part.parse().ok();
            }
        }

        Ok(PerfSample {
            comm,
            pid,
            tid,
            cpu,
            timestamp,
            period,
            event,
            frames: Vec::new(),
        })
    }

    fn parse_pid_tid(s: &str) -> std::result::Result<(u64, u64), String> {
        if let Some(slash_pos) = s.find('/') {
            let pid = s[..slash_pos].parse().map_err(|_| "invalid pid")?;
            let tid = s[slash_pos + 1..].parse().map_err(|_| "invalid tid")?;
            Ok((pid, tid))
        } else {
            let pid = s.parse().map_err(|_| "invalid pid")?;
            Ok((pid, pid)) // tid defaults to pid if not specified
        }
    }

    /// Parse a stack frame line.
    /// Format: `\t ip symbol+offset (dso)`
    /// Examples:
    ///   `\t 401234 main+0x54 (/usr/bin/myapp)`
    ///   `\t 7ffff7a12345 __libc_start_main+0x80 (/lib/x86_64-linux-gnu/libc.so.6)`
    ///   `\t ffffffff81234567 native_write_msr+0x6 ([kernel.kallsyms])`
    fn parse_frame(line: &str) -> Option<PerfFrame> {
        let line = line.trim();
        if line.is_empty() {
            return None;
        }

        // Find the DSO in parentheses at the end
        let dso_start = line.rfind('(')?;
        let dso_end = line.rfind(')')?;
        if dso_end <= dso_start {
            return None;
        }
        let dso = line[dso_start + 1..dso_end].to_string();

        // Parse the part before DSO
        let before_dso = line[..dso_start].trim();
        let parts: Vec<&str> = before_dso.split_whitespace().collect();
        if parts.is_empty() {
            return None;
        }

        let ip = parts[0].to_string();

        // Rest is symbol+offset (may contain spaces in demangled names)
        let symbol_part = if parts.len() > 1 {
            parts[1..].join(" ")
        } else {
            format!("0x{}", ip)
        };

        // Split symbol and offset
        let (symbol, offset) = if let Some(plus_pos) = symbol_part.rfind('+') {
            let sym = symbol_part[..plus_pos].to_string();
            let off = symbol_part[plus_pos + 1..].to_string();
            (sym, Some(off))
        } else {
            (symbol_part, None)
        };

        Some(PerfFrame {
            ip,
            symbol,
            offset,
            dso,
            srcline: None,
        })
    }

    fn classify_event(event: &str) -> EventKind {
        let event_lower = event.to_lowercase();
        if event_lower.contains("cycles")
            || event_lower.contains("instructions")
            || event_lower.contains("cache")
            || event_lower.contains("branch")
        {
            EventKind::Hardware
        } else if event_lower.contains("page-fault")
            || event_lower.contains("context-switch")
            || event_lower.contains("cpu-migration")
        {
            EventKind::Software
        } else if event_lower.contains(':') {
            // Tracepoint format: subsystem:event
            EventKind::Probe
        } else {
            EventKind::Hardware // Default assumption
        }
    }

    /// Write the parsed data as SPAA format to a writer.
    pub fn write_spaa<W: Write>(&self, mut writer: W) -> Result<()> {
        if self.samples.is_empty() {
            return Err(ConvertError::NoSamples);
        }

        // Build dictionaries
        let mut dso_map: HashMap<&str, u64> = HashMap::new();
        let mut frame_map: HashMap<&PerfFrame, u64> = HashMap::new();
        let mut thread_map: HashMap<(u64, u64), ()> = HashMap::new();

        // First pass: collect unique DSOs, frames, and threads
        for sample in &self.samples {
            thread_map.insert((sample.pid, sample.tid), ());
            for frame in &sample.frames {
                if !dso_map.contains_key(frame.dso.as_str()) {
                    let id = dso_map.len() as u64 + 1;
                    dso_map.insert(&frame.dso, id);
                }
                if !frame_map.contains_key(frame) {
                    let id = frame_map.len() as u64 + 1;
                    frame_map.insert(frame, id);
                }
            }
        }

        // Aggregate stacks
        let aggregated = self.aggregate_stacks(&frame_map);

        // Write header
        let header = self.build_header();
        self.write_record(&mut writer, "header", &header)?;

        // Write DSO dictionary
        for (dso_name, dso_id) in &dso_map {
            let is_kernel = dso_name.contains("[kernel")
                || dso_name.contains("kallsyms")
                || dso_name.starts_with("[k]");
            let dso = DsoRecord {
                id: *dso_id,
                name: (*dso_name).to_string(),
                build_id: None,
                is_kernel,
            };
            self.write_record(&mut writer, "dso", &dso)?;
        }

        // Write frame dictionary
        for (perf_frame, frame_id) in &frame_map {
            let dso_id = dso_map[perf_frame.dso.as_str()];
            let is_kernel =
                perf_frame.dso.contains("[kernel") || perf_frame.dso.contains("kallsyms");
            let frame = FrameRecord {
                id: *frame_id,
                func: perf_frame.symbol.clone(),
                func_resolved: !perf_frame.symbol.starts_with("0x"),
                dso: dso_id,
                ip: Some(format!("0x{}", perf_frame.ip)),
                symoff: perf_frame.offset.clone(),
                srcline: perf_frame.srcline.clone(),
                inlined: false,
                kind: if is_kernel {
                    FrameKind::Kernel
                } else {
                    FrameKind::User
                },
            };
            self.write_record(&mut writer, "frame", &frame)?;
        }

        // Write thread dictionary
        for (pid, tid) in thread_map.keys() {
            let thread = ThreadRecord {
                pid: *pid,
                tid: *tid,
                comm: None, // Could track this per-thread
            };
            self.write_record(&mut writer, "thread", &thread)?;
        }

        // Write stacks
        for (stack_key, stack_data) in &aggregated {
            let stack = StackRecord {
                id: stack_key.id.clone(),
                frames: stack_key.frame_ids.clone(),
                stack_type: StackType::Unified,
                context: StackContext {
                    event: stack_key.event.clone(),
                    pid: Some(stack_key.pid),
                    tid: Some(stack_key.tid),
                    cpu: None,
                    comm: Some(stack_key.comm.clone()),
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
                        value: stack_data.sample_count,
                        unit: None,
                    },
                    Weight {
                        metric: "period".to_string(),
                        value: stack_data.total_period,
                        unit: Some("events".to_string()),
                    },
                ],
                exclusive: stack_key.frame_ids.first().map(|&leaf| ExclusiveWeights {
                    frame: leaf,
                    weights: vec![Weight {
                        metric: "period".to_string(),
                        value: stack_data.total_period,
                        unit: Some("events".to_string()),
                    }],
                }),
                related_stacks: None,
            };
            self.write_record(&mut writer, "stack", &stack)?;
        }

        Ok(())
    }

    fn build_header(&self) -> Header {
        let events: Vec<EventDef> = self
            .events
            .values()
            .map(|e| EventDef {
                name: e.name.clone(),
                kind: e.kind,
                sampling: Sampling {
                    mode: SamplingMode::Period,
                    primary_metric: "period".to_string(),
                    sample_period: None,
                    frequency_hz: None,
                },
                allocation_tracking: None,
            })
            .collect();

        Header {
            format: "spaa".to_string(),
            version: "1.0".to_string(),
            source_tool: "perf".to_string(),
            frame_order: FrameOrder::LeafToRoot,
            events,
            time_range: self.time_range.map(|(start, end)| spaa_parse::TimeRange {
                start,
                end,
                unit: "seconds".to_string(),
            }),
            source: Some(spaa_parse::SourceInfo {
                tool: "perf".to_string(),
                command: None,
                tool_version: None,
            }),
            stack_id_mode: StackIdMode::ContentAddressable,
        }
    }

    fn aggregate_stacks(
        &self,
        frame_map: &HashMap<&PerfFrame, u64>,
    ) -> HashMap<StackKey, StackData> {
        let mut aggregated: HashMap<StackKey, StackData> = HashMap::new();

        for sample in &self.samples {
            let frame_ids: Vec<u64> = sample.frames.iter().map(|f| frame_map[f]).collect();

            if frame_ids.is_empty() {
                continue;
            }

            let stack_id = Self::compute_stack_id(&frame_ids);
            let key = StackKey {
                id: stack_id,
                frame_ids,
                event: sample.event.clone(),
                pid: sample.pid,
                tid: sample.tid,
                comm: sample.comm.clone(),
            };

            let data = aggregated.entry(key).or_insert(StackData {
                sample_count: 0,
                total_period: 0,
            });
            data.sample_count += 1;
            data.total_period += sample.period;
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
        // Create a combined structure with type field
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

impl Default for PerfConverter {
    fn default() -> Self {
        Self::new()
    }
}

// Serialization records (slightly different from spaa_parse types to control field order)
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
struct ThreadRecord {
    pid: u64,
    tid: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    comm: Option<String>,
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
    event: String,
    pid: u64,
    tid: u64,
    comm: String,
}

#[derive(Debug, Clone)]
struct StackData {
    sample_count: u64,
    total_period: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    const SAMPLE_PERF_OUTPUT: &str = r#"
myapp  1234 [000] 12345.678901:     100000 cycles:
	401234 main+0x54 (/usr/bin/myapp)
	7f1234567890 __libc_start_main+0x80 (/lib/x86_64-linux-gnu/libc.so.6)

myapp  1234 [001] 12345.679000:     100000 cycles:
	401234 main+0x54 (/usr/bin/myapp)
	7f1234567890 __libc_start_main+0x80 (/lib/x86_64-linux-gnu/libc.so.6)

myapp  1234 [000] 12345.680000:     100000 cycles:
	401280 foo+0x10 (/usr/bin/myapp)
	401234 main+0x54 (/usr/bin/myapp)
	7f1234567890 __libc_start_main+0x80 (/lib/x86_64-linux-gnu/libc.so.6)
"#;

    #[test]
    fn parse_sample_header_basic() {
        let line = "myapp  1234 [000] 12345.678901:     100000 cycles:";
        let sample = PerfConverter::parse_sample_header(line).unwrap();

        assert_eq!(sample.comm, "myapp");
        assert_eq!(sample.pid, 1234);
        assert_eq!(sample.tid, 1234);
        assert_eq!(sample.cpu, Some(0));
        assert!((sample.timestamp.unwrap() - 12345.678901).abs() < 0.000001);
        assert_eq!(sample.period, 100000);
        assert_eq!(sample.event, "cycles");
    }

    #[test]
    fn parse_sample_header_with_tid() {
        let line = "myapp  1234/5678 [002] 12345.678901:     200000 cycles:";
        let sample = PerfConverter::parse_sample_header(line).unwrap();

        assert_eq!(sample.pid, 1234);
        assert_eq!(sample.tid, 5678);
        assert_eq!(sample.cpu, Some(2));
        assert_eq!(sample.period, 200000);
    }

    #[test]
    fn parse_frame_basic() {
        let line = "\t401234 main+0x54 (/usr/bin/myapp)";
        let frame = PerfConverter::parse_frame(line).unwrap();

        assert_eq!(frame.ip, "401234");
        assert_eq!(frame.symbol, "main");
        assert_eq!(frame.offset, Some("0x54".to_string()));
        assert_eq!(frame.dso, "/usr/bin/myapp");
    }

    #[test]
    fn parse_frame_kernel() {
        let line = "\tffffffff81234567 native_write_msr+0x6 ([kernel.kallsyms])";
        let frame = PerfConverter::parse_frame(line).unwrap();

        assert_eq!(frame.ip, "ffffffff81234567");
        assert_eq!(frame.symbol, "native_write_msr");
        assert_eq!(frame.offset, Some("0x6".to_string()));
        assert_eq!(frame.dso, "[kernel.kallsyms]");
    }

    #[test]
    fn parse_frame_no_offset() {
        let line = "\t401234 main (/usr/bin/myapp)";
        let frame = PerfConverter::parse_frame(line).unwrap();

        assert_eq!(frame.symbol, "main");
        assert_eq!(frame.offset, None);
    }

    #[test]
    fn parse_frame_unresolved() {
        let line = "\t401234 0x401234 (/usr/bin/myapp)";
        let frame = PerfConverter::parse_frame(line).unwrap();

        assert_eq!(frame.symbol, "0x401234");
        assert_eq!(frame.offset, None);
    }

    #[test]
    fn parse_full_perf_output() {
        let cursor = Cursor::new(SAMPLE_PERF_OUTPUT);
        let mut converter = PerfConverter::new();
        converter.parse(cursor).unwrap();

        assert_eq!(converter.samples.len(), 3);
        assert_eq!(converter.events.len(), 1);
        assert!(converter.events.contains_key("cycles"));
    }

    #[test]
    fn convert_to_spaa() {
        let cursor = Cursor::new(SAMPLE_PERF_OUTPUT);
        let mut converter = PerfConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = output_str.lines().collect();

        // Should have header, DSOs, frames, threads, and stacks
        assert!(!lines.is_empty());

        // First line should be header
        assert!(lines[0].contains("\"type\":\"header\""));
        assert!(lines[0].contains("\"format\":\"spaa\""));
        assert!(lines[0].contains("\"source_tool\":\"perf\""));
    }

    #[test]
    fn spaa_output_validates() {
        let cursor = Cursor::new(SAMPLE_PERF_OUTPUT);
        let mut converter = PerfConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        // Parse the output with spaa_parse to validate it
        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        assert_eq!(spaa.header.source_tool, "perf");
        assert_eq!(spaa.header.frame_order, FrameOrder::LeafToRoot);
        assert!(!spaa.dsos.is_empty());
        assert!(!spaa.frames.is_empty());
        assert!(!spaa.stacks.is_empty());

        // Check aggregation: we had 2 samples with same stack, 1 with different
        assert_eq!(spaa.stacks.len(), 2);
    }

    #[test]
    fn empty_input_returns_error() {
        let cursor = Cursor::new("");
        let mut converter = PerfConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        let result = converter.write_spaa(&mut output);

        assert!(matches!(result, Err(ConvertError::NoSamples)));
    }

    #[test]
    fn classify_event_hardware() {
        assert!(matches!(
            PerfConverter::classify_event("cycles"),
            EventKind::Hardware
        ));
        assert!(matches!(
            PerfConverter::classify_event("instructions"),
            EventKind::Hardware
        ));
        assert!(matches!(
            PerfConverter::classify_event("cache-misses"),
            EventKind::Hardware
        ));
    }

    #[test]
    fn classify_event_software() {
        assert!(matches!(
            PerfConverter::classify_event("page-faults"),
            EventKind::Software
        ));
        assert!(matches!(
            PerfConverter::classify_event("context-switches"),
            EventKind::Software
        ));
    }

    #[test]
    fn classify_event_tracepoint() {
        assert!(matches!(
            PerfConverter::classify_event("sched:sched_switch"),
            EventKind::Probe
        ));
    }

    #[test]
    fn stacks_are_aggregated_correctly() {
        let input = r#"
app 100 [0] 1.0:     1000 cycles:
	1000 func_a (/bin/app)

app 100 [0] 2.0:     2000 cycles:
	1000 func_a (/bin/app)

app 100 [0] 3.0:     3000 cycles:
	1000 func_a (/bin/app)
"#;
        let cursor = Cursor::new(input);
        let mut converter = PerfConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // All 3 samples should be aggregated into 1 stack
        assert_eq!(spaa.stacks.len(), 1);

        let stack = spaa.stacks.values().next().unwrap();
        let samples_weight = stack
            .weights
            .iter()
            .find(|w| w.metric == "samples")
            .unwrap();
        let period_weight = stack.weights.iter().find(|w| w.metric == "period").unwrap();

        assert_eq!(samples_weight.value, 3);
        assert_eq!(period_weight.value, 6000); // 1000 + 2000 + 3000
    }
}
