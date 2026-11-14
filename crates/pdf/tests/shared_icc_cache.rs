//! Original receiver cache regression at the common owner's existing eight-level bound.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use craft_pdf::raster::{self, Options, RenderCache};

#[test]
fn a_profile_first_met_at_the_shared_depth_limit_keeps_its_near_alternate() {
    // Seven Separation levels put the profile at depth seven; its alternate is refused
    // at the existing depth-eight bound, without poisoning a later top-level read.
    let chain: String = (0..7).map(|i| format!("{} 0 obj [/Separation /A {} 0 R 8 0 R] endobj\n", 10 + i, 11 + i)).collect();
    let content = "/Deep cs /Near cs 1 scn 0 0 40 20 re f";
    let pdf = format!(
        "%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n\
         2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n\
         3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 40 20] /Resources << /ColorSpace << /Deep 10 0 R /Near 17 0 R >> >> /Contents 4 0 R >> endobj\n\
         4 0 obj << /Length {} >> stream\n{content}\nendstream endobj\n\
         7 0 obj << /N 1 /Alternate [/Separation /A /DeviceRGB 9 0 R] /Length 1 >> stream\nx\nendstream endobj\n\
         8 0 obj << /FunctionType 2 /Domain [0 1] /N 1 /C0 [0] /C1 [1] >> endobj\n\
         9 0 obj << /FunctionType 2 /Domain [0 1] /N 1 /C0 [1 1 1] /C1 [0 1 0] >> endobj\n\
         {chain}17 0 obj [/ICCBased 7 0 R] endobj\n\
         trailer << /Root 1 0 R >>\n%%EOF",
        content.len()
    );
    let pdf = craft_pdf::open_slice(pdf.as_bytes(), None).unwrap();
    let image =
        raster::render(&pdf.pages()[0], &RenderCache::new(), &Default::default(), &Options { background: [255; 4], ..Default::default() }).unwrap();
    assert_eq!(image.bytes().get(..4), Some(&[0, 255, 0, 255][..]));
}
