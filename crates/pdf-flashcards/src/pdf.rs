use crate::options::FlashcardOptions;
use crate::types::{Flashcard, FlashcardError, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, Stream};
use pdf_units::{mm_to_pt, pt_to_mm};
use std::collections::BTreeMap;
use std::fmt::Write;
use std::path::Path;

pub async fn generate_pdf(
    cards: &[Flashcard],
    options: &FlashcardOptions,
    output_path: impl AsRef<Path>,
) -> Result<()> {
    let cards = cards.to_vec();
    let options = options.clone();
    let output_path = output_path.as_ref().to_owned();

    let bytes = tokio::task::spawn_blocking(move || generate_flashcard_pdf_bytes(&cards, &options))
        .await??;

    tokio::fs::write(&output_path, bytes).await?;

    Ok(())
}

/// Collect all unique glyphs used across all cards.
/// Returns a map of `glyph_id` -> (char, advance in font units).
fn collect_glyphs(cards: &[Flashcard], face: &ttf_parser::Face<'_>) -> BTreeMap<u16, (char, u16)> {
    let mut glyphs = BTreeMap::new();
    for card in cards {
        for ch in card.front.chars().chain(card.back.chars()) {
            if let Some(glyph_id) = face.glyph_index(ch) {
                glyphs.entry(glyph_id.0).or_insert_with(|| {
                    let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0);
                    (ch, advance)
                });
            }
        }
    }
    glyphs
}

/// Measure text width in points.
fn measure_text_width(text: &str, face: &ttf_parser::Face<'_>, font_size_pt: f32) -> f32 {
    let units_per_em = f32::from(face.units_per_em());
    let mut width = 0.0;
    for ch in text.chars() {
        if let Some(glyph_id) = face.glyph_index(ch) {
            let advance = face.glyph_hor_advance(glyph_id).unwrap_or(0);
            width += (f32::from(advance) / units_per_em) * font_size_pt;
        }
    }
    width
}

/// Encode a text string as hex-encoded glyph IDs for Identity-H CID font.
fn encode_text_hex(text: &str, face: &ttf_parser::Face<'_>) -> String {
    let mut hex = String::with_capacity(text.len() * 4);
    for ch in text.chars() {
        let glyph_id = face.glyph_index(ch).map_or(0, |g| g.0);
        let _ = write!(hex, "{glyph_id:04X}");
    }
    hex
}

/// Build the W (widths) array for CID font from used glyphs.
/// Format: [cid [width] cid [width] ...] for individual entries.
fn build_w_array(glyphs: &BTreeMap<u16, (char, u16)>, units_per_em: u16) -> Object {
    let scale = 1000.0 / f32::from(units_per_em);
    let mut w = Vec::new();
    for (&glyph_id, &(_, advance)) in glyphs {
        w.push(Object::Integer(i64::from(glyph_id)));
        w.push(Object::Array(vec![Object::Integer(
            (f32::from(advance) * scale).round() as i64,
        )]));
    }
    Object::Array(w)
}

/// Build the `ToUnicode` `CMap` stream content.
fn build_tounicode_cmap(glyphs: &BTreeMap<u16, (char, u16)>) -> Vec<u8> {
    let mut cmap = String::new();
    cmap.push_str("/CIDInit /ProcSet findresource begin\n");
    cmap.push_str("12 dict begin\n");
    cmap.push_str("begincmap\n");
    cmap.push_str("/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n");
    cmap.push_str("/CMapName /Adobe-Identity-UCS def\n");
    cmap.push_str("/CMapType 2 def\n");
    cmap.push_str("1 begincodespacerange\n");
    cmap.push_str("<0000> <FFFF>\n");
    cmap.push_str("endcodespacerange\n");

    // Write bfchar entries in chunks of 100 (PDF spec limit per block)
    let entries: Vec<_> = glyphs.iter().collect();
    for chunk in entries.chunks(100) {
        let _ = writeln!(cmap, "{} beginbfchar", chunk.len());
        for (glyph_id, (ch, _)) in chunk {
            let unicode = *ch as u32;
            if unicode <= 0xFFFF {
                let _ = writeln!(cmap, "<{glyph_id:04X}> <{unicode:04X}>");
            } else {
                // Surrogate pair for supplementary plane characters
                let hi = ((unicode - 0x10000) >> 10) + 0xD800;
                let lo = ((unicode - 0x10000) & 0x3FF) + 0xDC00;
                let _ = writeln!(cmap, "<{glyph_id:04X}> <{hi:04X}{lo:04X}>");
            }
        }
        cmap.push_str("endbfchar\n");
    }

    cmap.push_str("endcmap\n");
    cmap.push_str("CMapName currentdict /CMap defineresource pop\n");
    cmap.push_str("end end\n");
    cmap.into_bytes()
}

