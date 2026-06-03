//! Convert source input (Plaintext / Markdown / HTML) into Typst body markup,
//! applying user page-break rules first.

use std::fmt::Write as _;

use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd,
};

use crate::config::{BreakPosition, InputFormat, PageBreakRule, TypesetInput};

/// Characters that carry markup meaning in Typst and must be backslash-escaped
/// when emitting literal text. Brackets are included so literal `[`/`]` don't
/// open or close content blocks (notably inside table cells and links).
const INLINE_SPECIALS: &[char] = &[
    '\\', '`', '*', '_', '#', '$', '<', '>', '@', '~', '=', '-', '+', '[', ']',
];

/// Convert an input document to a Typst body, inserting `#pagebreak()` at the
/// boundaries produced by `rules`. `smart` enables Markdown smart punctuation
/// (typographic dashes/ellipses); quote handling is set in the template.
pub fn to_typst_body(input: &TypesetInput, rules: &[PageBreakRule], smart: bool) -> String {
    let pages = paginate(&input.text, rules);
    let chunks: Vec<String> = pages
        .iter()
        .map(|p| convert(p, input.format, smart))
        .collect();
    chunks.join("\n\n#pagebreak()\n\n")
}

fn convert(text: &str, format: InputFormat, smart: bool) -> String {
    match format {
        InputFormat::Markdown => markdown_to_typst(text, smart),
        InputFormat::Plaintext => plaintext_to_typst(text),
        InputFormat::Html => plaintext_to_typst(&html_to_text(text)),
    }
}

// =============================================================================
// Page-break splitting
// =============================================================================

fn line_matches(line: &str, pattern: &str) -> bool {
    let pattern = pattern.trim();
    !pattern.is_empty() && line.trim() == pattern
}

/// Split source text into page chunks at lines matching a rule.
fn paginate(text: &str, rules: &[PageBreakRule]) -> Vec<String> {
    let mut pages: Vec<String> = vec![String::new()];
    for line in text.lines() {
        match rules.iter().find(|r| line_matches(line, &r.pattern)) {
            Some(rule) => match rule.position {
                BreakPosition::Replace => pages.push(String::new()),
                BreakPosition::Before => {
                    pages.push(String::new());
                    push_line(&mut pages, line);
                }
                BreakPosition::After => {
                    push_line(&mut pages, line);
                    pages.push(String::new());
                }
            },
            None => push_line(&mut pages, line),
        }
    }
    let out: Vec<String> = pages
        .into_iter()
        .filter(|p| !p.trim().is_empty())
        .collect();
    if out.is_empty() { vec![String::new()] } else { out }
}

fn push_line(pages: &mut [String], line: &str) {
    if let Some(last) = pages.last_mut() {
        last.push_str(line);
        last.push('\n');
    }
}

// =============================================================================
// Plaintext → Typst
// =============================================================================

