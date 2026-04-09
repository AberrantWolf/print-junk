//! Document I/O operations for imposition

use crate::types::Result;
use lopdf::Document;
use std::path::Path;

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
