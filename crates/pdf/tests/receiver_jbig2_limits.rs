//! Original synthetic JBIG2 stream-limit and dictionary-bound controls from e2464e14.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use craft_pdf::{
    interpret::InterpreterSettings,
    raster::{self, Options, RenderCache},
    syntax::Pdf,
};
struct Page {
    rgba: Vec<u8>,
    diagnostics: raster::Diagnostics,
}
fn render_shared(bytes: Vec<u8>, work: u64) -> Page {
    let parsed = Pdf::new(bytes).unwrap();
    let image = raster::render(
        &parsed.pages()[0],
        &RenderCache::new(),
        &InterpreterSettings { jbig2_work: work, ..Default::default() },
        &Options { background: [255; 4], ..Default::default() },
    )
    .unwrap();
    Page { rgba: image.bytes().to_vec(), diagnostics: image.diagnostics }
}
#[derive(Default)]
struct Bits {
    bytes: Vec<u8>,
    used: u32,
}

impl Bits {
    fn put(&mut self, value: u32, count: u32) {
        for i in (0..count).rev() {
            if self.used.is_multiple_of(8) {
                self.bytes.push(0);
            }
            if (value >> i) & 1 != 0 {
                *self.bytes.last_mut().unwrap() |= 0x80 >> (self.used % 8);
            }
            self.used += 1;
        }
    }

    fn align(&mut self) {
        self.used = self.used.div_ceil(8) * 8;
    }

    /// `value` in table B.1 (ISO 14492 Annex B).
    fn table_b1(&mut self, value: u32) {
        match value {
            0..=15 => {
                self.put(0, 1);
                self.put(value, 4);
            }
            16..=271 => {
                self.put(0b10, 2);
                self.put(value - 16, 8);
            }
            272..=65807 => {
                self.put(0b110, 3);
                self.put(value - 272, 16);
            }
            _ => {
                self.put(0b111, 3);
                self.put(value - 65808, 32);
            }
        }
    }
}

fn jbig2_segment(out: &mut Vec<u8>, number: u32, kind: u8, refs: &[u8], data: &[u8]) {
    out.extend_from_slice(&number.to_be_bytes());
    out.push(kind);
    if refs.len() <= 4 {
        out.push(u8::try_from(refs.len()).unwrap() << 5);
    } else {
        out.extend_from_slice(&(0xE000_0000u32 | u32::try_from(refs.len()).unwrap()).to_be_bytes());
        out.extend(std::iter::repeat_n(0u8, (refs.len() + 1).div_ceil(8)));
    }
    out.extend_from_slice(refs);
    out.push(1);
    out.extend_from_slice(&u32::try_from(data.len()).unwrap().to_be_bytes());
    out.extend_from_slice(data);
}

fn black_jbig2_page() -> Vec<u8> {
    black_jbig2_page_of(16, 2)
}

fn black_jbig2_page_of(width: u32, height: u32) -> Vec<u8> {
    let mut page = Vec::new();
    page.extend_from_slice(&width.to_be_bytes());
    page.extend_from_slice(&height.to_be_bytes());
    page.extend_from_slice(&[0; 8]); // resolution
    page.extend_from_slice(&[4, 0, 0]); // flags (default pixel black), striping
    let mut stream = Vec::new();
    jbig2_segment(&mut stream, 0, 48, &[], &page);
    stream
}

fn wide_jbig2_dictionary(width: u32, heights: &[u32]) -> Vec<u8> {
    let mut bits = Bits::default();
    let mut previous = 0;
    for &height in heights {
        match height - previous {
            // Height class delta (table B.4): 1, or the upper line.
            1 => bits.put(0, 1),
            delta => {
                bits.put(0b11111, 5);
                bits.put(delta - 76, 32);
            }
        }
        previous = height;
        bits.put(0b111110, 6); // width delta, the upper line of table B.2
        bits.put(width - 75, 32);
        bits.put(0b111111, 6); // end of the class (B.2)
        let rows = height.div_ceil(8) + 1; // an MMR-coded white bitmap: one bit a row
        bits.table_b1(rows); // BMSIZE (B.1)
        bits.align();
        for _ in 0..rows {
            bits.put(0xFF, 8);
        }
    }
    let symbols = u32::try_from(heights.len()).unwrap();
    bits.table_b1(symbols); // the first export run, not exported, is all of them
    let mut dictionary = vec![0x00, 0x01]; // Huffman, tables B.4, B.2, B.1, B.1
    dictionary.extend_from_slice(&0u32.to_be_bytes()); // exported
    dictionary.extend_from_slice(&symbols.to_be_bytes()); // new
    dictionary.extend_from_slice(&bits.bytes);
    dictionary
}

