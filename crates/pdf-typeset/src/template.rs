//! Build a complete Typst source document from a [`TypesetConfig`] and a body.

use std::fmt::Write as _;

use pdf_units::mm_to_pt;

use crate::config::{Color, TableBorder, TypesetConfig};

/// Quote a string as a Typst string literal.
fn typst_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' | '\\' => {
                out.push('\\');
                out.push(ch);
            }
            _ => out.push(ch),
        }
    }
    out.push('"');
    out
}

/// Assemble the full Typst source: a set of `#set`/`#show` rules derived from
/// `config`, the front matter (title page / table of contents), and the
/// already-converted `body` markup.
pub fn build_source(config: &TypesetConfig, body: &str) -> String {
    let mut s = String::new();

    document_metadata(&mut s, config);
    page_and_text(&mut s, config);
    paragraph(&mut s, config);
    heading_rules(&mut s, config);
    inline_color_rules(&mut s, config);
    table_rules(&mut s, config);

    s.push('\n');
    front_matter(&mut s, config);
    s.push_str(body);
    s.push('\n');
    s
}

/// `#set document(...)` so the exported PDF carries title/author/keywords.
fn document_metadata(s: &mut String, config: &TypesetConfig) {
    let title = config.doc_title.trim();
    let author = config.doc_author.trim();
    let keywords: Vec<&str> = config
        .doc_keywords
        .split(',')
        .map(str::trim)
        .filter(|k| !k.is_empty())
        .collect();

    if title.is_empty() && author.is_empty() && keywords.is_empty() {
        return;
    }

    let mut parts = Vec::new();
    if !title.is_empty() {
        parts.push(format!("title: {}", typst_str(title)));
    }
    if !author.is_empty() {
        parts.push(format!("author: {}", typst_str(author)));
    }
    if !keywords.is_empty() {
        let list = keywords
            .iter()
            .map(|k| typst_str(k))
            .collect::<Vec<_>>()
            .join(", ");
        parts.push(format!("keywords: ({list},)"));
    }
    let _ = writeln!(s, "#set document({})", parts.join(", "));
}

/// Page geometry plus body text font/size/color/language/hyphenation.
fn page_and_text(s: &mut String, config: &TypesetConfig) {
    let (w_pt, h_pt) = config
        .page_size
        .dimensions_pt_with_orientation(config.orientation);

    let top = mm_to_pt(config.margin_top_mm);
    let bottom = mm_to_pt(config.margin_bottom_mm);
    let inside = mm_to_pt(config.margin_inner_mm);
    let outside = mm_to_pt(config.margin_outer_mm);

    // `inside`/`outside` + `binding` give correct recto/verso gutters for a
    // bound book; numbering adds page numbers in the footer.
    let _ = write!(
        s,
        "#set page(width: {w_pt:.2}pt, height: {h_pt:.2}pt, \
         margin: (top: {top:.2}pt, bottom: {bottom:.2}pt, inside: {inside:.2}pt, outside: {outside:.2}pt), \
         binding: left"
    );
    if config.page_numbers {
        let _ = write!(s, ", numbering: \"1\"");
    }
    s.push_str(")\n");

    let lang = if config.lang.trim().is_empty() {
        "en"
    } else {
        config.lang.trim()
    };
    let _ = write!(s, "#set text(size: {:.2}pt", config.body_font.size_pt);
    if !config.body_font.family.trim().is_empty() {
        let _ = write!(s, ", font: {}", typst_str(&config.body_font.family));
    }
    if !config.body_color.is_black() {
        let _ = write!(s, ", fill: {}", config.body_color.to_typst());
    }
    let _ = writeln!(
        s,
        ", lang: {}, hyphenate: {})",
        typst_str(lang),
        config.hyphenate
    );

    // Smart quotes follow the smart-punctuation toggle (dashes/ellipses are
    // handled upstream by the Markdown parser).
    let _ = writeln!(s, "#set smartquote(enabled: {})", config.smart_punctuation);
}

/// `#set par(...)`: justification, leading, paragraph spacing, first-line indent.
fn paragraph(s: &mut String, config: &TypesetConfig) {
    let _ = write!(
        s,
        "#set par(justify: {}, leading: {:.3}em",
        config.justify, config.line_spacing_em
    );
    if config.paragraph_spacing_mm > 0.0 {
        let _ = write!(
            s,
            ", spacing: {:.2}pt",
            mm_to_pt(config.paragraph_spacing_mm)
        );
    }
    if config.paragraph_indent_mm > 0.0 {
        let _ = write!(
            s,
            ", first-line-indent: (amount: {:.2}pt, all: true)",
            mm_to_pt(config.paragraph_indent_mm)
        );
    }
    s.push_str(")\n");
}

