//! SPAA (Stack Profile for Agentic Analysis) parser library.
//!
//! This library parses SPAA files from any `Read`-able source and provides
//! structured access to profiling data.
//!
//! # Example
//!
//! ```no_run
//! use std::fs::File;
//! use spaa_parse::SpaaFile;
//!
//! let file = File::open("profile.spaa").unwrap();
//! let spaa = SpaaFile::parse(file).unwrap();
//!
//! println!("Source tool: {}", spaa.header.source_tool);
//! println!("Stacks: {}", spaa.stacks.len());
//! ```

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use thiserror::Error;

/// Errors that can occur during SPAA parsing.
#[derive(Error, Debug)]
pub enum ParseError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error at line {line}: {source}")]
    Json {
        line: usize,
        #[source]
        source: serde_json::Error,
    },

    #[error("missing header record")]
    MissingHeader,

    #[error("header must be first record, found at line {0}")]
    HeaderNotFirst(usize),

    #[error("duplicate header at line {0}")]
    DuplicateHeader(usize),

    #[error("frame {frame_id} references non-existent DSO {dso_id}")]
    InvalidDsoReference { frame_id: u64, dso_id: u64 },

    #[error("stack {stack_id} references non-existent frame {frame_id}")]
    InvalidFrameReference { stack_id: String, frame_id: u64 },

    #[error("stack {stack_id} missing primary metric '{metric}'")]
    MissingPrimaryMetric { stack_id: String, metric: String },

    #[error("sample references non-existent stack {0}")]
    InvalidStackReference(String),

    #[error("unknown record type '{0}' at line {1}")]
    UnknownRecordType(String, usize),
}

/// Result type for SPAA parsing operations.
pub type Result<T> = std::result::Result<T, ParseError>;

/// Errors that can occur during SPAA writing.
#[derive(Error, Debug)]
pub enum WriteError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

/// Result type for SPAA writing operations.
pub type WriteResult<T> = std::result::Result<T, WriteError>;

// ============================================================================
// Header types
// ============================================================================

/// Frame ordering within stacks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameOrder {
    LeafToRoot,
    RootToLeaf,
}

/// Stack ID mode for the file.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackIdMode {
    ContentAddressable,
    Local,
}

/// Sampling mode for an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingMode {
    Period,
    Frequency,
    Event,
}

/// Event kind classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventKind {
    Hardware,
    Software,
    Allocation,
    Deallocation,
    Timer,
    Probe,
}

/// Sampling configuration for an event.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sampling {
    pub mode: SamplingMode,
    pub primary_metric: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sample_period: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_hz: Option<u64>,
}

/// Allocation tracking metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AllocationTracking {
    #[serde(default)]
    pub tracks_frees: bool,
    #[serde(default)]
    pub has_timestamps: bool,
}

/// Event definition in the header.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EventDef {
    pub name: String,
    pub kind: EventKind,
    pub sampling: Sampling,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allocation_tracking: Option<AllocationTracking>,
}

/// Time range for the profile.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
    pub unit: String,
}

/// Source tool information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SourceInfo {
    pub tool: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_version: Option<String>,
}

/// SPAA file header record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Header {
    pub format: String,
    pub version: String,
    pub source_tool: String,
    pub frame_order: FrameOrder,
    pub events: Vec<EventDef>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time_range: Option<TimeRange>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<SourceInfo>,
    #[serde(default = "default_stack_id_mode")]
    pub stack_id_mode: StackIdMode,
}

fn default_stack_id_mode() -> StackIdMode {
    StackIdMode::ContentAddressable
}

// ============================================================================
// Dictionary types
// ============================================================================

/// DSO (Dynamic Shared Object) record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Dso {
    pub id: u64,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub build_id: Option<String>,
    #[serde(default)]
    pub is_kernel: bool,
}

/// Frame kind classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FrameKind {
    User,
    Kernel,
    Unknown,
}

/// Stack frame record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frame {
    pub id: u64,
    pub func: String,
    pub dso: u64,
    #[serde(default = "default_true")]
    pub func_resolved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ip: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub symoff: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub srcline: Option<String>,
    #[serde(default = "default_true")]
    pub srcline_resolved: bool,
    #[serde(default)]
    pub inlined: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub inline_depth: Option<u32>,
    #[serde(default = "default_frame_kind")]
    pub kind: FrameKind,
}

