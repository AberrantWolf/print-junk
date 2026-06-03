//! Public configuration surface for typesetting.

use pdf_units::{Orientation, PaperSize};
use serde::{Deserialize, Serialize};

/// Source document format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum InputFormat {
    /// `CommonMark` / `GitHub`-flavored Markdown.
    #[default]
    Markdown,
    /// Raw text; blank lines separate paragraphs.
    Plaintext,
    /// HTML markup.
    Html,
}

impl InputFormat {
    /// Best-guess format from a file extension (case-insensitive).
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "md" | "markdown" | "mdown" | "mkd" => Some(Self::Markdown),
            "txt" | "text" => Some(Self::Plaintext),
            "html" | "htm" | "xhtml" => Some(Self::Html),
            _ => None,
        }
    }
}

/// The text to typeset, plus its format.
#[derive(Debug, Clone)]
pub struct TypesetInput {
    pub text: String,
    pub format: InputFormat,
}

/// Where a page break falls relative to a line matching a [`PageBreakRule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum BreakPosition {
    /// Break after the matched line; the line stays on the current page.
    #[default]
    After,
    /// Break before the matched line; the line starts the next page.
    Before,
    /// Drop the matched line and break in its place.
    Replace,
}

/// A user-defined page break: when a line equals `pattern` (trimmed), insert a
/// page break before/after it or in place of it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageBreakRule {
    pub pattern: String,
    pub position: BreakPosition,
}

impl PageBreakRule {
    pub fn new(pattern: impl Into<String>, position: BreakPosition) -> Self {
        Self {
            pattern: pattern.into(),
            position,
        }
    }
}

/// A font selection. An empty `family` means "use the engine's default font".
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FontChoice {
    /// System font family name (e.g. "EB Garamond"). Empty = engine default.
    pub family: String,
    pub size_pt: f32,
}

impl FontChoice {
    pub fn new(family: impl Into<String>, size_pt: f32) -> Self {
        Self {
            family: family.into(),
            size_pt,
        }
    }
}

/// An sRGB color. Serializes as `{ r, g, b }`; `Default` is black.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Color {
    pub const BLACK: Self = Self { r: 0, g: 0, b: 0 };

    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// True when fully black — used to skip emitting redundant `fill:` rules.
    pub fn is_black(self) -> bool {
        self.r == 0 && self.g == 0 && self.b == 0
    }

    /// Render as a Typst `rgb(r, g, b)` color literal.
    pub fn to_typst(self) -> String {
        format!("rgb({}, {}, {})", self.r, self.g, self.b)
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::BLACK
    }
}

/// Horizontal alignment for headings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum HAlign {
    #[default]
    Left,
    Center,
    Right,
}

impl HAlign {
    /// Render as a Typst alignment keyword.
    pub fn to_typst(self) -> &'static str {
        match self {
            Self::Left => "left",
            Self::Center => "center",
            Self::Right => "right",
        }
    }
}

/// Per-level heading style. An empty `family` inherits the body font.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadingStyle {
    pub family: String,
    pub size_pt: f32,
    pub bold: bool,
    pub italic: bool,
    pub color: Color,
    pub align: HAlign,
    /// Space before the heading block, in millimeters.
    pub space_above_mm: f32,
    /// Space after the heading block, in millimeters.
    pub space_below_mm: f32,
    /// Begin this heading on a fresh page (chapter opener).
    pub start_new_page: bool,
}

impl HeadingStyle {
    /// The default style for heading `level` (1..=6): bold black, left-aligned,
    /// sized from a 14pt top level scaled down per level (never below 10.5pt).
    pub fn for_level(level: u8) -> Self {
        let scale = 0.84_f32.powi(i32::from(level) - 1);
        Self {
            family: String::new(),
            size_pt: (14.0 * scale).max(10.5),
            bold: true,
            italic: false,
            color: Color::BLACK,
            align: HAlign::Left,
            space_above_mm: if level <= 2 { 6.0 } else { 4.0 },
            space_below_mm: 3.0,
            start_new_page: false,
        }
    }
}

/// Which borders a table draws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TableBorder {
    /// Full grid (rows and columns).
    #[default]
    All,
    /// Horizontal rules only (between rows).
    Horizontal,
    /// No borders.
    None,
}

