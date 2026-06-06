//! Consumer-side wiring test: proves that print-junk's build correctly vendors
//! and binds `PDFium` *through the shared `junk-libs-pdfium` crate* at runtime.
//! The render core itself is unit-tested in junk-libs; this only guards that the
//! binary is reachable from a print-junk test binary (the "clean build doesn't
//! prove rendering works" gotcha).
#![cfg(all(not(target_arch = "wasm32"), feature = "pdf-viewer"))]

/// Minimal valid one-page PDF (612×792), enough to bind, load, and render.
const SAMPLE_PDF: &[u8] = b"%PDF-1.4
1 0 obj
<<
/Type /Catalog
/Pages 2 0 R
>>
endobj
2 0 obj
<<
/Type /Pages
/Kids [3 0 R]
/Count 1
>>
endobj
3 0 obj
<<
/Type /Page
/Parent 2 0 R
/Resources <<
/Font <<
/F1 <<
/Type /Font
/Subtype /Type1
/BaseFont /Helvetica
>>
>>
>>
/MediaBox [0 0 612 792]
/Contents 4 0 R
>>
endobj
4 0 obj
<<
/Length 44
>>
stream
BT
/F1 24 Tf
100 700 Td
(Hello World) Tj
ET
endstream
endobj
xref
0 5
0000000000 65535 f
0000000009 00000 n
0000000058 00000 n
0000000115 00000 n
0000000317 00000 n
trailer
<<
/Size 5
/Root 1 0 R
>>
startxref
410
%%EOF
";

#[test]
fn pdfium_binds_and_renders_through_junk_libs() {
    let pdfium = junk_libs_pdfium::instance().expect("bind vendored PDFium via junk-libs-pdfium");

    let (image, (width_pts, height_pts)) =
        junk_libs_pdfium::render_page_bitmap_from_bytes(pdfium, SAMPLE_PDF, 0, 1.0)
            .expect("render the sample PDF");

    // MediaBox is 612×792 pt; at scale 1.0 the raster matches in pixels.
    assert!(
        (width_pts - 612.0).abs() < 2.0 && (height_pts - 792.0).abs() < 2.0,
        "unexpected page size in points: {width_pts}×{height_pts}"
    );
    assert!(
        image.width() > 0 && image.height() > 0,
        "degenerate raster {}×{}",
        image.width(),
        image.height()
    );
    // The page has content, so the raster must contain non-blank pixels.
    let non_zero = image.as_raw().iter().filter(|&&b| b != 0).count();
    assert!(non_zero > 0, "rendered image was entirely zero");
}
