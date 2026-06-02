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
    pub heading_font: FontChoice,

    /// Leading (extra space between lines) as a multiple of font size (em).
    pub line_spacing_em: f32,
    /// Extra space between paragraphs, in millimeters.
    pub paragraph_spacing_mm: f32,
    /// First-line paragraph indent, in millimeters (book style; 0 disables).
    pub paragraph_indent_mm: f32,

    pub justify: bool,
    pub hyphenate: bool,
    pub page_numbers: bool,

    /// Page-break rules applied to the source before conversion, in order.
    pub page_breaks: Vec<PageBreakRule>,
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
            heading_font: FontChoice::new("", 14.0),
            line_spacing_em: 0.65,
            paragraph_spacing_mm: 0.0,
            paragraph_indent_mm: 5.0,
            justify: true,
            hyphenate: true,
            page_numbers: true,
            page_breaks: vec![
                PageBreakRule::new("<hr>", BreakPosition::Replace),
                PageBreakRule::new("-----", BreakPosition::Replace),
            ],
        }
    }
}
