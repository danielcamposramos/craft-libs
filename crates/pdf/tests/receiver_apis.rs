//! Receiver behaviors reconciled into the common reader; all documents are synthetic.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::sync::{Arc, Mutex};

use craft_pdf::interpret::InterpreterSettings;
use craft_pdf::raster::{self, Options, Raster, RenderCache};

fn render(resources: &str, content: &str, extra: &str) -> (Raster, Vec<String>) {
    let bytes = format!(
        "%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 40 20] /Contents 4 0 R /Resources << {resources} >> >> endobj\n4 0 obj << /Length {} >> stream\n{content}\nendstream endobj\n{extra}\ntrailer << /Root 1 0 R >>\n%%EOF",
        content.len()
    );
    let pdf = craft_pdf::open_slice(bytes.as_bytes(), None).unwrap();
    let warnings = Arc::new(Mutex::new(Vec::new()));
    let sink = warnings.clone();
    let settings = InterpreterSettings { warning_sink: Arc::new(move |w| sink.lock().unwrap().push(format!("{w:?}"))), ..Default::default() };
    let image = raster::render(&pdf.pages()[0], &RenderCache::new(), &settings, &Options { background: [255; 4], ..Default::default() }).unwrap();
    let warnings = warnings.lock().unwrap().clone();
    (image, warnings)
}

fn pixel(image: &Raster, x: usize, y: usize) -> &[u8] {
    &image.bytes()[(y * image.width as usize + x) * 4..(y * image.width as usize + x + 1) * 4]
}

#[test]
fn an_unreadable_tint_uses_the_alternate_initial_colour_and_reports_it() {
    let (image, warnings) = render(
        "/ColorSpace << /Bad [/Separation /Spot /DeviceRGB 5 0 R] >>",
        "/Bad cs 1 scn 0 0 10 20 re f 0 1 0 rg 20 0 10 20 re f",
        "5 0 obj << /FunctionType 999 >> endobj",
    );
    assert_eq!(pixel(&image, 5, 5), [0, 0, 0, 255]);
    assert_eq!(pixel(&image, 25, 5), [0, 255, 0, 255]);
    assert!(warnings.iter().any(|w| w == "ColorFallback"));
    assert!(!image.diagnostics.is_empty());
}

#[test]
fn a_short_indexed_lookup_is_omitted_with_a_diagnostic_and_later_art_still_draws() {
    let (image, warnings) = render(
        "/XObject << /Im 5 0 R >>",
        "q 20 0 0 20 0 0 cm /Im Do Q 0 1 0 rg 30 0 10 20 re f",
        "5 0 obj << /Type /XObject /Subtype /Image /Width 2 /Height 1 /ColorSpace [/Indexed /DeviceRGB 7 <FF000000FF00>] /BitsPerComponent 8 /Filter /ASCIIHexDecode /Length 5 >> stream\n0001>\nendstream endobj",
    );
    assert_eq!(pixel(&image, 5, 5), [255; 4]);
    assert_eq!(pixel(&image, 35, 5), [0, 255, 0, 255]);
    assert!(warnings.iter().any(|w| w == "MalformedIndexedImage"));
    assert!(!image.diagnostics.is_empty());
}

#[test]
fn a_type3_without_widths_advances_by_its_procedure_and_reports_the_malformed_font() {
    let glyph = "10 0 d0 1 0 0 rg 0 0 10 10 re f";
    let extra = format!(
        "5 0 obj << /Type /Font /Subtype /Type3 /FontBBox [0 0 10 10] /FontMatrix [0.1 0 0 0.1 0 0] /Encoding << /Type /Encoding /Differences [65 /A] >> /CharProcs << /A 6 0 R >> >> endobj\n6 0 obj << /Length {} >> stream\n{glyph}\nendstream endobj",
        glyph.len()
    );
    let (image, warnings) = render("/Font << /F 5 0 R >>", "BT /F 10 Tf 1 0 0 1 0 0 Tm (AA) Tj ET", &extra);
    assert_eq!(pixel(&image, 5, 15), [255, 0, 0, 255]);
    assert_eq!(pixel(&image, 15, 15), [255, 0, 0, 255]);
    assert!(warnings.iter().any(|w| w == "Type3WidthsMissing"));
    assert!(!image.diagnostics.is_empty());
}
