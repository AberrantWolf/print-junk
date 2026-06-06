//! Shared Typst `#table` emitter.
//!
//! Both the Markdown converter ([`crate::markup`]) and the structured HTML
//! importer ([`crate::html`]) build tables; this module owns the single place
//! that turns a grid model into `#table(...)` markup so visual styling stays
//! uniform (borders, header shading, and zebra striping are applied globally by
//! the template's `#set table` rules). Cell bodies are pre-rendered, inline-only
//! Typst markup; spans map to Typst's native `table.cell(colspan:, rowspan:)`.

use std::fmt::Write as _;

/// Horizontal alignment for a table column.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

impl Align {
    fn keyword(self) -> &'static str {
        match self {
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        }
    }
}

/// One table cell: pre-rendered Typst body markup plus its grid spans. A plain
/// 1×1 cell emits `[body]`; anything larger emits `table.cell(...)[body]`.
pub struct Cell {
    pub body: String,
    pub colspan: usize,
    pub rowspan: usize,
}

impl Cell {
    /// A plain 1×1 cell.
    pub fn new(body: impl Into<String>) -> Self {
        Self {
            body: body.into(),
            colspan: 1,
            rowspan: 1,
        }
    }
}

/// A table to emit: column count, per-column alignment, the number of leading
/// rows that form the header (wrapped in `table.header`), and the rows.
pub struct Table {
    pub columns: usize,
    pub aligns: Vec<Align>,
    pub header_rows: usize,
    pub rows: Vec<Vec<Cell>>,
}

impl Table {
    /// Render the model to a `#table(...)` block (leading/trailing blank lines
    /// included so it stands alone as a Typst block).
    pub fn render(&self) -> String {
        let cols = self.columns.max(1);
        let mut s = String::from("\n#table(\n");
        let _ = writeln!(s, "  columns: {cols},");

        s.push_str("  align: (");
        for i in 0..cols {
            // Columns past the alignment hints default to left.
            s.push_str(self.aligns.get(i).copied().unwrap_or(Align::Left).keyword());
            if i + 1 < cols {
                s.push_str(", ");
            }
        }
        s.push_str("),\n");

        let header_n = self.header_rows.min(self.rows.len());
        if header_n > 0 {
            s.push_str("  table.header(\n");
            for row in &self.rows[..header_n] {
                push_row(&mut s, row);
            }
            s.push_str("  ),\n");
        }
        for row in &self.rows[header_n..] {
            push_row(&mut s, row);
        }
        s.push_str(")\n\n");
        s
    }
}

fn push_row(s: &mut String, row: &[Cell]) {
    s.push_str("  ");
    for cell in row {
        if cell.colspan > 1 || cell.rowspan > 1 {
            s.push_str("table.cell(");
            if cell.colspan > 1 {
                let _ = write!(s, "colspan: {}, ", cell.colspan);
            }
            if cell.rowspan > 1 {
                let _ = write!(s, "rowspan: {}, ", cell.rowspan);
            }
            let _ = write!(s, ")[{}], ", cell.body);
        } else {
            let _ = write!(s, "[{}], ", cell.body);
        }
    }
    s.push('\n');
}
