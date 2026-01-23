//! Convert Chrome profiling data to SPAA format.
//!
//! This module parses Chrome DevTools profiling data and converts it
//! to the SPAA (Stack Profile for Agentic Analysis) format.
//!
//! # Supported Formats
//!
//! 1. **Chrome Performance trace** (`.json`): The DevTools Performance panel
//!    export format with `traceEvents` containing `Profile` and `ProfileChunk`
//!    events. This is the format you get from Chrome's Performance panel.
//!
//! 2. **Standalone cpuprofile** (`.cpuprofile`): The V8 JSON format with
//!    `nodes`, `samples`, and `timeDeltas` at the top level.
//!
//! 3. **Chrome heap snapshot** (`.heapsnapshot`): Memory profiling data from
//!    Chrome's Memory panel with allocation stack traces.
//!
//! 4. **Chrome heap timeline** (`.heaptimeline`): Temporal heap allocation data
//!    from Chrome's Memory panel. Similar to heap snapshots but includes
//!    timestamp samples for tracking allocations over time.
//!
//! # Example: CPU Profile
//!
//! ```no_run
//! use spaa::chrome::CpuProfileConverter;
//! use std::fs::File;
//! use std::io::{BufReader, BufWriter};
//!
//! let input = BufReader::new(File::open("trace.json").unwrap());
//! let output = BufWriter::new(File::create("profile.spaa").unwrap());
//!
//! let mut converter = CpuProfileConverter::new();
//! converter.parse(input).unwrap();
//! converter.write_spaa(output).unwrap();
//! ```
//!
//! # Example: Heap Snapshot
//!
//! ```no_run
//! use spaa::chrome::HeapSnapshotConverter;
//! use std::fs::File;
//! use std::io::{BufReader, BufWriter};
//!
//! let input = BufReader::new(File::open("Heap.heapsnapshot").unwrap());
//! let output = BufWriter::new(File::create("heap.spaa").unwrap());
//!
//! let mut converter = HeapSnapshotConverter::new();
//! converter.parse(input).unwrap();
//! converter.write_spaa(output).unwrap();
//! ```

use serde::{Deserialize, Serialize};
use spaa_parse::{
    EventDef, EventKind, ExclusiveWeights, FrameKind, FrameOrder, Header, Sampling, SamplingMode,
    StackContext, StackIdMode, StackType, Weight,
};
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::io::{Read, Write};
use thiserror::Error;

/// Errors that can occur during Chrome profile conversion.
#[derive(Error, Debug)]
pub enum ConvertError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid profile: {0}")]
    InvalidProfile(String),

    #[error("no samples found in profile")]
    NoSamples,

    #[error("no CPU profile data found in trace")]
    NoCpuProfileInTrace,

    #[error("no allocation trace data in heap snapshot")]
    NoAllocationTraceData,
}

pub type Result<T> = std::result::Result<T, ConvertError>;

// ============================================================================
// Standalone cpuprofile format types
// ============================================================================

/// A Chrome DevTools CPU profile (standalone format).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CpuProfile {
    /// Call tree nodes.
    pub nodes: Vec<ProfileNode>,
    /// Profile start time in microseconds.
    pub start_time: u64,
    /// Profile end time in microseconds.
    pub end_time: u64,
    /// Array of node IDs representing the top of the stack at each sample.
    #[serde(default)]
    pub samples: Vec<u64>,
    /// Time deltas between samples in microseconds.
    #[serde(default)]
    pub time_deltas: Vec<i64>,
}

/// A node in the profile call tree (standalone format).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileNode {
    /// Unique node ID.
    pub id: u64,
    /// Call frame information for this node.
    pub call_frame: CallFrame,
    /// Number of times this node was directly sampled.
    #[serde(default)]
    pub hit_count: u64,
    /// IDs of child nodes in the call tree (standalone format).
    #[serde(default)]
    pub children: Vec<u64>,
    /// Parent node ID (trace format uses this instead of children).
    #[serde(default)]
    pub parent: Option<u64>,
    /// Position ticks (optional, from some profilers).
    #[serde(default)]
    pub position_ticks: Vec<PositionTick>,
}

/// Information about a call frame.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CallFrame {
    /// Function name.
    pub function_name: String,
    /// Script ID (internal V8 identifier) - can be string or number.
    #[serde(default, deserialize_with = "deserialize_script_id")]
    pub script_id: String,
    /// Script URL (file path or URL).
    #[serde(default)]
    pub url: String,
    /// Line number (0-based, -1 if unknown).
    #[serde(default = "default_line")]
    pub line_number: i64,
    /// Column number (0-based, -1 if unknown).
    #[serde(default = "default_line")]
    pub column_number: i64,
}

fn default_line() -> i64 {
    -1
}

/// Deserialize script_id which can be either a string or number.
fn deserialize_script_id<'de, D>(deserializer: D) -> std::result::Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let value: serde_json::Value = Deserialize::deserialize(deserializer)?;
    match value {
        serde_json::Value::String(s) => Ok(s),
        serde_json::Value::Number(n) => Ok(n.to_string()),
        serde_json::Value::Null => Ok(String::new()),
        _ => Err(D::Error::custom("expected string or number for scriptId")),
    }
}

/// Position tick information (optional).
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PositionTick {
    /// Line number.
    pub line: i64,
    /// Number of ticks at this position.
    pub ticks: u64,
}

// ============================================================================
// Chrome Performance trace format types
// ============================================================================

/// A Chrome Performance trace file.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TraceFile {
    /// Trace events array.
    pub trace_events: Vec<TraceEvent>,
    /// Optional metadata.
    #[serde(default)]
    pub metadata: Option<serde_json::Value>,
}

/// A trace event in the Performance trace.
#[derive(Debug, Clone, Deserialize)]
pub struct TraceEvent {
    /// Event name.
    pub name: String,
    /// Event category.
    #[serde(default)]
    pub cat: String,
    /// Process ID.
    #[serde(default)]
    pub pid: u64,
    /// Thread ID.
    #[serde(default)]
    pub tid: u64,
    /// Timestamp in microseconds.
    #[serde(default)]
    pub ts: u64,
    /// Event arguments.
    #[serde(default)]
    pub args: serde_json::Value,
    /// Event ID (for async events) - can be string or number.
    #[serde(default, deserialize_with = "deserialize_optional_string_or_number")]
    pub id: Option<String>,
}

/// Deserialize an optional field that can be either a string or number.
fn deserialize_optional_string_or_number<'de, D>(
    deserializer: D,
) -> std::result::Result<Option<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let value: Option<serde_json::Value> = Deserialize::deserialize(deserializer)?;
    match value {
        Some(serde_json::Value::String(s)) => Ok(Some(s)),
        Some(serde_json::Value::Number(n)) => Ok(Some(n.to_string())),
        Some(serde_json::Value::Null) | None => Ok(None),
        Some(_) => Ok(None),
    }
}

/// Data from a Profile event.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileEventData {
    /// Profile start time.
    #[serde(default)]
    pub start_time: u64,
}

/// Data from a ProfileChunk event.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileChunkData {
    /// CPU profile data in this chunk.
    #[serde(default)]
    pub cpu_profile: Option<ProfileChunkCpuProfile>,
    /// Time deltas for samples in this chunk.
    #[serde(default)]
    pub time_deltas: Vec<i64>,
}

/// CPU profile data within a ProfileChunk.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileChunkCpuProfile {
    /// Nodes added in this chunk.
    #[serde(default)]
    pub nodes: Vec<ProfileNode>,
    /// Sample node IDs in this chunk.
    #[serde(default)]
    pub samples: Vec<u64>,
}

// ============================================================================
// Converter
// ============================================================================

