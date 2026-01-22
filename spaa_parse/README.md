# spaa_parse

Parser and writer for SPAA (Stack Profile for Agentic Analysis) files.

SPAA is a structured format for representing profiling data from tools like Linux `perf`, DTrace, and Chrome DevTools. It's designed for analysis by both humans and LLMs.

## Usage

### Reading SPAA Files

```rust
use std::fs::File;
use spaa_parse::SpaaFile;

let file = File::open("profile.spaa").unwrap();
let spaa = SpaaFile::parse(file).unwrap();

println!("Source tool: {}", spaa.header.source_tool);
println!("Stacks: {}", spaa.stacks.len());

// Iterate over stacks for a specific event
for stack in spaa.stacks_for_event("cycles") {
    println!("Stack {} has {} frames", stack.id, stack.frames.len());
}
```

### Writing SPAA Files

```rust
use std::fs::File;
use spaa_parse::{SpaaWriter, Header, Dso, Frame, Stack};

let file = File::create("output.spaa").unwrap();
let mut writer = SpaaWriter::new(file);

// Write header first, then dictionaries (DSOs, frames), then stacks
writer.write_header(&header).unwrap();
writer.write_dso(&dso).unwrap();
writer.write_frame(&frame).unwrap();
writer.write_stack(&stack).unwrap();
```

## License

MIT
