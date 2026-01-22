# spaa

Tools for converting profiling data to SPAA (Stack Profile for Agentic Analysis) format.

## Installation

```bash
cargo install spaa
```

This installs two binaries:
- `dtrace_to_spaa` - Convert DTrace output to SPAA
- `chrome_to_spaa` - Convert Chrome DevTools profiles to SPAA

## Usage

### DTrace

```bash
# Convert DTrace aggregated stack output
dtrace_to_spaa profile.out -o profile.spaa

# Specify event name and sampling frequency
dtrace_to_spaa profile.out --event syscall::read:entry --frequency 0
```

### Chrome DevTools

```bash
# Auto-detects format (Performance trace, cpuprofile, or heap snapshot)
chrome_to_spaa trace.json -o profile.spaa
chrome_to_spaa Profile.cpuprofile
chrome_to_spaa Heap.heapsnapshot
```

## Library Usage

```rust
use spaa::dtrace::{DtraceConverter, InputFormat};
use spaa::chrome::{CpuProfileConverter, HeapSnapshotConverter};
use spaa::perf::PerfConverter;

// Convert DTrace output
let mut converter = DtraceConverter::new(InputFormat::AggregatedStack);
converter.parse(input)?;
converter.write_spaa(output)?;
```

## License

MIT
