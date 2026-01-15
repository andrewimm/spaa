use spaa_parse::SpaaFile;
use std::env;
use std::fs::File;
use std::process::ExitCode;

fn main() -> ExitCode {
    let args: Vec<String> = env::args().collect();

    if args.len() != 2 {
        eprintln!("Usage: {} <file.spaa>", args[0]);
        return ExitCode::from(2);
    }

    let path = &args[1];

    let file = match File::open(path) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("Error opening '{}': {}", path, e);
            return ExitCode::FAILURE;
        }
    };

    match SpaaFile::parse(file) {
        Ok(spaa) => {
            println!("Valid SPAA file: {}", path);
            println!("  Format version: {}", spaa.header.version);
            println!("  Source tool: {}", spaa.header.source_tool);
            println!("  Events: {}", spaa.header.events.len());
            println!("  DSOs: {}", spaa.dsos.len());
            println!("  Frames: {}", spaa.frames.len());
            println!("  Stacks: {}", spaa.stacks.len());
            if !spaa.samples.is_empty() {
                println!("  Samples: {}", spaa.samples.len());
            }
            if !spaa.windows.is_empty() {
                println!("  Windows: {}", spaa.windows.len());
            }
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("Invalid SPAA file '{}': {}", path, e);
            ExitCode::FAILURE
        }
    }
}