fn default_true() -> bool {
    true
}

fn default_frame_kind() -> FrameKind {
    FrameKind::User
}

/// Thread information record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Thread {
    pub pid: u64,
    pub tid: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comm: Option<String>,
}

// ============================================================================
// Stack types
// ============================================================================

/// Stack type classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StackType {
    Unified,
    User,
    Kernel,
}

impl Default for StackType {
    fn default() -> Self {
        StackType::Unified
    }
}

/// Weight measurement for a stack.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Weight {
    pub metric: String,
    pub value: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

/// DTrace probe context information.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProbeContext {
    pub provider: String,
    #[serde(default)]
    pub module: String,
    pub function: String,
    pub name: String,
}

/// Stack context metadata.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StackContext {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pid: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tid: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comm: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub probe: Option<ProbeContext>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execname: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uid: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub zonename: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_fields: Option<HashMap<String, serde_json::Value>>,
    /// Extension fields not covered by standard schema.
    #[serde(flatten)]
    pub extra: HashMap<String, serde_json::Value>,
}

/// Exclusive weight attribution to leaf frame.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExclusiveWeights {
    pub frame: u64,
    pub weights: Vec<Weight>,
}

/// Aggregated stack record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stack {
    pub id: String,
    pub frames: Vec<u64>,
    #[serde(default)]
    pub stack_type: StackType,
    pub context: StackContext,
    pub weights: Vec<Weight>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusive: Option<ExclusiveWeights>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub related_stacks: Option<Vec<String>>,
}

// ============================================================================
// Optional record types
// ============================================================================

/// Raw sample record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Sample {
    pub timestamp: f64,
    pub pid: u64,
    pub tid: u64,
    pub cpu: u32,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub period: Option<u64>,
    pub stack_id: String,
    #[serde(default)]
    pub context: HashMap<String, serde_json::Value>,
}

/// Stack weight within a time window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WindowStackWeight {
    pub stack_id: String,
    pub weights: Vec<Weight>,
}

/// Time window record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Window {
    pub id: String,
    pub start: f64,
    pub end: f64,
    pub unit: String,
    pub by_stack: Vec<WindowStackWeight>,
}

// ============================================================================
// Internal parsing types
// ============================================================================

/// Raw record used during parsing to determine type.
#[derive(Debug, Deserialize)]
struct RawRecord {
    #[serde(rename = "type")]
    record_type: String,
}

/// Header record with type field for parsing.
#[derive(Debug, Deserialize)]
struct HeaderRecord {
    #[serde(flatten)]
    header: Header,
}

/// DSO record with type field for parsing.
#[derive(Debug, Deserialize)]
struct DsoRecord {
    #[serde(flatten)]
    dso: Dso,
}

/// Frame record with type field for parsing.
#[derive(Debug, Deserialize)]
struct FrameRecord {
    #[serde(flatten)]
    frame: Frame,
}

/// Thread record with type field for parsing.
#[derive(Debug, Deserialize)]
struct ThreadRecord {
    #[serde(flatten)]
    thread: Thread,
}

/// Stack record with type field for parsing.
#[derive(Debug, Deserialize)]
struct StackRecord {
    #[serde(flatten)]
    stack: Stack,
}

/// Sample record with type field for parsing.
#[derive(Debug, Deserialize)]
struct SampleRecord {
    #[serde(flatten)]
    sample: Sample,
}

/// Window record with type field for parsing.
#[derive(Debug, Deserialize)]
struct WindowRecord {
    #[serde(flatten)]
    window: Window,
}

// ============================================================================
// Main SpaaFile type
// ============================================================================

/// A parsed SPAA file containing all profiling data.
#[derive(Debug, Clone)]
pub struct SpaaFile {
    /// File header with metadata and event definitions.
    pub header: Header,
    /// DSO dictionary, keyed by DSO ID.
    pub dsos: HashMap<u64, Dso>,
    /// Frame dictionary, keyed by frame ID.
    pub frames: HashMap<u64, Frame>,
    /// Thread dictionary, keyed by thread ID.
    pub threads: HashMap<u64, Thread>,
    /// Stack records, keyed by stack ID.
    pub stacks: HashMap<String, Stack>,
    /// Raw sample records (optional).
    pub samples: Vec<Sample>,
    /// Time window records (optional).
    pub windows: Vec<Window>,
}

