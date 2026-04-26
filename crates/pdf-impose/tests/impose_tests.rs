use lopdf::{Dictionary, Document, Object, Stream};
use pdf_impose::*;
use std::path::PathBuf;

fn create_test_pdf(num_pages: usize) -> Document {
    let mut doc = Document::with_version("1.7");

    // Create page tree root ID
    let pages_id = doc.new_object_id();

    // Create pages array
    let mut kids = Vec::new();
    for _ in 0..num_pages {
        let content_id = doc.add_object(Stream::new(Dictionary::new(), b"q Q".to_vec()));

        let page_id = doc.add_object(Dictionary::from_iter(vec![
            ("Type", Object::Name(b"Page".to_vec())),
            ("Parent", Object::Reference(pages_id)),
            (
                "MediaBox",
                Object::Array(vec![
                    Object::Integer(0),
                    Object::Integer(0),
                    Object::Integer(612),
                    Object::Integer(792),
                ]),
            ),
            ("Resources", Object::Dictionary(Dictionary::new())),
            ("Contents", Object::Reference(content_id)),
        ]));
        kids.push(Object::Reference(page_id));
    }

    // Create pages dict
    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(kids)),
        ("Count", Object::Integer(num_pages as i64)),
    ]);
    doc.objects.insert(pages_id, Object::Dictionary(pages_dict));

    // Create catalog
    let catalog_id = doc.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_id)),
    ]));

    doc.trailer.set("Root", catalog_id);

    doc
}

#[tokio::test]
async fn test_load_pdf() {
    use tempfile::NamedTempFile;

    let mut doc = create_test_pdf(5);
    let temp = NamedTempFile::new().unwrap();
    let path = temp.path();

    // Save test PDF
    let mut writer = Vec::new();
    doc.save_to(&mut writer).unwrap();
    std::fs::write(path, writer).unwrap();

    // Load it back
    let loaded = load_pdf(path).await.unwrap();
    assert_eq!(loaded.get_pages().len(), 5);
}

#[tokio::test]
async fn test_load_multiple_pdfs() {
    use tempfile::NamedTempFile;

    let mut doc1 = create_test_pdf(3);
    let mut doc2 = create_test_pdf(4);

    let temp1 = NamedTempFile::new().unwrap();
    let temp2 = NamedTempFile::new().unwrap();

    let mut writer = Vec::new();
    doc1.save_to(&mut writer).unwrap();
    std::fs::write(temp1.path(), &writer).unwrap();

    writer.clear();
    doc2.save_to(&mut writer).unwrap();
    std::fs::write(temp2.path(), &writer).unwrap();

    let paths = vec![temp1.path(), temp2.path()];
    let docs = load_multiple_pdfs(&paths).await.unwrap();

    assert_eq!(docs.len(), 2);
    assert_eq!(docs[0].get_pages().len(), 3);
    assert_eq!(docs[1].get_pages().len(), 4);
}

#[tokio::test]
async fn test_save_pdf() {
    use tempfile::NamedTempFile;

    let doc = create_test_pdf(2);
    let temp = NamedTempFile::new().unwrap();

    save_pdf(doc, temp.path()).await.unwrap();

    // Verify file was created and can be loaded
    assert!(temp.path().exists());
    let loaded = Document::load(temp.path()).unwrap();
    assert_eq!(loaded.get_pages().len(), 2);
}

#[tokio::test]
async fn test_impose_and_save_split_by_signatures() {
    use tempfile::tempdir;

    let doc = create_test_pdf(24);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto;
    options.split_mode = SplitMode::BySignatures(2);

    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("imposed.pdf");

    let saved_paths = impose_and_save(vec![doc], &options, &output_path)
        .await
        .unwrap();

    assert_eq!(saved_paths.len(), 2);
    assert_eq!(
        saved_paths[0].file_name().and_then(|s| s.to_str()),
        Some("imposed-signature-1.pdf")
    );
    assert_eq!(
        saved_paths[1].file_name().and_then(|s| s.to_str()),
        Some("imposed-signature-2.pdf")
    );

    let first = Document::load(&saved_paths[0]).unwrap();
    let second = Document::load(&saved_paths[1]).unwrap();

    // 24 source pages with Quarto = 3 signatures total.
    // Split by 2 signatures => first file has 2 signatures (4 output pages),
    // second file has 1 signature (2 output pages).
    assert_eq!(first.get_pages().len(), 4);
    assert_eq!(second.get_pages().len(), 2);
}

