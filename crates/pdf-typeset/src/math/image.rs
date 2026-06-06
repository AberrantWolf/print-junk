//! Image fallback: render `LaTeX` math to a self-contained `SVG` via `RaTeX`.
//!
//! Used for the structural residue (matrices, aligned environments, limit
//! operators) that the native-`Typst` path can't express. The `SVG` embeds its
//! own outline glyphs (`embed_glyphs`), so it needs no fonts available at `Typst`
//! compile time and prints faithfully — it simply doesn't reflow.
//!
//! The `SVG` is rendered with zero padding and its geometry is reported in **em**
//! (font-relative) units so the caller can scale it to the surrounding text size
//! and shift its baseline — see [`SvgMath`].

use ratex_layout::{LayoutOptions, layout, to_display_list};
use ratex_parser::parser::parse;
use ratex_svg::{SvgOptions, render_to_svg};
use ratex_types::math_style::MathStyle;

/// A rendered math `SVG` plus the geometry needed to place it like real type.
///
/// `RaTeX` lays math out in em units relative to the font size, so these stay
/// valid whatever text size the importer renders at: scale the image to
/// [`height_em`](Self::height_em) ems and hang it [`depth_em`](Self::depth_em)
/// ems below the text baseline.
pub struct SvgMath {
    /// The standalone `SVG` document.
    pub svg: Vec<u8>,
    /// Total height (ascent + descent) in em — the image's natural size matches
    /// the text font when rendered at this many ems tall.
    pub height_em: f64,
    /// Descent below the baseline in em — how far the box must hang below the
    /// text baseline so the math's baseline sits on it.
    pub depth_em: f64,
}

/// Render `tex` (math-mode `LaTeX`) to an [`SvgMath`]. Returns `None` if `RaTeX`
/// cannot parse it or produces a degenerate result.
pub fn render_svg(tex: &str, display: bool) -> Option<SvgMath> {
    let style = if display {
        MathStyle::Display
    } else {
        MathStyle::Text
    };
    let opts = LayoutOptions::default().with_style(style);
    let ast = parse(tex).ok()?;
    let list = to_display_list(&layout(&ast, &opts));
    // Zero padding so the SVG box is exactly the math's bounding box: no stray
    // inline whitespace, and `font_size` cancels out of the em ratios below.
    let svg = render_to_svg(
        &list,
        &SvgOptions {
            embed_glyphs: true,
            padding: 0.0,
            ..Default::default()
        },
    );
    if !svg.contains("<svg") || svg.len() <= 200 {
        return None;
    }
    Some(SvgMath {
        svg: svg.into_bytes(),
        height_em: list.height + list.depth,
        depth_em: list.depth,
    })
}

#[cfg(test)]
mod tests {
    use super::render_svg;

    #[test]
    fn renders_a_matrix_the_native_path_cannot() {
        let m = render_svg("\\begin{pmatrix}1&0\\\\0&1\\end{pmatrix}", true)
            .expect("RaTeX renders pmatrix");
        let text = String::from_utf8(m.svg).unwrap();
        assert!(text.contains("<svg"));
        // A 2×2 matrix straddles the math axis, so it has real ascent and descent.
        assert!(m.height_em > m.depth_em && m.depth_em > 0.0);
    }
}