impl SpaaFile {
    /// Parse a SPAA file from any `Read`-able source.
    pub fn parse<R: Read>(reader: R) -> Result<Self> {
        let buf_reader = BufReader::new(reader);
        let mut header: Option<Header> = None;
        let mut dsos: HashMap<u64, Dso> = HashMap::new();
        let mut frames: HashMap<u64, Frame> = HashMap::new();
        let mut threads: HashMap<u64, Thread> = HashMap::new();
        let mut stacks: HashMap<String, Stack> = HashMap::new();
        let mut samples: Vec<Sample> = Vec::new();
        let mut windows: Vec<Window> = Vec::new();

        for (line_num, line_result) in buf_reader.lines().enumerate() {
            let line_num = line_num + 1; // 1-indexed for error messages
            let line = line_result?;

            // Skip empty lines
            if line.trim().is_empty() {
                continue;
            }

            // First, determine the record type
            let raw: RawRecord = serde_json::from_str(&line).map_err(|e| ParseError::Json {
                line: line_num,
                source: e,
            })?;

            match raw.record_type.as_str() {
                "header" => {
                    if header.is_some() {
                        return Err(ParseError::DuplicateHeader(line_num));
                    }
                    if line_num != 1 {
                        return Err(ParseError::HeaderNotFirst(line_num));
                    }
                    let record: HeaderRecord =
                        serde_json::from_str(&line).map_err(|e| ParseError::Json {
                            line: line_num,
                            source: e,
                        })?;
                    header = Some(record.header);
                }
                _ if header.is_none() => {
                    // First non-empty line must be a header
                    return Err(ParseError::HeaderNotFirst(line_num));
                }
                "dso" => {
                    let record: DsoRecord =
                        serde_json::from_str(&line).map_err(|e| ParseError::Json {
                            line: line_num,
                            source: e,
                        })?;
                    dsos.insert(record.dso.id, record.dso);
                }
                "frame" => {
                    let record: FrameRecord =
                        serde_json::from_str(&line).map_err(|e| ParseError::Json {
                            line: line_num,
                            source: e,
                        })?;
                    frames.insert(record.frame.id, record.frame);
                }
                "thread" => {
                    let record: ThreadRecord =
                        serde_json::from_str(&line).map_err(|e| ParseError::Json {
                            line: line_num,
                            source: e,
                        })?;
                    threads.insert(record.thread.tid, record.thread);
                }
                "stack" => {
                    let record: StackRecord =
                        serde_json::from_str(&line).map_err(|e| ParseError::Json {
                            line: line_num,
                            source: e,
                        })?;
                    stacks.insert(record.stack.id.clone(), record.stack);
                }
                "sample" => {
                    let record: SampleRecord =
                        serde_json::from_str(&line).map_err(|e| ParseError::Json {
                            line: line_num,
                            source: e,
                        })?;
                    samples.push(record.sample);
                }
                "window" => {
                    let record: WindowRecord =
                        serde_json::from_str(&line).map_err(|e| ParseError::Json {
                            line: line_num,
                            source: e,
                        })?;
                    windows.push(record.window);
                }
                other => {
                    return Err(ParseError::UnknownRecordType(other.to_string(), line_num));
                }
            }
        }

        let header = header.ok_or(ParseError::MissingHeader)?;

        let file = SpaaFile {
            header,
            dsos,
            frames,
            threads,
            stacks,
            samples,
            windows,
        };

        file.validate()?;

        Ok(file)
    }

