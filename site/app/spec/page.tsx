import { Navigation } from "@/components/navigation";
import { MarkdownRenderer } from "@/components/markdown-renderer";
import type { Metadata } from "next";
import { promises as fs } from "fs";
import path from "path";

export const metadata: Metadata = {
  title: "SPAA Specification",
  description:
    "Complete technical specification for the Stack Profile for Agentic Analysis file format.",
};

async function getSpecContent() {
  const filePath = path.join(process.cwd(), "..", "SPEC.md");
  const content = await fs.readFile(filePath, "utf-8");
  return content;
}

export default async function SpecPage() {
  const specContent = await getSpecContent();

  return (
    <div className="min-h-screen">
      <Navigation />

      <main className="mx-auto max-w-4xl px-6 py-16">
        {/* Header */}
        <div className="mb-16 border-b border-border pb-8">
          <p className="mb-2 font-mono text-sm text-primary">Specification</p>
          <h1 className="mb-4 text-3xl font-bold md:text-4xl">
            SPAA Specification v1.0
          </h1>
          <p className="text-lg text-muted-foreground">
            Stack Profile for Agentic Analysis
          </p>
        </div>

        <MarkdownRenderer content={specContent} />
      </main>

      {/* Footer */}
      <footer className="border-t border-border py-8">
        <div className="mx-auto max-w-4xl px-6 text-center text-sm text-muted-foreground">
          <p>
            SPAA is an open specification.{" "}
            <a
              href="https://github.com/andrewimm/spaa"
              className="text-primary hover:underline"
            >
              Contribute on GitHub
            </a>
          </p>
        </div>
      </footer>
    </div>
  );
}