/// Per-level heading show rules. Markdown setext headings (`===` => level 1,
/// `---` => level 2) and ATX headings (`#`…`######`) all map to these levels.
fn heading_rules(s: &mut String, config: &TypesetConfig) {
    for level in 1..=6u8 {
        let st = config.heading_style(level);
        // Never render a heading smaller than the body text.
        let size = st.size_pt.max(config.body_font.size_pt);

        let mut text = vec![format!("size: {size:.2}pt")];
        if !st.family.trim().is_empty() {
            text.push(format!("font: {}", typst_str(&st.family)));
        }
        text.push(format!(
            "weight: \"{}\"",
            if st.bold { "bold" } else { "regular" }
        ));
        if st.italic {
            text.push("style: \"italic\"".to_string());
        }
        if !st.color.is_black() {
            text.push(format!("fill: {}", st.color.to_typst()));
        }
        let _ = writeln!(
            s,
            "#show heading.where(level: {level}): set text({})",
            text.join(", ")
        );

        let _ = writeln!(
            s,
            "#show heading.where(level: {level}): set block(above: {:.2}pt, below: {:.2}pt)",
            mm_to_pt(st.space_above_mm),
            mm_to_pt(st.space_below_mm)
        );

        if st.align != crate::config::HAlign::Left {
            let _ = writeln!(
                s,
                "#show heading.where(level: {level}): set align({})",
                st.align.to_typst()
            );
        }
        if st.start_new_page {
            let _ = writeln!(
                s,
                "#show heading.where(level: {level}): it => pagebreak(weak: true) + it"
            );
        }
    }
}

/// Color rules for links and code (inline and block, plus an optional code
/// block background).
fn inline_color_rules(s: &mut String, config: &TypesetConfig) {
    if !config.link_color.is_black() {
        let _ = writeln!(
            s,
            "#show link: set text(fill: {})",
            config.link_color.to_typst()
        );
    }
    if !config.code_color.is_black() {
        let _ = writeln!(
            s,
            "#show raw: set text(fill: {})",
            config.code_color.to_typst()
        );
    }
    if let Some(bg) = config.code_background {
        let _ = writeln!(
            s,
            "#show raw.where(block: true): it => block(fill: {}, inset: 6pt, radius: 3pt, width: 100%, it)",
            bg.to_typst()
        );
    }
}

/// Global `#set table(...)` plus a header-cell show rule, driving borders,
/// padding, header shading, and zebra striping for every Markdown table.
fn table_rules(s: &mut String, config: &TypesetConfig) {
    let t = &config.table;

    let stroke = match t.border {
        TableBorder::All => format!("{:.2}pt + {}", t.border_width_pt, t.border_color.to_typst()),
        TableBorder::Horizontal => format!(
            "(y: {:.2}pt + {})",
            t.border_width_pt,
            t.border_color.to_typst()
        ),
        TableBorder::None => "none".to_string(),
    };
    let inset = mm_to_pt(t.cell_padding_mm);

    let _ = write!(s, "#set table(stroke: {stroke}, inset: {inset:.2}pt");
    if let Some(fill) = table_fill_fn(t.header_fill, t.zebra_fill) {
        let _ = write!(s, ", fill: {fill}");
    }
    s.push_str(")\n");

    if t.header_bold {
        s.push_str("#show table.cell.where(y: 0): set text(weight: \"bold\")\n");
    }
}

/// Build a Typst `(_, y) => …` fill function combining a header row fill (row 0)
/// and zebra striping of odd body rows. Returns `None` when neither is set.
fn table_fill_fn(header: Option<Color>, zebra: Option<Color>) -> Option<String> {
    let mut clauses = Vec::new();
    if let Some(h) = header {
        clauses.push(format!("if y == 0 {{ {} }}", h.to_typst()));
    }
    if let Some(z) = zebra {
        clauses.push(format!("if y > 0 and calc.odd(y) {{ {} }}", z.to_typst()));
    }
    if clauses.is_empty() {
        return None;
    }
    Some(format!("(_, y) => {{ {} else {{ none }} }}", clauses.join(" else ")))
}