    /// Validate the parsed file according to SPAA spec rules.
    fn validate(&self) -> Result<()> {
        // Validate frame DSO references
        for frame in self.frames.values() {
            if !self.dsos.contains_key(&frame.dso) {
                return Err(ParseError::InvalidDsoReference {
                    frame_id: frame.id,
                    dso_id: frame.dso,
                });
            }
        }

        // Build event primary metrics map
        let event_metrics: HashMap<&str, &str> = self
            .header
            .events
            .iter()
            .map(|e| (e.name.as_str(), e.sampling.primary_metric.as_str()))
            .collect();

        // Validate stack frame references and primary metrics
        for stack in self.stacks.values() {
            for &frame_id in &stack.frames {
                if !self.frames.contains_key(&frame_id) {
                    return Err(ParseError::InvalidFrameReference {
                        stack_id: stack.id.clone(),
                        frame_id,
                    });
                }
            }

            // Check primary metric is present
            if let Some(primary_metric) = event_metrics.get(stack.context.event.as_str()) {
                let has_primary = stack.weights.iter().any(|w| w.metric == *primary_metric);
                if !has_primary {
                    return Err(ParseError::MissingPrimaryMetric {
                        stack_id: stack.id.clone(),
                        metric: primary_metric.to_string(),
                    });
                }
            }
        }

        // Validate sample stack references
        for sample in &self.samples {
            if !self.stacks.contains_key(&sample.stack_id) {
                return Err(ParseError::InvalidStackReference(sample.stack_id.clone()));
            }
        }

        Ok(())
    }

    /// Get the primary metric name for a given event.
    pub fn primary_metric_for_event(&self, event_name: &str) -> Option<&str> {
        self.header
            .events
            .iter()
            .find(|e| e.name == event_name)
            .map(|e| e.sampling.primary_metric.as_str())
    }

    /// Get all stacks for a specific event.
    pub fn stacks_for_event(&self, event_name: &str) -> impl Iterator<Item = &Stack> {
        self.stacks
            .values()
            .filter(move |s| s.context.event == event_name)
    }

    /// Resolve a frame ID to its Frame record.
    pub fn resolve_frame(&self, frame_id: u64) -> Option<&Frame> {
        self.frames.get(&frame_id)
    }

    /// Resolve a DSO ID to its DSO record.
    pub fn resolve_dso(&self, dso_id: u64) -> Option<&Dso> {
        self.dsos.get(&dso_id)
    }

    /// Get the fully resolved stack frames for a stack.
    pub fn resolve_stack_frames(&self, stack: &Stack) -> Vec<Option<&Frame>> {
        stack
            .frames
            .iter()
            .map(|&id| self.resolve_frame(id))
            .collect()
    }

    /// Write this SPAA file to a writer in NDJSON format.
    ///
    /// Records are written in the correct order: header first, then dictionaries
    /// (DSOs, frames, threads), then stacks, samples, and windows.
    pub fn write<W: Write>(&self, writer: W) -> WriteResult<()> {
        let mut spaa_writer = SpaaWriter::new(writer);
        spaa_writer.write_header(&self.header)?;

        // Write dictionaries in deterministic order
        let mut dsos: Vec<_> = self.dsos.values().collect();
        dsos.sort_by_key(|d| d.id);
        for dso in dsos {
            spaa_writer.write_dso(dso)?;
        }

        let mut frames: Vec<_> = self.frames.values().collect();
        frames.sort_by_key(|f| f.id);
        for frame in frames {
            spaa_writer.write_frame(frame)?;
        }

        let mut threads: Vec<_> = self.threads.values().collect();
        threads.sort_by_key(|t| t.tid);
        for thread in threads {
            spaa_writer.write_thread(thread)?;
        }

        // Write stacks in deterministic order
        let mut stacks: Vec<_> = self.stacks.values().collect();
        stacks.sort_by(|a, b| a.id.cmp(&b.id));
        for stack in stacks {
            spaa_writer.write_stack(stack)?;
        }

        // Write samples and windows
        for sample in &self.samples {
            spaa_writer.write_sample(sample)?;
        }

        for window in &self.windows {
            spaa_writer.write_window(window)?;
        }

        Ok(())
    }
}

// ============================================================================
// Writer types
// ============================================================================

/// Helper struct for writing typed records with "type" field.
#[derive(Serialize)]
struct TypedRecord<'a, T: Serialize> {
    #[serde(rename = "type")]
    record_type: &'a str,
    #[serde(flatten)]
    data: &'a T,
}

