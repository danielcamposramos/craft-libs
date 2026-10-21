//! Isolated transparency groups whose colour space is `DeviceCMYK` blend in CMYK (ISO 32000-2
//! §11.3.4, §11.3.5.3).

#![cfg(feature = "raster")]
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use craft_pdf::{interpret::InterpreterSettings, raster::RenderCache, syntax::Pdf};
use hayro::{RenderSettings, vello_cpu::color::palette::css::WHITE};
struct RenderedPage {
    rgba: Vec<u8>,
    width: u32,
}

/// A print test file's patch of blend modes in isolated groups whose colour space is
/// `DeviceCMYK` drew an X in nearly every cell. The elements of a group blend in the
/// group's colour space, and a subtractive space's components are complemented before the
/// blend function and back after it (ISO 32000-2 §11.3.4); of the nonseparable modes,
/// §11.3.5.3 blends C, M and Y as complemented R, G and B and takes K from the backdrop
/// (from the source for Luminosity). PdfCraft blended the colours converted to RGB, so
/// magenta under black with Difference came out magenta instead of green. Vendored
/// hayro-interpret patch (45) and hayro patch (21): an isolated group (§11.4.7: its own
/// transparent backdrop) blending in DeviceCMYK is blended in CMYK.
///
/// Each cell is a square painted in the CMYK the blend should give, with a smaller isolated
/// group on it whose two elements are tint A and then tint B with the cell's blend mode:
/// drawn right, the group is the colour of the square around it.
#[test]
fn isolated_groups_blend_in_device_cmyk() {
    // (blend mode, tint A, tint B, A then B blended in DeviceCMYK, worked out by hand)
    type Cell = (&'static str, [f32; 4], [f32; 4], [f32; 4]);
    let cells: [Cell; 7] = [
        // |(1,0,1,1) − (1,1,1,0)| = (0,1,0,1), complemented back: green.
        ("Difference", [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0, 0.0, 1.0, 0.0]),
        ("Exclusion", [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 0.0, 1.0], [1.0, 0.0, 1.0, 0.0]),
        ("Multiply", [0.0, 0.0, 1.0, 0.0], [0.0, 0.5, 0.0, 0.0], [0.0, 0.5, 1.0, 0.0]),
        ("Screen", [0.0, 1.0, 1.0, 0.0], [0.5, 0.5, 1.0, 0.0], [0.0, 0.5, 1.0, 0.0]),
        ("Darken", [0.0, 0.0, 1.0, 0.0], [0.0, 0.5, 0.0, 0.0], [0.0, 0.5, 1.0, 0.0]),
        // Hue of (0,1,1) at the saturation and luminosity of (1,0.5,1): (0.355,0.855,0.855);
        // K is the backdrop's.
        ("Hue", [0.0, 0.5, 0.0, 0.3], [1.0, 0.0, 0.0, 0.0], [0.645, 0.145, 0.145, 0.3]),
        // (1,0,1) at the luminosity of (0,1,0), clipped: (1,0.3051,1); K is the source's.
        ("Luminosity", [0.0, 1.0, 0.0, 0.3], [1.0, 0.0, 1.0, 0.0], [0.0, 0.6949, 0.0, 0.0]),
    ];
    let tint = |t: [f32; 4]| format!("{} {} {} {} k", t[0], t[1], t[2], t[3]);
    // `group`: the cell group's /Group entries; `b`: what the second element paints.
    let render = |group: &str, b: &dyn Fn([f32; 4]) -> String| {
        let (mut page, mut objects, mut names) = (String::new(), String::new(), String::new());
        for (i, (mode, ta, tb, expected)) in cells.iter().enumerate() {
            let x = 10 + i * 50;
            page += &format!("{} {x} 10 40 40 re f /G{i} Do\n", tint(*expected));
            let rect = format!("{} 20 20 20 re f", x + 10);
            let n = 10 + i * 3;
            for (k, paint) in [(n, tint(*ta)), (n + 1, b(*tb))] {
                let body = format!("{paint} {rect}");
                objects += &format!(
                    "{k} 0 obj << /Type /XObject /Subtype /Form /BBox [{x} 10 {} 50] /Group << /S /Transparency /CS /DeviceCMYK >> /Length {} >> stream\n{body}\nendstream endobj\n",
                    x + 40,
                    body.len()
                );
            }
            let body = "/E1 Do /BM gs /E2 Do";
            objects += &format!(
                "{} 0 obj << /Type /XObject /Subtype /Form /BBox [{x} 10 {} 50] /Group << /S /Transparency {group} >> /Resources << /XObject << /E1 {n} 0 R /E2 {} 0 R >> /ExtGState << /BM << /BM /{mode} >> >> >> /Length {} >> stream\n{body}\nendstream endobj\n",
                n + 2,
                x + 40,
                n + 1,
                body.len()
            );
            names += &format!("/G{i} {} 0 R ", n + 2);
        }
        let pdf = format!(
            "%PDF-1.7
1 0 obj << /Type /Catalog /Pages 2 0 R >> endobj
2 0 obj << /Type /Pages /Kids [3 0 R] /Count 1 >> endobj
3 0 obj << /Type /Page /Parent 2 0 R /MediaBox [0 0 370 60] /Group << /S /Transparency /CS /DeviceCMYK >> /Contents 4 0 R /Resources << /XObject << {names}>> >> >> endobj
4 0 obj << /Length {} >> stream
{page}
endstream endobj
{objects}trailer << /Root 1 0 R >>
%%EOF",
            page.len()
        );
        let parsed = Pdf::new(pdf.into_bytes()).unwrap();
        let pixmap = hayro::render(
            &parsed.pages()[0],
            &RenderCache::new(),
            &InterpreterSettings::default(),
            &RenderSettings { bg_color: WHITE, ..Default::default() },
        );
        RenderedPage { rgba: pixmap.data_as_u8_slice().to_vec(), width: u32::from(pixmap.width()) }
    };
    let px = |p: &RenderedPage, x: usize, y: u32| p.rgba[((y * p.width + x as u32) * 4) as usize..][..3].to_vec();
    let distance = |a: &[u8], b: &[u8]| a.iter().zip(b).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);

    let p = render("/CS /DeviceCMYK /I true", &tint);
    for (i, (mode, ..)) in cells.iter().enumerate() {
        let (group, square) = (px(&p, 30 + i * 50, 30), px(&p, 14 + i * 50, 30));
        // The square is a solid CMYK fill; the group lands within rounding of it.
        assert!(distance(&group, &square) <= 3, "{mode}: group {group:?}, square {square:?}");
    }

    // A group whose contents have no ink tints (here the second element is RGB) is
    // composited in RGB exactly as before: the same as a group blending in DeviceRGB.
    let rgb = |_: [f32; 4]| "1 0 0 rg".to_string();
    let cmyk_group = render("/CS /DeviceCMYK /I true", &rgb);
    let rgb_group = render("/CS /DeviceRGB /I true", &rgb);
    assert_eq!(cmyk_group.rgba, rgb_group.rgba, "contents without tints are drawn as before");
}