/// Title page (when a document title is set) and table of contents (when
/// enabled), each followed by a page break.
fn front_matter(s: &mut String, config: &TypesetConfig) {
    let title = config.doc_title.trim();
    if !title.is_empty() {
        s.push_str("#align(center + horizon)[\n");
        let _ = writeln!(
            s,
            "  #text(size: 2.2em, weight: \"bold\")[{}]",
            typst_str(title).trim_matches('"')
        );
        let author = config.doc_author.trim();
        if !author.is_empty() {
            s.push_str("  #v(1.2em)\n");
            let _ = writeln!(
                s,
                "  #text(size: 1.2em)[{}]",
                typst_str(author).trim_matches('"')
            );
        }
        s.push_str("]\n#pagebreak()\n\n");
    }

    if config.generate_toc {
        let depth = config.toc_depth.clamp(1, 6);
        let _ = writeln!(s, "#outline(title: [Contents], depth: {depth})");
        s.push_str("#pagebreak()\n\n");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Color, FontChoice, HAlign, HeadingStyle, TableBorder, TypesetConfig};

    #[test]
    fn heading_size_rules_cover_all_levels() {
        let src = build_source(&TypesetConfig::default(), "");
        for level in 1..=6 {
            assert!(
                src.contains(&format!(
                    "#show heading.where(level: {level}): set text(size:"
                )),
                "missing heading size rule for level {level}"
            );
        }
    }

    #[test]
    fn headings_never_render_smaller_than_body() {
        let mut cfg = TypesetConfig {
            body_font: FontChoice::new("", 12.0),
            ..TypesetConfig::default()
        };
        cfg.heading_styles[5] = HeadingStyle {
            size_pt: 6.0, // would be smaller than the 12pt body
            ..HeadingStyle::for_level(6)
        };
        let src = build_source(&cfg, "");
        assert!(
            src.contains("#show heading.where(level: 6): set text(size: 12.00pt"),
            "deep heading should clamp up to the body size"
        );
    }

    #[test]
    fn per_level_heading_styles_emit_weight_and_color() {
        let mut cfg = TypesetConfig::default();
        cfg.heading_styles[0] = HeadingStyle {
            bold: false,
            italic: true,
            color: Color::new(200, 0, 0),
            align: HAlign::Center,
            start_new_page: true,
            ..HeadingStyle::for_level(1)
        };
        let src = build_source(&cfg, "");
        assert!(src.contains("weight: \"regular\""));
        assert!(src.contains("style: \"italic\""));
        assert!(src.contains("fill: rgb(200, 0, 0)"));
        assert!(src.contains("#show heading.where(level: 1): set align(center)"));
        assert!(src.contains("#show heading.where(level: 1): it => pagebreak(weak: true) + it"));
    }

    #[test]
    fn document_metadata_and_front_matter() {
        let cfg = TypesetConfig {
            doc_title: "My Book".to_string(),
            doc_author: "A. Author".to_string(),
            doc_keywords: "one, two".to_string(),
            generate_toc: true,
            toc_depth: 2,
            ..TypesetConfig::default()
        };
        let src = build_source(&cfg, "body");
        assert!(src.contains("#set document(title: \"My Book\", author: \"A. Author\""));
        assert!(src.contains("keywords: (\"one\", \"two\",)"));
        assert!(src.contains("#align(center + horizon)["));
        assert!(src.contains("#outline(title: [Contents], depth: 2)"));
    }

    #[test]
    fn table_style_emits_stroke_and_fill() {
        let cfg = TypesetConfig::default(); // header fill on, zebra off
        let src = build_source(&cfg, "");
        assert!(src.contains("#set table(stroke:"));
        assert!(src.contains("fill: (_, y) =>"));
        assert!(src.contains("#show table.cell.where(y: 0): set text(weight: \"bold\")"));

        let none = TypesetConfig {
            table: crate::config::TableStyle {
                border: TableBorder::None,
                header_fill: None,
                header_bold: false,
                zebra_fill: None,
                ..crate::config::TableStyle::default()
            },
            ..TypesetConfig::default()
        };
        let src = build_source(&none, "");
        assert!(src.contains("#set table(stroke: none"));
        assert!(!src.contains("fill: (_, y)"));
        assert!(!src.contains("#show table.cell"));
    }

    #[test]
    fn smartquote_follows_punctuation_toggle() {
        let on = build_source(&TypesetConfig::default(), "");
        assert!(on.contains("#set smartquote(enabled: true)"));
        let off = build_source(
            &TypesetConfig {
                smart_punctuation: false,
                ..TypesetConfig::default()
            },
            "",
        );
        assert!(off.contains("#set smartquote(enabled: false)"));
    }
}
