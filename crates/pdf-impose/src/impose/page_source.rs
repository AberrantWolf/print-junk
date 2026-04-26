//! Virtual page source for multi-document imposition
//!
//! `PageSource` provides a unified linear page index over multiple source PDF
//! documents. It replaces the previous approach of merging documents into one
//! before imposition, eliminating a redundant deep-copy of every page object.
//!
//! Flyleaves are modeled as virtual blank entries — matching the physical
//! reality that flyleaves are blank sheets added during binding, not pages
//! inserted into the PDF page tree.

use crate::constants::DEFAULT_PAGE_DIMENSIONS;
use crate::render::{create_page_xobject, get_page_dimensions};
use crate::types::{ImposeError, Result};
use lopdf::{Document, ObjectId};
use std::collections::HashMap;

/// A single entry in the linear page list.
#[derive(Debug, Clone, Copy)]
pub enum PageEntry {
    /// A real page from a source document.
    Source {
        doc_index: usize,
        page_id: ObjectId,
        dimensions: (f32, f32),
    },
    /// A virtual blank page (flyleaf or padding).
    Blank { dimensions: (f32, f32) },
}

/// Cache for `XObject` creation across multiple source documents.
///
/// Tracks both page-level `XObject` dedup and per-document deep-copy caches
/// for shared resources (fonts, images). This means a font shared by many
/// pages in the same source document is only copied to the output once,
/// even across different output sheets.
#[derive(Default)]
pub struct XObjectCache {
    page_xobjects: HashMap<(usize, ObjectId), ObjectId>,
    deep_copy_caches: HashMap<usize, HashMap<ObjectId, ObjectId>>,
}

impl XObjectCache {
    pub fn new() -> Self {
        Self::default()
    }
}

/// A unified view over pages from multiple source PDF documents.
///
/// Provides a linear index where page 0 is the first page of the first
/// document (or the first flyleaf), and page N is the last page of the
/// last document (or the last flyleaf).
pub struct PageSource {
    documents: Vec<Document>,
    pages: Vec<PageEntry>,
}

impl PageSource {
    /// Build a page source from documents with optional flyleaves.
    ///
    /// Flyleaf blank pages are inserted at the front and back. Each flyleaf
    /// contributes 2 pages (front and back of one leaf).
    pub fn new(
        documents: Vec<Document>,
        front_flyleaves: usize,
        back_flyleaves: usize,
    ) -> Result<Self> {
        Self::build(documents, front_flyleaves, back_flyleaves, None)
    }

    /// Build a page source limited to at most `max_source_pages` real pages.
    ///
    /// Used for preview generation to avoid processing entire large documents.
    /// Flyleaves are added on top of the limit (they're virtual and free).
    pub fn with_page_limit(
        documents: Vec<Document>,
        front_flyleaves: usize,
        back_flyleaves: usize,
        max_source_pages: usize,
    ) -> Result<Self> {
        Self::build(
            documents,
            front_flyleaves,
            back_flyleaves,
            Some(max_source_pages),
        )
    }

    /// Build a page source from a specific source-page range.
    ///
    /// `source_page_offset` skips that many real source pages before collection
    /// begins, and `max_source_pages` limits how many real pages are included
    /// after the skip. Flyleaves are added around the selected range.
    ///
    /// Used by `impose_and_save` when splitting output by signature.
    pub fn with_source_page_range(
        documents: Vec<Document>,
        front_flyleaves: usize,
        back_flyleaves: usize,
        source_page_offset: usize,
        max_source_pages: usize,
    ) -> Result<Self> {
        Self::build_range(
            documents,
            front_flyleaves,
            back_flyleaves,
            source_page_offset,
            Some(max_source_pages),
        )
    }

    fn build(
        documents: Vec<Document>,
        front_flyleaves: usize,
        back_flyleaves: usize,
        max_source_pages: Option<usize>,
    ) -> Result<Self> {
        Self::build_range(
            documents,
            front_flyleaves,
            back_flyleaves,
            0,
            max_source_pages,
        )
    }

