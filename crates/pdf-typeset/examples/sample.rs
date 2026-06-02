//! Render a sample Markdown document to a PDF for visual/format sanity checks.
//!
//! Run with: `cargo run -p pdf-typeset --example sample -- /tmp/sample.pdf`

use pdf_typeset::{InputFormat, TypesetConfig, TypesetInput, typeset};

const DOC: &str = "\
# The Typesetting Sample

This is a paragraph of body text that should be *justified* and _hyphenated_
across the line, demonstrating that the Typst engine is doing real paragraph
layout with a sensible book measure on an A5 page.

A Setext Heading (three hyphens under text → H2)
---

This paragraph follows a setext-style level-2 heading; it should be sized like
the ATX heading below, not left at the default size.

## A Second Heading

- A bullet list item
- Another item with `inline code`
- A third item

1. First ordered item
2. Second ordered item

> A block quote, set apart from the body.

-----

# A New Chapter

The `-----` rule above forced a page break, so this heading begins on a fresh
page — exactly what a chapter boundary needs.
";

fn main() {
    let out = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "/tmp/sample.pdf".to_string());

    let input = TypesetInput {
        text: DOC.to_string(),
        format: InputFormat::Markdown,
    };

    match typeset(&input, &TypesetConfig::default()) {
        Ok(pdf) => {
            std::fs::write(&out, &pdf).expect("write pdf");
            println!("Wrote {} bytes to {out}", pdf.len());
        }
        Err(e) => {
            eprintln!("typeset failed: {e}");
            std::process::exit(1);
        }
    }
}
