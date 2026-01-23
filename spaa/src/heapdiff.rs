//! Heap snapshot diff tool for memory leak analysis.
//!
//! This module compares two Chrome heap snapshots and produces an
//! agent-friendly diff showing what objects grew and their retention paths.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{Read, Write};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum HeapDiffError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid snapshot: {0}")]
    InvalidSnapshot(String),
}

pub type Result<T> = std::result::Result<T, HeapDiffError>;

// ============================================================================
// Heap snapshot parsing types
// ============================================================================

/// Raw heap snapshot file structure.
#[derive(Debug, Deserialize)]
pub struct RawHeapSnapshot {
    pub snapshot: SnapshotMeta,
    pub nodes: Vec<i64>,
    pub edges: Vec<i64>,
    pub strings: Vec<String>,
    #[serde(default)]
    pub trace_function_infos: Vec<i64>,
    #[serde(default)]
    pub trace_tree: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct SnapshotMeta {
    pub meta: FieldMeta,
    pub node_count: u64,
    pub edge_count: u64,
}

#[derive(Debug, Deserialize)]
pub struct FieldMeta {
    pub node_fields: Vec<String>,
    pub node_types: Vec<serde_json::Value>,
    pub edge_fields: Vec<String>,
    pub edge_types: Vec<serde_json::Value>,
    #[serde(default)]
    pub trace_function_info_fields: Vec<String>,
}

// ============================================================================
// Parsed snapshot representation
// ============================================================================

/// A parsed heap node.
#[derive(Debug, Clone)]
pub struct HeapNode {
    pub node_type: String,
    pub name: String,
    pub id: u64,
    pub self_size: u64,
    pub edge_count: usize,
    pub edges_start: usize,
}

/// A parsed heap edge.
#[derive(Debug, Clone)]
pub struct HeapEdge {
    pub edge_type: String,
    pub name_or_index: String,
    pub to_node_idx: usize,
}

/// Processed heap snapshot ready for analysis.
pub struct ParsedSnapshot {
    pub nodes: Vec<HeapNode>,
    pub edges: Vec<HeapEdge>,
    /// Map from node ID to node index for cross-snapshot comparison.
    pub id_to_idx: HashMap<u64, usize>,
    /// Node type strings (e.g., "hidden", "array", "string", "object", etc.)
    pub node_type_names: Vec<String>,
    /// Edge type strings (e.g., "context", "element", "property", etc.)
    pub edge_type_names: Vec<String>,
}

impl ParsedSnapshot {
    pub fn parse<R: Read>(reader: R) -> Result<Self> {
        let raw: RawHeapSnapshot = serde_json::from_reader(reader)?;
        Self::from_raw(raw)
    }

