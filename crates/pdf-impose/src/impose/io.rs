//! Document I/O operations for imposition

use super::{PageSource, impose_page_source};
use crate::options::ImpositionOptions;
use crate::types::{ImposeError, Result, SplitMode};
use lopdf::Document;
use std::path::{Path, PathBuf};

/// Load a single PDF document
pub async fn load_pdf(path: impl AsRef<Path>) -> Result<Document> {
    let path = path.as_ref().to_owned();
    let bytes = tokio::fs::read(&path).await?;
    let doc = tokio::task::spawn_blocking(move || Document::load_mem(&bytes)).await??;
    Ok(doc)
}

/// Load multiple PDF documents concurrently
pub async fn load_multiple_pdfs(paths: &[impl AsRef<Path>]) -> Result<Vec<Document>> {
    let mut set = tokio::task::JoinSet::new();
    for (i, path) in paths.iter().enumerate() {
        let path = path.as_ref().to_owned();
        set.spawn(async move {
            let bytes = tokio::fs::read(&path).await?;
            let doc = tokio::task::spawn_blocking(move || Document::load_mem(&bytes)).await??;
            Ok::<_, crate::types::ImposeError>((i, doc))
        });
    }
    let mut results: Vec<(usize, Document)> = Vec::with_capacity(paths.len());
    while let Some(result) = set.join_next().await {
        results.push(result??);
    }
    // Restore original order
    results.sort_by_key(|(i, _)| *i);
    Ok(results.into_iter().map(|(_, doc)| doc).collect())
}

/// Save the imposed document
pub async fn save_pdf(mut doc: Document, path: impl AsRef<Path>) -> Result<()> {
    let path = path.as_ref().to_owned();
    let bytes = tokio::task::spawn_blocking(move || {
        let mut writer = Vec::new();
        doc.save_to(&mut writer)?;
        Ok::<_, crate::types::ImposeError>(writer)
    })
    .await??;
    tokio::fs::write(&path, bytes).await?;
    Ok(())
}

/// Impose and save output, honoring `options.split_mode`.
///
/// Returns the list of paths actually written. When `split_mode` is `None`
/// or the chunk math collapses to a single file, returns a single-element
/// vec containing `path` verbatim. When splitting produces multiple files,
/// returns paths of the form `{stem}-signature-{N}.{ext}` where `N` is
/// zero-padded to the digit count of the total chunk count.
///
/// This is the canonical "impose to disk" entry point — both the CLI and
/// GUI handler call it instead of pairing `impose` with `save_pdf` directly.
pub async fn impose_and_save(
    documents: Vec<Document>,
    options: &ImpositionOptions,
    path: impl AsRef<Path>,
) -> Result<Vec<PathBuf>> {
    options.validate()?;
    let path = path.as_ref().to_owned();

    match options.split_mode {
        SplitMode::BySignatures(signatures_per_file) => {
            // validate() has already enforced signatures_per_file >= 1 and
            // the binding_type compatibility check.
            save_split_by_signatures(documents, options, path, signatures_per_file).await
        }
        SplitMode::None => {
            let imposed = super::impose(documents, options).await?;
            save_pdf(imposed, &path).await?;
            Ok(vec![path])
        }
    }
}

async fn save_split_by_signatures(
    documents: Vec<Document>,
    options: &ImpositionOptions,
    base_path: PathBuf,
    signatures_per_file: usize,
) -> Result<Vec<PathBuf>> {
    let total_source_pages: usize = documents.iter().map(|d| d.get_pages().len()).sum();
    if total_source_pages == 0 {
        return Err(ImposeError::NoPages);
    }

    // Virtual page accounting: flyleaves are virtual (each flyleaf = 2 pages).
    let pages_per_chunk = options.pages_per_signature() * signatures_per_file;
    let front_flyleaf_pages = options.front_flyleaves * 2;
    let back_flyleaf_pages = options.back_flyleaves * 2;
    let total_virtual_pages = front_flyleaf_pages + total_source_pages + back_flyleaf_pages;
    let total_chunks = total_virtual_pages.div_ceil(pages_per_chunk);
    let mut saved_paths = Vec::with_capacity(total_chunks);

    let source_section_start = front_flyleaf_pages;
    let source_section_end = front_flyleaf_pages + total_source_pages;

    for chunk_index in 0..total_chunks {
        let chunk_start = chunk_index * pages_per_chunk;
        let chunk_end = ((chunk_index + 1) * pages_per_chunk).min(total_virtual_pages);

        // Decompose the [chunk_start, chunk_end) virtual range into the three
        // sections (front flyleaves, source, back flyleaves) that PageSource
        // models separately.
        let front_in_chunk = overlap_len(chunk_start, chunk_end, 0, front_flyleaf_pages);
        let source_in_chunk = overlap_len(
            chunk_start,
            chunk_end,
            source_section_start,
            source_section_end,
        );
        let back_in_chunk = overlap_len(
            chunk_start,
            chunk_end,
            source_section_end,
            total_virtual_pages,
        );
        let source_page_offset = chunk_start.saturating_sub(front_flyleaf_pages);

        // Chunk sizes are multiples of pages_per_signature (which is even),
        // so flyleaf-page counts in any chunk are always even.
        let front_flyleaves_in_chunk = front_in_chunk / 2;
        let back_flyleaves_in_chunk = back_in_chunk / 2;

        // Documents are cloned per chunk: lopdf::Document is moved into
        // spawn_blocking. Cost is amortized over the whole split job.
        let page_source = PageSource::with_source_page_range(
            documents.clone(),
            front_flyleaves_in_chunk,
            back_flyleaves_in_chunk,
            source_page_offset,
            source_in_chunk,
        )?;

        let mut chunk_options = options.clone();
        // Page numbering continues across chunks.
        chunk_options.page_number_start += chunk_start;

        let imposed =
            tokio::task::spawn_blocking(move || impose_page_source(&page_source, &chunk_options))
                .await??;

        let chunk_path = if total_chunks == 1 {
            base_path.clone()
        } else {
            make_split_output_path(&base_path, chunk_index + 1, total_chunks)
        };
        save_pdf(imposed, &chunk_path).await?;
        saved_paths.push(chunk_path);
    }

    Ok(saved_paths)
}

/// Length of the intersection of two half-open ranges.
fn overlap_len(
    range_start: usize,
    range_end: usize,
    section_start: usize,
    section_end: usize,
) -> usize {
    range_end
        .min(section_end)
        .saturating_sub(range_start.max(section_start))
}

/// Build a per-chunk output path: `{stem}-signature-{N}.{ext}`.
///
/// `N` is zero-padded to the digit count of `total` so files sort lexically.
fn make_split_output_path(base_path: &Path, index: usize, total: usize) -> PathBuf {
    let parent = base_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_default();
    let stem = base_path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .unwrap_or("output");
    let ext = base_path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("pdf");
    let width = total.to_string().len();
    parent.join(format!("{stem}-signature-{index:0width$}.{ext}"))
}
