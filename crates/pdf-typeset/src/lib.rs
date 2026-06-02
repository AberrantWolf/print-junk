//! Lay out a text document (Plaintext / Markdown / HTML) into a typeset PDF.
//!
//! Built on the [Typst](https://typst.app) typesetting engine: input is parsed
//! and emitted as Typst markup ([`markup`]), wrapped in a configurable template
//! ([`template`]) derived from a [`TypesetConfig`], then compiled to PDF bytes.
//!
//! Desktop-only — Typst is a large, native-oriented dependency, so this crate is
//! excluded from the WASM build (gated by the GUI's `typesetting` feature).

mod config;
mod markup;
mod template;

pub use config::{
    BreakPosition, FontChoice, InputFormat, PageBreakRule, TypesetConfig, TypesetInput,
};

use typst::layout::PagedDocument;
use typst_as_lib::TypstEngine;
use typst_as_lib::typst_kit_options::TypstKitFontOptions;

/// Errors produced while typesetting.
#[derive(Debug, thiserror::Error)]
pub enum TypesetError {
    #[error("Typst compilation failed: {0}")]
    Compile(String),
    #[error("PDF export failed: {0}")]
    Pdf(String),
}

/// Preferred body fonts, in order, used when no font is chosen. The first one
/// installed on the system wins; if none are present, Typst's own default is used.
const DEFAULT_SERIF_PREFERENCES: &[&str] = &[
    "Libertinus Serif",
    "EB Garamond",
    "Georgia",
    "Times New Roman",
    "Liberation Serif",
    "Source Serif 4",
    "Noto Serif",
    "DejaVu Serif",
];

/// Typeset `input` into PDF bytes according to `config`.
pub fn typeset(input: &TypesetInput, config: &TypesetConfig) -> Result<Vec<u8>, TypesetError> {
    // When no body font is chosen (system-fonts-only), resolve a sensible serif
    // from the installed set so output isn't a monospace fallback. Headings left
    // empty inherit the body font.
    let mut effective = config.clone();
    if effective.body_font.family.trim().is_empty() {
        let families = available_font_families();
        if let Some(serif) = DEFAULT_SERIF_PREFERENCES
            .iter()
            .find(|pref| families.iter().any(|f| f.eq_ignore_ascii_case(pref)))
        {
            effective.body_font.family = (*serif).to_string();
        }
    }

    let body = markup::to_typst_body(input, &effective.page_breaks);
    let source = template::build_source(&effective, &body);
    compile_to_pdf(&source)
}

/// Compile a complete Typst source string to PDF bytes, resolving fonts from the
/// system font set.
fn compile_to_pdf(source: &str) -> Result<Vec<u8>, TypesetError> {
    let engine = TypstEngine::builder()
        .main_file(source.to_string())
        .search_fonts_with(TypstKitFontOptions::default())
        .build();

    let compiled = engine.compile::<PagedDocument>();
    for warning in &compiled.warnings {
        log::debug!("typst warning: {}", warning.message);
    }
    let document = compiled
        .output
        .map_err(|e| TypesetError::Compile(e.to_string()))?;

    typst_pdf::pdf(&document, &typst_pdf::PdfOptions::default()).map_err(|diags| {
        let msg = diags
            .iter()
            .map(|d| d.message.to_string())
            .collect::<Vec<_>>()
            .join("; ");
        TypesetError::Pdf(msg)
    })
}

/// Sorted, de-duplicated list of installed system font family names, for a font
/// picker. Falls back to an empty list if the system font set can't be read.
pub fn available_font_families() -> Vec<String> {
    let mut db = fontdb::Database::new();
    db.load_system_fonts();
    let mut names: Vec<String> = db
        .faces()
        .flat_map(|face| face.families.iter().map(|(name, _lang)| name.clone()))
        .collect();
    names.sort();
    names.dedup();
    names
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(text: &str) -> TypesetInput {
        TypesetInput {
            text: text.to_string(),
            format: InputFormat::Markdown,
        }
    }

    #[test]
    fn markdown_typesets_to_pdf() {
        let input = md("# Title\n\nA paragraph with *bold* and _italic_ text.\n");
        let pdf = typeset(&input, &TypesetConfig::default()).expect("typeset");
        assert!(pdf.starts_with(b"%PDF"), "output should be a PDF");
        assert!(pdf.len() > 500, "PDF unexpectedly tiny");
    }

    #[test]
    fn page_break_rule_adds_pages() {
        let cfg = TypesetConfig::default(); // default rules include "-----" => Replace
        let one = typeset(&md("Page one."), &cfg).expect("one");
        let two = typeset(&md("Page one.\n\n-----\n\nPage two."), &cfg).expect("two");
        // Forcing a page break should produce a larger document.
        assert!(two.len() > one.len(), "page break should grow the PDF");
    }

    #[test]
    fn body_font_size_is_applied() {
        // The same text at a much larger body size must reflow to more pages,
        // proving the size actually reaches the Typst output.
        let text = "word ".repeat(400);
        let input = md(&text);

        let small = TypesetConfig {
            body_font: FontChoice::new("", 8.0),
            ..TypesetConfig::default()
        };
        let large = TypesetConfig {
            body_font: FontChoice::new("", 30.0),
            ..TypesetConfig::default()
        };

        let small_pdf = typeset(&input, &small).expect("small");
        let large_pdf = typeset(&input, &large).expect("large");
        assert!(
            large_pdf.len() > small_pdf.len(),
            "larger body size should produce a larger (more pages) PDF"
        );
    }

    #[test]
    fn plaintext_escapes_specials() {
        let input = TypesetInput {
            text: "Costs $5 #1 *not bold* _not italic_".to_string(),
            format: InputFormat::Plaintext,
        };
        let pdf = typeset(&input, &TypesetConfig::default()).expect("typeset");
        assert!(pdf.starts_with(b"%PDF"));
    }
}
