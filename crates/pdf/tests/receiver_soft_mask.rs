//! Original synthetic soft-mask downsampling control from be5a9209.
#![cfg(all(feature = "raster", feature = "filters"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use craft_pdf::interpret::InterpreterSettings;
fn image_cache_pdf(content: &str, resources: &str, objects: &[(&str, &[u8])]) -> Vec<u8> {
    let mut pdf = format!(
        "%PDF-1.4
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 130 70] /Contents 4 0 R /Resources {resources} >> endobj
4 0 obj << /Length {} >> stream
{content}
endstream endobj
",
        content.len()
    )
    .into_bytes();
    for (i, (dict, data)) in objects.iter().enumerate() {
        pdf.extend(format!("{} 0 obj {dict} stream\n", i + 5).bytes());
        pdf.extend_from_slice(data);
        pdf.extend_from_slice(b"\nendstream endobj\n");
    }
    pdf.extend_from_slice(b"trailer << /Root 1 0 R >>\n%%EOF");
    pdf
}

#[test]
fn a_soft_mask_far_larger_than_where_it_is_drawn_is_shrunk_once() {
    use craft_pdf::interpret::font::Glyph;
    use craft_pdf::interpret::{
        BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, Paint, PathDrawMode, SoftMask, TransformExt, interpret_page,
    };
    use craft_pdf::syntax::Pdf;
    use kurbo::{Affine, BezPath, Rect};
    use std::io::Write;

    /// Asks for every raster image at the size it is drawn at, as the renderer does, and keeps
    /// the size of each alpha channel it was handed.
    #[derive(Default)]
    struct Draws(Vec<(u32, u32)>);
    impl<'a> Device<'a> for Draws {
        fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
        fn set_blend_mode(&mut self, _: BlendMode) {}
        fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}
        fn push_clip_path(&mut self, _: &ClipPath) {}
        fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
        fn draw_glyph(&mut self, _: &Glyph<'a>, _: Affine, _: Affine, _: &Paint<'a>, _: &GlyphDrawMode) {}
        fn draw_image(&mut self, image: Image<'a, '_>, transform: Affine) {
            let [a, b, c, d, ..] = transform.as_coeffs();
            let target = ((a.hypot(b) * f64::from(image.width())).ceil() as u32, (c.hypot(d) * f64::from(image.height())).ceil() as u32);
            if let Image::Raster(r) = image {
                r.with_rgba(|_, alpha| self.0.push(alpha.map_or((0, 0), |a| (a.width, a.height))), Some(target));
            }
        }
        fn pop_clip_path(&mut self) {}
        fn pop_transparency_group(&mut self) {}
    }

    let flate = |data: &[u8]| {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(data).unwrap();
        z.finish().unwrap()
    };
    // `side` × `side` mask samples, drawn ten times at 20 × 20 points (device pixels at 72 dpi).
    let draws = |side: u32, parent: u32| {
        let rgb = flate(&vec![0u8; (parent * parent * 3) as usize]);
        let image = format!(
            "<< /Type /XObject /Subtype /Image /Width {parent} /Height {parent} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /FlateDecode /SMask 6 0 R /Length {} >>",
            rgb.len()
        );

        let alpha = flate(&(0..side * side).map(|i| (i % 256) as u8).collect::<Vec<_>>());
        let mask = format!(
            "<< /Type /XObject /Subtype /Image /Width {side} /Height {side} /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode /Length {} >>",
            alpha.len()
        );
        let content = (0..10).map(|i| format!("q 20 0 0 20 {} 0 cm /A Do Q", i * 22)).collect::<Vec<_>>().join(" ");
        let bytes = image_cache_pdf(&content, "<< /XObject << /A 5 0 R >> >>", &[(&image, &rgb), (&mask, &alpha)]);
        let parsed = Pdf::new(bytes).expect("parses");
        let page = &parsed.pages()[0];
        let cache = InterpreterCache::new();
        let initial = page.initial_transform(true).to_kurbo();
        let mut ctx = Context::new(initial, Rect::new(0.0, 0.0, 230.0, 30.0), &cache, page.xref(), InterpreterSettings::default());
        let mut device = Draws::default();
        interpret_page(page, &mut ctx, &mut device);
        (device.0, cache.decoded_image_decodes())
    };
    let (alphas, decodes) = draws(1000, 2);
    assert_eq!(alphas, vec![(20, 20); 10], "averaged down to the size it is drawn at");
    // One decode for the image and one for its averaged-down mask, shared by all ten draws.
    assert_eq!(decodes, 2, "decoded once");
    let (alphas, _) = draws(40, 2);
    assert_eq!(alphas, vec![(40, 40); 10], "no more than twice the drawn size: kept as it is");
    let (alphas, _) = draws(100, 200);
    assert_eq!(alphas, vec![(100, 100); 10], "a mask smaller than its parent image keeps its own samples");
}
