//! Structured HTML → `Typst` import, targeting `LaTeXML` output (`arXiv` HTML and
//! `ar5iv`).
//!
//! Parses the page, isolates the document content (dropping the table of
//! contents, navigation, and page chrome), and walks the DOM into `Typst`
//! markup. `<math>` elements are routed through [`crate::MathPipeline`]; images
//! are fetched through an injected [`AssetResolver`] so this module stays
//! I/O-free and testable from fixtures. Math image fallbacks and fetched images
//! are returned as named assets for the caller to register as `Typst` files.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::hash::{Hash, Hasher};

use scraper::node::Node;
use scraper::{ElementRef, Html, Selector};

use crate::markup::escape_inline;
use crate::{MathPipeline, MathSource, Tier};

/// Resolves a referenced asset (an `<img src>`) to its bytes. Implemented by the
/// acquisition layer (HTTP/file); `None` skips the asset.
pub trait AssetResolver {
    fn fetch(&self, src: &str) -> Option<Vec<u8>>;
}

/// A resolver that fetches nothing — useful for tests and text-only imports.
pub struct NoAssets;
impl AssetResolver for NoAssets {
    fn fetch(&self, _src: &str) -> Option<Vec<u8>> {
        None
    }
}

/// Counts of how each part of the document was handled, for QA/logging.
#[derive(Debug, Default, Clone, Copy)]
pub struct ImportStats {
    pub math_tex: usize,
    pub math_image: usize,
    pub math_raw: usize,
    pub images_ok: usize,
    pub images_failed: usize,
    pub footnotes: usize,
}

/// The result of importing an HTML document.
pub struct ImportedDoc {
    /// `Typst` body markup (no template).
    pub body: String,
    /// Named in-memory files (math SVGs, fetched images) to register before
    /// compiling.
    pub assets: Vec<(String, Vec<u8>)>,
    pub title: Option<String>,
    pub stats: ImportStats,
}

/// Import an HTML document into `Typst` markup plus its assets.
pub fn import(html: &str, resolver: &dyn AssetResolver, regenerate_outline: bool) -> ImportedDoc {
    let doc = Html::parse_document(html);
    let mut imp = Importer::new(resolver);
    let content = content_root(&doc)
        .map(|root| imp.render_children(root))
        .unwrap_or_default();

    let mut body = String::new();
    if let Some(t) = &imp.title {
        let _ = write!(
            body,
            "#align(center)[#text(size: 1.6em, weight: \"bold\")[{}]]\n\n",
            escape_inline(t)
        );
    }
    if regenerate_outline {
        body.push_str("#outline()\n\n");
    }
    body.push_str(&content);

    ImportedDoc {
        body,
        assets: imp.out_assets,
        title: imp.title,
        stats: imp.stats,
    }
}

/// Locate the document content container, preferring `LaTeXML`'s.
fn content_root(doc: &Html) -> Option<ElementRef<'_>> {
    for sel in [
        "article.ltx_document",
        ".ltx_document",
        ".ltx_page_content",
        "main",
        "body",
    ] {
        if let Ok(s) = Selector::parse(sel)
            && let Some(e) = doc.select(&s).next()
        {
            return Some(e);
        }
    }
    None
}

/// Elements never emitted: page furniture and the source's own TOC.
fn is_skipped(el: ElementRef<'_>) -> bool {
    const SKIP_TAGS: &[&str] = &[
        "script", "style", "nav", "header", "footer", "button", "form",
    ];
    const SKIP_CLASSES: &[&str] = &[
        "ltx_TOC",
        "ltx_page_navbar",
        "ltx_page_header",
        "ltx_page_footer",
        "ltx_pagination",
        "ltx_rdf",
        // Affiliation reference marks (daggers/numbers) — noise after reflow.
        "ltx_role_footnotemark",
    ];
    let v = el.value();
    SKIP_TAGS.contains(&v.name()) || v.classes().any(|c| SKIP_CLASSES.contains(&c))
}

struct Importer<'r> {
    resolver: &'r dyn AssetResolver,
    math: MathPipeline,
    out_assets: Vec<(String, Vec<u8>)>,
    asset_names: HashMap<String, String>,
    title: Option<String>,
    stats: ImportStats,
    annotation_sel: Selector,
    note_content_sel: Selector,
    tr_sel: Selector,
    cell_sel: Selector,
}

impl<'r> Importer<'r> {
    fn new(resolver: &'r dyn AssetResolver) -> Self {
        Self {
            resolver,
            math: MathPipeline::default(),
            out_assets: Vec::new(),
            asset_names: HashMap::new(),
            title: None,
            stats: ImportStats::default(),
            annotation_sel: Selector::parse("annotation").unwrap(),
            note_content_sel: Selector::parse(".ltx_note_content").unwrap(),
            tr_sel: Selector::parse("tr").unwrap(),
            cell_sel: Selector::parse("td, th").unwrap(),
        }
    }

