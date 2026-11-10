//! Original bounded CMYK band equivalence control from e03d5a1c.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use craft_pdf::{interpret::InterpreterSettings, raster::RenderCache, syntax::Pdf};
use hayro::{RenderSettings, vello_cpu::color::palette::css::WHITE};
#[test]
fn a_cmyk_group_blended_in_bands_is_the_group_blended_whole() {
    let group = "0 1 0 0 k 0 0 m 200 0 l 0 200 l f /Diff gs 0 0 0 1 k 0 0 200 200 re f";
    let pdf = format!(
        "%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 220 220] /Contents 4 0 R /Resources << /XObject << /X0 5 0 R >> >> >> endobj
4 0 obj << /Length 28 >> stream
q 1 0 0 1 10 10 cm /X0 Do Q
endstream endobj
5 0 obj << /Type /XObject /Subtype /Form /BBox [0 0 200 200] /Group << /S /Transparency /CS /DeviceCMYK /I true >> /Resources << /ExtGState << /Diff << /BM /Difference >> >> >> /Length {} >> stream
{group}
endstream endobj
trailer << /Root 1 0 R >>
%%EOF",
        group.len()
    );
    let parsed = Pdf::new(pdf.into_bytes()).unwrap();
    let cache = RenderCache::new();
    let settings = RenderSettings { bg_color: WHITE, x_scale: 2.0, y_scale: 2.0, ..Default::default() };
    let render = || hayro::render(&parsed.pages()[0], &cache, &InterpreterSettings::default(), &settings);
    let whole = render();
    hayro::set_cmyk_band_pixels(Some(2_800));
    let banded = render();
    hayro::set_cmyk_band_pixels(None);
    assert_eq!((whole.width(), whole.height()), (banded.width(), banded.height()));
    let worst = whole.data_as_u8_slice().iter().zip(banded.data_as_u8_slice()).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
    assert!(worst <= 1, "bands differ from the whole group by up to {worst}");
    for page in [&whole, &banded] {
        let at = (340usize * usize::from(page.width()) + 100) * 4;
        let rgb = &page.data_as_u8_slice()[at..at + 3];
        assert!(rgb[1] > 120 && rgb[0] < 60 && rgb[2] < 120, "blended in CMYK, green: {rgb:?}");
    }
}