/// Writer for creating SPAA files incrementally.
///
/// This is useful for converters that build SPAA output without first
/// constructing a full `SpaaFile` in memory.
///
/// # Example
///
/// ```no_run
/// use spaa_parse::{SpaaWriter, Header, Dso, Frame, Stack};
/// use std::fs::File;
///
/// let file = File::create("output.spaa").unwrap();
/// let mut writer = SpaaWriter::new(file);
///
/// // Write header first (required)
/// # let header: Header = todo!();
/// writer.write_header(&header).unwrap();
///
/// // Write dictionaries
/// # let dso: Dso = todo!();
/// writer.write_dso(&dso).unwrap();
/// # let frame: Frame = todo!();
/// writer.write_frame(&frame).unwrap();
///
/// // Write stacks
/// # let stack: Stack = todo!();
/// writer.write_stack(&stack).unwrap();
/// ```
pub struct SpaaWriter<W: Write> {
    writer: W,
}

impl<W: Write> SpaaWriter<W> {
    /// Create a new SPAA writer.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Write a header record. This should be called first.
    pub fn write_header(&mut self, header: &Header) -> WriteResult<()> {
        self.write_record("header", header)
    }

    /// Write a DSO dictionary record.
    pub fn write_dso(&mut self, dso: &Dso) -> WriteResult<()> {
        self.write_record("dso", dso)
    }

    /// Write a frame dictionary record.
    pub fn write_frame(&mut self, frame: &Frame) -> WriteResult<()> {
        self.write_record("frame", frame)
    }

    /// Write a thread dictionary record.
    pub fn write_thread(&mut self, thread: &Thread) -> WriteResult<()> {
        self.write_record("thread", thread)
    }

    /// Write a stack record.
    pub fn write_stack(&mut self, stack: &Stack) -> WriteResult<()> {
        self.write_record("stack", stack)
    }

    /// Write a sample record.
    pub fn write_sample(&mut self, sample: &Sample) -> WriteResult<()> {
        self.write_record("sample", sample)
    }

    /// Write a window record.
    pub fn write_window(&mut self, window: &Window) -> WriteResult<()> {
        self.write_record("window", window)
    }

    /// Write a record with the given type tag.
    fn write_record<T: Serialize>(&mut self, record_type: &str, data: &T) -> WriteResult<()> {
        let typed = TypedRecord { record_type, data };
        let json = serde_json::to_string(&typed)?;
        writeln!(self.writer, "{}", json)?;
        Ok(())
    }

    /// Get a reference to the underlying writer.
    pub fn get_ref(&self) -> &W {
        &self.writer
    }

    /// Get a mutable reference to the underlying writer.
    pub fn get_mut(&mut self) -> &mut W {
        &mut self.writer
    }