/// Converter from Chrome cpuprofile to SPAA format.
pub struct CpuProfileConverter {
    profile: Option<CpuProfile>,
    /// Map from node ID to parent node ID.
    parent_map: HashMap<u64, u64>,
    /// Map from node ID to node index.
    node_map: HashMap<u64, usize>,
}

impl CpuProfileConverter {
    /// Create a new converter.
    pub fn new() -> Self {
        Self {
            profile: None,
            parent_map: HashMap::new(),
            node_map: HashMap::new(),
        }
    }

    /// Parse a cpuprofile or trace file from a reader.
    ///
    /// Automatically detects whether the input is a standalone cpuprofile
    /// or a Chrome Performance trace file.
    pub fn parse<R: Read>(&mut self, reader: R) -> Result<()> {
        // Read the entire input to detect format
        let mut contents = String::new();
        let mut buf_reader = std::io::BufReader::new(reader);
        buf_reader.read_to_string(&mut contents)?;

        // Try to detect format by looking for key fields
        // Chrome trace files have "traceEvents", standalone cpuprofiles have "nodes" at top level
        let value: serde_json::Value = serde_json::from_str(&contents)?;

        if value.get("traceEvents").is_some() {
            // Chrome Performance trace format
            self.parse_trace_format(&contents)
        } else if value.get("nodes").is_some() {
            // Standalone cpuprofile format
            self.parse_standalone_format(&contents)
        } else {
            Err(ConvertError::InvalidProfile(
                "unrecognized format: expected 'nodes' or 'traceEvents' field".into(),
            ))
        }
    }

    /// Parse standalone cpuprofile format.
    fn parse_standalone_format(&mut self, contents: &str) -> Result<()> {
        let profile: CpuProfile = serde_json::from_str(contents)?;

        if profile.nodes.is_empty() {
            return Err(ConvertError::InvalidProfile("no nodes in profile".into()));
        }

        // Build node ID to index map
        for (idx, node) in profile.nodes.iter().enumerate() {
            self.node_map.insert(node.id, idx);
        }

        // Build parent map - standalone format uses children array
        for node in &profile.nodes {
            for &child_id in &node.children {
                self.parent_map.insert(child_id, node.id);
            }
        }

        self.profile = Some(profile);
        Ok(())
    }

    /// Parse Chrome Performance trace format.
    fn parse_trace_format(&mut self, contents: &str) -> Result<()> {
        let trace: TraceFile = serde_json::from_str(contents)?;

        // Collect all ProfileChunk events, grouped by profile ID
        let mut profile_start_time: Option<u64> = None;
        let mut all_nodes: Vec<ProfileNode> = Vec::new();
        let mut all_samples: Vec<u64> = Vec::new();
        let mut all_time_deltas: Vec<i64> = Vec::new();
        let mut last_ts: u64 = 0;

        for event in &trace.trace_events {
            match event.name.as_str() {
                "Profile" => {
                    // Extract start time from Profile event
                    if let Some(data) = event.args.get("data") {
                        if let Ok(profile_data) =
                            serde_json::from_value::<ProfileEventData>(data.clone())
                        {
                            profile_start_time = Some(profile_data.start_time);
                        }
                    }
                }
                "ProfileChunk" => {
                    // Extract nodes, samples, and timeDeltas from ProfileChunk
                    if let Some(data) = event.args.get("data") {
                        if let Ok(chunk_data) =
                            serde_json::from_value::<ProfileChunkData>(data.clone())
                        {
                            // Add nodes from this chunk
                            if let Some(cpu_profile) = chunk_data.cpu_profile {
                                all_nodes.extend(cpu_profile.nodes);
                                all_samples.extend(cpu_profile.samples);
                            }
                            // Add time deltas
                            all_time_deltas.extend(chunk_data.time_deltas);
                            last_ts = event.ts;
                        }
                    }
                }
                _ => {}
            }
        }

        if all_nodes.is_empty() {
            return Err(ConvertError::NoCpuProfileInTrace);
        }

        // Build parent map - trace format uses parent field directly
        for node in &all_nodes {
            if let Some(parent_id) = node.parent {
                self.parent_map.insert(node.id, parent_id);
            }
        }

        // Build node ID to index map
        for (idx, node) in all_nodes.iter().enumerate() {
            self.node_map.insert(node.id, idx);
        }

        // Calculate end time from last timestamp
        let start_time = profile_start_time.unwrap_or(0);
        let total_delta: i64 = all_time_deltas.iter().sum();
        let end_time = start_time + total_delta.unsigned_abs();

        let profile = CpuProfile {
            nodes: all_nodes,
            start_time,
            end_time: end_time.max(last_ts),
            samples: all_samples,
            time_deltas: all_time_deltas,
        };

        self.profile = Some(profile);
        Ok(())
    }

    /// Get the stack trace for a node by walking up to the root.
    /// Returns frames in leaf-to-root order.
    fn get_stack_for_node(&self, node_id: u64) -> Vec<u64> {
        let mut stack = Vec::new();
        let mut current_id = node_id;

        // Walk up to root, collecting node IDs
        loop {
            stack.push(current_id);
            match self.parent_map.get(&current_id) {
                Some(&parent_id) => current_id = parent_id,
                None => break, // Reached root
            }
        }

        stack
    }

    /// Write the parsed data as SPAA format to a writer.
    pub fn write_spaa<W: Write>(&self, mut writer: W) -> Result<()> {
        let profile = self
            .profile
            .as_ref()
            .ok_or_else(|| ConvertError::InvalidProfile("no profile parsed".into()))?;

        if profile.samples.is_empty() {
            return Err(ConvertError::NoSamples);
        }

        // Build dictionaries
        // For cpuprofile, the "DSO" is the script URL
        let mut dso_map: HashMap<&str, u64> = HashMap::new();
        let mut frame_map: HashMap<u64, u64> = HashMap::new(); // node_id -> frame_id

        // Collect unique DSOs (scripts) and frames from all nodes used in stacks
        let mut used_nodes: std::collections::HashSet<u64> = std::collections::HashSet::new();
        for &sample_node_id in &profile.samples {
            let stack = self.get_stack_for_node(sample_node_id);
            for node_id in stack {
                used_nodes.insert(node_id);
            }
        }

        // Assign DSO and frame IDs
        for &node_id in &used_nodes {
            if let Some(&node_idx) = self.node_map.get(&node_id) {
                let node = &profile.nodes[node_idx];
                let url = if node.call_frame.url.is_empty() {
                    "(program)"
                } else {
                    &node.call_frame.url
                };

                if !dso_map.contains_key(url) {
                    let id = dso_map.len() as u64 + 1;
                    dso_map.insert(url, id);
                }

                if !frame_map.contains_key(&node_id) {
                    let id = frame_map.len() as u64 + 1;
                    frame_map.insert(node_id, id);
                }
            }
        }

        // Aggregate stacks from samples
        let aggregated = self.aggregate_stacks(profile, &frame_map);

        // Write header
        let header = self.build_header(profile);
        self.write_record(&mut writer, "header", &header)?;

        // Write DSO dictionary
        for (url, dso_id) in &dso_map {
            let dso = DsoRecord {
                id: *dso_id,
                name: (*url).to_string(),
                build_id: None,
                is_kernel: false,
            };
            self.write_record(&mut writer, "dso", &dso)?;
        }

        // Write frame dictionary
        for (&node_id, &frame_id) in &frame_map {
            if let Some(&node_idx) = self.node_map.get(&node_id) {
                let node = &profile.nodes[node_idx];
                let url = if node.call_frame.url.is_empty() {
                    "(program)"
                } else {
                    &node.call_frame.url
                };
                let dso_id = dso_map[url];

                // Build source line if we have valid line numbers
                let srcline = if node.call_frame.line_number >= 0 {
                    let line = node.call_frame.line_number + 1; // Convert 0-based to 1-based
                    if node.call_frame.column_number >= 0 {
                        Some(format!(
                            "{}:{}:{}",
                            url,
                            line,
                            node.call_frame.column_number + 1
                        ))
                    } else {
                        Some(format!("{}:{}", url, line))
                    }
                } else {
                    None
                };

                let func_name = if node.call_frame.function_name.is_empty() {
                    "(anonymous)".to_string()
                } else {
                    node.call_frame.function_name.clone()
                };

                let frame = FrameRecord {
                    id: frame_id,
                    func: func_name,
                    func_resolved: true,
                    dso: dso_id,
                    ip: None,
                    symoff: None,
                    srcline,
                    inlined: false,
                    kind: FrameKind::User,
                };
                self.write_record(&mut writer, "frame", &frame)?;
            }
        }

        // Write stacks
        for (stack_key, stack_data) in &aggregated {
            let stack = StackRecord {
                id: stack_key.id.clone(),
                frames: stack_key.frame_ids.clone(),
                stack_type: StackType::User,
                context: StackContext {
                    event: "cpu-profile".to_string(),
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
                        value: stack_data.sample_count,
                        unit: None,
                    },
                    Weight {
                        metric: "time_us".to_string(),
                        value: stack_data.total_time_us,
                        unit: Some("microseconds".to_string()),
                    },
                ],
                exclusive: stack_key.frame_ids.first().map(|&leaf| ExclusiveWeights {
                    frame: leaf,
                    weights: vec![
                        Weight {
                            metric: "samples".to_string(),
                            value: stack_data.sample_count,
                            unit: None,
                        },
                        Weight {
                            metric: "time_us".to_string(),
                            value: stack_data.total_time_us,
                            unit: Some("microseconds".to_string()),
                        },
                    ],
                }),
                related_stacks: None,
            };
            self.write_record(&mut writer, "stack", &stack)?;
        }

