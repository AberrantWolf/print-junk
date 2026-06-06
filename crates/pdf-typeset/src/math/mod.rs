//! Math conversion: `LaTeX` math → `Typst`.
//!
//! Format-agnostic — a [`MathSource`] in, a [`MathRender`] out — so any importer
//! (the HTML/`arXiv` path, or a future Markdown `$...$` path) can reuse it. The
//! pipeline degrades through tiers and reports which [`Tier`] it used:
//!
//! 1. [`Tier::Tex`] — native `Typst` via [`engine`], accepted only if it compiles
//!    ([`validate`]). Reflowable and font-matched; ~99% of real equations.
//! 2. [`Tier::Image`] — an `SVG` of the original math via [`image`] (`RaTeX`), for
//!    the structural residue. Faithful but fixed (doesn't reflow).
//! 3. [`Tier::Raw`] — the `TeX` verbatim. Last resort; never fails.
//!
//! Heuristic `TeX` rewriting lives entirely in [`fixup`], away from this logic.

mod engine;
mod fixup;
mod image;
mod validate;

pub use engine::{Tex2TypstRs, TexMathEngine};

use std::hash::{Hash, Hasher};

/// One formula to convert.
pub struct MathSource<'a> {
    /// Math-mode `LaTeX` (e.g. a `MathML` `<annotation encoding="application/x-tex">`).
    pub tex: &'a str,
    /// Block (display) vs inline.
    pub display: bool,
}

/// Which tier produced a [`MathRender`] — surfaced so callers can report how much
/// of a document degraded to images or raw text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// Native `Typst` math (best: reflowable, font-matched).
    Tex,
    /// `SVG` image via `RaTeX` (faithful, not reflowable).
    Image,
    /// Raw `TeX` shown verbatim (last resort).
    Raw,
}

/// An `SVG` asset referenced by [`MathRender::typst`]; the caller must make it
/// available to `Typst` (e.g. as a virtual file) under [`MathAsset::name`] before
/// compiling.
pub struct MathAsset {
    pub name: String,
    pub svg: Vec<u8>,
}

/// The converted formula: `Typst` markup ready to splice into the body, the tier
/// it came from, and any image assets the markup references.
pub struct MathRender {
    pub typst: String,
    pub tier: Tier,
    pub assets: Vec<MathAsset>,
}

/// Converts formulas to `Typst`, degrading through tiers. Owns the `TeX`
/// backend; cheap to construct, intended to be reused across a document.
pub struct MathPipeline<E: TexMathEngine = Tex2TypstRs> {
    engine: E,
}

impl Default for MathPipeline {
    fn default() -> Self {
        Self {
            engine: Tex2TypstRs,
        }
    }
}

impl<E: TexMathEngine> MathPipeline<E> {
    /// Build a pipeline with a specific `TeX` backend.
    pub fn with_engine(engine: E) -> Self {
        Self { engine }
    }

    /// Convert one formula, degrading through tiers as needed.
    pub fn render(&self, src: &MathSource) -> MathRender {
        let tex = fixup::normalize(src.tex);

        // Tier 1: native Typst, accepted only if it compiles.
        if let Ok(math) = self.engine.convert(&tex)
            && validate::compiles(&math, src.display)
        {
            return MathRender {
                typst: wrap_math(&math, src.display),
                tier: Tier::Tex,
                assets: Vec::new(),
            };
        }

        // Tier 2: SVG image of the original math (rendered from normalized TeX).
        // RaTeX reports geometry in em, so scale the image to the text font size
        // (`height: …em`) and, for inline math, hang it below the baseline by its
        // descent (`baseline: …em`) so it sits on the text baseline rather than
        // floating on the box's bottom edge. Display math gets its own centered line.
        if let Some(m) = image::render_svg(&tex, src.display) {
            let name = format!("math-{:016x}.svg", hash(src.tex));
            let img = format!("image(\"{name}\", height: {:.4}em)", m.height_em);
            let typst = if src.display {
                format!("#align(center, box({img}))")
            } else {
                format!("#box(baseline: {:.4}em, {img})", m.depth_em)
            };
            return MathRender {
                typst,
                tier: Tier::Image,
                assets: vec![MathAsset { name, svg: m.svg }],
            };
        }

        // Tier 3: raw TeX, verbatim. Always succeeds.
        MathRender {
            typst: format!("#raw({})", typst_string(src.tex)),
            tier: Tier::Raw,
            assets: Vec::new(),
        }
    }
}

/// Wrap converted math markup in `$...$`; surrounding spaces request display
/// (block) style in `Typst`.
fn wrap_math(math: &str, display: bool) -> String {
    if display {
        format!("$ {math} $")
    } else {
        format!("${math}$")
    }
}

/// Escape a string for a `Typst` double-quoted string literal.
fn typst_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push(' '),
            _ => out.push(c),
        }
    }
    out.push('"');
    out
}

fn hash(s: &str) -> u64 {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_fraction_uses_native_tex_tier() {
        let pipe = MathPipeline::default();
        let r = pipe.render(&MathSource {
            tex: "\\frac{a}{b}",
            display: false,
        });
        assert_eq!(r.tier, Tier::Tex);
        assert!(r.typst.starts_with('$') && r.typst.ends_with('$'));
        assert!(r.assets.is_empty());
    }

    #[test]
    fn bm_vector_renders_bold_italic_via_tex_tier() {
        let pipe = MathPipeline::default();
        let r = pipe.render(&MathSource {
            tex: "\\bm{x}",
            display: false,
        });
        assert_eq!(r.tier, Tier::Tex);
        assert!(r.typst.contains("bold(x)"), "got {:?}", r.typst);
        assert!(!r.typst.contains("upright"), "got {:?}", r.typst);
    }

    #[test]
    fn structural_residue_falls_back_to_image() {
        // `\operatornamewithlimits` defeats the native path but RaTeX renders it.
        let pipe = MathPipeline::default();
        let r = pipe.render(&MathSource {
            tex: "\\operatornamewithlimits{argmax}_{x} f(x)",
            display: true,
        });
        assert_eq!(r.tier, Tier::Image);
        assert_eq!(r.assets.len(), 1);
        // Display image math is scaled to the text em and centered on its own line.
        assert!(r.typst.contains("image("));
        assert!(r.typst.contains("height:") && r.typst.contains("em"));
        assert!(r.typst.starts_with("#align(center"));
        assert!(r.assets[0].svg.starts_with(b"<") || !r.assets[0].svg.is_empty());
    }

    #[test]
    fn inline_image_math_is_baseline_shifted_and_scaled() {
        // Same residue case as above (defeats the native path), but inline.
        let pipe = MathPipeline::default();
        let r = pipe.render(&MathSource {
            tex: "\\operatornamewithlimits{argmax}_{x} f(x)",
            display: false,
        });
        assert_eq!(r.tier, Tier::Image);
        // Inline images hang below the baseline by their descent and track text size.
        assert!(r.typst.contains("baseline:"), "got {:?}", r.typst);
        assert!(r.typst.contains("height:") && r.typst.contains("em"));
        assert!(!r.typst.contains("align(center"));
    }
}
