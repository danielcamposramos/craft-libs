//! Original CMYK group review controls from PdfCraft #3777 and #5228.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
struct RenderedPage {
    rgba: Vec<u8>,
    width: u32,
}
fn render_shared(bytes: String, scale: f32) -> RenderedPage {
    let pdf = craft_pdf::syntax::Pdf::new(bytes.into_bytes()).unwrap();
    let p = craft_pdf::raster::render(
        &pdf.pages()[0],
        &craft_pdf::raster::RenderCache::new(),
        &craft_pdf::interpret::InterpreterSettings::default(),
        &craft_pdf::raster::Options { scale: [scale; 2], background: [255; 4], ..Default::default() },
    )
    .unwrap();
    RenderedPage { rgba: p.bytes().to_vec(), width: p.width }
}
/// An element of a knockout group is first drawn in erase mode, which clears what the group
/// drew under the element's shape. An isolated `DeviceCMYK` group nested in that element must
/// clear too: when the erase pass drew it through the CMYK path, it painted over the red
/// beneath instead of clearing it. The page must match the same element with its nested group
/// blending in `DeviceRGB`, which never takes the CMYK path.
#[test]
fn an_isolated_cmyk_group_inside_a_knockout_element_is_erased_like_any_other() {
    let render = |inner_cs: &str| {
        let knockout = "1 0 0 rg 20 20 160 160 re f /El Do";
        let element = "/Cm Do";
        let inner = "/A gs 0 1 0 0 k 60 60 80 80 re f";
        let pdf = format!(
            "%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] /Contents 4 0 R /Resources << /XObject << /Ko 5 0 R >> >> >> endobj
4 0 obj << /Length 7 >> stream
/Ko Do
endstream endobj
5 0 obj << /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Group << /S /Transparency /I true /K true >> /Resources << /XObject << /El 6 0 R >> >> /Length {} >> stream
{knockout}
endstream endobj
6 0 obj << /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Group << /S /Transparency >> /Resources << /XObject << /Cm 7 0 R >> >> /Length {} >> stream
{element}
endstream endobj
7 0 obj << /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Group << /S /Transparency /CS {inner_cs} /I true >> /Resources << /ExtGState << /A << /ca 0.5 >> >> >> /Length {} >> stream
{inner}
endstream endobj
trailer << /Root 1 0 R >>
%%EOF",
            knockout.len(),
            element.len(),
            inner.len()
        );
        let p = render_shared(pdf, 1.0);
        // The centre of the inner square, (100, 100), is also the centre of the page.
        p.rgba[((100 * p.width + 100) * 4) as usize..][..3].to_vec()
    };
    let rgb = render("/DeviceRGB");
    let cmyk = render("/DeviceCMYK");
    assert!(rgb[2] > 150, "the RGB control shows half-opacity magenta over white, not red: {rgb:?}");
    assert!(rgb.iter().zip(&cmyk).all(|(a, b)| a.abs_diff(*b) <= 3), "CMYK {cmyk:?}, RGB {rgb:?}");
}
