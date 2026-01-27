import { Navigation } from "@/components/navigation";
import { CodeBlock, InlineCode } from "@/components/code-block";
import type { Metadata } from "next";
import { Terminal, Cpu, Chrome, MemoryStick, BookOpen, Wrench } from "lucide-react";

export const metadata: Metadata = {
  title: "Usage - SPAA",
  description:
    "Installation guide and usage documentation for SPAA CLI tools.",
};

export default function UsagePage() {
  return (
    <div className="min-h-screen">
      <Navigation />

      <main className="mx-auto max-w-4xl px-6 py-16">
        {/* Header */}
        <div className="mb-16 border-b border-border pb-8">
          <p className="mb-2 font-mono text-sm text-primary">Documentation</p>
          <h1 className="mb-4 text-3xl font-bold md:text-4xl">
            Installation & Usage
          </h1>
          <p className="text-lg text-muted-foreground">
            Get started with SPAA in minutes
          </p>
        </div>

        <div className="space-y-16">
          {/* Installation */}
          <section>
            <div className="mb-6 flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
                <Terminal className="h-5 w-5 text-primary" />
              </div>
              <h2 className="text-2xl font-bold">Installation</h2>
            </div>

            <p className="mb-6 text-muted-foreground">
              SPAA is distributed as a Rust crate and can be installed via Cargo.
              This installs three binaries: <InlineCode>dtrace_to_spaa</InlineCode>,{" "}
              <InlineCode>chrome_to_spaa</InlineCode>, and{" "}
              <InlineCode>heapdiff</InlineCode>.
            </p>

            <CodeBlock code="cargo install spaa" />

            <div className="mt-6 rounded-lg border border-border bg-card/50 p-4">
              <p className="text-sm text-muted-foreground">
                <strong className="text-foreground">Prerequisites:</strong> You need
                Rust and Cargo installed. If you don't have them, install via{" "}
                <a
                  href="https://rustup.rs"
                  target="_blank"
                  rel="noopener noreferrer"
                  className="text-primary hover:underline"
                >
                  rustup.rs
                </a>
              </p>
            </div>
          </section>

          {/* Quick Start */}
          <section>
            <div className="mb-6 flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
                <Cpu className="h-5 w-5 text-primary" />
              </div>
              <h2 className="text-2xl font-bold">Quick Start</h2>
            </div>

            <div className="space-y-8">
              {/* DTrace */}
              <div>
                <h3 className="mb-4 text-lg font-semibold">Convert a DTrace profile</h3>
                <p className="mb-4 text-muted-foreground">
                  Run DTrace to collect a CPU profile, then convert it to SPAA format:
                </p>
                <div className="space-y-3">
                  <CodeBlock
                    title="1. Collect profile with DTrace"
                    code={`sudo dtrace -n 'profile-997 { @[ustack()] = count(); }' -o profile.txt`}
                  />
                  <CodeBlock
                    title="2. Convert to SPAA"
                    code="dtrace_to_spaa profile.txt -o profile.spaa"
                  />
                </div>
              </div>

              {/* Chrome */}
              <div>
                <h3 className="mb-4 text-lg font-semibold">
                  Convert Chrome DevTools data
                </h3>
                <p className="mb-4 text-muted-foreground">
                  Convert CPU profiles or heap snapshots from Chrome DevTools:
                </p>
                <div className="space-y-3">
                  <CodeBlock
                    title="CPU profiling (Performance panel trace or .cpuprofile)"
                    code="chrome_to_spaa trace.json -o cpu.spaa"
                  />
                  <CodeBlock
                    title="Memory profiling (heap snapshot or heap timeline)"
                    code="chrome_to_spaa Heap.heapsnapshot -o heap.spaa"
                  />
                </div>
              </div>

              {/* Heapdiff */}
              <div>
                <h3 className="mb-4 text-lg font-semibold">
                  Find memory leaks with heapdiff
                </h3>
                <p className="mb-4 text-muted-foreground">
                  Compare two Chrome heap snapshots to identify memory leaks:
                </p>
                <CodeBlock
                  code="heapdiff baseline.heapsnapshot after-action.heapsnapshot -o diff.ndjson"
                />
              </div>
            </div>
          </section>

          {/* CLI Tools */}
          <section>
            <div className="mb-6 flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
                <Wrench className="h-5 w-5 text-primary" />
              </div>
              <h2 className="text-2xl font-bold">CLI Tools</h2>
            </div>

            <div className="space-y-10">
              {/* dtrace_to_spaa */}
              <div className="rounded-lg border border-border bg-card p-6">
                <h3 className="mb-2 font-mono text-lg font-semibold text-primary">
                  dtrace_to_spaa
                </h3>
                <p className="mb-4 text-muted-foreground">
                  Converts DTrace aggregated stack output to SPAA format.
                </p>

                <CodeBlock
                  code={`dtrace_to_spaa input.txt -o output.spaa
dtrace_to_spaa input.txt --event syscall::read:entry --frequency 0`}
                />

                <h4 className="mb-3 mt-6 font-medium">Options</h4>
                <div className="space-y-3 text-sm">
                  <div className="flex gap-4">
                    <code className="shrink-0 font-mono text-primary">
                      -o, --output
                    </code>
                    <span className="text-muted-foreground">
                      Output file (defaults to input with <InlineCode>.spaa</InlineCode>{" "}
                      extension)
                    </span>
                  </div>
                  <div className="flex gap-4">
                    <code className="shrink-0 font-mono text-primary">
                      -e, --event
                    </code>
                    <span className="text-muted-foreground">
                      Event name (default: <InlineCode>profile-997</InlineCode>)
                    </span>
                  </div>
                  <div className="flex gap-4">
                    <code className="shrink-0 font-mono text-primary">
                      -z, --frequency
                    </code>
                    <span className="text-muted-foreground">
                      Sampling frequency in Hz (inferred from event name if possible)
                    </span>
                  </div>
                  <div className="flex gap-4">
                    <code className="shrink-0 font-mono text-primary">
                      -f, --format
                    </code>
                    <span className="text-muted-foreground">
                      Input format: <InlineCode>aggregated</InlineCode> (default),{" "}
                      <InlineCode>split</InlineCode>, <InlineCode>per-probe</InlineCode>
                    </span>
                  </div>
                </div>
              </div>

              {/* chrome_to_spaa */}
              <div className="rounded-lg border border-border bg-card p-6">
                <div className="mb-2 flex items-center gap-2">
                  <Chrome className="h-5 w-5 text-primary" />
                  <h3 className="font-mono text-lg font-semibold text-primary">
                    chrome_to_spaa
                  </h3>
                </div>
                <p className="mb-4 text-muted-foreground">
                  Converts Chrome DevTools profiling data to SPAA format. Automatically
                  detects the input type.
                </p>

                <CodeBlock
                  code={`chrome_to_spaa trace.json        # Performance panel trace
chrome_to_spaa profile.cpuprofile # V8 CPU profile
chrome_to_spaa Heap.heapsnapshot  # Memory panel snapshot
chrome_to_spaa timeline.heaptimeline # Allocation timeline`}
                />

                <h4 className="mb-3 mt-6 font-medium">Options</h4>
                <div className="space-y-3 text-sm">
                  <div className="flex gap-4">
                    <code className="shrink-0 font-mono text-primary">
                      -o, --output
                    </code>
                    <span className="text-muted-foreground">
                      Output file (defaults to input with <InlineCode>.spaa</InlineCode>{" "}
                      extension)
                    </span>
                  </div>
                </div>
              </div>

              {/* heapdiff */}
              <div className="rounded-lg border border-border bg-card p-6">
                <div className="mb-2 flex items-center gap-2">
                  <MemoryStick className="h-5 w-5 text-primary" />
                  <h3 className="font-mono text-lg font-semibold text-primary">
                    heapdiff
                  </h3>
                </div>
                <p className="mb-4 text-muted-foreground">
                  Compares two Chrome heap snapshots to identify memory leaks. Outputs
                  an agent-friendly NDJSON format showing object type growth and
                  retention paths.
                </p>

                <CodeBlock
                  code="heapdiff baseline.heapsnapshot target.heapsnapshot -o diff.ndjson"
                />

                <h4 className="mb-3 mt-6 font-medium">Options</h4>
                <div className="space-y-3 text-sm">
                  <div className="flex gap-4">
                    <code className="shrink-0 font-mono text-primary">
                      -o, --output
                    </code>
                    <span className="text-muted-foreground">
                      Output file (defaults to stdout)
                    </span>
                  </div>
                  <div className="flex gap-4">
                    <code className="shrink-0 font-mono text-primary">
                      -n, --max-retained
                    </code>
                    <span className="text-muted-foreground">
                      Maximum retained objects to analyze (default: 100)
                    </span>
                  </div>
                </div>
              </div>
            </div>
          </section>

          {/* Library Usage */}
          <section>
            <div className="mb-6 flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
                <BookOpen className="h-5 w-5 text-primary" />
              </div>
              <h2 className="text-2xl font-bold">Library Usage</h2>
            </div>

            <p className="mb-6 text-muted-foreground">
              The <InlineCode>spaa_parse</InlineCode> crate provides types and
              parsers for working with SPAA files in Rust:
            </p>

            <CodeBlock
              code={`use spaa_parse::{SpaaReader, Record};
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
}`}
            />
          </section>

          {/* Agent Skill */}
          <section>
            <div className="mb-6 flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
                <svg
                  className="h-5 w-5 text-primary"
                  viewBox="0 0 24 24"
                  fill="none"
                  stroke="currentColor"
                  strokeWidth="2"
                >
                  <title>AI Agent</title>
                  <circle cx="12" cy="12" r="10" />
                  <circle cx="12" cy="10" r="3" />
                  <path d="M7 20.662V19a2 2 0 0 1 2-2h6a2 2 0 0 1 2 2v1.662" />
                </svg>
              </div>
              <h2 className="text-2xl font-bold">Agent Skill</h2>
            </div>

            <p className="mb-6 text-muted-foreground">
              Install the SPAA analysis skill to give your AI coding agent the
              ability to analyze performance profiles:
            </p>

            <CodeBlock code="npx skills add andrewimm/spaa" />

            <div className="mt-6 space-y-4">
              <h3 className="font-medium">The skill teaches agents how to:</h3>
              <ul className="list-inside list-disc space-y-2 text-muted-foreground">
                <li>
                  Parse SPAA files using <InlineCode>head</InlineCode>,{" "}
                  <InlineCode>tail</InlineCode>, <InlineCode>grep</InlineCode>, and{" "}
                  <InlineCode>jq</InlineCode>
                </li>
                <li>Find CPU hotspots and hot functions (exclusive time)</li>
                <li>
                  Identify memory leaks via <InlineCode>live_bytes</InlineCode> metrics
                </li>
                <li>Reconstruct call stacks from frame IDs</li>
                <li>Filter by thread, event type, or time window</li>
              </ul>
            </div>

            <div className="mt-8 grid gap-4 md:grid-cols-3">
              <div className="rounded-lg border border-border bg-secondary/30 p-4 text-center">
                <p className="font-mono text-sm text-foreground">Claude Code</p>
                <p className="text-xs text-muted-foreground">Supported</p>
              </div>
              <div className="rounded-lg border border-border bg-secondary/30 p-4 text-center">
                <p className="font-mono text-sm text-foreground">Cursor</p>
                <p className="text-xs text-muted-foreground">Supported</p>
              </div>
              <div className="rounded-lg border border-border bg-secondary/30 p-4 text-center">
                <p className="font-mono text-sm text-foreground">
                  Skills-compatible
                </p>
                <p className="text-xs text-muted-foreground">Supported</p>
              </div>
            </div>
          </section>

          {/* Working with SPAA Files */}
          <section>
            <div className="mb-6 flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
                <Terminal className="h-5 w-5 text-primary" />
              </div>
              <h2 className="text-2xl font-bold">Working with SPAA Files</h2>
            </div>

            <p className="mb-6 text-muted-foreground">
              SPAA files are NDJSON (newline-delimited JSON), making them easy to
              query with standard command-line tools:
            </p>

            <div className="space-y-6">
              <div>
                <h3 className="mb-3 font-medium">View the header (first line)</h3>
                <CodeBlock code="head -1 profile.spaa | jq ." />
              </div>

              <div>
                <h3 className="mb-3 font-medium">Find all stack records</h3>
                <CodeBlock code={`grep '"type":"stack"' profile.spaa | head -5`} />
              </div>

              <div>
                <h3 className="mb-3 font-medium">Extract frame information</h3>
                <CodeBlock
                  code={`grep '"type":"frame"' profile.spaa | jq '{id, func, srcline}'`}
                />
              </div>

              <div>
                <h3 className="mb-3 font-medium">Find hottest stacks by sample count</h3>
                <CodeBlock
                  code={`grep '"type":"stack"' profile.spaa | \\
  jq -r '[.weights[] | select(.metric=="samples") | .value][0] as $v | "\\($v)\\t\\(.id)"' | \\
  sort -rn | head -10`}
                />
              </div>

              <div>
                <h3 className="mb-3 font-medium">Filter stacks by thread ID</h3>
                <CodeBlock
                  code={`grep '"type":"stack"' profile.spaa | jq 'select(.context.tid == 4511)'`}
                />
              </div>
            </div>
          </section>
        </div>
      </main>

      {/* Footer */}
      <footer className="border-t border-border bg-card/30">
        <div className="mx-auto max-w-5xl px-6 py-8">
          <div className="flex flex-col items-center justify-between gap-4 md:flex-row">
            <div className="font-mono text-sm text-muted-foreground">
              SPAA • MIT License
            </div>
            <div className="flex items-center gap-6">
              <a
                href="https://github.com/andrewimm/spaa"
                target="_blank"
                rel="noopener noreferrer"
                className="font-mono text-sm text-muted-foreground transition-colors hover:text-primary"
              >
                GitHub
              </a>
            </div>
          </div>
        </div>
      </footer>
    </div>
  );
}
