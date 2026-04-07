//! Preview generation for imposition
//!
//! Generates a limited preview of the imposition for quick display.

use crate::impose::impose;
use crate::options::ImpositionOptions;
use crate::render::copy_object_deep;
use crate::types::*;
use lopdf::{Dictionary, Document, Object};
use std::collections::HashMap;

/// Result of preview generation, including truncation metadata.
pub struct PreviewResult {
    /// The imposed preview document
    pub document: Document,
    /// How many signatures are represented in the preview
    pub signatures_shown: usize,
}

/// Maximum output sheets to render in a preview.
///
/// The actual signature limit is derived from this based on sheets_per_signature,
/// so simpler arrangements (folio) show more signatures than complex ones (octavo).
const MAX_PREVIEW_SHEETS: usize = 16;

/// Generate a preview of the imposition
///
/// Returns an imposed document limited to `max_signatures` complete signatures.
/// If `None`, a smart default is computed targeting ~[`MAX_PREVIEW_SHEETS`] output sheets.
pub async fn generate_preview(
    documents: &[Document],
    options: &ImpositionOptions,
    max_signatures: Option<usize>,
) -> Result<PreviewResult> {
    let total_source_pages: usize = documents.iter().map(|d| d.get_pages().len()).sum();

    let (source_pages_needed, signatures_shown) = if options.binding_type.uses_signatures() {
        let pages_per_sig = options.pages_per_signature();
        let effective_max = max_signatures.unwrap_or_else(|| {
            (MAX_PREVIEW_SHEETS / options.sheets_per_signature).max(1)
        });
        let total_sigs = (total_source_pages + pages_per_sig - 1) / pages_per_sig;
        let sigs = total_sigs.min(effective_max);
        (sigs * pages_per_sig, sigs)
    } else {
        let pages_needed = max_signatures.unwrap_or(MAX_PREVIEW_SHEETS) * 2;
        let sigs = pages_needed.min(total_source_pages);
        (sigs, 0)
    };

    // Create preview documents with limited pages
    let preview_docs = limit_document_pages(documents, source_pages_needed)?;

    // Impose with limited pages
    let document = impose(&preview_docs, options).await?;
    Ok(PreviewResult {
        document,
        signatures_shown,
    })
}

/// Limit documents to a maximum number of pages
fn limit_document_pages(documents: &[Document], max_pages: usize) -> Result<Vec<Document>> {
    if documents.is_empty() {
        return Err(ImposeError::NoPages);
    }

    let doc = &documents[0];
    let pages = doc.get_pages();
    let total_pages = pages.len();

    if total_pages <= max_pages {
        return Ok(documents.to_vec());
    }

    // Create a new document with limited pages
    let page_ids: Vec<_> = pages.iter().take(max_pages).map(|(_, &id)| id).collect();
    let limited_doc = copy_pages_to_new_document(doc, &page_ids)?;

    Ok(vec![limited_doc])
}

/// Copy specified pages to a new document
fn copy_pages_to_new_document(source: &Document, page_ids: &[lopdf::ObjectId]) -> Result<Document> {
    let mut dest = Document::with_version(source.version.as_str());
    let mut cache = HashMap::new();

    // Create pages tree
    let pages_tree_id = dest.new_object_id();
    let mut kids = Vec::with_capacity(page_ids.len());

    for &page_id in page_ids {
        if let Ok(page_obj) = source.get_object(page_id) {
            let new_page_id = copy_page_object(&mut dest, source, page_obj, &mut cache)?;
            // Set Parent to point to the new pages tree
            if let Ok(page_dict) = dest.get_dictionary_mut(new_page_id) {
                page_dict.set("Parent", Object::Reference(pages_tree_id));
            }
            kids.push(Object::Reference(new_page_id));
        }
    }

    // Create pages dictionary
    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(kids)),
        ("Count", Object::Integer(page_ids.len() as i64)),
    ]);
    dest.objects
        .insert(pages_tree_id, Object::Dictionary(pages_dict));

    // Create catalog
    let catalog_id = dest.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_tree_id)),
    ]));

    dest.trailer.set("Root", catalog_id);

    Ok(dest)
}

/// Copy a page object and its resources to a new document
fn copy_page_object(
    dest: &mut Document,
    source: &Document,
    obj: &Object,
    cache: &mut HashMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<lopdf::ObjectId> {
    match obj {
        Object::Reference(id) => {
            if let Some(&new_id) = cache.get(id) {
                Ok(new_id)
            } else {
                let referenced = source.get_object(*id)?;
                let new_id = copy_page_object(dest, source, referenced, cache)?;
                cache.insert(*id, new_id);
                Ok(new_id)
            }
        }
        Object::Dictionary(dict) => {
            let mut new_dict = Dictionary::new();
            for (key, value) in dict.iter() {
                // Skip "Parent" to avoid circular reference (Page → Pages tree → Kids → Page)
                // The page will get a new parent when added to the destination pages tree.
                if key == b"Parent" {
                    continue;
                }
                let new_value = copy_value_for_page(dest, source, value, cache)?;
                new_dict.set(key.clone(), new_value);
            }
            Ok(dest.add_object(new_dict))
        }
        Object::Stream(stream) => {
            let mut new_dict = Dictionary::new();
            for (key, value) in stream.dict.iter() {
                let new_value = copy_value_for_page(dest, source, value, cache)?;
                new_dict.set(key.clone(), new_value);
            }
            let new_stream = lopdf::Stream {
                dict: new_dict,
                content: stream.content.clone(),
                allows_compression: stream.allows_compression,
                start_position: None,
            };
            Ok(dest.add_object(new_stream))
        }
        _ => Ok(dest.add_object(obj.clone())),
    }
}

/// Copy a value, following references as needed
fn copy_value_for_page(
    dest: &mut Document,
    source: &Document,
    value: &Object,
    cache: &mut HashMap<lopdf::ObjectId, lopdf::ObjectId>,
) -> Result<Object> {
    match value {
        Object::Reference(id) => {
            let new_id = if let Some(&cached_id) = cache.get(id) {
                cached_id
            } else {
                let referenced = source.get_object(*id)?;
                let new_id = copy_page_object(dest, source, referenced, cache)?;
                cache.insert(*id, new_id);
                new_id
            };
            Ok(Object::Reference(new_id))
        }
        _ => copy_object_deep(dest, source, value, cache),
    }
}
