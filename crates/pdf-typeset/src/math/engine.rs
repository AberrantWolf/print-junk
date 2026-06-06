//! The swappable `TeX`-math → `Typst` backend.
//!
//! Isolating the backend behind a trait keeps the pipeline independent of any one
//! crate: the [`Tex2TypstRs`] implementation can be replaced (e.g. with a
//! `mitex`-based one) without touching [`super::MathPipeline`].

/// Converts math-mode `LaTeX` into `Typst` math markup (without surrounding `$`).
pub trait TexMathEngine {
    /// Returns the `Typst` math markup, or an error message if conversion fails.
    fn convert(&self, tex: &str) -> Result<String, String>;
}

/// Backend built on the `tex2typst-rs` crate, which emits standalone, idiomatic
/// `Typst` math. This is the validated primary path (see the math spike): ~99% of
/// real `arXiv` equations convert and compile after [`super::fixup`] normalization.
#[derive(Debug, Default, Clone, Copy)]
pub struct Tex2TypstRs;

impl TexMathEngine for Tex2TypstRs {
    fn convert(&self, tex: &str) -> Result<String, String> {
        tex2typst_rs::tex2typst(tex)
    }
}
