//! A refused opacity stream must not reveal a picture, nor poison the next page's allowance.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use craft_pdf::interpret::InterpreterSettings;
use craft_pdf::raster::{self, Options, RenderCache};

#[test]
fn a_spent_jbig2_mask_hides_its_picture_and_the_next_page_starts_fresh() {
    let mut segments = Vec::new();
    segments.extend_from_slice(&0_u32.to_be_bytes());
    segments.extend_from_slice(&[48, 0, 1]);
    segments.extend_from_slice(&19_u32.to_be_bytes());
    segments.extend_from_slice(&1_u32.to_be_bytes());
    segments.extend_from_slice(&1_u32.to_be_bytes());
    segments.extend_from_slice(&[0; 11]);
    let encoded = segments.iter().map(|byte| format!("{byte:02X}")).collect::<String>() + ">";
    let content = "q 40 0 0 20 0 0 cm /Im Do Q";
    let bytes = format!(
        "%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 40 20] /Resources << /XObject << /Im 5 0 R >> >> /Contents 4 0 R >> endobj\n4 0 obj << /Length {} >> stream\n{content}\nendstream endobj\n5 0 obj << /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /SMask 6 0 R /Filter /ASCIIHexDecode /Length 7 >> stream\nFF0000>\nendstream endobj\n6 0 obj << /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter [/ASCIIHexDecode /JBIG2Decode] /Length {} >> stream\n{encoded}\nendstream endobj\ntrailer << /Root 1 0 R >>\n%%EOF",
        content.len(),
        encoded.len()
    );
    let pdf = craft_pdf::open_slice(bytes.as_bytes(), None).unwrap();
    let page = &pdf.pages()[0];
    let cache = RenderCache::new();
    let options = Options { background: [255; 4], ..Default::default() };
    let refused = raster::render(page, &cache, &InterpreterSettings { jbig2_work: 0, ..Default::default() }, &options).unwrap();
    assert_eq!(&refused.bytes()[0..4], [255; 4]);
    assert!(refused.diagnostics.content_truncated);
    assert!(!craft_pdf::syntax::page_jbig2_work_spent(), "the page allowance ends even after a refusal");
    let fresh = raster::render(page, &cache, &InterpreterSettings::default(), &options).unwrap();
    assert_eq!(&fresh.bytes()[0..4], [255, 0, 0, 255]);
    assert!(fresh.diagnostics.is_empty());
}
