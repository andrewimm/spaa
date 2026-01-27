import { Navigation } from "@/components/navigation";
import { AsciiLogo } from "@/components/ascii-art";
import { CodeBlock, InlineCode } from "@/components/code-block";
import {
  ArrowRight,
  FileJson,
  Zap,
  Search,
  GitBranch,
  Layers,
  Terminal,
} from "lucide-react";
import Link from "next/link";

export default function HomePage() {
  return (
    <div className="min-h-screen">
      <Navigation />

      {/* Hero Section */}
      <section className="relative overflow-hidden border-b border-border">
        <div className="mx-auto max-w-5xl px-6 py-20 md:py-32">
          <div className="flex flex-col items-center text-center">
            <div className="mb-8">
              <AsciiLogo />
            </div>
            <p className="mb-4 font-mono text-lg text-muted-foreground">
              Stack Profile for Agentic Analysis
            </p>
            <h1 className="mb-6 max-w-3xl text-balance text-3xl font-bold leading-tight md:text-5xl">
              Make profiling data{" "}
              <span className="text-primary">accessible to AI agents</span>
            </h1>
            <p className="mb-10 max-w-2xl text-pretty text-lg leading-relaxed text-muted-foreground">
              SPAA is a file format and toolset designed to convert raw profiler
              output into a structured, queryable format that AI agents can
              analyze using simple command-line tools.
            </p>

            <div className="flex flex-col gap-4 sm:flex-row">
              <Link
                href="/usage"
                className="inline-flex items-center gap-2 rounded-lg bg-primary px-6 py-3 font-mono text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
              >
                Get Started
                <ArrowRight className="h-4 w-4" />
              </Link>
              <Link
                href="/spec"
                className="inline-flex items-center gap-2 rounded-lg border border-border bg-secondary px-6 py-3 font-mono text-sm font-medium text-foreground transition-colors hover:bg-secondary/80"
              >
                Read the Spec
              </Link>
            </div>
          </div>
        </div>
      </section>

      {/* Install Command */}
      <section className="border-b border-border bg-card/50">
        <div className="mx-auto max-w-5xl px-6 py-12">
          <div className="flex flex-col items-center gap-4">
            <p className="font-mono text-sm text-muted-foreground">
              Install with Cargo
            </p>
            <CodeBlock code="cargo install spaa" />
          </div>
        </div>
      </section>

      {/* Why SPAA Section */}
      <section className="border-b border-border">
        <div className="mx-auto max-w-5xl px-6 py-20">
          <h2 className="mb-4 text-center font-mono text-sm uppercase tracking-wider text-primary">
            The Problem
          </h2>
          <h3 className="mb-12 text-balance text-center text-2xl font-bold md:text-4xl">
            Raw profiler output is problematic for AI analysis
          </h3>

          <div className="overflow-x-auto">
            <table className="w-full border-collapse">
              <thead>
                <tr className="border-b border-border">
                  <th className="p-4 text-left font-mono text-sm font-medium text-muted-foreground">
                    Raw Traces
                  </th>
                  <th className="p-4 text-left font-mono text-sm font-medium text-primary">
                    SPAA
                  </th>
                </tr>
              </thead>
              <tbody className="font-mono text-sm">
                <tr className="border-b border-border">
                  <td className="p-4 text-muted-foreground">
                    Huge files (100MB+ for less than 1 minute of profiling)
                  </td>
                  <td className="p-4 text-foreground">
                    Pre-aggregated stacks reduce size 10-100x
                  </td>
                </tr>
                <tr className="border-b border-border">
                  <td className="p-4 text-muted-foreground">
                    Binary or tool-specific formats
                  </td>
                  <td className="p-4 text-foreground">
                    NDJSON - each line is self-contained JSON
                  </td>
                </tr>
                <tr className="border-b border-border">
                  <td className="p-4 text-muted-foreground">
                    Requires specialized parsers
                  </td>
                  <td className="p-4 text-foreground">
                    Queryable with{" "}
                    <InlineCode>grep</InlineCode>,{" "}
                    <InlineCode>jq</InlineCode>,{" "}
                    <InlineCode>head</InlineCode>,{" "}
                    <InlineCode>tail</InlineCode>
                  </td>
                </tr>
                <tr className="border-b border-border">
                  <td className="p-4 text-muted-foreground">
                    Redundant data (same stack thousands of times)
                  </td>
                  <td className="p-4 text-foreground">
                    Each unique stack appears once with weights
                  </td>
                </tr>
                <tr>
                  <td className="p-4 text-muted-foreground">
                    Implicit semantics
                  </td>
                  <td className="p-4 text-foreground">
                    Explicit metrics, frame order, and event types
                  </td>
                </tr>
              </tbody>
            </table>
          </div>
        </div>
      </section>

      {/* Progressive Disclosure */}
      <section className="border-b border-border bg-card/30">
        <div className="mx-auto max-w-5xl px-6 py-20">
          <h2 className="mb-4 text-center font-mono text-sm uppercase tracking-wider text-primary">
            Design Philosophy
          </h2>
          <h3 className="mb-6 text-balance text-center text-2xl font-bold md:text-4xl">
            Progressive Disclosure
          </h3>
          <p className="mx-auto mb-12 max-w-2xl text-pretty text-center text-lg leading-relaxed text-muted-foreground">
            SPAA files are designed so agents can incrementally access exactly
            what they need — without loading the entire file into context.
          </p>

          <div className="grid gap-6 md:grid-cols-3">
            <div className="rounded-lg border border-border bg-card p-6">
              <div className="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
                <Terminal className="h-5 w-5 text-primary" />
              </div>
              <h4 className="mb-2 font-mono font-medium">head -1</h4>
              <p className="text-sm leading-relaxed text-muted-foreground">
                Understand the profiler, metrics, and file structure from just
                the first line (header).
              </p>
            </div>

            <div className="rounded-lg border border-border bg-card p-6">
              <div className="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
                <Search className="h-5 w-5 text-primary" />
              </div>
              <h4 className="mb-2 font-mono font-medium">{"grep 'type'"}</h4>
              <p className="text-sm leading-relaxed text-muted-foreground">
                Filter for specific record types — stacks, frames, threads, or
                samples.
              </p>
            </div>

            <div className="rounded-lg border border-border bg-card p-6">
              <div className="mb-4 flex h-10 w-10 items-center justify-center rounded-lg bg-primary/10">
                <FileJson className="h-5 w-5 text-primary" />
              </div>
              <h4 className="mb-2 font-mono font-medium">jq</h4>
              <p className="text-sm leading-relaxed text-muted-foreground">
                Extract exactly what you need with powerful JSON queries and
                transformations.
              </p>
            </div>
          </div>
        </div>
      </section>

      {/* Features */}
      <section className="border-b border-border">
        <div className="mx-auto max-w-5xl px-6 py-20">
          <h2 className="mb-4 text-center font-mono text-sm uppercase tracking-wider text-primary">
            Features
          </h2>
          <h3 className="mb-12 text-balance text-center text-2xl font-bold md:text-4xl">
            Built for modern profiling workflows
          </h3>

          <div className="grid gap-8 md:grid-cols-2">
            <div className="flex gap-4">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10">
                <Zap className="h-5 w-5 text-primary" />
              </div>
              <div>
                <h4 className="mb-2 font-medium">Multi-Tool Support</h4>
                <p className="text-sm leading-relaxed text-muted-foreground">
                  Convert from DTrace, Chrome DevTools, perf, and more. Unified
                  format for all your profiling data.
                </p>
              </div>
            </div>

            <div className="flex gap-4">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10">
                <Layers className="h-5 w-5 text-primary" />
              </div>
              <div>
                <h4 className="mb-2 font-medium">Memory Profiling</h4>
                <p className="text-sm leading-relaxed text-muted-foreground">
                  Track allocations, deallocations, and memory leaks with
                  dedicated metrics like{" "}
                  <InlineCode>live_bytes</InlineCode> and{" "}
                  <InlineCode>alloc_count</InlineCode>.
                </p>
              </div>
            </div>

            <div className="flex gap-4">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10">
                <GitBranch className="h-5 w-5 text-primary" />
              </div>
              <div>
                <h4 className="mb-2 font-medium">Content-Addressable Stacks</h4>
                <p className="text-sm leading-relaxed text-muted-foreground">
                  Deterministic stack IDs enable diffing across profile runs for
                  regression detection.
                </p>
              </div>
            </div>

            <div className="flex gap-4">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-lg bg-primary/10">
                <FileJson className="h-5 w-5 text-primary" />
              </div>
              <div>
                <h4 className="mb-2 font-medium">Lossless & Streamable</h4>
                <p className="text-sm leading-relaxed text-muted-foreground">
                  Preserves full fidelity of profiler data. NDJSON format
                  enables single-pass streaming parsers.
                </p>
              </div>
            </div>
          </div>
        </div>
      </section>

      {/* Agent Skill */}
      <section className="border-b border-border bg-card/30">
        <div className="mx-auto max-w-5xl px-6 py-20">
          <h2 className="mb-4 text-center font-mono text-sm uppercase tracking-wider text-primary">
            For AI Agents
          </h2>
          <h3 className="mb-6 text-balance text-center text-2xl font-bold md:text-4xl">
            Install the SPAA Agent Skill
          </h3>
          <p className="mx-auto mb-8 max-w-2xl text-pretty text-center text-lg leading-relaxed text-muted-foreground">
            Give your AI coding agent the ability to analyze performance
            profiles with a single command.
          </p>

          <div className="mx-auto max-w-md">
            <CodeBlock code="npx skills add andrewimm/spaa" />
          </div>

          <div className="mt-12 grid gap-4 text-center md:grid-cols-3">
            <div className="text-sm text-muted-foreground">
              Works with <span className="text-foreground">Claude Code</span>
            </div>
            <div className="text-sm text-muted-foreground">
              Works with <span className="text-foreground">Cursor</span>
            </div>
            <div className="text-sm text-muted-foreground">
              Works with{" "}
              <span className="text-foreground">skills-compatible agents</span>
            </div>
          </div>
        </div>
      </section>

      {/* Example */}
      <section className="border-b border-border">
        <div className="mx-auto max-w-5xl px-6 py-20">
          <h2 className="mb-4 text-center font-mono text-sm uppercase tracking-wider text-primary">
            Example
          </h2>
          <h3 className="mb-12 text-balance text-center text-2xl font-bold md:text-4xl">
            See what SPAA looks like
          </h3>

          <div className="space-y-8">
            <div>
              <h4 className="mb-3 font-mono text-sm font-medium text-muted-foreground">
                Header Record
              </h4>
              <pre className="overflow-x-auto rounded-lg border border-border bg-card p-4 font-mono text-sm leading-relaxed">
                <code>
                  {`{
  "type": "header",
  "format": "spaa",
  "version": "1.0",
  "source_tool": "dtrace",           `}
                  <span className="text-muted-foreground">
                    {"// Tool that generated the original profile"}
                  </span>
                  {`
  "frame_order": "leaf_to_root",     `}
                  <span className="text-muted-foreground">
                    {"// How frames[] are ordered in stack records"}
                  </span>
                  {`
  "events": [{
    "name": "profile-997",           `}
                  <span className="text-muted-foreground">
                    {"// Referenced by stack.context.event"}
                  </span>
                  {`
    "kind": "timer",
    "sampling": {
      "mode": "frequency",
      "primary_metric": "samples",   `}
                  <span className="text-muted-foreground">
                    {"// Authoritative weight for this event"}
                  </span>
                  {`
      "frequency_hz": 997
    }
  }]
}`}
                </code>
              </pre>
            </div>

            <div>
              <h4 className="mb-3 font-mono text-sm font-medium text-muted-foreground">
                Stack Record
              </h4>
              <pre className="overflow-x-auto rounded-lg border border-border bg-card p-4 font-mono text-sm leading-relaxed">
                <code>
                  {`{
  "type": "stack",
  "id": "0xabc123",                  `}
                  <span className="text-muted-foreground">
                    {"// Content-addressable ID for cross-file diffing"}
                  </span>
                  {`
  "frames": [101, 77, 42],           `}
                  <span className="text-muted-foreground">
                    {"// References frame.id records (leaf first)"}
                  </span>
                  {`
  "context": {
    "event": "profile-997",          `}
                  <span className="text-muted-foreground">
                    {"// References header.events[].name"}
                  </span>
                  {`
    "tid": 1234                      `}
                  <span className="text-muted-foreground">
                    {"// References thread.tid (if present)"}
                  </span>
                  {`
  },
  "weights": [{
    "metric": "samples",             `}
                  <span className="text-muted-foreground">
                    {"// Must match event's primary_metric"}
                  </span>
                  {`
    "value": 847
  }]
}`}
                </code>
              </pre>
            </div>

            <div>
              <h4 className="mb-3 font-mono text-sm font-medium text-muted-foreground">
                Frame Record
              </h4>
              <pre className="overflow-x-auto rounded-lg border border-border bg-card p-4 font-mono text-sm leading-relaxed">
                <code>
                  {`{
  "type": "frame",
  "id": 101,                         `}
                  <span className="text-muted-foreground">
                    {"// Referenced by stack.frames[]"}
                  </span>
                  {`
  "func": "mycrate::parse::parse_file",
  "dso": 12,                         `}
                  <span className="text-muted-foreground">
                    {"// References dso.id record"}
                  </span>
                  {`
  "ip": "0x401234",                  `}
                  <span className="text-muted-foreground">
                    {"// Instruction pointer address"}
                  </span>
                  {`
  "srcline": "src/parse.rs:214",     `}
                  <span className="text-muted-foreground">
                    {"// Source location (file:line)"}
                  </span>
                  {`
  "kind": "user"                     `}
                  <span className="text-muted-foreground">
                    {"// user | kernel | native | jit | interpreted"}
                  </span>
                  {`
}`}
                </code>
              </pre>
            </div>
          </div>
        </div>
      </section>

      {/* CTA */}
      <section className="border-b border-border">
        <div className="mx-auto max-w-5xl px-6 py-20 text-center">
          <h2 className="mb-6 text-2xl font-bold md:text-4xl">
            Ready to get started?
          </h2>
          <p className="mx-auto mb-10 max-w-xl text-lg text-muted-foreground">
            Install SPAA and start converting your profiling data into an
            AI-friendly format today.
          </p>
          <div className="flex flex-col items-center justify-center gap-4 sm:flex-row">
            <Link
              href="/usage"
              className="inline-flex items-center gap-2 rounded-lg bg-primary px-6 py-3 font-mono text-sm font-medium text-primary-foreground transition-colors hover:bg-primary/90"
            >
              Installation Guide
              <ArrowRight className="h-4 w-4" />
            </Link>
            <a
              href="https://github.com/andrewimm/spaa"
              target="_blank"
              rel="noopener noreferrer"
              className="inline-flex items-center gap-2 rounded-lg border border-border bg-secondary px-6 py-3 font-mono text-sm font-medium text-foreground transition-colors hover:bg-secondary/80"
            >
              View on GitHub
            </a>
          </div>
        </div>
      </section>

      {/* Footer */}
      <footer className="bg-card/30">
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
              <Link
                href="/spec"
                className="font-mono text-sm text-muted-foreground transition-colors hover:text-primary"
              >
                Spec
              </Link>
              <Link
                href="/usage"
                className="font-mono text-sm text-muted-foreground transition-colors hover:text-primary"
              >
                Usage
              </Link>
            </div>
          </div>
        </div>
      </footer>
    </div>
  );
}
