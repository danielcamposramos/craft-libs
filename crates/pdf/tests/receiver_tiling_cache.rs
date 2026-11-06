//! Original synthetic tiling-cache controls from 1d968fea; shared renderer entry point.
#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use craft_pdf::{interpret::InterpreterSettings, raster::RenderCache, syntax::Pdf};
use hayro::{RenderSettings, vello_cpu::color::palette::css::WHITE};
struct Page {
    rgba: Vec<u8>,
    width: u32,
}
fn render_first_page(bytes: Vec<u8>) -> Page {
    let parsed = Pdf::new(bytes).unwrap();
    let pix = hayro::render(
        &parsed.pages()[0],
        &RenderCache::new(),
        &InterpreterSettings::default(),
        &RenderSettings { bg_color: WHITE, ..Default::default() },
    );
    Page { rgba: pix.data_as_u8_slice().to_vec(), width: u32::from(pix.width()) }
}
#[test]
fn fills_of_one_pattern_setting_share_its_cell() {
    let mut content = String::from("/Pattern cs /P scn ");
    for i in 0..3000u32 {
        content.push_str(&format!("{} {} 4 4 re f ", (i % 60) * 5, (i / 60) * 5));
    }
    // The cell is blue with a red lower-left quarter; at `/Matrix` scale 10 a cell is 100 units,
    // so with a step of 10 red shows where both coordinates, modulo 100, are below 50. With a
    // step of 100000 the one cell shows and nothing past it.
    let cell = "0 0 1 rg 0 0 10 10 re f 1 0 0 rg 0 0 5 5 re f";
    for (step, past_the_first_cell) in [("10", [0, 0, 255]), ("100000", [255, 255, 255])] {
        let pdf = format!(
            "%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R /Resources << /Pattern << /P 5 0 R >> >> >> endobj
4 0 obj << /Length {} >> stream
{content}
endstream endobj
5 0 obj << /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 10 10] /XStep {step} /YStep {step} /Matrix [10 0 0 10 0 0] /Resources << >> /Length {} >> stream
{cell}
endstream endobj
trailer << /Root 1 0 R >>
%%EOF",
            content.len(),
            cell.len()
        );
        let before = hayro::offscreen_draws();
        let page = render_first_page(pdf.into_bytes());
        let cells = hayro::offscreen_draws().tiling_cells - before.tiling_cells;
        assert_eq!(cells, 1, "step {step}: cells drawn for 3,000 fills of one setting");
        // Pixel rows run top down: PDF (x, y) is pixel (x, 300 - y).
        let px = |x: u32, y: u32| page.rgba[(((300 - y) * page.width + x) * 4) as usize..][..3].to_vec();
        // The first square, at (0, 0), and the last, at (295, 245).
        assert_eq!(px(1, 1), [255, 0, 0], "step {step}");
        assert_eq!(px(296, 246), past_the_first_cell, "step {step}");
    }
}
#[test]
fn fills_of_interleaved_pattern_settings_each_show_their_own_cell() {
    let content = "/Pattern cs /P scn 60 60 4 4 re f 10 10 4 4 re f q 2 0 0 2 0 0 cm /X Do Q \
                   /Q scn 110 10 4 4 re f 160 160 4 4 re f /P scn 210 210 4 4 re f q 2 0 0 2 0 0 cm /X Do Q \
                   /Q scn 260 260 4 4 re f 110 110 4 4 re f";
    // In page space, squares at (60, 260) and (260, 60), where `/P` at scale 1 is blue.
    let form = "/Pattern cs /P scn 30 130 2 2 re f 130 30 2 2 re f";
    let p = "0 0 1 rg 0 0 10 10 re f 1 0 0 rg 0 0 5 5 re f";
    let q = "0 1 0 rg 0 0 10 10 re f 1 1 0 rg 0 0 5 5 re f";
    let pdf = format!(
        "%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 300 300] /Contents 4 0 R /Resources << /Pattern << /P 5 0 R /Q 6 0 R >> /XObject << /X 7 0 R >> >> >> endobj
4 0 obj << /Length {} >> stream
{content}
endstream endobj
5 0 obj << /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 10 10] /XStep 10 /YStep 10 /Matrix [10 0 0 10 0 0] /Resources << >> /Length {} >> stream
{p}
endstream endobj
6 0 obj << /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 10 10] /XStep 10 /YStep 10 /Matrix [10 0 0 10 0 0] /Resources << >> /Length {} >> stream
{q}
endstream endobj
7 0 obj << /Type /XObject /Subtype /Form /BBox [0 0 150 150] /Resources << /Pattern << /P 5 0 R >> >> /Length {} >> stream
{form}
endstream endobj
trailer << /Root 1 0 R >>
%%EOF",
        content.len(),
        p.len(),
        q.len(),
        form.len()
    );
    let before = hayro::offscreen_draws();
    let page = render_first_page(pdf.into_bytes());
    assert_eq!(hayro::offscreen_draws().tiling_cells - before.tiling_cells, 6, "one cell per setting");
    // The middle of the 4 × 4 square at PDF (x, y); pixel rows run top down.
    let px = |x: u32, y: u32| page.rgba[(((300 - 2 - y) * page.width + x + 2) * 4) as usize..][..3].to_vec();
    let (red, blue, green, yellow) = ([255, 0, 0], [0, 0, 255], [0, 255, 0], [255, 255, 0]);
    for (x, y, colour, what) in [
        (60, 60, blue, "/P"),
        (10, 10, red, "/P"),
        (60, 260, red, "/P at scale 2"),
        (260, 60, red, "/P at scale 2"),
        (110, 10, yellow, "/Q"),
        (160, 160, green, "/Q"),
        (210, 210, red, "/P set again"),
        (260, 260, green, "/Q set again"),
        (110, 110, yellow, "/Q set again"),
    ] {
        assert_eq!(px(x, y), colour, "{what} at ({x}, {y})");
    }
}
#[test]
fn an_uncoloured_pattern_shows_each_settings_colour() {
    let content = "/C cs 1 0 0 /U scn 10 10 20 20 re f 0 1 0 /U scn 40 10 20 20 re f \
                   1 0 0 /U scn 70 10 20 20 re f 0 1 0 /U scn 100 10 20 20 re f \
                   /H gs 0 0 1 /U scn 10 50 20 20 re f q 20 0 0 20 40 50 cm /Msk Do Q";
    let tile = "0 0 10 10 re f";
    let pdf = format!(
        "%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 150 100] /Contents 4 0 R /Resources << /ColorSpace << /C [/Pattern /DeviceRGB] >> /Pattern << /U 6 0 R >> /XObject << /Msk 5 0 R >> /ExtGState << /H << /ca 0.5 >> >> >> >> endobj
4 0 obj << /Length {} >> stream
{content}
endstream endobj
5 0 obj << /Type /XObject /Subtype /Image /Width 4 /Height 4 /ImageMask true /Filter /ASCIIHexDecode /Length 9 >> stream
00000000>
endstream endobj
6 0 obj << /Type /Pattern /PatternType 1 /PaintType 2 /TilingType 1 /BBox [0 0 10 10] /XStep 10 /YStep 10 /Resources << >> /Length {} >> stream
{tile}
endstream endobj
trailer << /Root 1 0 R >>
%%EOF",
        content.len(),
        tile.len()
    );
    let page = render_first_page(pdf.into_bytes());
    // Pixel rows run top down: PDF (x, y) is pixel (x, 100 - y).
    let px = |x: u32, y: u32| page.rgba[(((100 - y) * page.width + x) * 4) as usize..][..3].to_vec();
    for (x, colour) in [(20, [255, 0, 0]), (50, [0, 255, 0]), (80, [255, 0, 0]), (110, [0, 255, 0])] {
        assert_eq!(px(x, 20), colour, "the square at x {x}");
    }
    let (square, stencil) = (px(20, 60), px(50, 60));
    assert!(square[2] == 255 && square[0].abs_diff(128) <= 1 && square[1] == square[0], "half blue: {square:?}");
    assert!(square.iter().zip(&stencil).all(|(a, b)| a.abs_diff(*b) <= 2), "square {square:?}, stencil {stencil:?}");
}