        Ok(())
    }

    fn build_header(&self, profile: &CpuProfile) -> Header {
        let duration_us = profile.end_time.saturating_sub(profile.start_time);
        let sample_count = profile.samples.len() as u64;
        let frequency_hz = if duration_us > 0 && sample_count > 0 {
            // Estimate sampling frequency
            Some((sample_count * 1_000_000) / duration_us)
        } else {
            None
        };

        let sampling = Sampling {
            mode: SamplingMode::Frequency,
            primary_metric: "samples".to_string(),
            sample_period: None,
            frequency_hz,
        };

        let event = EventDef {
            name: "cpu-profile".to_string(),
            kind: EventKind::Timer,
            sampling,
            allocation_tracking: None,
        };

        Header {
            format: "spaa".to_string(),
            version: "1.0".to_string(),
            source_tool: "chrome-cpuprofile".to_string(),
            frame_order: FrameOrder::LeafToRoot,
            events: vec![event],
            time_range: Some(spaa_parse::TimeRange {
                start: profile.start_time as f64 / 1_000_000.0,
                end: profile.end_time as f64 / 1_000_000.0,
                unit: "seconds".to_string(),
            }),
            source: Some(spaa_parse::SourceInfo {
                tool: "chrome-devtools".to_string(),
                command: None,
                tool_version: None,
            }),
            stack_id_mode: StackIdMode::ContentAddressable,
        }
    }

    fn aggregate_stacks(
        &self,
        profile: &CpuProfile,
        frame_map: &HashMap<u64, u64>,
    ) -> HashMap<StackKey, StackData> {
        let mut aggregated: HashMap<StackKey, StackData> = HashMap::new();

        for (sample_idx, &sample_node_id) in profile.samples.iter().enumerate() {
            // Get the stack for this sample
            let node_stack = self.get_stack_for_node(sample_node_id);

            // Convert node IDs to frame IDs
            let frame_ids: Vec<u64> = node_stack
                .iter()
                .filter_map(|node_id| frame_map.get(node_id).copied())
                .collect();

            if frame_ids.is_empty() {
                continue;
            }

            // Get time delta for this sample (or estimate if not available)
            let time_us = if sample_idx < profile.time_deltas.len() {
                profile.time_deltas[sample_idx].unsigned_abs()
            } else {
                // If no time deltas, estimate based on total duration
                if !profile.samples.is_empty() {
                    let duration_us = profile.end_time.saturating_sub(profile.start_time);
                    duration_us / profile.samples.len() as u64
                } else {
                    0
                }
            };

            let stack_id = Self::compute_stack_id(&frame_ids);
            let key = StackKey {
                id: stack_id,
                frame_ids,
            };

            let data = aggregated.entry(key).or_insert(StackData {
                sample_count: 0,
                total_time_us: 0,
            });
            data.sample_count += 1;
            data.total_time_us += time_us;
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

impl Default for CpuProfileConverter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Chrome Heap Snapshot format types
// ============================================================================

/// Chrome heap snapshot file structure.
#[derive(Debug, Clone, Deserialize)]
pub struct HeapSnapshot {
    /// Snapshot metadata.
    pub snapshot: SnapshotMeta,
    /// Flat array of node fields.
    pub nodes: Vec<u64>,
    /// Flat array of edge fields.
    pub edges: Vec<u64>,
    /// Function info for allocation traces (flat array, chunked by trace_function_info_fields).
    #[serde(default)]
    pub trace_function_infos: Vec<i64>,
    /// Allocation trace tree - a single root node: [id, func_idx, count, size, children_array].
    #[serde(default)]
    pub trace_tree: serde_json::Value,
    /// String table.
    pub strings: Vec<String>,
    /// Source locations (optional).
    #[serde(default)]
    pub locations: Vec<u64>,
    /// Temporal samples for heap timeline (flat array of [timestamp_us, last_assigned_id] pairs).
    #[serde(default)]
    pub samples: Vec<u64>,
}

/// Snapshot metadata.
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotMeta {
    /// Field layout metadata.
    pub meta: SnapshotFieldMeta,
    /// Number of nodes.
    pub node_count: u64,
    /// Number of edges.
    pub edge_count: u64,
    /// Total trace function count.
    #[serde(default)]
    pub trace_function_count: u64,
}

/// Describes the field layout for nodes and edges.
#[derive(Debug, Clone, Deserialize)]
pub struct SnapshotFieldMeta {
    /// Node field names.
    pub node_fields: Vec<String>,
    /// Node type names.
    pub node_types: Vec<serde_json::Value>,
    /// Edge field names.
    pub edge_fields: Vec<String>,
    /// Edge type names.
    pub edge_types: Vec<serde_json::Value>,
    /// Trace function info field names.
    #[serde(default)]
    pub trace_function_info_fields: Vec<String>,
    /// Trace node field names.
    #[serde(default)]
    pub trace_node_fields: Vec<String>,
    /// Sample field names (for heap timeline).
    #[serde(default)]
    pub sample_fields: Vec<String>,
    /// Location field names.
    #[serde(default)]
    pub location_fields: Vec<String>,
}

// ============================================================================
// Heap Snapshot Converter
// ============================================================================

/// Parsed function info from trace_function_infos.
#[derive(Debug, Clone)]
struct FunctionInfo {
    name: String,
    script_name: String,
    line: i64,
    column: i64,
}

/// Parsed trace tree node.
#[derive(Debug, Clone)]
struct ParsedTraceNode {
    #[allow(dead_code)]
    id: u64,
    function_info_index: usize,
    count: u64,
    size: u64,
    children: Vec<usize>,
}

/// Parsed heap timeline sample.
#[derive(Debug, Clone)]
struct HeapTimelineSample {
    /// Timestamp in microseconds.
    timestamp_us: u64,
    /// Last assigned object ID at this sample point.
    #[allow(dead_code)]
    last_assigned_id: u64,
}

/// Converter for Chrome heap snapshot and heap timeline files to SPAA format.
pub struct HeapSnapshotConverter {
    snapshot: Option<HeapSnapshot>,
    function_infos: Vec<FunctionInfo>,
    trace_nodes: Vec<ParsedTraceNode>,
    /// Whether this is a heap timeline (has temporal samples).
    is_timeline: bool,
    /// Parsed timeline samples (only for heap timeline format).
    timeline_samples: Vec<HeapTimelineSample>,
}

impl HeapSnapshotConverter {
    /// Create a new heap snapshot converter.
    pub fn new() -> Self {
        Self {
            snapshot: None,
            function_infos: Vec::new(),
            trace_nodes: Vec::new(),
            is_timeline: false,
            timeline_samples: Vec::new(),
        }
    }

    /// Parse a heap snapshot or heap timeline from a reader.
    pub fn parse<R: Read>(&mut self, reader: R) -> Result<()> {
        let snapshot: HeapSnapshot = serde_json::from_reader(reader)?;

        // Detect if this is a heap timeline by checking for sample_fields and samples
        self.is_timeline =
            !snapshot.snapshot.meta.sample_fields.is_empty() && !snapshot.samples.is_empty();

        // Parse function infos
        self.function_infos = self.parse_function_infos(&snapshot)?;

        // Parse trace tree
        self.trace_nodes = self.parse_trace_tree(&snapshot)?;

        // Parse timeline samples if present
        if self.is_timeline {
            self.timeline_samples = self.parse_timeline_samples(&snapshot)?;
        }

        self.snapshot = Some(snapshot);
        Ok(())
    }

    fn parse_timeline_samples(&self, snapshot: &HeapSnapshot) -> Result<Vec<HeapTimelineSample>> {
        let mut samples = Vec::new();

        // Sample fields are typically: [timestamp_us, last_assigned_id]
        // Each sample is a pair of values in the flat array
        let fields_per_sample = snapshot.snapshot.meta.sample_fields.len();
        if fields_per_sample == 0 {
            return Ok(samples);
        }

        // Find indices for timestamp_us and last_assigned_id
        let timestamp_idx = snapshot
            .snapshot
            .meta
            .sample_fields
            .iter()
            .position(|f| f == "timestamp_us")
            .unwrap_or(0);
        let last_id_idx = snapshot
            .snapshot
            .meta
            .sample_fields
            .iter()
            .position(|f| f == "last_assigned_id")
            .unwrap_or(1);

        for chunk in snapshot.samples.chunks(fields_per_sample) {
            if chunk.len() >= fields_per_sample {
                let timestamp_us = if timestamp_idx < chunk.len() {
                    chunk[timestamp_idx]
                } else {
                    0
                };
                let last_assigned_id = if last_id_idx < chunk.len() {
                    chunk[last_id_idx]
                } else {
                    0
                };
                samples.push(HeapTimelineSample {
                    timestamp_us,
                    last_assigned_id,
                });
            }
        }

        Ok(samples)
    }

    fn parse_function_infos(&self, snapshot: &HeapSnapshot) -> Result<Vec<FunctionInfo>> {
        let mut infos = Vec::new();

        // Get field count from metadata (typically 6: function_id, name, script_name, script_id, line, column)
        let fields = &snapshot.snapshot.meta.trace_function_info_fields;
        let fields_per_info = fields.len();
        if fields_per_info == 0 {
            return Ok(infos);
        }

        // Find indices for the fields we need
        let name_field_idx = fields.iter().position(|f| f == "name").unwrap_or(1);
        let script_name_field_idx = fields.iter().position(|f| f == "script_name").unwrap_or(2);
        let line_field_idx = fields.iter().position(|f| f == "line").unwrap_or(4);
        let column_field_idx = fields.iter().position(|f| f == "column").unwrap_or(5);

        // Parse flat array in chunks
        for chunk in snapshot.trace_function_infos.chunks(fields_per_info) {
            if chunk.len() >= fields_per_info {
                let name_idx = chunk.get(name_field_idx).copied().unwrap_or(0) as usize;
                let script_name_idx =
                    chunk.get(script_name_field_idx).copied().unwrap_or(0) as usize;
                let line = chunk.get(line_field_idx).copied().unwrap_or(-1);
                let column = chunk.get(column_field_idx).copied().unwrap_or(-1);

                let name = snapshot.strings.get(name_idx).cloned().unwrap_or_default();
                let script_name = snapshot
                    .strings
                    .get(script_name_idx)
                    .cloned()
                    .unwrap_or_default();

                infos.push(FunctionInfo {
                    name,
                    script_name,
                    line,
                    column,
                });
            }
        }

        Ok(infos)
    }

    fn parse_trace_tree(&self, snapshot: &HeapSnapshot) -> Result<Vec<ParsedTraceNode>> {
        let mut nodes = Vec::new();

        // trace_tree is a single root node: [id, func_info_idx, count, size, children_array]
        // The children_array contains children as flat groups of 5 values:
        //   [child1_id, child1_func, child1_count, child1_size, child1_children, child2_id, ...]
        if snapshot.trace_tree.is_array() {
            self.parse_trace_node_recursive(&snapshot.trace_tree, &mut nodes);
        }

        Ok(nodes)
    }

    fn parse_trace_node_recursive(
        &self,
        node: &serde_json::Value,
        nodes: &mut Vec<ParsedTraceNode>,
    ) {
        let arr = match node.as_array() {
            Some(a) => a,
            None => return,
        };

        if arr.len() < 5 {
            return;
        }

        let id = arr[0].as_u64().unwrap_or(0);
        let function_info_index = arr[1].as_u64().unwrap_or(0) as usize;
        let count = arr[2].as_u64().unwrap_or(0);
        let size = arr[3].as_u64().unwrap_or(0);

        // Current node index in our flat list
        let current_idx = nodes.len();

        // Add placeholder node (we'll fill in children after)
        nodes.push(ParsedTraceNode {
            id,
            function_info_index,
            count,
            size,
            children: Vec::new(),
        });

        // Element 4 is the children array, containing flat groups of 5 values
        let mut child_indices = Vec::new();
        if let Some(children_arr) = arr[4].as_array() {
            let mut i = 0;
            while i + 4 < children_arr.len() {
                // Each child is 5 consecutive values: id, func_idx, count, size, grandchildren_array
                let child_id = children_arr[i].as_u64().unwrap_or(0);
                let child_func_idx = children_arr[i + 1].as_u64().unwrap_or(0) as usize;
                let child_count = children_arr[i + 2].as_u64().unwrap_or(0);
                let child_size = children_arr[i + 3].as_u64().unwrap_or(0);
                let grandchildren = &children_arr[i + 4];

                let child_idx = nodes.len();
                child_indices.push(child_idx);

                // Add child node
                nodes.push(ParsedTraceNode {
                    id: child_id,
                    function_info_index: child_func_idx,
                    count: child_count,
                    size: child_size,
                    children: Vec::new(),
                });

                // Recursively parse grandchildren
                if let Some(gc_arr) = grandchildren.as_array() {
                    if !gc_arr.is_empty() {
                        let gc_indices = self.parse_children_array(gc_arr, nodes);
                        nodes[child_idx].children = gc_indices;
                    }
                }

                i += 5;
            }
        }

        // Update the node with its children
        nodes[current_idx].children = child_indices;
    }

    fn parse_children_array(
        &self,
        children_arr: &[serde_json::Value],
        nodes: &mut Vec<ParsedTraceNode>,
    ) -> Vec<usize> {
        let mut child_indices = Vec::new();
        let mut i = 0;

        while i + 4 < children_arr.len() {
            let child_id = children_arr[i].as_u64().unwrap_or(0);
            let child_func_idx = children_arr[i + 1].as_u64().unwrap_or(0) as usize;
            let child_count = children_arr[i + 2].as_u64().unwrap_or(0);
            let child_size = children_arr[i + 3].as_u64().unwrap_or(0);
            let grandchildren = &children_arr[i + 4];

            let child_idx = nodes.len();
            child_indices.push(child_idx);

            nodes.push(ParsedTraceNode {
                id: child_id,
                function_info_index: child_func_idx,
                count: child_count,
                size: child_size,
                children: Vec::new(),
            });

            // Recursively parse grandchildren
            if let Some(gc_arr) = grandchildren.as_array() {
                if !gc_arr.is_empty() {
                    let gc_indices = self.parse_children_array(gc_arr, nodes);
                    nodes[child_idx].children = gc_indices;
                }
            }

            i += 5;
        }

        child_indices
    }

    /// Write the parsed heap snapshot as SPAA format.
    pub fn write_spaa<W: Write>(&self, mut writer: W) -> Result<()> {
        let _snapshot = self
            .snapshot
            .as_ref()
            .ok_or_else(|| ConvertError::InvalidProfile("no snapshot parsed".into()))?;

        if self.trace_nodes.is_empty() {
            return Err(ConvertError::NoAllocationTraceData);
        }

        // Build stacks by walking the trace tree
        let mut stacks: Vec<(Vec<usize>, u64, u64)> = Vec::new(); // (function_info_indices, count, size)
        self.collect_stacks(0, &mut Vec::new(), &mut stacks);

        if stacks.is_empty() {
            return Err(ConvertError::NoAllocationTraceData);
        }

        // Build DSO map (script_name -> dso_id)
        let mut dso_map: HashMap<&str, u64> = HashMap::new();
        // Collect all unique DSOs and frames
        for (stack, _, _) in &stacks {
            for &func_idx in stack {
                if func_idx < self.function_infos.len() {
                    let func = &self.function_infos[func_idx];
                    let script = if func.script_name.is_empty() {
                        "(program)"
                    } else {
                        &func.script_name
                    };

                    if !dso_map.contains_key(script) {
                        let id = dso_map.len() as u64 + 1;
                        dso_map.insert(script, id);
                    }
                }
            }
        }

        // Write header
        let header = self.build_header();
        self.write_record(&mut writer, "header", &header)?;

        // Write DSOs
        for (script, dso_id) in &dso_map {
            let dso = DsoRecord {
                id: *dso_id,
                name: (*script).to_string(),
                build_id: None,
                is_kernel: false,
            };
            self.write_record(&mut writer, "dso", &dso)?;
        }

        // Assign frame IDs and write frames
        let mut frame_id_counter: u64 = 1;
        let mut func_to_frame: HashMap<usize, u64> = HashMap::new();

        for (stack, _, _) in &stacks {
            for &func_idx in stack {
                if !func_to_frame.contains_key(&func_idx) && func_idx < self.function_infos.len() {
                    let func = &self.function_infos[func_idx];
                    let script = if func.script_name.is_empty() {
                        "(program)"
                    } else {
                        &func.script_name
                    };
                    let dso_id = dso_map[script];

                    let srcline = if func.line >= 0 {
                        if func.column >= 0 {
                            Some(format!("{}:{}:{}", script, func.line + 1, func.column + 1))
                        } else {
                            Some(format!("{}:{}", script, func.line + 1))
                        }
                    } else {
                        None
                    };

                    let func_name = if func.name.is_empty() {
                        "(anonymous)".to_string()
                    } else {
                        func.name.clone()
                    };

                    let frame = FrameRecord {
                        id: frame_id_counter,
                        func: func_name,
                        func_resolved: true,
                        dso: dso_id,
                        ip: None,
                        symoff: None,
                        srcline,
                        inlined: false,
                        kind: FrameKind::User,
                    };
                    self.write_record(&mut writer, "frame", &frame)?;

                    func_to_frame.insert(func_idx, frame_id_counter);
                    frame_id_counter += 1;
                }
            }
        }

        // Write stacks
        for (stack, count, size) in &stacks {
            if *count == 0 && *size == 0 {
                continue; // Skip empty stacks
            }

            // Convert function indices to frame IDs (leaf to root order)
            let frame_ids: Vec<u64> = stack
                .iter()
                .rev() // Reverse to get leaf-to-root
                .filter_map(|&idx| func_to_frame.get(&idx).copied())
                .collect();

            if frame_ids.is_empty() {
                continue;
            }

            let stack_id = Self::compute_stack_id(&frame_ids);

            let stack_record = StackRecord {
                id: stack_id,
                frames: frame_ids.clone(),
                stack_type: StackType::User,
                context: StackContext {
                    event: "allocation".to_string(),
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
                        metric: "alloc_bytes".to_string(),
                        value: *size,
                        unit: Some("bytes".to_string()),
                    },
                    Weight {
                        metric: "alloc_count".to_string(),
                        value: *count,
                        unit: None,
                    },
                ],
                exclusive: frame_ids.first().map(|&leaf| ExclusiveWeights {
                    frame: leaf,
                    weights: vec![
                        Weight {
                            metric: "alloc_bytes".to_string(),
                            value: *size,
                            unit: Some("bytes".to_string()),
                        },
                        Weight {
                            metric: "alloc_count".to_string(),
                            value: *count,
                            unit: None,
                        },
                    ],
                }),
                related_stacks: None,
            };
            self.write_record(&mut writer, "stack", &stack_record)?;
        }

        Ok(())
    }

    /// Recursively collect stacks from the trace tree.
    fn collect_stacks(
        &self,
        node_idx: usize,
        current_stack: &mut Vec<usize>,
        stacks: &mut Vec<(Vec<usize>, u64, u64)>,
    ) {
        if node_idx >= self.trace_nodes.len() {
            return;
        }

        let node = &self.trace_nodes[node_idx];

        // Add this function to the stack (skip root nodes with no function)
        if node.function_info_index < self.function_infos.len() {
            current_stack.push(node.function_info_index);
        }

        // If this node has allocations, record the stack
        if node.count > 0 || node.size > 0 {
            stacks.push((current_stack.clone(), node.count, node.size));
        }

        // Recurse into children
        for &child_idx in &node.children {
            self.collect_stacks(child_idx, current_stack, stacks);
        }

        // Pop this function from the stack
        if node.function_info_index < self.function_infos.len() {
            current_stack.pop();
        }
    }

    fn build_header(&self) -> Header {
        let sampling = Sampling {
            mode: SamplingMode::Event,
            primary_metric: "alloc_bytes".to_string(),
            sample_period: None,
            frequency_hz: None,
        };

        let event = EventDef {
            name: "allocation".to_string(),
            kind: EventKind::Allocation,
            sampling,
            allocation_tracking: Some(spaa_parse::AllocationTracking {
                tracks_frees: false,
                has_timestamps: self.is_timeline,
            }),
        };

        // Compute time range from timeline samples if available
        let time_range = if self.is_timeline && !self.timeline_samples.is_empty() {
            let start_us = self
                .timeline_samples
                .iter()
                .map(|s| s.timestamp_us)
                .min()
                .unwrap_or(0);
            let end_us = self
                .timeline_samples
                .iter()
                .map(|s| s.timestamp_us)
                .max()
                .unwrap_or(0);
            Some(spaa_parse::TimeRange {
                start: start_us as f64 / 1_000_000.0,
                end: end_us as f64 / 1_000_000.0,
                unit: "seconds".to_string(),
            })
        } else {
            None
        };

        let source_tool = if self.is_timeline {
            "chrome-heaptimeline"
        } else {
            "chrome-heapsnapshot"
        };

        Header {
            format: "spaa".to_string(),
            version: "1.0".to_string(),
            source_tool: source_tool.to_string(),
            frame_order: FrameOrder::LeafToRoot,
            events: vec![event],
            time_range,
            source: Some(spaa_parse::SourceInfo {
                tool: "chrome-devtools".to_string(),
                command: None,
                tool_version: None,
            }),
            stack_id_mode: StackIdMode::ContentAddressable,
        }
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

impl Default for HeapSnapshotConverter {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Unified converter for auto-detection
// ============================================================================

/// The type of Chrome profile detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileType {
    /// Chrome Performance trace with CPU profile data.
    PerformanceTrace,
    /// Standalone V8 cpuprofile.
    CpuProfile,
    /// Chrome heap snapshot.
    HeapSnapshot,
    /// Chrome heap timeline (heap snapshot with temporal samples).
    HeapTimeline,
}

/// Detect the type of Chrome profile from JSON content.
pub fn detect_profile_type(contents: &str) -> Result<ProfileType> {
    let value: serde_json::Value = serde_json::from_str(contents)?;

    if value.get("snapshot").is_some() && value.get("nodes").is_some() {
        // Both heap snapshot and heap timeline have "snapshot" and "nodes".
        // Heap timeline has a non-empty "samples" array with timestamp data.
        if let Some(samples) = value.get("samples") {
            if let Some(arr) = samples.as_array() {
                if !arr.is_empty() {
                    // Check if snapshot.meta has sample_fields (heap timeline indicator)
                    if let Some(snapshot) = value.get("snapshot") {
                        if let Some(meta) = snapshot.get("meta") {
                            if meta.get("sample_fields").is_some() {
                                return Ok(ProfileType::HeapTimeline);
                            }
                        }
                    }
                }
            }
        }
        Ok(ProfileType::HeapSnapshot)
    } else if value.get("traceEvents").is_some() {
        Ok(ProfileType::PerformanceTrace)
    } else if value.get("nodes").is_some() {
        // Standalone cpuprofile has "nodes" but not "snapshot"
        Ok(ProfileType::CpuProfile)
    } else {
        Err(ConvertError::InvalidProfile(
            "unrecognized format: expected Chrome profile data".into(),
        ))
    }
}

// ============================================================================
// Serialization records
// ============================================================================

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
    sample_count: u64,
    total_time_us: u64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn sample_cpuprofile() -> &'static str {
        r#"{
            "nodes": [
                {
                    "id": 1,
                    "callFrame": {
                        "functionName": "(root)",
                        "scriptId": "0",
                        "url": "",
                        "lineNumber": -1,
                        "columnNumber": -1
                    },
                    "hitCount": 0,
                    "children": [2]
                },
                {
                    "id": 2,
                    "callFrame": {
                        "functionName": "(program)",
                        "scriptId": "0",
                        "url": "",
                        "lineNumber": -1,
                        "columnNumber": -1
                    },
                    "hitCount": 0,
                    "children": [3]
                },
                {
                    "id": 3,
                    "callFrame": {
                        "functionName": "main",
                        "scriptId": "1",
                        "url": "app.js",
                        "lineNumber": 10,
                        "columnNumber": 0
                    },
                    "hitCount": 2,
                    "children": [4, 5]
                },
                {
                    "id": 4,
                    "callFrame": {
                        "functionName": "processData",
                        "scriptId": "1",
                        "url": "app.js",
                        "lineNumber": 25,
                        "columnNumber": 4
                    },
                    "hitCount": 5,
                    "children": []
                },
                {
                    "id": 5,
                    "callFrame": {
                        "functionName": "computeHash",
                        "scriptId": "2",
                        "url": "utils.js",
                        "lineNumber": 5,
                        "columnNumber": 0
                    },
                    "hitCount": 3,
                    "children": []
                }
            ],
            "startTime": 1000000,
            "endTime": 2000000,
            "samples": [3, 4, 4, 4, 5, 5, 4, 4, 3, 5],
            "timeDeltas": [100000, 100000, 100000, 100000, 100000, 100000, 100000, 100000, 100000, 100000]
        }"#
    }

    fn sample_trace_format() -> &'static str {
        r#"{
            "traceEvents": [
                {
                    "name": "Profile",
                    "cat": "disabled-by-default-v8.cpu_profiler",
                    "ph": "P",
                    "pid": 1,
                    "tid": 1,
                    "ts": 1000000,
                    "args": {
                        "data": {
                            "startTime": 1000000
                        }
                    }
                },
                {
                    "name": "ProfileChunk",
                    "cat": "disabled-by-default-v8.cpu_profiler",
                    "ph": "P",
                    "pid": 1,
                    "tid": 1,
                    "ts": 1100000,
                    "args": {
                        "data": {
                            "cpuProfile": {
                                "nodes": [
                                    {
                                        "id": 1,
                                        "callFrame": {
                                            "functionName": "(root)",
                                            "scriptId": 0,
                                            "url": ""
                                        }
                                    },
                                    {
                                        "id": 2,
                                        "callFrame": {
                                            "functionName": "main",
                                            "scriptId": 1,
                                            "url": "app.js",
                                            "lineNumber": 10
                                        },
                                        "parent": 1
                                    }
                                ],
                                "samples": [2, 2, 2]
                            },
                            "timeDeltas": [10000, 10000, 10000]
                        }
                    }
                },
                {
                    "name": "ProfileChunk",
                    "cat": "disabled-by-default-v8.cpu_profiler",
                    "ph": "P",
                    "pid": 1,
                    "tid": 1,
                    "ts": 1200000,
                    "args": {
                        "data": {
                            "cpuProfile": {
                                "nodes": [
                                    {
                                        "id": 3,
                                        "callFrame": {
                                            "functionName": "helper",
                                            "scriptId": 1,
                                            "url": "app.js",
                                            "lineNumber": 20
                                        },
                                        "parent": 2
                                    }
                                ],
                                "samples": [3, 3]
                            },
                            "timeDeltas": [10000, 10000]
                        }
                    }
                }
            ]
        }"#
    }

    #[test]
    fn parse_cpuprofile() {
        let cursor = Cursor::new(sample_cpuprofile());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let profile = converter.profile.as_ref().unwrap();
        assert_eq!(profile.nodes.len(), 5);
        assert_eq!(profile.samples.len(), 10);
        assert_eq!(profile.time_deltas.len(), 10);
    }

    #[test]
    fn parse_trace_format() {
        let cursor = Cursor::new(sample_trace_format());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let profile = converter.profile.as_ref().unwrap();
        // Should have merged nodes from both chunks
        assert_eq!(profile.nodes.len(), 3);
        // Should have merged samples from both chunks
        assert_eq!(profile.samples.len(), 5);
        assert_eq!(profile.time_deltas.len(), 5);
    }

    #[test]
    fn trace_format_builds_parent_map() {
        let cursor = Cursor::new(sample_trace_format());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        // Node 2's parent should be node 1
        assert_eq!(converter.parent_map.get(&2), Some(&1));
        // Node 3's parent should be node 2
        assert_eq!(converter.parent_map.get(&3), Some(&2));
        // Node 1 (root) has no parent
        assert_eq!(converter.parent_map.get(&1), None);
    }

    #[test]
    fn trace_format_converts_to_spaa() {
        let cursor = Cursor::new(sample_trace_format());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        assert_eq!(spaa.header.source_tool, "chrome-cpuprofile");
        assert!(!spaa.dsos.is_empty());
        assert!(!spaa.frames.is_empty());
        assert!(!spaa.stacks.is_empty());
    }

    #[test]
    fn build_parent_map() {
        let cursor = Cursor::new(sample_cpuprofile());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        // Node 2's parent should be node 1
        assert_eq!(converter.parent_map.get(&2), Some(&1));
        // Node 3's parent should be node 2
        assert_eq!(converter.parent_map.get(&3), Some(&2));
        // Node 4's parent should be node 3
        assert_eq!(converter.parent_map.get(&4), Some(&3));
        // Node 5's parent should be node 3
        assert_eq!(converter.parent_map.get(&5), Some(&3));
        // Node 1 (root) has no parent
        assert_eq!(converter.parent_map.get(&1), None);
    }

    #[test]
    fn get_stack_for_leaf_node() {
        let cursor = Cursor::new(sample_cpuprofile());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        // Stack for node 4 should be: 4 -> 3 -> 2 -> 1 (leaf to root)
        let stack = converter.get_stack_for_node(4);
        assert_eq!(stack, vec![4, 3, 2, 1]);

        // Stack for node 5 should be: 5 -> 3 -> 2 -> 1
        let stack = converter.get_stack_for_node(5);
        assert_eq!(stack, vec![5, 3, 2, 1]);
    }

    #[test]
    fn convert_to_spaa() {
        let cursor = Cursor::new(sample_cpuprofile());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let output_str = String::from_utf8(output).unwrap();
        let lines: Vec<&str> = output_str.lines().collect();

        assert!(!lines.is_empty());
        assert!(lines[0].contains("\"type\":\"header\""));
        assert!(lines[0].contains("\"source_tool\":\"chrome-cpuprofile\""));
    }

    #[test]
    fn spaa_output_validates() {
        let cursor = Cursor::new(sample_cpuprofile());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        // Parse with spaa_parse to validate
        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        assert_eq!(spaa.header.source_tool, "chrome-cpuprofile");
        assert_eq!(spaa.header.frame_order, FrameOrder::LeafToRoot);
        assert!(!spaa.dsos.is_empty());
        assert!(!spaa.frames.is_empty());
        assert!(!spaa.stacks.is_empty());
    }

    #[test]
    fn empty_samples_returns_error() {
        let profile = r#"{
            "nodes": [{"id": 1, "callFrame": {"functionName": "root"}}],
            "startTime": 0,
            "endTime": 1000000,
            "samples": [],
            "timeDeltas": []
        }"#;

        let cursor = Cursor::new(profile);
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        let result = converter.write_spaa(&mut output);

        assert!(matches!(result, Err(ConvertError::NoSamples)));
    }

    #[test]
    fn handles_anonymous_functions() {
        let profile = r#"{
            "nodes": [
                {
                    "id": 1,
                    "callFrame": {"functionName": "(root)", "url": ""},
                    "children": [2]
                },
                {
                    "id": 2,
                    "callFrame": {"functionName": "", "url": "app.js", "lineNumber": 5},
                    "children": []
                }
            ],
            "startTime": 0,
            "endTime": 1000000,
            "samples": [2],
            "timeDeltas": [1000000]
        }"#;

        let cursor = Cursor::new(profile);
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // Should have an "(anonymous)" function
        let has_anonymous = spaa.frames.values().any(|f| f.func == "(anonymous)");
        assert!(has_anonymous);
    }

    #[test]
    fn handles_missing_url() {
        let profile = r#"{
            "nodes": [
                {
                    "id": 1,
                    "callFrame": {"functionName": "(root)", "url": ""},
                    "children": [2]
                },
                {
                    "id": 2,
                    "callFrame": {"functionName": "native", "url": ""},
                    "children": []
                }
            ],
            "startTime": 0,
            "endTime": 1000000,
            "samples": [2],
            "timeDeltas": [1000000]
        }"#;

        let cursor = Cursor::new(profile);
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // DSO should be "(program)" for empty URLs
        let has_program_dso = spaa.dsos.values().any(|d| d.name == "(program)");
        assert!(has_program_dso);
    }

    #[test]
    fn stacks_are_aggregated() {
        let cursor = Cursor::new(sample_cpuprofile());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // We have samples: [3, 4, 4, 4, 5, 5, 4, 4, 3, 5]
        // Node 3: 2 samples (indices 0, 8)
        // Node 4: 5 samples (indices 1, 2, 3, 6, 7)
        // Node 5: 3 samples (indices 4, 5, 9)
        // Unique stacks: node 3, node 4, node 5 = 3 unique stacks
        assert_eq!(spaa.stacks.len(), 3);

        // Collect sample counts
        let mut sample_counts: Vec<u64> = spaa
            .stacks
            .values()
            .filter_map(|s| s.weights.iter().find(|w| w.metric == "samples"))
            .map(|w| w.value)
            .collect();
        sample_counts.sort();

        // Should have stacks with 2, 3, and 5 samples
        assert_eq!(sample_counts, vec![2, 3, 5]);
    }

    #[test]
    fn time_range_is_set() {
        let cursor = Cursor::new(sample_cpuprofile());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        let time_range = spaa.header.time_range.unwrap();
        assert_eq!(time_range.start, 1.0); // 1000000 us = 1 second
        assert_eq!(time_range.end, 2.0); // 2000000 us = 2 seconds
        assert_eq!(time_range.unit, "seconds");
    }

    #[test]
    fn srcline_includes_line_numbers() {
        let cursor = Cursor::new(sample_cpuprofile());
        let mut converter = CpuProfileConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // Find the main function frame
        let main_frame = spaa.frames.values().find(|f| f.func == "main");
        assert!(main_frame.is_some());
        let main_frame = main_frame.unwrap();

        // srcline should be "app.js:11:1" (line 10 is 0-based, so 11 is 1-based)
        assert_eq!(main_frame.srcline, Some("app.js:11:1".to_string()));
    }

    // ========================================================================
    // Heap Snapshot tests
    // ========================================================================

    fn sample_heap_snapshot() -> &'static str {
        r#"{
            "snapshot": {
                "meta": {
                    "node_fields": ["type", "name", "id", "self_size", "edge_count", "trace_node_id"],
                    "node_types": [["hidden", "array", "string", "object"], "string", "number", "number", "number", "number"],
                    "edge_fields": ["type", "name_or_index", "to_node"],
                    "edge_types": [["context", "element", "property"], "string_or_number", "node"],
                    "trace_function_info_fields": ["function_id", "name", "script_name", "script_id", "line", "column"],
                    "trace_node_fields": ["id", "function_info_index", "count", "size", "children"]
                },
                "node_count": 2,
                "edge_count": 1,
                "trace_function_count": 3
            },
            "nodes": [0, 0, 1, 100, 1, 0, 3, 1, 2, 200, 0, 1],
            "edges": [0, 0, 6],
            "trace_function_infos": [0, 0, 0, 0, -1, -1, 1, 1, 2, 1, 10, 5, 2, 3, 4, 2, 25, 10],
            "trace_tree": [0, 0, 0, 0, [1, 1, 5, 1000, [2, 2, 10, 5000, []]]],
            "strings": ["(root)", "allocateBuffer", "app.js", "processData", "utils.js"],
            "locations": []
        }"#
    }

    #[test]
    fn detect_heap_snapshot_format() {
        let profile_type = detect_profile_type(sample_heap_snapshot()).unwrap();
        assert_eq!(profile_type, ProfileType::HeapSnapshot);
    }

    #[test]
    fn detect_cpuprofile_format() {
        let profile_type = detect_profile_type(sample_cpuprofile()).unwrap();
        assert_eq!(profile_type, ProfileType::CpuProfile);
    }

    #[test]
    fn detect_trace_format() {
        let profile_type = detect_profile_type(sample_trace_format()).unwrap();
        assert_eq!(profile_type, ProfileType::PerformanceTrace);
    }

    #[test]
    fn parse_heap_snapshot() {
        let cursor = Cursor::new(sample_heap_snapshot());
        let mut converter = HeapSnapshotConverter::new();
        converter.parse(cursor).unwrap();

        // Should have parsed 3 function infos
        assert_eq!(converter.function_infos.len(), 3);
        assert_eq!(converter.function_infos[1].name, "allocateBuffer");
        assert_eq!(converter.function_infos[1].script_name, "app.js");
        assert_eq!(converter.function_infos[1].line, 10);

        // Should have parsed 3 trace nodes
        assert_eq!(converter.trace_nodes.len(), 3);
    }

    #[test]
    fn heap_snapshot_converts_to_spaa() {
        let cursor = Cursor::new(sample_heap_snapshot());
        let mut converter = HeapSnapshotConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        assert_eq!(spaa.header.source_tool, "chrome-heapsnapshot");
        assert_eq!(spaa.header.events[0].name, "allocation");
        assert_eq!(spaa.header.events[0].kind, EventKind::Allocation);
        assert!(!spaa.dsos.is_empty());
        assert!(!spaa.frames.is_empty());
        assert!(!spaa.stacks.is_empty());
    }

    #[test]
    fn heap_snapshot_has_allocation_metrics() {
        let cursor = Cursor::new(sample_heap_snapshot());
        let mut converter = HeapSnapshotConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // Check that stacks have alloc_bytes and alloc_count metrics
        for stack in spaa.stacks.values() {
            let has_alloc_bytes = stack.weights.iter().any(|w| w.metric == "alloc_bytes");
            let has_alloc_count = stack.weights.iter().any(|w| w.metric == "alloc_count");
            assert!(has_alloc_bytes, "Stack should have alloc_bytes metric");
            assert!(has_alloc_count, "Stack should have alloc_count metric");
        }
    }

    #[test]
    fn heap_snapshot_stacks_have_correct_values() {
        let cursor = Cursor::new(sample_heap_snapshot());
        let mut converter = HeapSnapshotConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // Collect all alloc_bytes values
        let mut alloc_bytes: Vec<u64> = spaa
            .stacks
            .values()
            .filter_map(|s| s.weights.iter().find(|w| w.metric == "alloc_bytes"))
            .map(|w| w.value)
            .collect();
        alloc_bytes.sort();

        // From trace_tree: node 1 has 1000 bytes, node 2 has 5000 bytes
        assert!(alloc_bytes.contains(&1000));
        assert!(alloc_bytes.contains(&5000));
    }

    // ========================================================================
    // Heap Timeline tests
    // ========================================================================

    fn sample_heap_timeline() -> &'static str {
        r#"{
            "snapshot": {
                "meta": {
                    "node_fields": ["type", "name", "id", "self_size", "edge_count", "trace_node_id", "detachedness"],
                    "node_types": [["hidden", "array", "string", "object", "code", "closure"], "string", "number", "number", "number", "number", "number"],
                    "edge_fields": ["type", "name_or_index", "to_node"],
                    "edge_types": [["context", "element", "property", "internal", "hidden"], "string_or_number", "node"],
                    "trace_function_info_fields": ["function_id", "name", "script_name", "script_id", "line", "column"],
                    "trace_node_fields": ["id", "function_info_index", "count", "size", "children"],
                    "sample_fields": ["timestamp_us", "last_assigned_id"],
                    "location_fields": ["object_index", "script_id", "line", "column"]
                },
                "node_count": 3,
                "edge_count": 2,
                "trace_function_count": 3
            },
            "nodes": [0, 0, 1, 100, 1, 0, 0, 3, 1, 2, 200, 0, 1, 0, 3, 2, 3, 300, 1, 2, 0],
            "edges": [0, 0, 7, 0, 1, 14],
            "trace_function_infos": [0, 0, 0, 0, -1, -1, 1, 1, 2, 1, 10, 5, 2, 3, 4, 2, 25, 10],
            "trace_tree": [0, 0, 0, 0, [1, 1, 5, 1000, [2, 2, 10, 5000, []]]],
            "samples": [1000000, 100, 2000000, 150, 3000000, 200, 4000000, 250],
            "locations": [],
            "strings": ["(root)", "allocateBuffer", "app.js", "processData", "utils.js"]
        }"#
    }

    #[test]
    fn detect_heap_timeline_format() {
        let profile_type = detect_profile_type(sample_heap_timeline()).unwrap();
        assert_eq!(profile_type, ProfileType::HeapTimeline);
    }

    #[test]
    fn parse_heap_timeline() {
        let cursor = Cursor::new(sample_heap_timeline());
        let mut converter = HeapSnapshotConverter::new();
        converter.parse(cursor).unwrap();

        // Should be detected as timeline
        assert!(converter.is_timeline);

        // Should have parsed 3 function infos
        assert_eq!(converter.function_infos.len(), 3);

        // Should have parsed 3 trace nodes
        assert_eq!(converter.trace_nodes.len(), 3);

        // Should have parsed 4 timeline samples
        assert_eq!(converter.timeline_samples.len(), 4);
        assert_eq!(converter.timeline_samples[0].timestamp_us, 1000000);
        assert_eq!(converter.timeline_samples[3].timestamp_us, 4000000);
    }

    #[test]
    fn heap_timeline_converts_to_spaa() {
        let cursor = Cursor::new(sample_heap_timeline());
        let mut converter = HeapSnapshotConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        assert_eq!(spaa.header.source_tool, "chrome-heaptimeline");
        assert_eq!(spaa.header.events[0].name, "allocation");
        assert_eq!(spaa.header.events[0].kind, EventKind::Allocation);
        assert!(!spaa.dsos.is_empty());
        assert!(!spaa.frames.is_empty());
        assert!(!spaa.stacks.is_empty());
    }

    #[test]
    fn heap_timeline_has_time_range() {
        let cursor = Cursor::new(sample_heap_timeline());
        let mut converter = HeapSnapshotConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // Timeline should have time_range
        let time_range = spaa
            .header
            .time_range
            .expect("timeline should have time_range");
        assert_eq!(time_range.start, 1.0); // 1000000 us = 1 second
        assert_eq!(time_range.end, 4.0); // 4000000 us = 4 seconds
        assert_eq!(time_range.unit, "seconds");
    }

    #[test]
    fn heap_timeline_has_timestamps_flag() {
        let cursor = Cursor::new(sample_heap_timeline());
        let mut converter = HeapSnapshotConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // allocation_tracking.has_timestamps should be true for timeline
        let allocation_tracking = spaa.header.events[0]
            .allocation_tracking
            .as_ref()
            .expect("should have allocation_tracking");
        assert!(allocation_tracking.has_timestamps);
    }

    #[test]
    fn heap_snapshot_does_not_have_timestamps_flag() {
        let cursor = Cursor::new(sample_heap_snapshot());
        let mut converter = HeapSnapshotConverter::new();
        converter.parse(cursor).unwrap();

        let mut output = Vec::new();
        converter.write_spaa(&mut output).unwrap();

        let spaa = spaa_parse::SpaaFile::parse(Cursor::new(output)).unwrap();

        // allocation_tracking.has_timestamps should be false for snapshot
        let allocation_tracking = spaa.header.events[0]
            .allocation_tracking
            .as_ref()
            .expect("should have allocation_tracking");
        assert!(!allocation_tracking.has_timestamps);
    }
}