/// Styling applied to every Markdown table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TableStyle {
    pub header_bold: bool,
    /// Fill behind the header row, if any.
    pub header_fill: Option<Color>,
    pub border: TableBorder,
    pub border_width_pt: f32,
    pub border_color: Color,
    /// Padding inside each cell, in millimeters.
    pub cell_padding_mm: f32,
    /// Alternating-row shading for body rows, if any.
    pub zebra_fill: Option<Color>,
}

impl Default for TableStyle {
    fn default() -> Self {
        Self {
            header_bold: true,
            header_fill: Some(Color::new(230, 230, 230)),
            border: TableBorder::All,
            border_width_pt: 0.5,
            border_color: Color::new(120, 120, 120),
            cell_padding_mm: 1.8,
            zebra_fill: None,
        }
    }
}

/// Full typesetting configuration. Produces a Typst template when rendered.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct TypesetConfig {
    pub page_size: PaperSize,
    pub orientation: Orientation,

    /// Margins in millimeters. `inner` is the binding/spine side (larger by
    /// default so the text block clears the gutter once bound); `outer` is the
    /// fore-edge side. On the recto/verso these map to left/right automatically
    /// via Typst's `binding`-aware margins.
    pub margin_top_mm: f32,
    pub margin_bottom_mm: f32,
    pub margin_inner_mm: f32,
    pub margin_outer_mm: f32,

    pub body_font: FontChoice,
    /// Per-level heading styles, H1..H6 (index 0 = level 1).
    pub heading_styles: [HeadingStyle; 6],

    /// Text colors for body, links, and inline/block code.
    pub body_color: Color,
    pub link_color: Color,
    pub code_color: Color,
    /// Fill behind code blocks, if any.
    pub code_background: Option<Color>,

    /// Leading (extra space between lines) as a multiple of font size (em).
    pub line_spacing_em: f32,
    /// Extra space between paragraphs, in millimeters.
    pub paragraph_spacing_mm: f32,
    /// First-line paragraph indent, in millimeters (book style; 0 disables).
    pub paragraph_indent_mm: f32,

    pub justify: bool,
    pub hyphenate: bool,
    pub page_numbers: bool,

    /// Table styling applied to every Markdown table.
    pub table: TableStyle,

    // --- Front matter & document metadata ---
    /// PDF document title; also rendered as a title page when non-empty.
    pub doc_title: String,
    pub doc_author: String,
    /// Comma-separated keywords for the PDF metadata.
    pub doc_keywords: String,
    /// Generate a table of contents from the headings.
    pub generate_toc: bool,
    /// Deepest heading level included in the table of contents.
    pub toc_depth: u8,
    /// Convert straight quotes/dashes to typographic forms.
    pub smart_punctuation: bool,
    /// Hyphenation / smart-quote language (BCP-47, e.g. "en", "de").
    pub lang: String,

    /// Page-break rules applied to the source before conversion, in order.
    pub page_breaks: Vec<PageBreakRule>,
}

impl TypesetConfig {
    /// The style for heading `level`, clamped into the 1..=6 range.
    pub fn heading_style(&self, level: u8) -> &HeadingStyle {
        let idx = usize::from(level.clamp(1, 6) - 1);
        &self.heading_styles[idx]
    }
}

impl Default for TypesetConfig {
    fn default() -> Self {
        Self {
            // A5 is a common hardcover trade size and a sensible book default.
            page_size: PaperSize::A5,
            orientation: Orientation::Portrait,
            margin_top_mm: 18.0,
            margin_bottom_mm: 20.0,
            // Larger inner (spine/gutter) margin so text clears the binding.
            margin_inner_mm: 20.0,
            margin_outer_mm: 15.0,
            body_font: FontChoice::new("", 10.5),
            heading_styles: std::array::from_fn(|i| HeadingStyle::for_level(i as u8 + 1)),
            body_color: Color::BLACK,
            link_color: Color::new(0, 0, 160),
            code_color: Color::BLACK,
            code_background: Some(Color::new(244, 244, 244)),
            line_spacing_em: 0.65,
            paragraph_spacing_mm: 0.0,
            paragraph_indent_mm: 5.0,
            justify: true,
            hyphenate: true,
            page_numbers: true,
            table: TableStyle::default(),
            doc_title: String::new(),
            doc_author: String::new(),
            doc_keywords: String::new(),
            generate_toc: false,
            toc_depth: 3,
            smart_punctuation: true,
            lang: "en".to_string(),
            page_breaks: vec![
                PageBreakRule::new("<hr>", BreakPosition::Replace),
                PageBreakRule::new("-----", BreakPosition::Replace),
            ],
        }
    }
}
