//! Original synthetic property-scope controls from Ahmad Mujahid's upstream subject.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use craft_pdf::{interpret::InterpreterSettings, raster::RenderCache, syntax::Pdf};
use hayro::{RenderSettings, vello_cpu::color::palette::css::WHITE};
use std::sync::Arc;
fn pdf(objects: &[String]) -> Vec<u8> {
    let mut bytes = b"%PDF-1.7\n".to_vec();
    let mut offsets = Vec::new();
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n{object}\nendobj\n", i + 1).as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(format!("xref\n0 {}\n0000000000 65535 f \n", objects.len() + 1).as_bytes());
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(format!("trailer << /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n", objects.len() + 1).as_bytes());
    bytes
}
fn stream(dict: &str, body: &str) -> String {
    format!("<< {dict} /Length {} >>\nstream\n{body}\nendstream", body.len())
}
fn resources(properties: &str) -> String {
    format!("<< /Font << /F1 3 0 R >> /XObject << /Fm 6 0 R /Inner 7 0 R >> {properties} >>")
}
fn form(resources: &str, body: &str) -> String {
    stream(&format!("/Type /XObject /Subtype /Form /BBox [0 0 200 200] {resources}"), body)
}
fn objects(page_resources: &str, form_resources: &str, body: &str) -> Vec<String> {
    vec![
        "<< /Type /Catalog /Pages 2 0 R /OCProperties << /OCGs [8 0 R 9 0 R] /D << /OFF [8 0 R] >> >> >>".into(),
        "<< /Type /Pages /Kids [4 0 R] /Count 1 >>".into(),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>".into(),
        format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 5 0 R {page_resources} >>"),
        stream("", "/Fm Do"),
        form(form_resources, body),
        form("", GLYPHS),
        "<< /Type /OCG /Name (PageOff) >>".into(),
        "<< /Type /OCG /Name (FormOn) >>".into(),
    ]
}
const GLYPHS: &str = "unused in pixel controls";
const OFF_PAINT: &str = "/OC /O BDC 1 0 0 rg 10 100 20 20 re f EMC 0 0 1 rg 50 100 20 20 re f";
fn assert_paint(bytes: Vec<u8>, page: usize, layers: Vec<(i32, i32, bool)>, visible: bool) {
    let parsed = Pdf::new(bytes).unwrap();
    let settings = InterpreterSettings { ocg_overrides: Arc::new(layers), ..Default::default() };
    let image = hayro::render(&parsed.pages()[page], &RenderCache::new(), &settings, &RenderSettings { bg_color: WHITE, ..Default::default() });
    assert_eq!((image.width(), image.height()), (200, 200));
    let rgba = image.data_as_u8_slice();
    let pixel = |x: usize, y: usize| &rgba[(y * 200 + x) * 4..(y * 200 + x) * 4 + 4];
    assert_eq!(pixel(60, 90), [0, 0, 255, 255], "unmarked control remains blue");
    assert_eq!(pixel(20, 90), if visible { [255, 0, 0, 255] } else { [255; 4] });
}
#[test]
fn indirect_page_properties_preserve_ocg_identity_and_visibility_overrides() {
    let page = format!("/Resources {}", resources("/Properties 10 0 R"));
    let mut fixture = objects(&page, "", OFF_PAINT);
    fixture.push("<< /O 8 0 R >>".into());
    let bytes = pdf(&fixture);
    assert_paint(bytes.clone(), 0, vec![], false);
    assert_paint(bytes.clone(), 0, vec![(9, 0, true)], false);
    assert_paint(bytes.clone(), 0, vec![(8, 0, true)], true);
    assert_paint(bytes, 0, vec![(8, 0, false)], false);
}
#[test]
fn nested_ocg_uses_page_origin_and_own_dictionary_boundaries() {
    let page = format!("/Resources {}", resources("/Properties << /O 8 0 R >>"));
    let caller = format!("/Resources {}", resources("/Properties << /O 9 0 R >>"));
    for (inner, visible) in [
        ("", false),
        ("/Resources << /Properties << /O 9 0 R >> >>", true),
        ("/Resources << >>", true),
        ("/Resources << /Properties << /Other 8 0 R >> >>", true),
        ("/Resources << /Properties << /O 42 >> >>", true),
        ("/Resources << /Properties << /O 99 0 R >> >>", true),
    ] {
        let mut fixture = objects(&page, &caller, "/Inner Do");
        fixture[6] = form(inner, OFF_PAINT);
        assert_paint(pdf(&fixture), 0, vec![], visible);
    }
}
