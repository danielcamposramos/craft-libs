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
/// Two cases from the review of #3777. A group wider than about two of vello_cpu's tiles lost
/// its blend mode in the tiles a paint covered completely, which drew black over magenta at
/// 300 dpi where Difference gives green; each pass now draws inside a layer. And a nested
/// isolated group that names another blending space (here DeviceRGB) blends in that space
/// (ISO 32000-2 §11.3.4), so the enclosing CMYK group is drawn as before; a non-isolated
/// one ignores its /CS and blends in the enclosing group's CMYK (Table 145).
#[test]
fn isolated_cmyk_groups_blend_at_any_size_and_respect_nested_spaces() {
    let render = |inner_cs: Option<&str>, scale: f32| {
        // `inner_cs`: the nested group's /CS and /I entries.
        let group = "0 1 0 0 k 0 0 200 200 re f /Diff gs 0 0 0 1 k 0 0 200 200 re f";
        let (outer_body, extra) = match inner_cs {
            None => (group.to_string(), String::new()),
            Some(cs) => (
                "/In Do".to_string(),
                format!(
                    "6 0 obj << /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Group << /S /Transparency {cs} >> /Resources << /ExtGState << /Diff << /BM /Difference >> >> >> /Length {} >> stream\n{group}\nendstream endobj\n",
                    group.len()
                ),
            ),
        };
        let page = "q 1 0 0 1 10 70 cm /X0 Do Q 1 0 1 0 k 10 10 200 50 re f";
        let pdf = format!(
            "%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 220 280] /Contents 4 0 R /Resources << /XObject << /X0 5 0 R >> >> >> endobj
4 0 obj << /Length {} >> stream
{page}
endstream endobj
5 0 obj << /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Group << /S /Transparency /CS /DeviceCMYK /I true >> /Resources << /XObject << /In 6 0 R >> /ExtGState << /Diff << /BM /Difference >> >> >> /Length {} >> stream
{outer_body}
endstream endobj
{extra}trailer << /Root 1 0 R >>
%%EOF",
            page.len(),
            outer_body.len()
        );
        render_shared(pdf, scale)
    };
    let px = |p: &RenderedPage, x: f32, y: f32, scale: f32| {
        let (x, y) = ((x * scale) as u32, (y * scale) as u32);
        p.rgba[((y * p.width + x) * 4) as usize..][..3].to_vec()
    };
    let distance = |a: &[u8], b: &[u8]| a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);

    // 200 pt at scale 4 is 800 px: the centre lies in tiles the paints cover completely.
    for scale in [1.0, 4.0] {
        let p = render(None, scale);
        let strip = px(&p, 110.0, 245.0, scale);
        for (x, y) in [(110.0, 110.0), (15.0, 15.0), (110.0, 205.0)] {
            let group = px(&p, x, y, scale);
            assert!(distance(&group, &strip) <= 3, "scale {scale} at ({x}, {y}): group {group:?}, strip {strip:?}");
        }
    }

    // A nested DeviceRGB group: the whole CMYK group falls back, so the page is drawn exactly
    // as when the enclosing group is not offered to the CMYK path at all.
    let nested_rgb = render(Some("/CS /DeviceRGB /I true"), 1.0);
    let green = px(&nested_rgb, 110.0, 245.0, 1.0);
    assert!(distance(&px(&nested_rgb, 110.0, 110.0, 1.0), &green) > 100, "the nested group blends in RGB, not CMYK");
    // A nested DeviceCMYK group stays in the CMYK path.
    let nested_cmyk = render(Some("/CS /DeviceCMYK /I true"), 1.0);
    assert!(distance(&px(&nested_cmyk, 110.0, 110.0, 1.0), &green) <= 3);
    // A non-isolated nested group's /CS is ignored (ISO 32000-2 Table 145): it blends in the
    // enclosing group's CMYK, even when it names DeviceRGB.
    let non_isolated_rgb = render(Some("/CS /DeviceRGB /I false /K false"), 1.0);
    assert!(distance(&px(&non_isolated_rgb, 110.0, 110.0, 1.0), &green) <= 3, "a non-isolated nested group inherits CMYK");
}
