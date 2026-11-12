use craft_stereo_window::DepthLimits;

#[test]
fn limits_are_scaled_from_output_to_native_samples_once() {
    let limits = DepthLimits::new(10.0, 4.0, 1280, 800).unwrap();
    let (sunk, popped) = limits.native_pixels(800);
    assert!((sunk - 20.0 / 3.0).abs() < 1e-12 && (popped - 8.0 / 3.0).abs() < 1e-12);
    let (sunk, popped) = limits.native_pixels(400);
    assert!((sunk - 10.0 / 3.0).abs() < 1e-12 && (popped - 4.0 / 3.0).abs() < 1e-12);
    let narrow = DepthLimits::new(10.0, 4.0, 1280, 400).unwrap();
    assert_eq!(narrow.native_pixels(400), limits.native_pixels(800));
}

#[test]
fn absent_or_malformed_geometry_does_not_invent_limits() {
    for (sunk, popped, output, view) in [
        (f64::NAN, 4.0, 1280, 800),
        (10.0, f64::INFINITY, 1280, 800),
        (-1.0, 4.0, 1280, 800),
        (10.0, -1.0, 1280, 800),
        (10.0, 4.0, 0, 800),
        (10.0, 4.0, 1280, 0),
    ] {
        assert!(DepthLimits::new(sunk, popped, output, view).is_none());
    }
    assert_eq!(DepthLimits::new(0.0, 0.0, 1280, 800).unwrap().native_pixels(400), (0.0, 0.0));
}

#[test]
fn fixed_point_x11_limits_require_exactly_two_nonnegative_words() {
    assert_eq!(DepthLimits::from_fixed(&[2560, 2432], 1280, 800), DepthLimits::new(10.0, 9.5, 1280, 800));
    for words in [&[][..], &[2560][..], &[2560, 1024, 0][..], &[u32::MAX, 1024][..]] {
        assert!(DepthLimits::from_fixed(words, 1280, 800).is_none());
    }
}