    fn render_children(&mut self, el: ElementRef<'_>) -> String {
        let mut s = String::new();
        for child in el.children() {
            match child.value() {
                Node::Text(t) => s.push_str(&escape_inline(&collapse_ws(&t.text))),
                Node::Element(_) => {
                    if let Some(ce) = ElementRef::wrap(child) {
                        s.push_str(&self.render_element(ce));
                    }
                }
                _ => {}
            }
        }
        s
    }

    fn render_element(&mut self, el: ElementRef<'_>) -> String {
        if is_skipped(el) {
            return String::new();
        }
        let v = el.value();
        if v.classes().any(|c| c == "ltx_role_footnote") {
            return self.render_footnote(el);
        }
        if v.classes().any(|c| c == "ltx_authors") {
            return self.authors(el);
        }
        match v.name() {
            "math" => self.render_math(el),
            "h1" => self.render_title_or_heading(el),
            "h2" => self.heading(el, 1),
            "h3" => self.heading(el, 2),
            "h4" => self.heading(el, 3),
            "h5" => self.heading(el, 4),
            "h6" => self.heading(el, 5),
            "p" => format!("{}\n\n", self.render_children(el).trim()),
            "ul" => self.list(el, '-'),
            "ol" => self.list(el, '+'),
            "li" => format!("{}\n", self.render_children(el).trim()),
            "table" => self.table(el),
            "img" => self.image(el),
            "a" => self.link(el),
            "em" | "i" => format!("_{}_", self.render_children(el)),
            "strong" | "b" => format!("*{}*", self.render_children(el)),
            "sup" => format!("#super[{}]", self.render_children(el)),
            "sub" => format!("#sub[{}]", self.render_children(el)),
            "code" | "tt" => format!("#raw({})", typst_string(&el.text().collect::<String>())),
            "br" => "#linebreak() ".to_string(),
            "blockquote" => format!(
                "#quote(block: true)[{}]\n\n",
                self.render_children(el).trim()
            ),
            "figcaption" => {
                format!(
                    "\n#align(center)[#emph[{}]]\n\n",
                    self.render_children(el).trim()
                )
            }
            _ => self.render_children(el),
        }
    }

    fn render_title_or_heading(&mut self, el: ElementRef<'_>) -> String {
        let is_title =
            el.value().classes().any(|c| c == "ltx_title_document") || self.title.is_none();
        if is_title {
            let t = collapse_ws(&el.text().collect::<String>())
                .trim()
                .to_string();
            if !t.is_empty() {
                self.title = Some(t);
            }
            String::new()
        } else {
            self.heading(el, 1)
        }
    }

    fn heading(&mut self, el: ElementRef<'_>, depth: usize) -> String {
        format!(
            "\n{} {}\n\n",
            "=".repeat(depth),
            self.render_children(el).trim()
        )
    }

    fn list(&mut self, el: ElementRef<'_>, marker: char) -> String {
        let mut s = String::from("\n");
        for child in el.children() {
            if let Some(ce) = ElementRef::wrap(child)
                && ce.value().name() == "li"
            {
                let _ = writeln!(s, "{marker} {}", self.render_children(ce).trim());
            }
        }
        s.push('\n');
        s
    }

    fn link(&mut self, el: ElementRef<'_>) -> String {
        let children = self.render_children(el);
        match el.value().attr("href") {
            // Internal anchors won't resolve after reflow; keep the text only.
            Some(href) if !href.starts_with('#') => {
                format!("#link({})[{children}]", typst_string(href))
            }
            _ => children,
        }
    }

    fn render_math(&mut self, el: ElementRef<'_>) -> String {
        let display = el.value().attr("display") == Some("block");
        let tex = el
            .select(&self.annotation_sel)
            .find(|a| a.value().attr("encoding") == Some("application/x-tex"))
            .map(|a| a.text().collect::<String>());
        let Some(tex) = tex else {
            return String::new();
        };
        let r = self.math.render(&MathSource { tex: &tex, display });
        match r.tier {
            Tier::Tex => self.stats.math_tex += 1,
            Tier::Image => self.stats.math_image += 1,
            Tier::Raw => self.stats.math_raw += 1,
        }
        for asset in r.assets {
            self.out_assets.push((asset.name, asset.svg));
        }
        if display {
            format!("\n\n{}\n\n", r.typst)
        } else {
            r.typst
        }
    }

    /// The author block, centered. The `\And` separators `LaTeXML` emits as a
    /// literal "&" between authors are removed; affiliation marks are already
    /// skipped via [`is_skipped`].
    fn authors(&mut self, el: ElementRef<'_>) -> String {
        let inner = self.render_children(el).replace('&', "");
        let inner = inner.trim();
        if inner.is_empty() {
            String::new()
        } else {
            format!("\n#align(center)[{inner}]\n\n")
        }
    }

