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

/// From the review of #5228: a shading pattern painted before the group left its pattern
/// transform on the context, and the finished group was drawn through it, so the cyan half
/// of the group showed magenta. The group must look the same whether or not a pattern was
/// painted first.
#[test]
fn a_pattern_painted_before_an_isolated_cmyk_group_does_not_move_it() {
    let group = "0 1 0 0 k 20 20 30 60 re f 1 0 0 0 k 50 20 30 60 re f";
    let render = |page: &str| cmyk_group_pixel([100, 100], page, [20, 20, 80, 80], group, "", (55, 50));
    let after_pattern = render("/Pattern cs /P scn 0 0 5 5 re f /G Do");
    let alone = render("/G Do");
    assert!(alone[2] > 150 && alone[0] < 100, "the control is cyan: {alone:?}");
    assert!(alone.iter().zip(&after_pattern).all(|(a, b)| a.abs_diff(*b) <= 3), "after a pattern {after_pattern:?}, alone {alone:?}");
}

/// From the review of #5228: the Color blend mode over an isolated group's empty backdrop
/// keeps the source colour (ISO 32000-2 §11.3.5.3 with nothing beneath). The K ink pass
/// draws gray, and vello_cpu's nonseparable blend of a gray darkened some K levels to
/// near black. Every 8-bit K level is checked against a plain fill of the same ink.
#[test]
fn the_color_blend_mode_over_an_empty_cmyk_group_keeps_every_k_level() {
    let mut wrong = Vec::new();
    for level in 0..=255u8 {
        let k = format!("0 0 0 {} k", f32::from(level) / 255.0);
        let page = format!("{k} 0 0 20 20 re f /G Do");
        let group = format!("/B gs {k} 20 0 20 20 re f");
        let plain = cmyk_group_pixel([40, 20], &page, [20, 0, 40, 20], &group, "/ExtGState << /B << /BM /Color >> >>", (10, 10));
        let blended = cmyk_group_pixel([40, 20], &page, [20, 0, 40, 20], &group, "/ExtGState << /B << /BM /Color >> >>", (30, 10));
        if plain.iter().zip(&blended).any(|(a, b)| a.abs_diff(*b) > 3) {
            wrong.push((level, plain, blended));
        }
    }
    assert!(wrong.is_empty(), "K levels drawn wrong (level, plain, blended): {wrong:?}");
}