fn jbig2_image_draws(stream: &[u8]) -> bool {
    jbig2_image_draws_in(stream, "/DeviceGray")
}

fn jbig2_image_draws_in(stream: &[u8], space: &str) -> bool {
    jbig2_image_draws_with(stream, space, "", b"")
}

fn jbig2_image_draws_with(stream: &[u8], space: &str, entries: &str, objects: &[u8]) -> bool {
    let content = b"q 20 0 0 20 5 5 cm /Im1 Do Q 1 0 0 rg 0 0 4 4 re f";
    let mut pdf = b"%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 40 40] /Contents 4 0 R /Resources << /XObject << /Im1 5 0 R >> >> >> endobj\n".to_vec();
    pdf.extend_from_slice(format!("4 0 obj << /Length {} >> stream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream endobj\n");
    pdf.extend_from_slice(
        format!("5 0 obj << /Type /XObject /Subtype /Image /Width 16 /Height 2 /BitsPerComponent 1 /ColorSpace {space} {entries} /Filter /JBIG2Decode /Length {} >> stream\n", stream.len())
            .as_bytes(),
    );
    pdf.extend_from_slice(stream);
    pdf.extend_from_slice(b"\nendstream endobj\n");
    pdf.extend_from_slice(objects);
    pdf.extend_from_slice(b"trailer << /Root 1 0 R >>\n%%EOF\n");
    // What one image may take, which the page's allowance, tested on its own, would otherwise reach
    // first: it is the same size, and the page itself comes out of it.
    let page = render_shared(pdf, u64::MAX);
    assert_eq!(&page.rgba[((38 * 40 + 1) * 4)..][..4], &[255, 0, 0, 255], "the rest of the page draws");
    // Inside the image: top-down rows 15..35.
    match &page.rgba[((25 * 40 + 15) * 4)..][..4] {
        [0, 0, 0, 255] => true,
        [255, 255, 255, 255] => false,
        other => panic!("neither the image nor the page: {other:?}"),
    }
}

#[test]
fn a_symbol_dictionary_holds_as_many_pixels_as_a_page() {
    let stream = |heights: &[u32]| {
        let mut stream = black_jbig2_page();
        jbig2_segment(&mut stream, 1, 0, &[], &wide_jbig2_dictionary(65535, heights));
        stream
    };
    assert!(jbig2_image_draws(&stream(&[1100])), "72,088,500 pixels, past 2^26");
    assert!(jbig2_image_draws(&stream(&[2047, 2048])), "268,365,825 pixels, within 2^28");
    assert!(!jbig2_image_draws(&stream(&[2048, 2049])), "268,496,895 pixels, past 2^28");
}

#[test]
fn a_picture_whose_jbig2_mask_a_decoder_limit_refuses_is_not_drawn() {
    let draws = |entry: &str, (width, height): (u32, u32), mask: &[u8]| {
        let content = b"q 20 0 0 20 5 5 cm /Im1 Do Q";
        let mut pdf = b"%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 40 40] /Contents 4 0 R /Resources << /XObject << /Im1 5 0 R >> >> >> endobj\n".to_vec();
        pdf.extend_from_slice(format!("4 0 obj << /Length {} >> stream\n", content.len()).as_bytes());
        pdf.extend_from_slice(content);
        pdf.extend_from_slice(b"\nendstream endobj\n");
        pdf.extend_from_slice(
            format!("5 0 obj << /Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray {entry} 6 0 R /Length 1 >> stream\n\0\nendstream endobj\n").as_bytes(),
        );
        let kind = if entry == "/Mask" { "/ImageMask true" } else { "/ColorSpace /DeviceGray" };
        pdf.extend_from_slice(
            format!(
                "6 0 obj << /Type /XObject /Subtype /Image /Width {width} /Height {height} /BitsPerComponent 1 {kind} /Filter /JBIG2Decode /Length {} >> stream\n",
                mask.len()
            )
            .as_bytes(),
        );
        pdf.extend_from_slice(mask);
        pdf.extend_from_slice(b"\nendstream endobj\ntrailer << /Root 1 0 R >>\n%%EOF\n");
        let page = render_shared(pdf, craft_pdf::syntax::MAX_PAGE_JBIG2_WORK);
        let shown = match &page.rgba[((25 * 40 + 15) * 4)..][..4] {
            [0, 0, 0, 255] => true,
            [255, 255, 255, 255] => false,
            other => panic!("neither the picture nor the page: {other:?}"),
        };
        (shown, page.diagnostics.content_truncated)
    };
    let out = (false, true);
    // A page a column wider than the decoder's bound on a side, as the mask's dictionary says too.
    let wide = black_jbig2_page_of(65536, 2);
    assert_eq!(draws("/SMask", (65536, 2), &wide), out, "a soft mask past the decoder's bound on a side");
    assert_eq!(draws("/Mask", (65536, 2), &wide), out, "a stencil mask past it");
    // A page of 65,535 rows and columns, past the pixels the renderer decodes, as the dictionary
    // says too.
    let large = black_jbig2_page_of(65535, 65535);
    assert_eq!(draws("/SMask", (65535, 65535), &large), out, "a soft mask past the renderer's bound on pixels");
    assert_eq!(draws("/Mask", (65535, 65535), &large), out, "a stencil mask past it");
    // A dictionary of more symbol pixels than a page holds.
    let mut dictionary = black_jbig2_page();
    jbig2_segment(&mut dictionary, 1, 0, &[], &wide_jbig2_dictionary(65535, &[2048, 2049]));
    assert_eq!(draws("/SMask", (16, 2), &dictionary), out, "a soft mask whose dictionary is past its bound");
    // Cut short in its dictionary: the file's damage, not a limit.
    let mut damaged = black_jbig2_page();
    jbig2_segment(&mut damaged, 1, 0, &[], &wide_jbig2_dictionary(100, &[100])[..12]);
    assert_eq!(draws("/SMask", (16, 2), &damaged), (true, false), "a damaged mask is no mask");
    // Control: the stream's page is far larger than the dictionary says. That is damage, which
    // reads as past the decoder's bound on pixels, and is taken for it: it can't be told from a
    // hostile size, and leaving the picture out is the safe side.
    let disagreeing = black_jbig2_page_of(65535, 65535);
    assert_eq!(draws("/SMask", (16, 2), &disagreeing), out, "a size damaged past a limit is taken for one");

    // A stencil mask a limit refused is drawn on its own: it takes nothing else out, and is a
    // failed image, not content left out (`a_jbig2_image_drawn_on_its_own_that_a_limit_refuses_is_a_failed_image`).
    // Nor does it leave anything behind for the next picture: one whose mask is never decoded (it
    // has no /Width) is drawn whole, as upstream draws it.
    let content = b"q 4 0 0 4 30 30 cm /St Do Q q 20 0 0 20 5 5 cm /Im1 Do Q";
    let mut pdf = b"%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 40 40] /Contents 4 0 R /Resources << /XObject << /Im1 5 0 R /St 7 0 R >> >> >> endobj\n".to_vec();
    pdf.extend_from_slice(format!("4 0 obj << /Length {} >> stream\n", content.len()).as_bytes());
    pdf.extend_from_slice(content);
    pdf.extend_from_slice(b"\nendstream endobj\n5 0 obj << /Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray /SMask 6 0 R /Length 1 >> stream\n\0\nendstream endobj\n");
    let mask = black_jbig2_page();
    pdf.extend_from_slice(format!("6 0 obj << /Type /XObject /Subtype /Image /Height 2 /BitsPerComponent 1 /ColorSpace /DeviceGray /Filter /JBIG2Decode /Length {} >> stream\n", mask.len()).as_bytes());
    pdf.extend_from_slice(&mask);
    pdf.extend_from_slice(format!("\nendstream endobj\n7 0 obj << /Type /XObject /Subtype /Image /Width 65536 /Height 2 /ImageMask true /Filter /JBIG2Decode /Length {} >> stream\n", wide.len()).as_bytes());
    pdf.extend_from_slice(&wide);
    pdf.extend_from_slice(b"\nendstream endobj\ntrailer << /Root 1 0 R >>\n%%EOF\n");
    let page = render_shared(pdf, craft_pdf::syntax::MAX_PAGE_JBIG2_WORK);
    assert_eq!(&page.rgba[((25 * 40 + 15) * 4)..][..4], [0, 0, 0, 255], "the picture after the stencil is drawn whole");
    // PdfCraft's page warning projection omits ordinary image/font decode failures.
    assert!(
        !page.diagnostics.content_truncated
            && !page.diagnostics.color_fallback
            && !page.diagnostics.malformed_indexed_image
            && !page.diagnostics.type3_widths_missing,
        "the stencil left no page content out: {:?}",
        page.diagnostics
    );
    assert!(page.diagnostics.image_decode_failed, "the standalone stencil remains an image failure");
}

/// Ordinary byte limits and known-image budgets remain distinct. A header past its declared image budget is a mask refusal.
fn masked_budget_fixture() -> String {
    let mask = black_jbig2_page_of(64, 16);
    let encoded = mask.iter().map(|b| format!("{b:02X}")).collect::<String>() + ">";
    let content = "q 20 0 0 20 0 0 cm /Im Do Q";
    format!(
        "%PDF-1.7\n1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj\n2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj\n3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 40 20] /Resources << /XObject << /Im 5 0 R >> >> /Contents 4 0 R >> endobj\n4 0 obj << /Length {} >> stream\n{content}\nendstream endobj\n5 0 obj << /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /SMask 6 0 R /Filter /ASCIIHexDecode /Length 3 >> stream\n00>\nendstream endobj\n6 0 obj << /Type /XObject /Subtype /Image /Width 64 /Height 16 /ColorSpace /DeviceGray /BitsPerComponent 1 /Filter [/ASCIIHexDecode /JBIG2Decode] /Length {} >> stream\n{encoded}\nendstream endobj\ntrailer << /Root 1 0 R >>\n%%EOF",
        content.len(),
        encoded.len()
    )
}

fn assert_known_image_budget(plate: Option<craft_pdf::interpret::color::Plate>) {
    use craft_pdf::syntax::DecodeLimits;
    let text = masked_budget_fixture();
    let render = |bytes: Vec<u8>, limits| {
        let parsed = Pdf::new_with_limits(bytes, limits).unwrap();
        raster::render(
            &parsed.pages()[0],
            &RenderCache::new(),
            &InterpreterSettings { plate: plate.clone(), ..Default::default() },
            &Options { background: [255; 4], ..Default::default() },
        )
        .unwrap()
    };
    let ordinary = render(text.as_bytes().to_vec(), DecodeLimits::default());
    assert_eq!(&ordinary.bytes()[(5 * 40 + 5) * 4..][..4], [255; 4], "the actual mask hides the picture");
    assert!(ordinary.diagnostics.is_empty());
    let no_mask = render(text.replace("/SMask 6 0 R", "").into_bytes(), DecodeLimits::default());
    assert_eq!(&no_mask.bytes()[(5 * 40 + 5) * 4..][..4], [0, 0, 0, 255], "the image really draws without its mask");
    let ordinary_lower = render(text.as_bytes().to_vec(), DecodeLimits::default().with_max_decoded_stream(64));
    assert_eq!(ordinary_lower.bytes(), ordinary.bytes(), "known images keep their format-derived budget");
    assert!(ordinary_lower.diagnostics.is_empty(), "the ordinary materialization cap did not refuse this image");
    let mismatched = text.replace("/Width 64 /Height 16 /ColorSpace", "/Width 1 /Height 1 /ColorSpace");
    let bounded = render(mismatched.into_bytes(), DecodeLimits::default());
    assert_eq!(&bounded.bytes()[(5 * 40 + 5) * 4..][..4], [255; 4], "a refused mask must not uncover its image");
    assert!(bounded.diagnostics.content_truncated, "the declared image byte budget refusal is reported");
}

#[test]
fn a_declared_image_budget_never_uncovers_its_masked_picture() {
    assert_known_image_budget(None);
}

#[test]
fn a_declared_image_budget_never_uncovers_its_masked_plate() {
    use craft_pdf::interpret::color::{Ink, Plate};
    assert_known_image_budget(Some(Plate { ink: Ink::Black, overprint: true, simulation: None, spots_as_process: false }));
}

/// The common raw-ink API has an opacity consumer beyond the receiver's RGB renderer.
#[test]
fn extracting_inks_never_unmasks_an_image_refused_by_its_mask_budget() {
    use craft_pdf::interpret::font::Glyph;
    use craft_pdf::interpret::{
        BlendMode, ClipPath, Context, Device, GlyphDrawMode, Image, InterpreterCache, InterpreterWarning, Paint, PathDrawMode, SoftMask,
        TransformExt, interpret_page,
    };
    use kurbo::{Affine, BezPath, Rect};
    use std::sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    };
    #[derive(Default)]
    struct InkDraws(usize);
    impl<'a> Device<'a> for InkDraws {
        fn set_soft_mask(&mut self, _: Option<SoftMask<'a>>) {}
        fn set_blend_mode(&mut self, _: BlendMode) {}
        fn draw_path(&mut self, _: &BezPath, _: Affine, _: &Paint<'a>, _: &PathDrawMode) {}
        fn push_clip_path(&mut self, _: &ClipPath) {}
        fn push_transparency_group(&mut self, _: f32, _: Option<SoftMask<'a>>, _: BlendMode) {}
        fn draw_glyph(&mut self, _: &Glyph<'a>, _: Affine, _: Affine, _: &Paint<'a>, _: &GlyphDrawMode) {}
        fn draw_image(&mut self, image: Image<'a, '_>, _: Affine) {
            if let Image::Raster(image) = image {
                image.with_inks(None, |_| self.0 += 1);
            }
        }
        fn pop_clip_path(&mut self) {}
        fn pop_transparency_group(&mut self) {}
    }
    let text = masked_budget_fixture();
    for (what, bytes, expected) in [
        ("actual mask", text.clone().into_bytes(), 1),
        ("refused mask", text.replace("/Width 64 /Height 16 /ColorSpace", "/Width 1 /Height 1 /ColorSpace").into_bytes(), 0),
    ] {
        let parsed = Pdf::new(bytes).unwrap();
        let page = &parsed.pages()[0];
        let cache = InterpreterCache::new();
        let truncated = Arc::new(AtomicBool::new(false));
        let receiver = truncated.clone();
        let settings = InterpreterSettings {
            warning_sink: Arc::new(move |w| {
                if matches!(w, InterpreterWarning::ContentTruncated) {
                    receiver.store(true, Ordering::Relaxed);
                }
            }),
            ..Default::default()
        };
        let mut context = Context::new(page.initial_transform(true).to_kurbo(), Rect::new(0.0, 0.0, 40.0, 20.0), &cache, page.xref(), settings);
        let mut device = InkDraws::default();
        interpret_page(page, &mut context, &mut device);
        assert_eq!(device.0, expected, "{what}: only a complete masked ink image reaches the consumer");
        assert_eq!(truncated.load(Ordering::Relaxed), expected == 0, "{what}: refusal is reported");
    }
}