    fn render_footnote(&mut self, el: ElementRef<'_>) -> String {
        self.stats.footnotes += 1;
        let content = el.select(&self.note_content_sel).next();
        let body = match content {
            Some(c) => self.render_children(c),
            None => self.render_children(el),
        };
        format!("#footnote[{}]", body.trim())
    }

    fn image(&mut self, el: ElementRef<'_>) -> String {
        let Some(src) = el.value().attr("src") else {
            return String::new();
        };
        if let Some(name) = self.asset_for(src) {
            format!("#box(image({}))", typst_string(&name))
        } else {
            self.stats.images_failed += 1;
            String::new()
        }
    }

    /// Fetch and register an image asset, returning its `Typst` file name. Deduped
    /// by source so a repeated image is fetched once.
    fn asset_for(&mut self, src: &str) -> Option<String> {
        if let Some(name) = self.asset_names.get(src) {
            return Some(name.clone());
        }
        let bytes = self.resolver.fetch(src)?;
        let name = format!("img-{:016x}{}", hash(src), ext_of(src));
        self.out_assets.push((name.clone(), bytes));
        self.asset_names.insert(src.to_string(), name.clone());
        self.stats.images_ok += 1;
        Some(name)
    }

    fn table(&mut self, el: ElementRef<'_>) -> String {
        // Collect cell elements first (borrows the DOM, not `self`), then render.
        let rows: Vec<Vec<ElementRef<'_>>> = el
            .select(&self.tr_sel)
            .map(|tr| tr.select(&self.cell_sel).collect())
            .collect();
        let cols = rows.iter().map(Vec::len).max().unwrap_or(0);
        if cols == 0 {
            return String::new();
        }
        let mut out = format!("\n#table(\n  columns: {cols},\n");
        for row in &rows {
            out.push_str("  ");
            for cell in row {
                let _ = write!(out, "[{}], ", self.render_children(*cell).trim());
            }
            // Pad short rows so the grid stays rectangular.
            for _ in row.len()..cols {
                out.push_str("[], ");
            }
            out.push('\n');
        }
        out.push_str(")\n\n");
        out
    }
}

/// Collapse runs of whitespace to single spaces (HTML inline-text semantics).
fn collapse_ws(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_ws = false;
    for c in s.chars() {
        if c.is_whitespace() {
            if !prev_ws {
                out.push(' ');
            }
            prev_ws = true;
        } else {
            out.push(c);
            prev_ws = false;
        }
    }
    out
}

/// File extension from a URL/path (lowercased, incl. the dot), defaulting to
/// `.png` when none is present.
fn ext_of(src: &str) -> String {
    let path = src.split(['?', '#']).next().unwrap_or(src);
    match path.rsplit('/').next().and_then(|f| f.rsplit_once('.')) {
        Some((_, ext)) if ext.len() <= 5 && ext.chars().all(char::is_alphanumeric) => {
            format!(".{}", ext.to_ascii_lowercase())
        }
        _ => ".png".to_string(),
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

    const SAMPLE: &str = r#"
    <html><body>
      <nav class="ltx_page_navbar">junk nav</nav>
      <article class="ltx_document">
        <h1 class="ltx_title ltx_title_document">A Tiny Paper</h1>
        <div class="ltx_TOC">Contents... 1 Intro 2</div>
        <section class="ltx_section">
          <h2 class="ltx_title ltx_title_section">Introduction</h2>
          <div class="ltx_para"><p class="ltx_p">Hello <em>world</em> with a
            footnote<span class="ltx_note ltx_role_footnote"><sup class="ltx_note_mark">1</sup><span class="ltx_note_outer"><span class="ltx_note_content">the note body</span></span></span>
            and math <math><semantics><mfrac><mi>a</mi><mi>b</mi></mfrac><annotation encoding="application/x-tex">\frac{a}{b}</annotation></semantics></math>.</p></div>
        </section>
      </article>
      <footer class="ltx_page_footer">junk footer</footer>
    </body></html>
    "#;

    #[test]
    fn extracts_content_and_drops_chrome() {
        let doc = import(SAMPLE, &NoAssets, true);
        assert_eq!(doc.title.as_deref(), Some("A Tiny Paper"));
        // chrome and the source TOC are gone
        assert!(!doc.body.contains("junk nav"));
        assert!(!doc.body.contains("junk footer"));
        assert!(!doc.body.contains("Contents..."));
        // regenerated outline + section heading present
        assert!(doc.body.contains("#outline()"));
        assert!(doc.body.contains("= Introduction"));
    }

    #[test]
    fn renders_inline_math_emphasis_and_footnote() {
        let doc = import(SAMPLE, &NoAssets, false);
        assert!(doc.body.contains("_world_"), "emphasis: {}", doc.body);
        assert!(
            doc.body.contains("#footnote[the note body]"),
            "{}",
            doc.body
        );
        assert!(doc.body.contains('$'), "inline math present: {}", doc.body);
        assert_eq!(doc.stats.math_tex, 1);
        assert_eq!(doc.stats.footnotes, 1);
    }
}
