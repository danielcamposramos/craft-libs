//! Type 3 clipping must not install the empty clip needed by an empty outline glyph.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::panic)]

use craft_pdf::interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, Paint, PathDrawMode, SoftMask, TransformExt,
    font::Glyph,
};
use craft_pdf::raster::{self, Options, RenderCache};
use kurbo::{Affine, BezPath, Rect};

fn page(mode: u8, glyph: &str) -> Vec<u8> {
    let content = format!("BT /T3 10 Tf {mode} Tr 5 5 Td (A) Tj ET 1 0 0 rg 0 0 40 20 re f");
    format!(
        "%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n\
         2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n\
         3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 40 20] /Contents 4 0 R /Resources << /Font << /T3 5 0 R >> >> >> endobj\n\
         4 0 obj << /Length {} >> stream\n{content}\nendstream endobj\n\
         5 0 obj << /Type /Font /Subtype /Type3 /FontBBox [0 0 1000 1000] /FontMatrix [0.001 0 0 0.001 0 0] /CharProcs << /A 6 0 R >> /Encoding << /Type /Encoding /Differences [65 /A] >> /FirstChar 65 /LastChar 65 /Widths [1000] >> endobj\n\
         6 0 obj << /Length {} >> stream\n{glyph}\nendstream endobj\n\
         trailer << /Root 1 0 R >>\n%%EOF",
        content.len(), glyph.len(),
    ).into_bytes()
}

#[derive(Default)]
struct Clips {
    paths: usize,
    clips: usize,
    clipping_glyphs: usize,
    painted_glyphs: usize,
    groups: usize,
}

impl Device<'_> for Clips {
    fn set_soft_mask(&mut self, _: Option<SoftMask<'_>>) {}
    fn set_blend_mode(&mut self, _: BlendMode) {}
    fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'_>, _: &PathDrawMode) {
        self.paths += 1;
    }
    fn push_clip_path(&mut self, _: &ClipPath) {
        self.clips += 1;
    }
    fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'_>>, _: BlendMode) {
        self.groups += 1;
    }
    fn draw_glyph(&mut self, _: &Glyph<'_>, _: Affine, _: Affine, _: &Paint<'_>, mode: &GlyphDrawMode) {
        self.clipping_glyphs += usize::from(matches!(mode, GlyphDrawMode::Clip));
        self.painted_glyphs += usize::from(matches!(mode, GlyphDrawMode::Fill | GlyphDrawMode::Stroke(_)));
    }
    fn draw_image(&mut self, _: Image<'_, '_>, _: Affine) {}
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

const GLYPHS: [&str; 2] = ["1000 0 0 0 1000 1000 d1 0 0 1000 1000 re f", "1000 0 d0 0 0 1 rg 0 0 1000 1000 re f"];

#[test]
fn type3_clip_text_is_reported_without_installing_any_clip() {
    for glyph in GLYPHS {
        for mode in [3, 7] {
            let pdf = craft_pdf::open_slice(&page(mode, glyph), None).unwrap();
            let page = &pdf.pages()[0];
            let cache = InterpreterCache::new();
            let mut context = Context::new(
                page.initial_transform(true).to_kurbo(),
                Rect::new(0.0, 0.0, 40.0, 20.0),
                &cache,
                page.xref(),
                InterpreterSettings::default(),
            );
            let mut device = Clips::default();
            craft_pdf::interpret_page(page, &mut context, &mut device).unwrap();
            assert_eq!(device.clips, 0, "Type 3 mode {mode} adds no clipping path");
            assert_eq!(device.paths, 1, "the subsequent fill still reaches the device");
            assert_eq!(device.clipping_glyphs, usize::from(mode == 7), "mode 7 remains clipping text for extraction");
        }
    }
}

#[test]
fn type3_clip_text_leaves_a_following_page_fill_visible() {
    for glyph in GLYPHS {
        for mode in [3, 7] {
            let pdf = craft_pdf::open_slice(&page(mode, glyph), None).unwrap();
            let image =
                raster::render(&pdf.pages()[0], &RenderCache::new(), &Default::default(), &Options { background: [255; 4], ..Default::default() })
                    .unwrap();
            assert!(
                image.bytes().as_chunks::<4>().0.iter().all(|pixel| *pixel == [255, 0, 0, 255]),
                "the entire later fill remains red in mode {mode}"
            );
        }
    }
}

#[test]
fn type3_fill_stroke_modes_paint_once_without_an_outline_knockout_group() {
    for glyph in GLYPHS {
        for mode in [0, 2, 6] {
            let pdf = craft_pdf::open_slice(&page(mode, glyph), None).unwrap();
            let page = &pdf.pages()[0];
            let cache = InterpreterCache::new();
            let mut context = Context::new(
                page.initial_transform(true).to_kurbo(),
                Rect::new(0.0, 0.0, 40.0, 20.0),
                &cache,
                page.xref(),
                InterpreterSettings::default(),
            );
            let mut device = Clips::default();
            craft_pdf::interpret_page(page, &mut context, &mut device).unwrap();
            assert_eq!(device.painted_glyphs, 1, "Type3 mode {mode} paints its procedure only once");
            assert_eq!(device.groups, 0, "Type3 mode {mode} does not acquire an outline fill/stroke group");
            assert_eq!(device.clips, 0);
            assert_eq!(device.paths, 1, "the later page fill remains independent");
        }
    }
}