#[tokio::test]
async fn test_split_by_signatures_accounts_for_front_flyleaves() {
    use tempfile::tempdir;

    let doc = create_test_pdf(17);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto;
    options.front_flyleaves = 2; // 4 extra virtual pages at the beginning
    options.split_mode = SplitMode::BySignatures(2);

    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("imposed.pdf");

    let saved_paths = impose_and_save(vec![doc], &options, &output_path)
        .await
        .unwrap();

    assert_eq!(saved_paths.len(), 2);

    let first = Document::load(&saved_paths[0]).unwrap();
    let second = Document::load(&saved_paths[1]).unwrap();

    // Virtual pages = 4 flyleaf + 17 source = 21.
    // Quarto = 8 pp/sig, split by 2 sigs/file => 16 virtual pages/file.
    // First file stays capped at 2 signatures (4 output pages); second has 1 (2 output pages).
    assert_eq!(first.get_pages().len(), 4);
    assert_eq!(second.get_pages().len(), 2);
}

#[tokio::test]
async fn test_split_by_signatures_final_chunk_page_count_matches_requested_signatures() {
    use tempfile::tempdir;

    let doc = create_test_pdf(1947);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto;
    options.sheets_per_signature = 10;
    options.split_mode = SplitMode::BySignatures(5);

    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("imposed.pdf");

    let saved_paths = impose_and_save(vec![doc], &options, &output_path)
        .await
        .unwrap();

    assert_eq!(saved_paths.len(), 5);

    let last = Document::load(&saved_paths[4]).unwrap();

    // 5 signatures/file × 10 sheets/signature × 2 sides/sheet = 100 imposed pages/file.
    assert_eq!(last.get_pages().len(), 100);
}

#[tokio::test]
async fn test_validate_rejects_split_by_signatures_zero() {
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::Signature;
    options.split_mode = SplitMode::BySignatures(0);

    match options.validate() {
        Err(ImposeError::Config(msg)) => {
            assert!(
                msg.contains("Signatures per file"),
                "expected zero-signatures error, got: {msg}"
            );
        }
        other => panic!("expected Config error, got {other:?}"),
    }
}

