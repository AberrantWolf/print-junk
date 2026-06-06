//! Image fallback: render `LaTeX` math to a self-contained `SVG` via `RaTeX`.
//!
//! Used for the structural residue (matrices, aligned environments, limit
//! operators) that the native-`Typst` path can't express. The `SVG` embeds its
//! own outline glyphs (`embed_glyphs`), so it needs no fonts available at `Typst`
//! compile time and prints faithfully — it simply doesn't reflow.

use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parser::parse;
use ratex_svg::{SvgOptions, render_to_svg};
use ratex_types::math_style::MathStyle;

/// Render `tex` (math-mode `LaTeX`) to a standalone `SVG` document. Returns
/// `None` if `RaTeX` cannot parse it or produces a degenerate result.
pub fn render_svg(tex: &str, display: bool) -> Option<Vec<u8>> {
    let style = if display {
        MathStyle::Display
    } else {
        MathStyle::Text
    };
    let opts = LayoutOptions::default().with_style(style);
    let ast = parse(tex).ok()?;
    let list = to_display_list(&layout(&ast, &opts));
    let svg = render_to_svg(
        &list,
        &SvgOptions {
            embed_glyphs: true,
            ..Default::default()
        },
    );
    (svg.contains("<svg") && svg.len() > 200).then(|| svg.into_bytes())
}

#[cfg(test)]
mod tests {
    use super::render_svg;

    #[test]
    fn renders_a_matrix_the_native_path_cannot() {
        let svg = render_svg("\\begin{pmatrix}1&0\\\\0&1\\end{pmatrix}", true)
            .expect("RaTeX renders pmatrix");
        let text = String::from_utf8(svg).unwrap();
        assert!(text.contains("<svg"));
    }
}