    fn from_raw(raw: RawHeapSnapshot) -> Result<Self> {
        let meta = &raw.snapshot.meta;

        // Extract node type names from first element of node_types
        let node_type_names: Vec<String> = meta
            .node_types
            .first()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Extract edge type names from first element of edge_types
        let edge_type_names: Vec<String> = meta
            .edge_types
            .first()
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Find field indices for nodes
        let node_field_count = meta.node_fields.len();
        let type_idx = meta
            .node_fields
            .iter()
            .position(|f| f == "type")
            .unwrap_or(0);
        let name_idx = meta
            .node_fields
            .iter()
            .position(|f| f == "name")
            .unwrap_or(1);
        let id_idx = meta.node_fields.iter().position(|f| f == "id").unwrap_or(2);
        let size_idx = meta
            .node_fields
            .iter()
            .position(|f| f == "self_size")
            .unwrap_or(3);
        let edge_count_idx = meta
            .node_fields
            .iter()
            .position(|f| f == "edge_count")
            .unwrap_or(4);

        // Find field indices for edges
        let edge_field_count = meta.edge_fields.len();
        let edge_type_idx = meta
            .edge_fields
            .iter()
            .position(|f| f == "type")
            .unwrap_or(0);
        let edge_name_idx = meta
            .edge_fields
            .iter()
            .position(|f| f == "name_or_index")
            .unwrap_or(1);
        let edge_to_idx = meta
            .edge_fields
            .iter()
            .position(|f| f == "to_node")
            .unwrap_or(2);

        // Parse nodes
        let mut nodes = Vec::with_capacity(raw.snapshot.node_count as usize);
        let mut id_to_idx = HashMap::new();
        let mut edge_offset = 0usize;

        for (node_idx, chunk) in raw.nodes.chunks(node_field_count).enumerate() {
            if chunk.len() < node_field_count {
                break;
            }

            let type_id = chunk[type_idx] as usize;
            let node_type = node_type_names
                .get(type_id)
                .cloned()
                .unwrap_or_else(|| format!("type_{}", type_id));

            let name_id = chunk[name_idx] as usize;
            let name = raw.strings.get(name_id).cloned().unwrap_or_default();

            let id = chunk[id_idx] as u64;
            let self_size = chunk[size_idx] as u64;
            let edge_count = chunk[edge_count_idx] as usize;

            id_to_idx.insert(id, node_idx);

            nodes.push(HeapNode {
                node_type,
                name,
                id,
                self_size,
                edge_count,
                edges_start: edge_offset,
            });

            edge_offset += edge_count;
        }

        // Parse edges
        let mut edges = Vec::with_capacity(raw.snapshot.edge_count as usize);

        for chunk in raw.edges.chunks(edge_field_count) {
            if chunk.len() < edge_field_count {
                break;
            }

            let type_id = chunk[edge_type_idx] as usize;
            let edge_type = edge_type_names
                .get(type_id)
                .cloned()
                .unwrap_or_else(|| format!("edge_{}", type_id));

            // name_or_index is either a string index or a numeric index
            let name_or_index_raw = chunk[edge_name_idx];
            let name_or_index = if edge_type == "element" || edge_type == "hidden" {
                // Numeric index
                format!("[{}]", name_or_index_raw)
            } else {
                // String index
                raw.strings
                    .get(name_or_index_raw as usize)
                    .cloned()
                    .unwrap_or_else(|| format!("{}", name_or_index_raw))
            };

            // to_node is an index into the nodes array (as byte offset, need to divide by field count)
            let to_node_idx = (chunk[edge_to_idx] as usize) / node_field_count;

            edges.push(HeapEdge {
                edge_type,
                name_or_index,
                to_node_idx,
            });
        }

        Ok(ParsedSnapshot {
            nodes,
            edges,
            id_to_idx,
            node_type_names,
            edge_type_names,
        })
    }

