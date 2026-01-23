# SPAA - Stack Profile for Agentic Analysis

SPAA is a file format and toolset designed to make performance profiling data accessible to AI agents and LLMs. It converts raw profiler output into a structured, queryable format that agents can analyze using simple command-line tools.

## Why SPAA?

Raw profiler output is problematic for AI-assisted analysis:

| Raw Traces | SPAA |
|------------|------|
| Huge files (100MB+ for minutes of profiling) | Pre-aggregated stacks reduce size 10-100x |
| Binary or tool-specific formats | NDJSON - each line is self-contained JSON |
| Requires specialized parsers | Queryable with `grep`, `jq`, `head`, `tail` |
| Redundant data (same stack thousands of times) | Each unique stack appears once with weights |
| Implicit semantics | Explicit metrics, frame order, and event types |

SPAA files are designed for **progressive disclosure**: an agent can `head -1` to understand the profiler and metrics, `grep` for specific record types, and `jq` to extract exactly what it needs - all without loading the entire file into context.

## Installation

```bash
cargo install spaa
```

This installs three binaries: `dtrace_to_spaa`, `chrome_to_spaa`, and `heapdiff`.

## Quick Start

### Convert a DTrace profile

```bash
# Run DTrace to collect a CPU profile
sudo dtrace -n 'profile-997 { @[ustack()] = count(); }' -o profile.txt

# Convert to SPAA
dtrace_to_spaa profile.txt -o profile.spaa
```

### Convert Chrome DevTools data

```bash
# CPU profiling (Performance panel trace or .cpuprofile)
chrome_to_spaa trace.json -o cpu.spaa

# Memory profiling (heap snapshot or heap timeline)
chrome_to_spaa Heap.heapsnapshot -o heap.spaa
```

### Find memory leaks with heapdiff

```bash
# Take two heap snapshots in Chrome DevTools, then compare them
heapdiff baseline.heapsnapshot after-action.heapsnapshot -o diff.ndjson
```

## CLI Tools

### dtrace_to_spaa

Converts DTrace aggregated stack output to SPAA format.

```bash
dtrace_to_spaa input.txt -o output.spaa
dtrace_to_spaa input.txt --event syscall::read:entry --frequency 0
```

Options:
- `-o, --output` - Output file (defaults to input with `.spaa` extension)
- `-e, --event` - Event name (default: `profile-997`)
- `-z, --frequency` - Sampling frequency in Hz (inferred from event name if possible)
- `-f, --format` - Input format: `aggregated` (default), `split`, `per-probe`

### chrome_to_spaa

Converts Chrome DevTools profiling data to SPAA format. Automatically detects the input type.

```bash
chrome_to_spaa trace.json              # Performance panel trace
chrome_to_spaa profile.cpuprofile      # V8 CPU profile
chrome_to_spaa Heap.heapsnapshot       # Memory panel snapshot
chrome_to_spaa timeline.heaptimeline   # Allocation timeline
```

Options:
- `-o, --output` - Output file (defaults to input with `.spaa` extension)

### heapdiff

Compares two Chrome heap snapshots to identify memory leaks. Outputs an agent-friendly NDJSON format showing object type growth and retention paths.

```bash
heapdiff baseline.heapsnapshot target.heapsnapshot -o diff.ndjson
```

Options:
- `-o, --output` - Output file (defaults to stdout)
- `-n, --max-retained` - Maximum retained objects to analyze (default: 100)

## Library Usage

The `spaa_parse` crate provides types and parsers for working with SPAA files in Rust:

```rust
use spaa_parse::{SpaaReader, Record};
use std::fs::File;
use std::io::BufReader;

let file = File::open("profile.spaa")?;
let reader = SpaaReader::new(BufReader::new(file));

for record in reader {
    match record? {
        Record::Header(h) => println!("Source: {}", h.source_tool),
        Record::Stack(s) => println!("Stack {} has {} frames", s.id, s.frames.len()),
        _ => {}
    }
}
```

## Agent Skill

Install the SPAA analysis skill to give your AI coding agent the ability to analyze performance profiles:

```bash
npx skills add andrewimm/spaa
```

The skill teaches agents how to:
- Parse SPAA files using `head`, `tail`, `grep`, and `jq`
- Find CPU hotspots and hot functions (exclusive time)
- Identify memory leaks via `live_bytes` metrics
- Reconstruct call stacks from frame IDs
- Filter by thread, event type, or time window

Works with [Claude Code](https://claude.ai/code), [Cursor](https://cursor.com), and other [skills-compatible agents](https://agentskills.io).

## File Format

See [SPEC.md](SPEC.md) for the complete format specification.

SPAA files are NDJSON (newline-delimited JSON) with these record types:

- **header** - File metadata, profiler info, event definitions
- **dso** - Shared libraries and binaries
- **frame** - Stack frame definitions (function, source location)
- **thread** - Process/thread information
- **stack** - Aggregated call stacks with weights (the main data)
- **sample** - Individual sample events (optional, for temporal analysis)

Example header:
```json
{"type":"header","format":"spaa","version":"1.0","source_tool":"dtrace","frame_order":"leaf_to_root","events":[{"name":"profile-997","kind":"timer","sampling":{"mode":"frequency","primary_metric":"samples","frequency_hz":997}}]}
```

Example stack:
```json
{"type":"stack","id":"0xabc123","frames":[101,77,42],"context":{"event":"profile-997","tid":1234},"weights":[{"metric":"samples","value":847}]}
```

## License

MIT
