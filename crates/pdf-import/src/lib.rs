//! Acquire an HTML document from a URL, an `arXiv` id/URL, or a local file, and
//! resolve its image assets.
//!
//! This is the I/O layer for the structured importer in `pdf-typeset`: it fetches
//! the markup and provides an [`AssetResolver`] so `pdf-typeset` itself stays
//! network-free. `arXiv` references are normalized to their HTML rendering
//! (native `arxiv.org/html`, falling back to `ar5iv`), since the abstract page
//! carries no full text.

use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use pdf_typeset::AssetResolver;
use regex::Regex;

static NEW_FIND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\d{4}\.\d{4,5}(?:v\d+)?").unwrap());
static OLD_FIND: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"[a-z][a-z\-]+(?:\.[A-Z]{2})?/\d{7}(?:v\d+)?").unwrap());
static NEW_FULL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^\d{4}\.\d{4,5}(?:v\d+)?$").unwrap());
static OLD_FULL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^[a-z][a-z\-]+(?:\.[A-Z]{2})?/\d{7}(?:v\d+)?$").unwrap());

/// Errors from acquiring a document.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("fetch failed: {0}")]
    Fetch(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("invalid source: {0}")]
    Invalid(String),
}

/// An acquired document: its HTML plus the base it was loaded from. Implements
/// [`AssetResolver`] so it can be handed straight to `pdf_typeset::typeset_html`.
pub struct Imported {
    pub html: String,
    /// The resolved URL the HTML came from (`None` for a local file).
    pub source_url: Option<String>,
    base: Base,
}

enum Base {
    Url(url::Url),
    Dir(PathBuf),
}

/// Acquire a document from `source`: a local file path, an `arXiv` id/abs/pdf
/// URL, or a generic URL.
pub fn fetch(source: &str) -> Result<Imported, ImportError> {
    let source = source.trim();

    // 1. Local file.
    let path = Path::new(source);
    if path.exists() {
        let html = std::fs::read_to_string(path)?;
        let dir = path
            .parent()
            .map_or_else(|| PathBuf::from("."), Path::to_path_buf);
        return Ok(Imported {
            html,
            source_url: None,
            base: Base::Dir(dir),
        });
    }

    // 2. arXiv reference -> HTML rendering (native, then ar5iv fallback).
    if let Some(id) = arxiv_id(source) {
        let mut last = String::new();
        for url in [
            format!("https://arxiv.org/html/{id}"),
            format!("https://ar5iv.labs.arxiv.org/html/{id}"),
        ] {
            match get(&url) {
                Ok(html) if looks_like_document(&html) => {
                    let base =
                        url::Url::parse(&url).map_err(|e| ImportError::Invalid(e.to_string()))?;
                    return Ok(Imported {
                        html,
                        source_url: Some(url),
                        base: Base::Url(base),
                    });
                }
                Ok(_) => last = format!("{url}: no document content"),
                Err(e) => last = format!("{url}: {e}"),
            }
        }
        return Err(ImportError::Fetch(format!(
            "no HTML version for arXiv {id} ({last})"
        )));
    }

    // 3. Generic URL.
    let base = url::Url::parse(source)
        .map_err(|_| ImportError::Invalid(format!("not a file, arXiv id, or URL: {source}")))?;
    let html = get(source)?;
    Ok(Imported {
        html,
        source_url: Some(source.to_string()),
        base: Base::Url(base),
    })
}

fn looks_like_document(html: &str) -> bool {
    html.contains("ltx_document") || html.contains("application/x-tex")
}

fn get(url: &str) -> Result<String, ImportError> {
    ureq::get(url)
        .call()
        .map_err(|e| ImportError::Fetch(e.to_string()))?
        .body_mut()
        .read_to_string()
        .map_err(|e| ImportError::Fetch(e.to_string()))
}

/// Extract an `arXiv` id from a URL or bare reference, if `source` is one.
fn arxiv_id(source: &str) -> Option<String> {
    if source.contains("arxiv.org") || source.contains("ar5iv") {
        return NEW_FIND
            .find(source)
            .or_else(|| OLD_FIND.find(source))
            .map(|m| m.as_str().to_string());
    }
    if !source.contains("://") {
        let cand = source.trim_end_matches(".pdf");
        if NEW_FULL.is_match(cand) || OLD_FULL.is_match(cand) {
            return Some(cand.to_string());
        }
    }
    None
}

impl AssetResolver for Imported {
    fn fetch(&self, src: &str) -> Option<Vec<u8>> {
        match &self.base {
            Base::Url(base) => {
                let abs = base.join(src).ok()?;
                ureq::get(abs.as_str())
                    .call()
                    .ok()?
                    .body_mut()
                    .read_to_vec()
                    .ok()
            }
            Base::Dir(dir) => {
                if src.contains("://") {
                    return None;
                }
                std::fs::read(dir.join(src)).ok()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::arxiv_id;

    #[test]
    fn recognizes_arxiv_forms() {
        assert_eq!(arxiv_id("2310.12345").as_deref(), Some("2310.12345"));
        assert_eq!(arxiv_id("2310.12345v2").as_deref(), Some("2310.12345v2"));
        assert_eq!(
            arxiv_id("https://arxiv.org/abs/1706.03762").as_deref(),
            Some("1706.03762")
        );
        assert_eq!(
            arxiv_id("arxiv.org/pdf/1706.03762v5").as_deref(),
            Some("1706.03762v5")
        );
        assert_eq!(
            arxiv_id("hep-th/9901001").as_deref(),
            Some("hep-th/9901001")
        );
    }

    #[test]
    fn ignores_non_arxiv() {
        assert_eq!(arxiv_id("https://example.com/page.html"), None);
        assert_eq!(arxiv_id("notes.html"), None);
    }
}
