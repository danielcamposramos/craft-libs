//! Original missing-MediaBox behavior from PdfCraft cdb94e66, through the shared reader.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used)]
use craft_pdf::{
    interpret::InterpreterSettings,
    raster::{self, Options, RenderCache},
    syntax::Pdf,
};
#[test]
fn a_page_without_a_media_box_is_letter() {
    for media in ["", "/MediaBox [0 0 0 0]"] {
        let body = "0 0 1 rg 10 10 50 50 re f";
        let bytes = format!(
            "%PDF-1.4\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R {media} /Contents 4 0 R >> endobj\n4 0 obj << /Length {} >> stream\n{body}\nendstream endobj\ntrailer << /Root 1 0 R >>\n%%EOF",
            body.len()
        );
        let pdf = Pdf::new(bytes.into_bytes()).unwrap();
        assert_eq!(pdf.pages()[0].base_dimensions(), (612.0, 792.0), "{media:?}");
        let page = raster::render(&pdf.pages()[0], &RenderCache::new(), &InterpreterSettings::default(), &Options::default()).unwrap();
        assert_eq!((page.width, page.height), (612, 792));
        let at = ((762 * page.width + 30) * 4) as usize;
        if media.is_empty() {
            assert_eq!(&page.bytes()[at..at + 4], &[0, 0, 255, 255]);
        }
    }
}
