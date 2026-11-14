//! Reference following retains the shared reader's repaired-header and later-content rules.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use craft_pdf::raster::{self, Options, Raster, RenderCache};
use craft_pdf::syntax::{
    Pdf,
    object::{Object, ObjectIdentifier},
};

fn document(alias: &str, contents: &str) -> Pdf {
    let red = "1 0 0 rg 0 0 20 20 re f";
    let green = "0 1 0 rg 20 0 20 20 re f";
    let bytes = format!(
        "%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n\
         2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n\
         3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 40 20] /Contents {contents} >> endobj\n\
         {alias}\n\
         5 0 obj << /Length {} >> stream\n{red}\nendstream endobj\n\
         6 0 obj << /Length {} >> stream\n{green}\nendstream endobj\n\
         trailer << /Root 1 0 R >>\n%%EOF",
        red.len(),
        green.len()
    );
    craft_pdf::open_slice(bytes.as_bytes(), None).unwrap()
}

fn render(pdf: &Pdf) -> Raster {
    raster::render(&pdf.pages()[0], &RenderCache::new(), &Default::default(), &Options { background: [255; 4], ..Default::default() }).unwrap()
}

fn pixel(image: &Raster, x: usize) -> &[u8] {
    &image.bytes()[x * 4..(x + 1) * 4]
}

#[test]
fn a_reference_behind_an_invalid_generation_header_is_read_only_after_repair() {
    let pdf = document("4 18446744073709551616 obj 5 0 R endobj", "[4 0 R 6 0 R]");
    let image = render(&pdf);
    assert_eq!(pixel(&image, 5), [255, 0, 0, 255]);
    assert_eq!(pixel(&image, 25), [0, 255, 0, 255]);
}

#[test]
fn a_reference_cycle_does_not_block_a_later_object_read() {
    let pdf = document("4 0 obj 7 0 R endobj\n7 0 obj 4 0 R endobj", "6 0 R");
    assert!(pdf.xref().get::<Object<'_>>(ObjectIdentifier::new(4, 0)).is_none());
    assert!(pdf.xref().get::<Object<'_>>(ObjectIdentifier::new(5, 0)).is_some());
    let image = render(&pdf);
    assert_eq!(pixel(&image, 5), [255; 4]);
    assert_eq!(pixel(&image, 25), [0, 255, 0, 255]);
}
