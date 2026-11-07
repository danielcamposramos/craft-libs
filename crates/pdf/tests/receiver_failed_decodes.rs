//! A picture that doesn't decode is decoded once however often a page draws it (vendored
//! hayro-interpret patch 91). The decoded-image cache kept only decodes that succeeded, so a
//! damaged picture, or a JPX or JBIG2 stream its decoder refuses, was decoded again at every
//! draw: a page drawing it a thousand times decoded it a thousand times. The failure is kept for
//! the same key now, and every draw still reports it as it did.

#![cfg(all(feature = "raster", feature = "filters"))]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

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

use craft_pdf::interpret::font::Glyph;
use craft_pdf::interpret::{
    BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterSettings, InterpreterWarning, Paint, PathDrawMode,
    SoftMask, TransformExt, interpret_page,
};
use craft_pdf::syntax::Pdf;
use kurbo::{Affine, BezPath, Rect};
use std::io::Write;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

/// What a page handed its device: the raster images with their pixels, those of them with a
/// mask, and the stencil masks with theirs. Every image is asked for at the size it is drawn
/// at, as the renderer does.
#[derive(Default)]
struct Draws {
    rasters: usize,
    masked: usize,
    stencils: usize,
}

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
        match image {
            Image::Raster(r) => r.with_rgba(
                |_, alpha| {
                    self.rasters += 1;
                    self.masked += usize::from(alpha.is_some());
                },
                Some(target),
            ),
            Image::Stencil(s) => s.with_stencil(|_, _| self.stencils += 1, Some(target)),
        }
    }
    fn pop_clip_path(&mut self) {}
    fn pop_transparency_group(&mut self) {}
}

/// What drawing `/name` five times at one size, and then the picture that decodes (`/G`) once
/// on the right, did: the decodes the cache ran, the image decode failures reported, and what
/// the device was handed.
struct Outcome {
    decodes: usize,
    failures: usize,
    draws: Draws,
}

fn draw_five_times(name: &str) -> Outcome {
    let flate = |data: &[u8]| {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::fast());
        z.write_all(data).unwrap();
        z.finish().unwrap()
    };
    let pixels = flate(&(0..64 * 64 * 3).map(|i| (i % 251) as u8).collect::<Vec<_>>());
    let image = |filter: &str, extra: &str, len: usize| {
        format!(
            "<< /Type /XObject /Subtype /Image /Width 64 /Height 64 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /{filter} {extra}/Length {len} >>"
        )
    };
    let good = image("FlateDecode", "", pixels.len());
    // A JPEG 2000 picture whose codestream is four zero bytes.
    let bad_jpx = image("JPXDecode", "", 4);
    // A picture whose soft mask is such a JPEG 2000 stream.
    let bad_mask = image("FlateDecode", "/SMask 9 0 R ", pixels.len());
    // A stencil mask whose JBIG2 data is four zero bytes.
    let bad_stencil = "<< /Type /XObject /Subtype /Image /Width 64 /Height 64 /ImageMask true /Filter /JBIG2Decode /Length 4 >>";
    let jpx_mask =
        "<< /Type /XObject /Subtype /Image /Width 64 /Height 64 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /JPXDecode /Length 4 >>";
    let content = [0, 24, 48, 72, 96].map(|x| format!("q 20 0 0 20 {x} 10 cm /{name} Do Q")).join(" ") + " q 10 0 0 10 118 10 cm /G Do Q";
    let bytes = image_cache_pdf(
        &content,
        "<< /XObject << /G 5 0 R /J 6 0 R /S 7 0 R /M 8 0 R >> >>",
        &[(&good, &pixels), (&bad_jpx, b"\0\0\0\0"), (&bad_mask, &pixels), (bad_stencil, b"\0\0\0\0"), (jpx_mask, b"\0\0\0\0")],
    );
    let parsed = Pdf::new(bytes).expect("parses");
    let page = &parsed.pages()[0];
    let cache = InterpreterCache::new();
    let failures = Arc::new(AtomicUsize::new(0));
    let others = Arc::new(Mutex::new(Vec::new()));
    let settings = InterpreterSettings {
        warning_sink: {
            let (failures, others) = (failures.clone(), others.clone());
            Arc::new(move |warning| match warning {
                InterpreterWarning::ImageDecodeFailure => {
                    failures.fetch_add(1, Ordering::Relaxed);
                }
                other => others.lock().unwrap().push(format!("{other:?}")),
            })
        },
        ..InterpreterSettings::default()
    };
    let initial = page.initial_transform(true).to_kurbo();
    let mut ctx = Context::new(initial, Rect::new(0.0, 0.0, 130.0, 70.0), &cache, page.xref(), settings);
    let mut draws = Draws::default();
    interpret_page(page, &mut ctx, &mut draws);
    assert!(others.lock().unwrap().is_empty(), "/{name} reported {:?}", others.lock().unwrap());
    Outcome { decodes: cache.decoded_image_decodes(), failures: failures.load(Ordering::Relaxed), draws }
}

/// A JPEG 2000 picture that doesn't decode, drawn five times at one size, is decoded once, and
/// each of the five draws still reports the failure.
#[test]
fn a_picture_that_does_not_decode_is_decoded_once_however_often_it_is_drawn() {
    let out = draw_five_times("J");
    assert_eq!(out.draws.rasters, 1, "only the picture after it drew");
    assert_eq!(out.decodes, 2, "decoded again at every draw");
    assert_eq!(out.failures, 5, "every draw reports the failure, as before");
}

/// A JBIG2 stencil mask that doesn't decode, drawn five times, is decoded once, and each draw
/// still reports the failure: it isn't the page's JBIG2 allowance that refused it, which a
/// later page would have afresh.
#[test]
fn a_stencil_mask_that_does_not_decode_is_decoded_once_however_often_it_is_drawn() {
    let out = draw_five_times("M");
    assert_eq!((out.draws.stencils, out.draws.rasters), (0, 1), "only the picture after it drew");
    assert_eq!(out.decodes, 2, "decoded again at every draw");
    assert_eq!(out.failures, 5, "every draw reports the failure, as before");
}

/// A picture that decodes is still decoded once and handed to every draw.
#[test]
fn a_picture_that_decodes_is_still_decoded_once_and_drawn_every_time() {
    let out = draw_five_times("G");
    assert_eq!(out.draws.rasters, 6);
    assert_eq!(out.decodes, 1);
    assert_eq!(out.failures, 0);
}