    /// Get edges for a node.
    pub fn edges_for_node(&self, node_idx: usize) -> &[HeapEdge] {
        let node = &self.nodes[node_idx];
        let start = node.edges_start;
        let end = start + node.edge_count;
        &self.edges[start..end.min(self.edges.len())]
    }
}

// ============================================================================
// Diff computation
// ============================================================================

/// Statistics for a constructor/type.
#[derive(Debug, Clone, Default)]
pub struct TypeStats {
    pub count: u64,
    pub total_size: u64,
}

/// Growth info for a type.
#[derive(Debug, Clone, Serialize)]
pub struct TypeGrowth {
    pub constructor: String,
    pub count_before: u64,
    pub count_after: u64,
    pub count_delta: i64,
    pub size_before: u64,
    pub size_after: u64,
    pub size_delta: i64,
}

/// A retained object with its retention path.
#[derive(Debug, Clone, Serialize)]
pub struct RetainedObject {
    pub constructor: String,
    pub size: u64,
    pub retention_path: Vec<String>,
}

/// Heap diff result.
pub struct HeapDiff {
    pub baseline_path: String,
    pub target_path: String,
    pub type_growth: Vec<TypeGrowth>,
    pub retained_objects: Vec<RetainedObject>,
}

/// Reverse edge map: node_idx -> [(from_node_idx, edge_name)]
type ReverseEdgeMap = HashMap<usize, Vec<(usize, String)>>;

impl HeapDiff {
    /// Compute diff between two snapshots.
    pub fn compute(
        baseline: &ParsedSnapshot,
        target: &ParsedSnapshot,
        baseline_path: &str,
        target_path: &str,
        max_retained_objects: usize,
    ) -> Self {
        // Compute type stats for baseline
        let baseline_stats = Self::compute_type_stats(baseline);

        // Compute type stats for target
        let target_stats = Self::compute_type_stats(target);

        // Compute growth
        let mut type_growth: Vec<TypeGrowth> = Vec::new();

        // Get all type names
        let mut all_types: std::collections::HashSet<&str> = std::collections::HashSet::new();
        for key in baseline_stats.keys() {
            all_types.insert(key);
        }
        for key in target_stats.keys() {
            all_types.insert(key);
        }

        for type_name in all_types {
            let before = baseline_stats.get(type_name).cloned().unwrap_or_default();
            let after = target_stats.get(type_name).cloned().unwrap_or_default();

            let count_delta = after.count as i64 - before.count as i64;
            let size_delta = after.total_size as i64 - before.total_size as i64;

            // Only include types that grew
            if count_delta > 0 || size_delta > 0 {
                type_growth.push(TypeGrowth {
                    constructor: type_name.to_string(),
                    count_before: before.count,
                    count_after: after.count,
                    count_delta,
                    size_before: before.total_size,
                    size_after: after.total_size,
                    size_delta,
                });
            }
        }

        // Sort by size delta descending
        type_growth.sort_by(|a, b| b.size_delta.cmp(&a.size_delta));

        // Find objects that are new in target (not in baseline)
        let mut retained_objects = Vec::new();
        let top_growing_types: Vec<String> = type_growth
            .iter()
            .take(10)
            .map(|g| g.constructor.clone())
            .collect();

        // Build reverse edge map once (this is expensive but only done once)
        eprintln!("  Building reverse edge map ({} edges)...", target.edges.len());
        let reverse_edges = Self::build_reverse_edge_map(target);
        eprintln!("  Analyzing retained objects...");

        // Find new objects of top growing types and get their retention paths
        for (node_idx, node) in target.nodes.iter().enumerate() {
            if retained_objects.len() >= max_retained_objects {
                break;
            }

            // Check if this object is new (not in baseline)
            if baseline.id_to_idx.contains_key(&node.id) {
                continue;
            }

            // Get the constructor name (for objects, it's the name field)
            let constructor = if node.node_type == "object" || node.node_type == "closure" {
                if node.name.is_empty() {
                    node.node_type.clone()
                } else {
                    node.name.clone()
                }
            } else {
                node.node_type.clone()
            };

            // Only analyze top growing types
            if !top_growing_types.contains(&constructor) {
                continue;
            }

            // Get retention path
            let retention_path = Self::find_retention_path(target, node_idx, &reverse_edges);

            if !retention_path.is_empty() {
                retained_objects.push(RetainedObject {
                    constructor,
                    size: node.self_size,
                    retention_path,
                });
            }
        }

        HeapDiff {
            baseline_path: baseline_path.to_string(),
            target_path: target_path.to_string(),
            type_growth,
            retained_objects,
        }
    }

    fn compute_type_stats(snapshot: &ParsedSnapshot) -> HashMap<String, TypeStats> {
        let mut stats: HashMap<String, TypeStats> = HashMap::new();

        for node in &snapshot.nodes {
            // Use name for objects (constructor name), node_type otherwise
            let key = if node.node_type == "object" || node.node_type == "closure" {
                if node.name.is_empty() {
                    node.node_type.clone()
                } else {
                    node.name.clone()
                }
            } else {
                node.node_type.clone()
            };

            let entry = stats.entry(key).or_default();
            entry.count += 1;
            entry.total_size += node.self_size;
        }

        stats
    }

