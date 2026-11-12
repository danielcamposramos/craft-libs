//! Original synthetic controls for PdfCraft RGB-CMYK and reviewed group state.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
/// A minimal page with `page` as its content, a shading pattern `/P` and an isolated
/// `DeviceCMYK` group `/G` over `bbox` drawing `group`; the RGB of device pixel (`x`, `y`).
fn cmyk_group_pixel(media: [u32; 2], page: &str, bbox: [u32; 4], group: &str, group_resources: &str, (x, y): (u32, u32)) -> Vec<u8> {
    let pdf = format!(
        "%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 {} {}] /Contents 4 0 R /Resources << /XObject << /G 5 0 R >> /Pattern << /P 6 0 R >> >> >> endobj
4 0 obj << /Length {} >> stream
{page}
endstream endobj
5 0 obj << /Type /XObject /Subtype /Form /BBox [{} {} {} {}] /Group << /S /Transparency /CS /DeviceCMYK /I true >> /Resources << {group_resources} >> /Length {} >> stream
{group}
endstream endobj
6 0 obj << /Type /Pattern /PatternType 2 /Matrix [2 0 0 2 30 40] /Shading << /ShadingType 2 /ColorSpace /DeviceRGB /Coords [0 0 10 0] /Extend [true true] /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >> >> endobj
trailer << /Root 1 0 R >>
%%EOF",
        media[0],
        media[1],
        page.len(),
        bbox[0],
        bbox[1],
        bbox[2],
        bbox[3],
        group.len()
    );
    let pdf = craft_pdf::syntax::Pdf::new(pdf.into_bytes()).unwrap();
    let p = craft_pdf::raster::render(
        &pdf.pages()[0],
        &craft_pdf::raster::RenderCache::new(),
        &craft_pdf::interpret::InterpreterSettings::default(),
        &craft_pdf::raster::Options::default(),
    )
    .unwrap();
    p.bytes()[((y * p.width + x) * 4) as usize..][..3].to_vec()
}

/// An RGB colour painted in an isolated `DeviceCMYK` group is blended as the process ink that
/// draws back to it through the CMYK profile (hayro-interpret patch (104)), instead of
/// sending the whole group back to RGB blending.
///
/// Under `/BM /Difference` with black ink, a colour's cyan, magenta and yellow are
/// complemented and its black kept (ISO 32000-2 §11.3.4), so 50% grey, mostly cyan, magenta
/// and yellow ink, comes out a warm grey: neither the neutral grey of RGB blending nor the
/// near-black of a grey made from black ink alone. Acrobat draws the same blend on a CMYK page
/// as (137, 112, 106). Painted without a blend, a colour inside the profile's gamut draws as
/// itself, and pure red is clipped to the profile's red, as in Acrobat.
#[test]
fn an_rgb_colour_in_an_isolated_cmyk_group_blends_as_its_ink() {
    let resources = "/ExtGState << /D << /BM /Difference >> >>";
    let pixel = |group: &str| cmyk_group_pixel([100, 100], "/G Do", [20, 20, 80, 80], group, resources, (50, 50));
    let near = |a: &[u8], b: [u8; 3], within: u8| a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= within);

    let blended = pixel("0.5 0.5 0.5 rg 20 20 60 60 re f /D gs 0 0 0 1 k 20 20 60 60 re f");
    assert!(near(&blended, [137, 112, 106], 10), "grey under Difference with black: {blended:?}");

    // (colour, expected pixel, how far from it)
    for (rgb, expected, within) in [("0.8 0.4 0.2", [204, 102, 51], 3), ("0.3 0.7 0.4", [77, 179, 102], 3), ("1 0 0", [238, 32, 37], 8)] {
        let drawn = pixel(&format!("{rgb} rg 20 20 60 60 re f"));
        assert!(near(&drawn, expected, within), "{rgb} drew as {drawn:?}, expected {expected:?}");
    }
}

/// An RGB colour with a non-finite component has no tints: converting it to RGB first would
/// give some finite colour that is not its ink.
#[test]
fn an_rgb_colour_with_a_non_finite_component_has_no_tints() {
    use craft_pdf::interpret::color::AlphaColor;
    use craft_pdf::interpret::color::Color;
    let rgb = |c: [f32; 3]| Color::from_rgba(AlphaColor::new([c[0], c[1], c[2], 1.0])).ink_tints();
    assert!(rgb([0.5, 0.5, 0.5]).is_some());
    assert_eq!(rgb([f32::NAN, 0.5, 0.5]), None);
    assert_eq!(rgb([0.5, f32::INFINITY, 0.5]), None);
}