/// Embed a `TrueType` font as a CID-keyed Type0 font in the document.
/// Returns the `ObjectId` of the Type0 font dictionary.
fn embed_font(
    doc: &mut Document,
    font_bytes: &[u8],
    face: &ttf_parser::Face<'_>,
    glyphs: &BTreeMap<u16, (char, u16)>,
) -> ObjectId {
    let units_per_em = face.units_per_em();
    let scale = 1000.0 / f32::from(units_per_em);
    let bbox = face.global_bounding_box();

    // FontFile2: the raw TTF data, compressed
    let mut font_stream = Stream::new(Dictionary::new(), font_bytes.to_vec());
    font_stream
        .dict
        .set("Length1", Object::Integer(font_bytes.len() as i64));
    let _ = font_stream.compress();
    let font_file2_id = doc.add_object(font_stream);

    // FontDescriptor
    let mut fd = Dictionary::new();
    fd.set("Type", Object::Name(b"FontDescriptor".to_vec()));
    fd.set("FontName", Object::Name(b"NotoSansJP-Bold".to_vec()));
    fd.set("Flags", Object::Integer(4)); // Symbolic
    fd.set(
        "FontBBox",
        Object::Array(vec![
            Object::Integer((f32::from(bbox.x_min) * scale).round() as i64),
            Object::Integer((f32::from(bbox.y_min) * scale).round() as i64),
            Object::Integer((f32::from(bbox.x_max) * scale).round() as i64),
            Object::Integer((f32::from(bbox.y_max) * scale).round() as i64),
        ]),
    );
    fd.set("ItalicAngle", Object::Integer(0));
    fd.set(
        "Ascent",
        Object::Integer((f32::from(face.ascender()) * scale).round() as i64),
    );
    fd.set(
        "Descent",
        Object::Integer((f32::from(face.descender()) * scale).round() as i64),
    );
    fd.set(
        "CapHeight",
        Object::Integer(
            face.capital_height()
                .map_or(700, |h| (f32::from(h) * scale).round() as i64),
        ),
    );
    fd.set("StemV", Object::Integer(80));
    fd.set("FontFile2", Object::Reference(font_file2_id));
    let fd_id = doc.add_object(fd);

    // CIDSystemInfo
    let cid_system_info = Dictionary::from_iter(vec![
        (
            "Registry",
            Object::String(b"Adobe".to_vec(), lopdf::StringFormat::Literal),
        ),
        (
            "Ordering",
            Object::String(b"Identity".to_vec(), lopdf::StringFormat::Literal),
        ),
        ("Supplement", Object::Integer(0)),
    ]);

    // CIDFont (DescendantFont)
    let mut cid_font = Dictionary::new();
    cid_font.set("Type", Object::Name(b"Font".to_vec()));
    cid_font.set("Subtype", Object::Name(b"CIDFontType2".to_vec()));
    cid_font.set("BaseFont", Object::Name(b"NotoSansJP-Bold".to_vec()));
    cid_font.set("CIDSystemInfo", Object::Dictionary(cid_system_info));
    cid_font.set("W", build_w_array(glyphs, units_per_em));
    cid_font.set("DW", Object::Integer((1000.0_f32 * scale).round() as i64));
    cid_font.set("FontDescriptor", Object::Reference(fd_id));
    // CIDToGIDMap: Identity mapping (glyph IDs = CIDs)
    cid_font.set("CIDToGIDMap", Object::Name(b"Identity".to_vec()));
    let cid_font_id = doc.add_object(cid_font);

    // ToUnicode CMap
    let cmap_data = build_tounicode_cmap(glyphs);
    let mut cmap_stream = Stream::new(Dictionary::new(), cmap_data);
    let _ = cmap_stream.compress();
    let tounicode_id = doc.add_object(cmap_stream);

    // Type0 font (top-level)
    let mut type0 = Dictionary::new();
    type0.set("Type", Object::Name(b"Font".to_vec()));
    type0.set("Subtype", Object::Name(b"Type0".to_vec()));
    type0.set("BaseFont", Object::Name(b"NotoSansJP-Bold".to_vec()));
    type0.set("Encoding", Object::Name(b"Identity-H".to_vec()));
    type0.set(
        "DescendantFonts",
        Object::Array(vec![Object::Reference(cid_font_id)]),
    );
    type0.set("ToUnicode", Object::Reference(tounicode_id));
    doc.add_object(type0)
}