    /// Build reverse edge map: for each node, which nodes point to it.
    fn build_reverse_edge_map(snapshot: &ParsedSnapshot) -> ReverseEdgeMap {
        let mut reverse_edges: ReverseEdgeMap = HashMap::new();

        for (from_idx, _node) in snapshot.nodes.iter().enumerate() {
            for edge in snapshot.edges_for_node(from_idx) {
                reverse_edges
                    .entry(edge.to_node_idx)
                    .or_default()
                    .push((from_idx, edge.name_or_index.clone()));
            }
        }

        reverse_edges
    }

    /// Find retention path from GC roots to a node (BFS from node backwards to root).
    /// Returns path like ["Window", "app", "cache", "items[42]"]
    fn find_retention_path(
        snapshot: &ParsedSnapshot,
        target_idx: usize,
        reverse_edges: &ReverseEdgeMap,
    ) -> Vec<String> {
        // BFS from target back to root (with iteration limit to avoid very long searches)
        let mut visited: HashMap<usize, (usize, String)> = HashMap::new();
        let mut queue: std::collections::VecDeque<usize> = std::collections::VecDeque::new();
        const MAX_BFS_ITERATIONS: usize = 10_000;

        queue.push_back(target_idx);
        visited.insert(target_idx, (usize::MAX, String::new()));

        let mut root_idx: Option<usize> = None;
        let mut iterations = 0;

        while let Some(current) = queue.pop_front() {
            iterations += 1;
            if iterations > MAX_BFS_ITERATIONS {
                break;
            }

            let node = &snapshot.nodes[current];

            // Check if this is a root (GC root types)
            if node.node_type == "synthetic" && node.name.contains("root") {
                root_idx = Some(current);
                break;
            }
            if node.name == "Window" || node.name == "global" {
                root_idx = Some(current);
                break;
            }

            // Add predecessors
            if let Some(predecessors) = reverse_edges.get(&current) {
                for (pred_idx, edge_name) in predecessors {
                    if !visited.contains_key(pred_idx) {
                        visited.insert(*pred_idx, (current, edge_name.clone()));
                        queue.push_back(*pred_idx);
                    }
                }
            }
        }

        // Build path from root to target
        let mut path = Vec::new();

        if let Some(root) = root_idx {
            let mut current = root;
            path.push(snapshot.nodes[current].name.clone());

            while current != target_idx {
                // Find next in path
                let mut found = false;
                for (node_idx, (prev, edge_name)) in &visited {
                    if *prev == current {
                        if edge_name.is_empty() {
                            path.push(snapshot.nodes[*node_idx].name.clone());
                        } else {
                            path.push(edge_name.clone());
                        }
                        current = *node_idx;
                        found = true;
                        break;
                    }
                }
                if !found {
                    break;
                }
                // Limit path length
                if path.len() > 20 {
                    path.push("...".to_string());
                    break;
                }
            }
        }

        path
    }

    /// Write diff as NDJSON.
    pub fn write_ndjson<W: Write>(&self, mut writer: W) -> Result<()> {
        // Write header
        let header = serde_json::json!({
            "type": "header",
            "format": "heap-diff",
            "version": "0.1",
            "baseline": self.baseline_path,
            "target": self.target_path
        });
        writeln!(writer, "{}", serde_json::to_string(&header)?)?;

        // Write type growth records
        for growth in &self.type_growth {
            let record = serde_json::json!({
                "type": "growth",
                "constructor": growth.constructor,
                "count_before": growth.count_before,
                "count_after": growth.count_after,
                "count_delta": growth.count_delta,
                "size_before": growth.size_before,
                "size_after": growth.size_after,
                "size_delta": growth.size_delta
            });
            writeln!(writer, "{}", serde_json::to_string(&record)?)?;
        }

        // Write retained object records
        for obj in &self.retained_objects {
            let record = serde_json::json!({
                "type": "retained",
                "constructor": obj.constructor,
                "size": obj.size,
                "retention_path": obj.retention_path
            });
            writeln!(writer, "{}", serde_json::to_string(&record)?)?;
        }

        Ok(())
    }
}