    fn build_range(
        documents: Vec<Document>,
        front_flyleaves: usize,
        back_flyleaves: usize,
        source_page_offset: usize,
        max_source_pages: Option<usize>,
    ) -> Result<Self> {
        // Determine blank page dimensions from the first real page we find
        let blank_dims = documents
            .iter()
            .find_map(|doc| {
                let pages = doc.get_pages();
                let (&_page_num, &page_id) = pages.iter().next()?;
                get_page_dimensions(doc, page_id).ok()
            })
            .unwrap_or(DEFAULT_PAGE_DIMENSIONS);

        let front_blank_count = front_flyleaves * 2;
        let back_blank_count = back_flyleaves * 2;

        // Estimate capacity
        let total_source: usize = documents.iter().map(|d| d.get_pages().len()).sum();
        let source_offset = source_page_offset.min(total_source);
        let available_source = total_source.saturating_sub(source_offset);
        let source_limit = max_source_pages
            .unwrap_or(available_source)
            .min(available_source);
        let capacity = front_blank_count + source_limit + back_blank_count;
        let mut pages = Vec::with_capacity(capacity);

        // Front flyleaves
        for _ in 0..front_blank_count {
            pages.push(PageEntry::Blank {
                dimensions: blank_dims,
            });
        }

        // Source pages from all documents
        let mut to_skip = source_offset;
        let mut remaining = source_limit;
        for (doc_index, doc) in documents.iter().enumerate() {
            if remaining == 0 {
                break;
            }
            let doc_pages = doc.get_pages();
            for (&_page_num, &page_id) in &doc_pages {
                if to_skip > 0 {
                    to_skip -= 1;
                    continue;
                }
                if remaining == 0 {
                    break;
                }
                let dimensions =
                    get_page_dimensions(doc, page_id).unwrap_or(DEFAULT_PAGE_DIMENSIONS);
                pages.push(PageEntry::Source {
                    doc_index,
                    page_id,
                    dimensions,
                });
                remaining -= 1;
            }
        }

        // Back flyleaves
        for _ in 0..back_blank_count {
            pages.push(PageEntry::Blank {
                dimensions: blank_dims,
            });
        }

        if pages.is_empty() {
            return Err(ImposeError::NoPages);
        }

        Ok(Self { documents, pages })
    }

    /// Total number of pages (source + flyleaves).
    pub fn len(&self) -> usize {
        self.pages.len()
    }

    /// Whether the page source is empty.
    pub fn is_empty(&self) -> bool {
        self.pages.is_empty()
    }

    /// Get the dimensions of a page at the given index.
    pub fn dimensions(&self, index: usize) -> (f32, f32) {
        match self.pages[index] {
            PageEntry::Source { dimensions, .. } | PageEntry::Blank { dimensions } => dimensions,
        }
    }

    /// Get dimensions for all pages as a vec.
    pub fn all_dimensions(&self) -> Vec<(f32, f32)> {
        self.pages
            .iter()
            .map(|p| match p {
                PageEntry::Source { dimensions, .. } | PageEntry::Blank { dimensions } => {
                    *dimensions
                }
            })
            .collect()
    }

    /// Create an `XObject` for the page at `index` in the output document.
    ///
    /// Returns `None` for blank pages (flyleaves). For source pages, creates
    /// a Form `XObject` by copying from the correct source document.
    ///
    /// The `cache` should be shared across all calls within a single imposition
    /// job. This ensures:
    /// - The same page is never converted to an `XObject` twice
    /// - Resources (fonts, images) shared across pages in the same source
    ///   document are only deep-copied once
    pub fn create_xobject(
        &self,
        output: &mut Document,
        index: usize,
        cache: &mut XObjectCache,
    ) -> Result<Option<ObjectId>> {
        match self.pages[index] {
            PageEntry::Blank { .. } => Ok(None),
            PageEntry::Source {
                doc_index, page_id, ..
            } => {
                let cache_key = (doc_index, page_id);
                if let Some(&xobject_id) = cache.page_xobjects.get(&cache_key) {
                    return Ok(Some(xobject_id));
                }

                let source = &self.documents[doc_index];
                let deep_copy_cache = cache.deep_copy_caches.entry(doc_index).or_default();
                let xobject_id = create_page_xobject(output, source, page_id, deep_copy_cache)?;

                cache.page_xobjects.insert(cache_key, xobject_id);
                Ok(Some(xobject_id))
            }
        }
    }
}