fn escape_inline(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if INLINE_SPECIALS.contains(&ch) {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

/// Escape one plaintext line: inline specials everywhere, plus a leading numbered
/// enumerator separator (`1.` / `2)`) so it isn't read as a Typst list.
fn escape_plaintext_line(line: &str) -> String {
    let chars: Vec<char> = line.chars().collect();
    let lead = chars.iter().take_while(|c| c.is_whitespace()).count();
    let digits = chars[lead..].iter().take_while(|c| c.is_ascii_digit()).count();

    let mut enum_sep: Option<usize> = None;
    if digits > 0 {
        let sep_i = lead + digits;
        if let Some(&sep) = chars.get(sep_i)
            && (sep == '.' || sep == ')')
            && chars.get(sep_i + 1).is_none_or(|c| *c == ' ')
        {
            enum_sep = Some(sep_i);
        }
    }

    let mut out = String::with_capacity(chars.len() + 4);
    for (i, &ch) in chars.iter().enumerate() {
        if INLINE_SPECIALS.contains(&ch) || Some(i) == enum_sep {
            out.push('\\');
        }
        out.push(ch);
    }
    out
}

fn plaintext_to_typst(text: &str) -> String {
    // Escape each line; blank lines survive so Typst sees paragraph breaks.
    let mut out = String::new();
    for line in text.lines() {
        out.push_str(&escape_plaintext_line(line));
        out.push('\n');
    }
    out
}

// =============================================================================
// Markdown → Typst
// =============================================================================

fn heading_level_num(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn markdown_to_typst(md: &str, smart: bool) -> String {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TABLES);
    if smart {
        opts.insert(Options::ENABLE_SMART_PUNCTUATION);
    }
    let parser = Parser::new_ext(md, opts);

    let mut out = String::new();
    // For each open list: the running ordered index, or None for a bullet list.
    let mut list_stack: Vec<Option<u64>> = Vec::new();
    let mut in_code_block = false;
    // Set while accumulating a Markdown table; cells contain only inline content.
    let mut table: Option<TableBuilder> = None;

    for event in parser {
        // Inside a table, inline content is routed into the current cell and the
        // structural events build up the grid; everything else is suppressed.
        if let Some(tb) = table.as_mut() {
            if tb.handle(&event) {
                continue;
            }
            // End(Table) falls through here so we can flush and clear the state.
            if matches!(event, Event::End(TagEnd::Table)) {
                out.push_str(&table.take().expect("in table").render());
                continue;
            }
        }

        match event {
            Event::Start(Tag::Table(aligns)) => table = Some(TableBuilder::new(aligns)),
            Event::Start(Tag::Heading { level, .. }) => {
                out.push('\n');
                for _ in 0..heading_level_num(level) {
                    out.push('=');
                }
                out.push(' ');
            }
            Event::End(TagEnd::Heading(_) | TagEnd::Paragraph) => {
                out.push_str("\n\n");
            }

            Event::Start(Tag::Emphasis) | Event::End(TagEnd::Emphasis) => out.push('_'),
            Event::Start(Tag::Strong) | Event::End(TagEnd::Strong) => out.push('*'),
            Event::Start(Tag::Strikethrough) => out.push_str("#strike["),

            Event::Start(Tag::BlockQuote(_)) => out.push_str("#quote(block: true)[\n"),
            Event::End(TagEnd::BlockQuote(_)) => out.push_str("]\n\n"),

            Event::Start(Tag::CodeBlock(kind)) => {
                in_code_block = true;
                let lang = match kind {
                    CodeBlockKind::Fenced(l) => l.to_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                out.push_str("```");
                out.push_str(&lang);
                out.push('\n');
            }
            Event::End(TagEnd::CodeBlock) => {
                in_code_block = false;
                out.push_str("```\n\n");
            }

            Event::Start(Tag::List(start)) => list_stack.push(start),
            Event::End(TagEnd::List(_)) => {
                list_stack.pop();
                if list_stack.is_empty() {
                    out.push('\n');
                }
            }
            Event::Start(Tag::Item) => {
                let depth = list_stack.len().saturating_sub(1);
                out.push('\n');
                for _ in 0..depth {
                    out.push_str("  ");
                }
                match list_stack.last_mut() {
                    Some(Some(n)) => {
                        let _ = write!(out, "{n}. ");
                        *n += 1;
                    }
                    _ => out.push_str("- "),
                }
            }

            Event::Start(Tag::Link { dest_url, .. }) => {
                let _ = write!(out, "#link(\"{}\")[", escape_url(&dest_url));
            }
            // Close both inline wrappers (strikethrough and links) with `]`.
            // Images can't be resolved from an in-memory compile, so their alt
            // text falls through to normal text via the catch-all.
            Event::End(TagEnd::Strikethrough | TagEnd::Link) => out.push(']'),

            Event::Text(t) => {
                if in_code_block {
                    out.push_str(&t);
                } else {
                    out.push_str(&escape_inline(&t));
                }
            }
            Event::Code(t) => {
                out.push('`');
                out.push_str(&t);
                out.push('`');
            }
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push_str(" \\\n"),
            Event::Rule => out.push_str("\n#line(length: 100%)\n\n"),

            _ => {}
        }
    }
    out
}

fn escape_url(url: &str) -> String {
    url.replace('\\', "\\\\").replace('"', "\\\"")
}

// =============================================================================
// Markdown tables → Typst `#table`
// =============================================================================

/// Accumulates a Markdown table's cells, then renders a Typst `#table`. Cell
/// content is inline-only Typst markup; visual styling (borders, header shading,
/// zebra striping) is applied globally by the template's `#set table` rules.
struct TableBuilder {
    aligns: Vec<Alignment>,
    rows: Vec<Vec<String>>,
    cur_row: Vec<String>,
    cur_cell: String,
    /// Number of leading rows that form the header (wrapped in `table.header`).
    header_rows: usize,
}

impl TableBuilder {
    fn new(aligns: Vec<Alignment>) -> Self {
        Self {
            aligns,
            rows: Vec::new(),
            cur_row: Vec::new(),
            cur_cell: String::new(),
            header_rows: 0,
        }
    }

    /// Feed one parser event, routing inline content into the current cell and
    /// building up the grid from the structural events. Returns `true` if the
    /// event was consumed; only `End(Table)` returns `false`, signalling the
    /// caller to flush and render. Structural row/head starts and any stray
    /// events fall through the catch-all (consumed, no effect).
    fn handle(&mut self, event: &Event) -> bool {
        match event {
            Event::End(TagEnd::Table) => return false, // caller flushes and renders
            Event::End(TagEnd::TableHead) => {
                self.finish_row();
                self.header_rows = self.rows.len();
            }
            Event::End(TagEnd::TableRow) => self.finish_row(),
            Event::Start(Tag::TableCell) => self.cur_cell.clear(),
            Event::End(TagEnd::TableCell) => self.finish_cell(),

            // Inline content routed into the current cell.
            Event::Text(t) => self.cur_cell.push_str(&escape_inline(t)),
            Event::Code(t) => {
                self.cur_cell.push('`');
                self.cur_cell.push_str(t);
                self.cur_cell.push('`');
            }
            Event::Start(Tag::Emphasis) | Event::End(TagEnd::Emphasis) => self.cur_cell.push('_'),
            Event::Start(Tag::Strong) | Event::End(TagEnd::Strong) => self.cur_cell.push('*'),
            Event::Start(Tag::Strikethrough) => self.cur_cell.push_str("#strike["),
            Event::End(TagEnd::Strikethrough | TagEnd::Link) => self.cur_cell.push(']'),
            Event::Start(Tag::Link { dest_url, .. }) => {
                let _ = write!(self.cur_cell, "#link(\"{}\")[", escape_url(dest_url));
            }
            Event::SoftBreak | Event::HardBreak => self.cur_cell.push(' '),
            _ => {}
        }
        true
    }

    fn finish_cell(&mut self) {
        let cell = std::mem::take(&mut self.cur_cell);
        self.cur_row.push(cell.trim().to_string());
    }

    fn finish_row(&mut self) {
        if !self.cur_row.is_empty() {
            self.rows.push(std::mem::take(&mut self.cur_row));
        }
    }

    fn render(&self) -> String {
        let cols = self
            .aligns
            .len()
            .max(self.rows.iter().map(Vec::len).max().unwrap_or(0))
            .max(1);

        let mut s = String::from("\n#table(\n");
        let _ = writeln!(s, "  columns: {cols},");

        s.push_str("  align: (");
        for i in 0..cols {
            let a = self.aligns.get(i).copied().unwrap_or(Alignment::None);
            s.push_str(align_to_typst(a));
            if i + 1 < cols {
                s.push_str(", ");
            }
        }
        s.push_str("),\n");

        let header_n = self.header_rows.min(self.rows.len());
        if header_n > 0 {
            s.push_str("  table.header(\n");
            for row in &self.rows[..header_n] {
                push_cells(&mut s, row);
            }
            s.push_str("  ),\n");
        }
        for row in &self.rows[header_n..] {
            push_cells(&mut s, row);
        }
        s.push_str(")\n\n");
        s
    }
}

fn push_cells(s: &mut String, row: &[String]) {
    s.push_str("  ");
    for cell in row {
        let _ = write!(s, "[{cell}], ");
    }
    s.push('\n');
}

fn align_to_typst(a: Alignment) -> &'static str {
    match a {
        Alignment::Left | Alignment::None => "left",
        Alignment::Center => "center",
        Alignment::Right => "right",
    }
}

// =============================================================================
// HTML → text (basic; structured HTML support is a follow-up)
// =============================================================================

/// Strip HTML tags to readable text, turning block-level tags into blank lines
/// and decoding common entities. Good enough for simple HTML; rich structure
/// (bold/headings) is a planned follow-up using a real HTML parser.
fn html_to_text(html: &str) -> String {
    const BLOCK_TAGS: &[&str] = &[
        "p", "div", "br", "h1", "h2", "h3", "h4", "h5", "h6", "li", "ul", "ol", "blockquote",
        "section", "article", "header", "footer", "pre", "table", "tr",
    ];

    let mut out = String::new();
    let mut chars = html.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '<' {
            // Consume the tag.
            let mut tag = String::new();
            for c in chars.by_ref() {
                if c == '>' {
                    break;
                }
                tag.push(c);
            }
            let name: String = tag
                .trim_start_matches('/')
                .chars()
                .take_while(char::is_ascii_alphanumeric)
                .collect::<String>()
                .to_ascii_lowercase();
            if BLOCK_TAGS.contains(&name.as_str()) {
                out.push_str("\n\n");
            }
        } else {
            out.push(ch);
        }
    }
    decode_entities(&out)
}

fn decode_entities(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&apos;", "'")
        .replace("&nbsp;", " ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_table_becomes_typst_table() {
        let md = "\
| Name | Qty |
|:-----|----:|
| Ink  | 3   |
| Paper| 12  |
";
        let out = markdown_to_typst(md, false);
        assert!(out.contains("#table("), "expected a #table call:\n{out}");
        assert!(out.contains("columns: 2"), "two columns expected:\n{out}");
        // Left/right alignment carried from the `:---`/`---:` delimiter row.
        assert!(out.contains("align: (left, right)"), "alignment:\n{out}");
        // Header row is wrapped so the template can style it.
        assert!(out.contains("table.header("), "header wrapper:\n{out}");
        assert!(out.contains("[Name]") && out.contains("[Paper]"), "cells:\n{out}");
    }

    #[test]
    fn table_cell_brackets_are_escaped() {
        let md = "| A |\n|---|\n| x[y] |\n";
        let out = markdown_to_typst(md, false);
        assert!(out.contains("x\\[y\\]"), "brackets must be escaped:\n{out}");
    }

    #[test]
    fn smart_punctuation_toggles_dashes() {
        let plain = markdown_to_typst("a -- b", false);
        let smart = markdown_to_typst("a -- b", true);
        assert_ne!(plain, smart, "smart punctuation should change the output");
    }
}
