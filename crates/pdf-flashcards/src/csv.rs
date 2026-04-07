use crate::types::{Flashcard, FlashcardWarning, Result};
use std::path::Path;

pub async fn load_from_csv(
    path: impl AsRef<Path>,
) -> Result<(Vec<Flashcard>, Vec<FlashcardWarning>)> {
    let path = path.as_ref().to_owned();

    log::info!("Loading flashcards from {}", path.display());
    let contents = tokio::fs::read_to_string(&path).await?;

    let (cards, warnings) = tokio::task::spawn_blocking(move || {
        let mut reader = csv::Reader::from_reader(contents.as_bytes());
        let mut cards = Vec::new();
        let mut warnings = Vec::new();

        for (i, result) in reader.records().enumerate() {
            let record = result?;
            let row_number = i + 2; // 1-indexed, +1 for header row
            if record.len() >= 2 {
                cards.push(Flashcard {
                    front: record[0].to_string(),
                    back: record[1].to_string(),
                });
            } else {
                log::warn!(
                    "Row {}: skipping (has {} column(s), need >= 2)",
                    row_number,
                    record.len()
                );
                warnings.push(FlashcardWarning::CsvRowSkipped {
                    row_number,
                    column_count: record.len(),
                });
            }
        }

        if cards.is_empty() {
            warnings.push(FlashcardWarning::EmptyCsv);
        }

        Ok::<_, crate::types::FlashcardError>((cards, warnings))
    })
    .await??;

    log::info!("Loaded {} flashcards", cards.len());

    Ok((cards, warnings))
}
