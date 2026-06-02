//! Build a complete Typst source document from a [`TypesetConfig`] and a body.

use std::fmt::Write as _;

use pdf_units::mm_to_pt;

use crate::config::TypesetConfig;

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
/// `config`, followed by the already-converted `body` markup.
pub fn build_source(config: &TypesetConfig, body: &str) -> String {
    let (w_pt, h_pt) = config
        .page_size
        .dimensions_pt_with_orientation(config.orientation);

    let top = mm_to_pt(config.margin_top_mm);
    let bottom = mm_to_pt(config.margin_bottom_mm);
    let inside = mm_to_pt(config.margin_inner_mm);
    let outside = mm_to_pt(config.margin_outer_mm);

    let mut s = String::new();

    // Page geometry. `inside`/`outside` + `binding` give correct recto/verso
    // gutters for a bound book; numbering adds page numbers in the footer.
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

    // Body text: font, size, language, hyphenation.
    let _ = write!(s, "#set text(size: {:.2}pt", config.body_font.size_pt);
    if !config.body_font.family.trim().is_empty() {
        let _ = write!(s, ", font: {}", typst_str(&config.body_font.family));
    }
    let _ = writeln!(s, ", lang: \"en\", hyphenate: {})", config.hyphenate);

    // Paragraph layout: justification, leading, spacing, first-line indent.
    let _ = write!(
        s,
        "#set par(justify: {}, leading: {:.3}em",
        config.justify, config.line_spacing_em
    );
    if config.paragraph_spacing_mm > 0.0 {
        let _ = write!(s, ", spacing: {:.2}pt", mm_to_pt(config.paragraph_spacing_mm));
    }
    if config.paragraph_indent_mm > 0.0 {
        let _ = write!(
            s,
            ", first-line-indent: (amount: {:.2}pt, all: true)",
            mm_to_pt(config.paragraph_indent_mm)
        );
    }
    s.push_str(")\n");

    // Heading font (separate from body), if specified.
    if !config.heading_font.family.trim().is_empty() {
        let _ = writeln!(
            s,
            "#show heading: set text(font: {})",
            typst_str(&config.heading_font.family)
        );
    }
    // Heading sizes: the configured size sets the top level; deeper levels scale
    // down to preserve hierarchy, but never below the body size. Applies to every
    // level so Markdown setext headings (`===` => level 1, `---` => level 2) and
    // ATX headings (`#`…`######`) all honor the setting.
    for level in 1..=6u8 {
        let scale = 0.84_f32.powi(i32::from(level) - 1);
        let size = (config.heading_font.size_pt * scale).max(config.body_font.size_pt);
        let _ = writeln!(
            s,
            "#show heading.where(level: {level}): set text(size: {size:.2}pt)"
        );
    }

    s.push('\n');
    s.push_str(body);
    s.push('\n');
    s
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{FontChoice, TypesetConfig};

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
        let cfg = TypesetConfig {
            body_font: FontChoice::new("", 12.0),
            heading_font: FontChoice::new("", 13.0),
            ..TypesetConfig::default()
        };
        let src = build_source(&cfg, "");
        // Level 1 keeps the configured heading size…
        assert!(src.contains("#show heading.where(level: 1): set text(size: 13.00pt)"));
        // …and a deep level that would scale below the body size clamps to it.
        assert!(src.contains("#show heading.where(level: 6): set text(size: 12.00pt)"));
    }
}
