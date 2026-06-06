//! Per-fragment compile validation.
//!
//! A converted equation is accepted only if it actually compiles as `Typst`;
//! this is what lets the pipeline fall back per equation instead of letting one
//! bad formula break the whole document.

use typst::layout::PagedDocument;
use typst_as_lib::typst_kit_options::TypstKitFontOptions;

use crate::build_engine;

/// True if `math` (the contents of `$...$`) compiles as valid `Typst`.
///
/// Uses embedded fonts only — a deterministic math font with no system-font
/// scan, so per-equation validation stays cheap. Glyph availability doesn't
/// affect compile success, so this is a sound proxy for the real render.
pub fn compiles(math: &str, display: bool) -> bool {
    let (open, close) = if display { ("$ ", " $") } else { ("$", "$") };
    let source =
        format!("#set page(width: auto, height: auto, margin: 2pt)\n{open}{math}{close}\n");
    let fonts = TypstKitFontOptions::default().include_system_fonts(false);
    build_engine(source, fonts, &[])
        .compile::<PagedDocument>()
        .output
        .is_ok()
}
