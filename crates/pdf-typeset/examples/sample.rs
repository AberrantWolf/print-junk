//! Render a sample Markdown document to a PDF for visual/format sanity checks.
//!
//! Run with: `cargo run -p pdf-typeset --example sample -- /tmp/sample.pdf`

use pdf_typeset::{Color, InputFormat, TypesetConfig, TypesetInput, typeset};

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

### A Table

| Material   | Qty | Unit cost |
|:-----------|:---:|----------:|
| Book board | 2   | \\$1.20    |
| Bookcloth  | 1   | \\$4.50    |
| Headband   | 2   | \\$0.30    |

A [link to typst.app](https://typst.app) and some `inline code` round it out.

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

    // Exercise the new features: title page, TOC, a colored H1, and a chapter
    // that starts on a fresh page.
    let mut config = TypesetConfig {
        doc_title: "The Typesetting Sample".to_string(),
        doc_author: "print-junk".to_string(),
        doc_keywords: "typesetting, bookbinding, sample".to_string(),
        generate_toc: true,
        ..TypesetConfig::default()
    };
    config.heading_styles[0].color = Color::new(40, 70, 130);
    config.heading_styles[0].start_new_page = true;

    match typeset(&input, &config) {
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