fn generate_flashcard_pdf_bytes(
    cards: &[Flashcard],
    options: &FlashcardOptions,
) -> Result<Vec<u8>> {
    let font_bytes: &[u8] = include_bytes!("../fonts/NotoSansJP-Bold.ttf");
    let face = ttf_parser::Face::parse(font_bytes, 0)
        .map_err(|e| FlashcardError::Pdf(format!("Failed to parse font: {e}")))?;

    let mut doc = Document::with_version("1.7");
    let pages_tree_id = doc.new_object_id();

    // Collect all glyphs used across all cards
    let glyphs = collect_glyphs(cards, &face);

    // Embed font
    let font_id = embed_font(&mut doc, font_bytes, &face, &glyphs);

    // Build Resources dictionary (shared across all pages)
    let mut font_dict = Dictionary::new();
    font_dict.set("F1", Object::Reference(font_id));
    let mut resources = Dictionary::new();
    resources.set("Font", Object::Dictionary(font_dict));
    let resources_id = doc.add_object(resources);

    let cards_per_page = options.rows * options.columns;
    let page_width_pt = mm_to_pt(options.page_width_mm);
    let page_height_pt = mm_to_pt(options.page_height_mm);

    let mut page_refs = Vec::new();

    for chunk in cards.chunks(cards_per_page) {
        let mut front_ops = String::new();
        let mut back_ops = String::new();

        for (i, card) in chunk.iter().enumerate() {
            let row = i / options.columns;
            let col = i % options.columns;

            // Front side positioning
            let cell_x_front = options.margin_left_mm
                + col as f32 * (options.card_width_mm + options.column_spacing_mm);
            let cell_y_front = options.page_height_mm
                - options.margin_top_mm
                - (row + 1) as f32 * options.card_height_mm
                - row as f32 * options.row_spacing_mm;

            let center_x_front = cell_x_front + options.card_width_mm / 2.0;
            let y_front =
                cell_y_front + (options.card_height_mm - options.font_size_pt * 25.4 / 72.0) / 2.0;

            let text_width_front = measure_text_width(&card.front, &face, options.font_size_pt);
            let text_width_mm_front = pt_to_mm(text_width_front);
            let x_front = center_x_front - text_width_mm_front / 2.0;

            let hex_front = encode_text_hex(&card.front, &face);
            let x_pt = mm_to_pt(x_front);
            let y_pt = mm_to_pt(y_front);
            let _ = writeln!(
                front_ops,
                "BT /F1 {} Tf {} {} Td <{hex_front}> Tj ET",
                options.font_size_pt, x_pt, y_pt
            );

            // Back side positioning (mirrored horizontally)
            let mirrored_col = options.columns - 1 - col;
            let cell_x_back = options.margin_right_mm
                + mirrored_col as f32 * (options.card_width_mm + options.column_spacing_mm);
            let cell_y_back = cell_y_front;

            let center_x_back = cell_x_back + options.card_width_mm / 2.0;
            let y_back =
                cell_y_back + (options.card_height_mm - options.font_size_pt * 25.4 / 72.0) / 2.0;

            let text_width_back = measure_text_width(&card.back, &face, options.font_size_pt);
            let text_width_mm_back = pt_to_mm(text_width_back);
            let x_back = center_x_back - text_width_mm_back / 2.0;

            let hex_back = encode_text_hex(&card.back, &face);
            let x_pt = mm_to_pt(x_back);
            let y_pt = mm_to_pt(y_back);
            let _ = writeln!(
                back_ops,
                "BT /F1 {} Tf {} {} Td <{hex_back}> Tj ET",
                options.font_size_pt, x_pt, y_pt
            );
        }

        // Create content streams
        let front_stream = Stream::new(Dictionary::new(), front_ops.into_bytes());
        let front_content_id = doc.add_object(front_stream);

        let back_stream = Stream::new(Dictionary::new(), back_ops.into_bytes());
        let back_content_id = doc.add_object(back_stream);

        // Create front page
        let mut front_page = Dictionary::new();
        front_page.set("Type", Object::Name(b"Page".to_vec()));
        front_page.set("Parent", Object::Reference(pages_tree_id));
        front_page.set(
            "MediaBox",
            Object::Array(vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Real(page_width_pt),
                Object::Real(page_height_pt),
            ]),
        );
        front_page.set("Resources", Object::Reference(resources_id));
        front_page.set("Contents", Object::Reference(front_content_id));
        let front_page_id = doc.add_object(front_page);
        page_refs.push(Object::Reference(front_page_id));

        // Create back page
        let mut back_page = Dictionary::new();
        back_page.set("Type", Object::Name(b"Page".to_vec()));
        back_page.set("Parent", Object::Reference(pages_tree_id));
        back_page.set(
            "MediaBox",
            Object::Array(vec![
                Object::Integer(0),
                Object::Integer(0),
                Object::Real(page_width_pt),
                Object::Real(page_height_pt),
            ]),
        );
        back_page.set("Resources", Object::Reference(resources_id));
        back_page.set("Contents", Object::Reference(back_content_id));
        let back_page_id = doc.add_object(back_page);
        page_refs.push(Object::Reference(back_page_id));
    }

    // Finalize document structure
    let count = page_refs.len() as i64;
    let pages_dict = Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Pages".to_vec())),
        ("Kids", Object::Array(page_refs)),
        ("Count", Object::Integer(count)),
    ]);
    doc.objects
        .insert(pages_tree_id, Object::Dictionary(pages_dict));

    let catalog_id = doc.add_object(Dictionary::from_iter(vec![
        ("Type", Object::Name(b"Catalog".to_vec())),
        ("Pages", Object::Reference(pages_tree_id)),
    ]));
    doc.trailer.set("Root", catalog_id);

    let mut writer = Vec::new();
    doc.save_to(&mut writer)
        .map_err(|e| FlashcardError::Pdf(format!("Failed to save PDF: {e}")))?;

    Ok(writer)
}
