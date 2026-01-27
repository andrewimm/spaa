"use client";

import { useMemo } from "react";

interface MarkdownRendererProps {
  content: string;
}

interface TocItem {
  id: string;
  title: string;
  level: number;
}

export function MarkdownRenderer({ content }: MarkdownRendererProps) {
  const { html, toc } = useMemo(() => parseMarkdown(content), [content]);

  return (
    <div className="flex flex-col gap-16">
      {/* Table of Contents */}
      {toc.length > 0 && (
        <nav className="rounded-lg border border-border bg-card p-6">
          <h2 className="mb-4 font-mono text-sm font-medium uppercase tracking-wider text-primary">
            Contents
          </h2>
          <ol className="grid gap-2 font-mono text-sm md:grid-cols-2">
            {toc
              .filter((item) => item.level === 2)
              .map((item) => (
                <li key={item.id}>
                  <a
                    href={`#${item.id}`}
                    className="text-muted-foreground transition-colors hover:text-primary"
                  >
                    {item.title}
                  </a>
                </li>
              ))}
          </ol>
        </nav>
      )}

      {/* Rendered Content */}
      <div
        className="prose-custom"
        // biome-ignore lint/security/noDangerouslySetInnerHtml: markdown rendering
        dangerouslySetInnerHTML={{ __html: html }}
      />
    </div>
  );
}

function parseMarkdown(markdown: string): { html: string; toc: TocItem[] } {
  const toc: TocItem[] = [];
  const lines = markdown.split("\n");
  const htmlLines: string[] = [];
  let inCodeBlock = false;
  let codeBlockContent: string[] = [];
  let codeBlockLang = "";
  let inList = false;
  let listItems: string[] = [];
  let inTable = false;
  let tableRows: string[] = [];

  const flushList = () => {
    if (listItems.length > 0) {
      htmlLines.push(
        `<ul class="mb-6 list-inside list-disc space-y-2 text-muted-foreground">${listItems.join("")}</ul>`
      );
      listItems = [];
      inList = false;
    }
  };

  const flushTable = () => {
    if (tableRows.length > 0) {
      const headerRow = tableRows[0];
      const bodyRows = tableRows.slice(2); // Skip header and separator
      let tableHtml = `<div class="overflow-x-auto mb-6"><table class="w-full border-collapse text-sm">`;
      tableHtml += `<thead><tr class="border-b border-border">${headerRow}</tr></thead>`;
      tableHtml += `<tbody>${bodyRows.join("")}</tbody>`;
      tableHtml += `</table></div>`;
      htmlLines.push(tableHtml);
      tableRows = [];
      inTable = false;
    }
  };

  const parseInline = (text: string): string => {
    // Code (must be before bold to handle `code` inside **bold**)
    text = text.replace(
      /`([^`]+)`/g,
      '<code class="rounded bg-secondary px-1.5 py-0.5 font-mono text-sm text-primary">$1</code>'
    );
    // Bold
    text = text.replace(
      /\*\*([^*]+)\*\*/g,
      '<strong class="text-foreground">$1</strong>'
    );
    // Italic
    text = text.replace(/\*([^*]+)\*/g, "<em>$1</em>");
    // Links
    text = text.replace(
      /\[([^\]]+)\]\(([^)]+)\)/g,
      '<a href="$2" class="text-primary hover:underline">$1</a>'
    );
    return text;
  };

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];

    // Code blocks
    if (line.startsWith("```")) {
      if (inCodeBlock) {
        // End code block
        const escapedCode = codeBlockContent
          .join("\n")
          .replace(/&/g, "&amp;")
          .replace(/</g, "&lt;")
          .replace(/>/g, "&gt;");
        htmlLines.push(`
          <div class="group relative mb-6">
            <pre class="overflow-x-auto rounded-lg border border-border bg-card p-4 font-mono text-sm leading-relaxed"><code>${escapedCode}</code></pre>
          </div>
        `);
        codeBlockContent = [];
        inCodeBlock = false;
      } else {
        // Start code block
        flushList();
        flushTable();
        codeBlockLang = line.slice(3).trim();
        inCodeBlock = true;
      }
      continue;
    }

    if (inCodeBlock) {
      codeBlockContent.push(line);
      continue;
    }

    // Horizontal rules
    if (line.match(/^---+$/)) {
      flushList();
      flushTable();
      continue;
    }

    // Tables
    if (line.includes("|") && line.trim().startsWith("|")) {
      flushList();
      if (!inTable) {
        inTable = true;
      }

      // Check if separator row
      if (line.match(/^\|[\s-:|]+\|$/)) {
        tableRows.push("separator");
        continue;
      }

      const cells = line
        .split("|")
        .filter((c) => c.trim() !== "")
        .map((c) => c.trim());

      if (tableRows.length === 0) {
        // Header row
        tableRows.push(
          cells
            .map(
              (c) =>
                `<th class="p-3 text-left font-mono font-medium text-muted-foreground">${parseInline(c)}</th>`
            )
            .join("")
        );
      } else {
        // Body row
        tableRows.push(
          `<tr class="border-b border-border">${cells.map((c) => `<td class="p-3 text-muted-foreground">${parseInline(c)}</td>`).join("")}</tr>`
        );
      }
      continue;
    }

    if (inTable && !line.includes("|")) {
      flushTable();
    }

    // Headers
    const headerMatch = line.match(/^(#{1,4})\s+(.+)$/);
    if (headerMatch) {
      flushList();
      flushTable();
      const level = headerMatch[1].length;
      const title = headerMatch[2];
      const id = title
        .toLowerCase()
        .replace(/[^a-z0-9\s]/g, "")
        .replace(/\s+/g, "-");

      toc.push({ id, title, level });

      const classes: Record<number, string> = {
        1: "mb-4 text-3xl font-bold md:text-4xl",
        2: "mb-6 mt-16 border-b border-border pb-2 text-2xl font-bold",
        3: "mb-4 mt-10 text-xl font-semibold",
        4: "mb-3 mt-6 font-semibold",
      };

      htmlLines.push(
        `<h${level} id="${id}" class="${classes[level]}">${parseInline(title)}</h${level}>`
      );
      continue;
    }

    // Lists
    if (line.match(/^\s*\*\s+/) || line.match(/^\s*-\s+/)) {
      flushTable();
      inList = true;
      const content = line.replace(/^\s*[\*-]\s+/, "");
      listItems.push(`<li>${parseInline(content)}</li>`);
      continue;
    }

    // Flush list if we hit a non-list line
    if (inList && line.trim() !== "") {
      flushList();
    }

    // Empty lines
    if (line.trim() === "") {
      if (inList) {
        flushList();
      }
      continue;
    }

    // Paragraphs
    flushList();
    flushTable();
    htmlLines.push(
      `<p class="mb-4 leading-relaxed text-muted-foreground">${parseInline(line)}</p>`
    );
  }

  // Flush any remaining content
  flushList();
  flushTable();

  return { html: htmlLines.join("\n"), toc };
}