#[tokio::test]
async fn test_validate_rejects_split_by_signatures_with_non_signature_binding() {
    for binding in [
        BindingType::PerfectBinding,
        BindingType::SideStitch,
        BindingType::Spiral,
    ] {
        let mut options = ImpositionOptions::default();
        options.input_files.push(PathBuf::from("test.pdf"));
        options.binding_type = binding;
        options.split_mode = SplitMode::BySignatures(1);

        match options.validate() {
            Err(ImposeError::Config(msg)) => {
                assert!(
                    msg.contains("signature-based binding"),
                    "binding {binding:?}: expected binding-mismatch error, got: {msg}"
                );
            }
            other => panic!("binding {binding:?}: expected Config error, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn test_impose_and_save_single_chunk_uses_base_path_verbatim() {
    use tempfile::tempdir;

    // 8 source pages with Quarto = 1 signature; split-by-1 = 1 chunk.
    let doc = create_test_pdf(8);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto;
    options.split_mode = SplitMode::BySignatures(1);

    let temp_dir = tempdir().unwrap();
    let output_path = temp_dir.path().join("imposed.pdf");

    let saved_paths = impose_and_save(vec![doc], &options, &output_path)
        .await
        .unwrap();

    assert_eq!(saved_paths.len(), 1);
    assert_eq!(
        saved_paths[0], output_path,
        "single-chunk case must not append a suffix"
    );
    assert!(output_path.exists());
}

#[tokio::test]
async fn test_impose_no_pages() {
    let doc = create_test_pdf(0);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));

    let result = impose(vec![doc], &options).await;
    assert!(result.is_err());
    match result {
        Err(ImposeError::NoPages) => {}
        _ => panic!("Expected NoPages error"),
    }
}

#[tokio::test]
async fn test_impose_validation_fails() {
    let doc = create_test_pdf(5);
    let options = ImpositionOptions::default(); // No input files

    let result = impose(vec![doc], &options).await;
    assert!(result.is_err());
    match result {
        Err(ImposeError::Config(_)) => {}
        _ => panic!("Expected Config error"),
    }
}

#[tokio::test]
async fn test_impose_signature_basic() {
    let doc = create_test_pdf(8);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto;

    let result = impose(vec![doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Quarto: 8 pages per signature = 1 signature = 1 sheet with 4 pages per side = 2 output pages
    assert_eq!(output.get_pages().len(), 2);
}

#[tokio::test]
async fn test_impose_perfect_binding() {
    let doc = create_test_pdf(10);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::PerfectBinding;

    let result = impose(vec![doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // PerfectBinding with 10 pages results in 5 output pages
    assert_eq!(output.get_pages().len(), 5);
}

#[tokio::test]
async fn test_impose_with_different_paper_sizes() {
    let doc = create_test_pdf(4);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));

    // Test different paper sizes
    let paper_sizes = vec![
        PaperSize::A3,
        PaperSize::A4,
        PaperSize::A5,
        PaperSize::Letter,
        PaperSize::Legal,
        PaperSize::Tabloid,
        PaperSize::Custom {
            width_mm: 200.0,
            height_mm: 300.0,
        },
    ];

    for paper_size in paper_sizes {
        options.output_paper_size = paper_size;
        let result = impose(vec![doc.clone()], &options).await;
        assert!(result.is_ok(), "Failed for paper size: {paper_size:?}");
    }
}

#[tokio::test]
async fn test_impose_with_scaling_modes() {
    let doc = create_test_pdf(4);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));

    let scaling_modes = vec![
        ScalingMode::Fit,
        ScalingMode::Fill,
        ScalingMode::None,
        ScalingMode::Stretch,
    ];

    for mode in scaling_modes {
        options.scaling_mode = mode;
        let result = impose(vec![doc.clone()], &options).await;
        assert!(result.is_ok(), "Failed for scaling mode: {mode:?}");
    }
}

#[tokio::test]
async fn test_impose_folio() {
    let doc = create_test_pdf(4);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.page_arrangement = PageArrangement::Folio;

    let result = impose(vec![doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Folio: 4 pages per signature = 1 signature = 1 sheet with 2 pages per side = 2 output pages
    assert_eq!(output.get_pages().len(), 2);
}

#[tokio::test]
async fn test_impose_octavo() {
    let doc = create_test_pdf(16);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.page_arrangement = PageArrangement::Octavo;

    let result = impose(vec![doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Octavo: 16 pages per signature = 1 signature = 1 sheet with 8 pages per side = 2 output pages
    assert_eq!(output.get_pages().len(), 2);
}

#[tokio::test]
async fn test_impose_with_multi_sheet_folio() {
    let doc = create_test_pdf(12);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.page_arrangement = PageArrangement::Folio;
    options.sheets_per_signature = 3; // 3 sheets × 4 pages = 12 pages per signature

    let result = impose(vec![doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // 1 signature × 3 sheets × 2 sides = 6 output pages
    assert_eq!(output.get_pages().len(), 6);
}

#[tokio::test]
async fn test_impose_side_stitch() {
    let doc = create_test_pdf(6);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::SideStitch;

    let result = impose(vec![doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // SideStitch: simple 2-up layout, 6 pages = 3 sheets × 2 sides = 3 output pages (alternating front/back)
    assert_eq!(output.get_pages().len(), 3);
}

#[tokio::test]
async fn test_impose_spiral() {
    let doc = create_test_pdf(8);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::Spiral;

    let result = impose(vec![doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // Spiral: simple 2-up layout, 8 pages = 4 sheets × 2 sides = 4 output pages (alternating front/back)
    assert_eq!(output.get_pages().len(), 4);
}

#[tokio::test]
async fn test_impose_case_binding() {
    let doc = create_test_pdf(16);
    let mut options = ImpositionOptions::default();
    options.input_files.push(PathBuf::from("test.pdf"));
    options.binding_type = BindingType::CaseBinding;

    let result = impose(vec![doc], &options).await;
    assert!(result.is_ok());

    let output = result.unwrap();
    // CaseBinding uses default Quarto: 16 pages = 2 signatures × 8 pages = 2 sheets × 2 sides = 4 output pages
    assert_eq!(output.get_pages().len(), 4);
}

#[tokio::test]
async fn test_full_workflow() {
    use tempfile::TempDir;

    let temp_dir = TempDir::new().unwrap();
    let input_path = temp_dir.path().join("input.pdf");
    let output_path = temp_dir.path().join("output.pdf");

    // Create and save input PDF
    let mut doc = create_test_pdf(10);
    let mut writer = Vec::new();
    doc.save_to(&mut writer).unwrap();
    std::fs::write(&input_path, writer).unwrap();

    // Load the PDF
    let loaded = load_pdf(&input_path).await.unwrap();
    assert_eq!(loaded.get_pages().len(), 10);

    // Set up imposition options
    let mut options = ImpositionOptions::default();
    options.input_files.push(input_path.clone());
    options.binding_type = BindingType::Signature;
    options.page_arrangement = PageArrangement::Quarto;
    options.output_paper_size = PaperSize::Letter;

    // Perform imposition
    let imposed = impose(vec![loaded], &options).await.unwrap();

    // Save output
    save_pdf(imposed, &output_path).await.unwrap();

    // Verify output exists
    assert!(output_path.exists());
}

// Test correct page ordering for traditional bookbinding formats
// These tests verify spread-based page assignments match traditional bookbinding standards

#[test]
fn test_folio_page_order() {
    // Folio: 1 fold = 4 pages per signature = 1 spread per side
    use pdf_impose::PageArrangement;
    use pdf_impose::layout::assign_pages_to_spreads;

    let sheets = assign_pages_to_spreads(PageArrangement::Folio, 1, 0, 4);
    let assignment = &sheets[0];

    // Front side: 1 spread [verso=4, recto=1]
    assert_eq!(assignment.front.len(), 1);
    assert_eq!(assignment.front[0].verso_page, Some(3)); // page 4 (0-indexed)
    assert_eq!(assignment.front[0].recto_page, Some(0)); // page 1 (0-indexed)

    // Back side: 1 spread [verso=2, recto=3]
    assert_eq!(assignment.back.len(), 1);
    assert_eq!(assignment.back[0].verso_page, Some(1)); // page 2 (0-indexed)
    assert_eq!(assignment.back[0].recto_page, Some(2)); // page 3 (0-indexed)
}

#[test]
fn test_quarto_page_order() {
    // Quarto: 2 folds = 8 pages per signature = 2 spreads per side
    use pdf_impose::PageArrangement;
    use pdf_impose::layout::assign_pages_to_spreads;

    let sheets = assign_pages_to_spreads(PageArrangement::Quarto, 1, 0, 8);
    let assignment = &sheets[0];

    // Front side: 2 spreads [bottom, top]
    assert_eq!(assignment.front.len(), 2);
    // Bottom spread: [verso=8, recto=1]
    assert_eq!(assignment.front[0].verso_page, Some(7)); // page 8
    assert_eq!(assignment.front[0].recto_page, Some(0)); // page 1
    // Top spread (rotated): [verso=5, recto=4]
    assert_eq!(assignment.front[1].verso_page, Some(4)); // page 5
    assert_eq!(assignment.front[1].recto_page, Some(3)); // page 4

    // Back side: 2 spreads [bottom, top]
    assert_eq!(assignment.back.len(), 2);
    // Bottom spread: [verso=2, recto=7]
    assert_eq!(assignment.back[0].verso_page, Some(1)); // page 2
    assert_eq!(assignment.back[0].recto_page, Some(6)); // page 7
    // Top spread (rotated): [verso=3, recto=6]
    assert_eq!(assignment.back[1].verso_page, Some(2)); // page 3
    assert_eq!(assignment.back[1].recto_page, Some(5)); // page 6
}

#[test]
fn test_octavo_page_order() {
    // Octavo: 3 folds = 16 pages per signature = 4 spreads per side
    use pdf_impose::PageArrangement;
    use pdf_impose::layout::assign_pages_to_spreads;

    let sheets = assign_pages_to_spreads(PageArrangement::Octavo, 1, 0, 16);
    let assignment = &sheets[0];

    // Front side: 4 spreads [bottom-left, bottom-right, top-left, top-right]
    assert_eq!(assignment.front.len(), 4);
    // Bottom-left: [verso=4, recto=13]
    assert_eq!(assignment.front[0].verso_page, Some(3));
    assert_eq!(assignment.front[0].recto_page, Some(12));
    // Bottom-right: [verso=16, recto=1]
    assert_eq!(assignment.front[1].verso_page, Some(15));
    assert_eq!(assignment.front[1].recto_page, Some(0));
    // Top-left (rotated): [verso=5, recto=12]
    assert_eq!(assignment.front[2].verso_page, Some(4));
    assert_eq!(assignment.front[2].recto_page, Some(11));
    // Top-right (rotated): [verso=9, recto=8]
    assert_eq!(assignment.front[3].verso_page, Some(8));
    assert_eq!(assignment.front[3].recto_page, Some(7));

    // Back side: 4 spreads (mirrored due to sheet flip)
    assert_eq!(assignment.back.len(), 4);
    // Bottom-left (was bottom-right on A): [verso=2, recto=15]
    assert_eq!(assignment.back[0].verso_page, Some(1));
    assert_eq!(assignment.back[0].recto_page, Some(14));
    // Bottom-right (was bottom-left on A): [verso=14, recto=3]
    assert_eq!(assignment.back[1].verso_page, Some(13));
    assert_eq!(assignment.back[1].recto_page, Some(2));
    // Top-left (was top-right on A, rotated): [verso=7, recto=10]
    assert_eq!(assignment.back[2].verso_page, Some(6));
    assert_eq!(assignment.back[2].recto_page, Some(9));
    // Top-right (was top-left on A, rotated): [verso=11, recto=6]
    assert_eq!(assignment.back[3].verso_page, Some(10));
    assert_eq!(assignment.back[3].recto_page, Some(5));
}

#[test]
fn test_multiple_signatures() {
    // Test with 2 quarto signatures (16 pages total)
    use pdf_impose::PageArrangement;
    use pdf_impose::layout::assign_pages_to_spreads;

    // First signature (pages 1-8)
    let sig1 = assign_pages_to_spreads(PageArrangement::Quarto, 1, 0, 16);
    // Second signature (pages 9-16)
    let sig2 = assign_pages_to_spreads(PageArrangement::Quarto, 1, 8, 16);

    // First signature front, bottom spread: [verso=8, recto=1]
    assert_eq!(sig1[0].front[0].verso_page, Some(7)); // page 8
    assert_eq!(sig1[0].front[0].recto_page, Some(0)); // page 1

    // Second signature front, bottom spread: [verso=16, recto=9]
    assert_eq!(sig2[0].front[0].verso_page, Some(15)); // page 16
    assert_eq!(sig2[0].front[0].recto_page, Some(8)); // page 9
}