    /// Consume this writer and return the underlying writer.
    pub fn into_inner(self) -> W {
        self.writer
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn minimal_spaa() -> String {
        r#"{"type":"header","format":"spaa","version":"1.0","source_tool":"perf","frame_order":"leaf_to_root","events":[{"name":"cycles","kind":"hardware","sampling":{"mode":"period","primary_metric":"period"}}]}"#.to_string()
    }

    #[test]
    fn parse_minimal_header() {
        let data = minimal_spaa();
        let cursor = Cursor::new(data);
        let spaa = SpaaFile::parse(cursor).unwrap();

        assert_eq!(spaa.header.format, "spaa");
        assert_eq!(spaa.header.version, "1.0");
        assert_eq!(spaa.header.source_tool, "perf");
        assert_eq!(spaa.header.frame_order, FrameOrder::LeafToRoot);
        assert_eq!(spaa.header.events.len(), 1);
        assert_eq!(spaa.header.events[0].name, "cycles");
    }

    #[test]
    fn parse_dso_and_frame() {
        let data = format!(
            "{}\n{}\n{}",
            minimal_spaa(),
            r#"{"type":"dso","id":1,"name":"/usr/bin/app","is_kernel":false}"#,
            r#"{"type":"frame","id":101,"func":"main","dso":1,"kind":"user"}"#
        );
        let cursor = Cursor::new(data);
        let spaa = SpaaFile::parse(cursor).unwrap();

        assert_eq!(spaa.dsos.len(), 1);
        assert_eq!(spaa.dsos[&1].name, "/usr/bin/app");

        assert_eq!(spaa.frames.len(), 1);
        assert_eq!(spaa.frames[&101].func, "main");
        assert_eq!(spaa.frames[&101].dso, 1);
    }

    #[test]
    fn parse_stack_with_weights() {
        let data = format!(
            "{}\n{}\n{}\n{}",
            minimal_spaa(),
            r#"{"type":"dso","id":1,"name":"/usr/bin/app","is_kernel":false}"#,
            r#"{"type":"frame","id":101,"func":"main","dso":1,"kind":"user"}"#,
            r#"{"type":"stack","id":"0xabc","frames":[101],"context":{"event":"cycles"},"weights":[{"metric":"period","value":12345}]}"#
        );
        let cursor = Cursor::new(data);
        let spaa = SpaaFile::parse(cursor).unwrap();

        assert_eq!(spaa.stacks.len(), 1);
        let stack = &spaa.stacks["0xabc"];
        assert_eq!(stack.frames, vec![101]);
        assert_eq!(stack.weights[0].metric, "period");
        assert_eq!(stack.weights[0].value, 12345);
    }

    #[test]
    fn missing_header_fails() {
        let data = "";
        let cursor = Cursor::new(data);
        let result = SpaaFile::parse(cursor);

        assert!(matches!(result, Err(ParseError::MissingHeader)));
    }

    #[test]
    fn non_header_first_fails() {
        let data = r#"{"type":"dso","id":1,"name":"/usr/bin/app"}"#;
        let cursor = Cursor::new(data);
        let result = SpaaFile::parse(cursor);

        assert!(matches!(result, Err(ParseError::HeaderNotFirst(1))));
    }

    #[test]
    fn invalid_dso_reference_fails() {
        let data = format!(
            "{}\n{}",
            minimal_spaa(),
            r#"{"type":"frame","id":101,"func":"main","dso":999,"kind":"user"}"#
        );
        let cursor = Cursor::new(data);
        let result = SpaaFile::parse(cursor);

        assert!(matches!(
            result,
            Err(ParseError::InvalidDsoReference {
                frame_id: 101,
                dso_id: 999
            })
        ));
    }

    #[test]
    fn invalid_frame_reference_fails() {
        let data = format!(
            "{}\n{}\n{}",
            minimal_spaa(),
            r#"{"type":"dso","id":1,"name":"/usr/bin/app","is_kernel":false}"#,
            r#"{"type":"stack","id":"0xabc","frames":[999],"context":{"event":"cycles"},"weights":[{"metric":"period","value":1}]}"#
        );
        let cursor = Cursor::new(data);
        let result = SpaaFile::parse(cursor);

        assert!(matches!(
            result,
            Err(ParseError::InvalidFrameReference {
                stack_id,
                frame_id: 999
            }) if stack_id == "0xabc"
        ));
    }

    #[test]
    fn missing_primary_metric_fails() {
        let data = format!(
            "{}\n{}\n{}\n{}",
            minimal_spaa(),
            r#"{"type":"dso","id":1,"name":"/usr/bin/app","is_kernel":false}"#,
            r#"{"type":"frame","id":101,"func":"main","dso":1,"kind":"user"}"#,
            r#"{"type":"stack","id":"0xabc","frames":[101],"context":{"event":"cycles"},"weights":[{"metric":"samples","value":1}]}"#
        );
        let cursor = Cursor::new(data);
        let result = SpaaFile::parse(cursor);

        assert!(matches!(
            result,
            Err(ParseError::MissingPrimaryMetric { stack_id, metric })
                if stack_id == "0xabc" && metric == "period"
        ));
    }

    #[test]
    fn parse_thread_record() {
        let data = format!(
            "{}\n{}",
            minimal_spaa(),
            r#"{"type":"thread","pid":1234,"tid":5678,"comm":"myapp"}"#
        );
        let cursor = Cursor::new(data);
        let spaa = SpaaFile::parse(cursor).unwrap();

        assert_eq!(spaa.threads.len(), 1);
        let thread = &spaa.threads[&5678];
        assert_eq!(thread.pid, 1234);
        assert_eq!(thread.comm, Some("myapp".to_string()));
    }

    #[test]
    fn parse_sample_record() {
        let data = format!(
            "{}\n{}\n{}\n{}\n{}",
            minimal_spaa(),
            r#"{"type":"dso","id":1,"name":"/usr/bin/app","is_kernel":false}"#,
            r#"{"type":"frame","id":101,"func":"main","dso":1,"kind":"user"}"#,
            r#"{"type":"stack","id":"0xabc","frames":[101],"context":{"event":"cycles"},"weights":[{"metric":"period","value":1}]}"#,
            r#"{"type":"sample","timestamp":123.456,"pid":1000,"tid":1001,"cpu":0,"event":"cycles","stack_id":"0xabc"}"#
        );
        let cursor = Cursor::new(data);
        let spaa = SpaaFile::parse(cursor).unwrap();

        assert_eq!(spaa.samples.len(), 1);
        assert_eq!(spaa.samples[0].timestamp, 123.456);
        assert_eq!(spaa.samples[0].stack_id, "0xabc");
    }

    #[test]
    fn parse_window_record() {
        let data = format!(
            "{}\n{}\n{}\n{}\n{}",
            minimal_spaa(),
            r#"{"type":"dso","id":1,"name":"/usr/bin/app","is_kernel":false}"#,
            r#"{"type":"frame","id":101,"func":"main","dso":1,"kind":"user"}"#,
            r#"{"type":"stack","id":"0xabc","frames":[101],"context":{"event":"cycles"},"weights":[{"metric":"period","value":1}]}"#,
            r#"{"type":"window","id":"w1","start":0.0,"end":1.0,"unit":"seconds","by_stack":[{"stack_id":"0xabc","weights":[{"metric":"period","value":1}]}]}"#
        );
        let cursor = Cursor::new(data);
        let spaa = SpaaFile::parse(cursor).unwrap();

        assert_eq!(spaa.windows.len(), 1);
        assert_eq!(spaa.windows[0].id, "w1");
        assert_eq!(spaa.windows[0].by_stack.len(), 1);
    }

    #[test]
    fn resolve_stack_frames_works() {
        let data = format!(
            "{}\n{}\n{}\n{}\n{}",
            minimal_spaa(),
            r#"{"type":"dso","id":1,"name":"/usr/bin/app","is_kernel":false}"#,
            r#"{"type":"frame","id":101,"func":"main","dso":1,"kind":"user"}"#,
            r#"{"type":"frame","id":102,"func":"foo","dso":1,"kind":"user"}"#,
            r#"{"type":"stack","id":"0xabc","frames":[101,102],"context":{"event":"cycles"},"weights":[{"metric":"period","value":1}]}"#
        );
        let cursor = Cursor::new(data);
        let spaa = SpaaFile::parse(cursor).unwrap();

        let stack = &spaa.stacks["0xabc"];
        let resolved = spaa.resolve_stack_frames(stack);

        assert_eq!(resolved.len(), 2);
        assert_eq!(resolved[0].unwrap().func, "main");
        assert_eq!(resolved[1].unwrap().func, "foo");
    }

    #[test]
    fn stacks_for_event_filters_correctly() {
        let header = r#"{"type":"header","format":"spaa","version":"1.0","source_tool":"perf","frame_order":"leaf_to_root","events":[{"name":"cycles","kind":"hardware","sampling":{"mode":"period","primary_metric":"period"}},{"name":"cache-misses","kind":"hardware","sampling":{"mode":"period","primary_metric":"period"}}]}"#;
        let data = format!(
            "{}\n{}\n{}\n{}\n{}",
            header,
            r#"{"type":"dso","id":1,"name":"/usr/bin/app","is_kernel":false}"#,
            r#"{"type":"frame","id":101,"func":"main","dso":1,"kind":"user"}"#,
            r#"{"type":"stack","id":"0xabc","frames":[101],"context":{"event":"cycles"},"weights":[{"metric":"period","value":1}]}"#,
            r#"{"type":"stack","id":"0xdef","frames":[101],"context":{"event":"cache-misses"},"weights":[{"metric":"period","value":2}]}"#
        );
        let cursor = Cursor::new(data);
        let spaa = SpaaFile::parse(cursor).unwrap();

        let cycles_stacks: Vec<_> = spaa.stacks_for_event("cycles").collect();
        assert_eq!(cycles_stacks.len(), 1);
        assert_eq!(cycles_stacks[0].id, "0xabc");

        let cache_stacks: Vec<_> = spaa.stacks_for_event("cache-misses").collect();
        assert_eq!(cache_stacks.len(), 1);
        assert_eq!(cache_stacks[0].id, "0xdef");
    }

    #[test]
    fn skips_empty_lines() {
        let data = format!("{}\n\n\n", minimal_spaa());
        let cursor = Cursor::new(data);
        let result = SpaaFile::parse(cursor);
        assert!(result.is_ok());
    }

    #[test]
    fn duplicate_header_fails() {
        let data = format!("{}\n{}", minimal_spaa(), minimal_spaa());
        let cursor = Cursor::new(data);
        let result = SpaaFile::parse(cursor);

        assert!(matches!(result, Err(ParseError::DuplicateHeader(2))));
    }

    #[test]
    fn unknown_record_type_fails() {
        let data = format!(
            "{}\n{}",
            minimal_spaa(),
            r#"{"type":"unknown","foo":"bar"}"#
        );
        let cursor = Cursor::new(data);
        let result = SpaaFile::parse(cursor);

        assert!(matches!(
            result,
            Err(ParseError::UnknownRecordType(t, 2)) if t == "unknown"
        ));
    }

    #[test]
    fn write_and_read_roundtrip() {
        // Parse a file
        let data = format!(
            "{}\n{}\n{}\n{}",
            minimal_spaa(),
            r#"{"type":"dso","id":1,"name":"/usr/bin/app","is_kernel":false}"#,
            r#"{"type":"frame","id":101,"func":"main","dso":1,"kind":"user"}"#,
            r#"{"type":"stack","id":"0xabc","frames":[101],"context":{"event":"cycles"},"weights":[{"metric":"period","value":12345}]}"#
        );
        let original = SpaaFile::parse(Cursor::new(data)).unwrap();

        // Write it out
        let mut output = Vec::new();
        original.write(&mut output).unwrap();

        // Parse the output
        let roundtrip = SpaaFile::parse(Cursor::new(output)).unwrap();

        // Verify key fields match
        assert_eq!(roundtrip.header.format, original.header.format);
        assert_eq!(roundtrip.header.source_tool, original.header.source_tool);
        assert_eq!(roundtrip.dsos.len(), original.dsos.len());
        assert_eq!(roundtrip.frames.len(), original.frames.len());
        assert_eq!(roundtrip.stacks.len(), original.stacks.len());
        assert_eq!(
            roundtrip.stacks["0xabc"].weights[0].value,
            original.stacks["0xabc"].weights[0].value
        );
    }

    #[test]
    fn spaa_writer_creates_valid_output() {
        let mut output = Vec::new();
        {
            let mut writer = super::SpaaWriter::new(&mut output);

            let header = Header {
                format: "spaa".to_string(),
                version: "1.0".to_string(),
                source_tool: "test".to_string(),
                frame_order: FrameOrder::LeafToRoot,
                events: vec![EventDef {
                    name: "cycles".to_string(),
                    kind: EventKind::Hardware,
                    sampling: Sampling {
                        mode: SamplingMode::Period,
                        primary_metric: "period".to_string(),
                        sample_period: None,
                        frequency_hz: None,
                    },
                    allocation_tracking: None,
                }],
                time_range: None,
                source: None,
                stack_id_mode: StackIdMode::ContentAddressable,
            };
            writer.write_header(&header).unwrap();

            let dso = Dso {
                id: 1,
                name: "/bin/test".to_string(),
                build_id: None,
                is_kernel: false,
            };
            writer.write_dso(&dso).unwrap();

            let frame = Frame {
                id: 1,
                func: "main".to_string(),
                dso: 1,
                func_resolved: true,
                ip: None,
                symoff: None,
                srcline: None,
                srcline_resolved: true,
                inlined: false,
                inline_depth: None,
                kind: FrameKind::User,
            };
            writer.write_frame(&frame).unwrap();

            let stack = Stack {
                id: "0x1".to_string(),
                frames: vec![1],
                stack_type: StackType::Unified,
                context: StackContext {
                    event: "cycles".to_string(),
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
                weights: vec![Weight {
                    metric: "period".to_string(),
                    value: 100,
                    unit: None,
                }],
                exclusive: None,
                related_stacks: None,
            };
            writer.write_stack(&stack).unwrap();
        }

        // Verify the output is valid SPAA
        let spaa = SpaaFile::parse(Cursor::new(output)).unwrap();
        assert_eq!(spaa.header.source_tool, "test");
        assert_eq!(spaa.dsos.len(), 1);
        assert_eq!(spaa.frames.len(), 1);
        assert_eq!(spaa.stacks.len(), 1);
    }
}
