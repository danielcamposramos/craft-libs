use craft_stereo_camera::{Budget, Camera, DepthRange, ProjectionKind};
use glam::{Mat4, Vec3};

fn camera(width: u32, height: u32, kind: ProjectionKind) -> Camera {
    let d = 5.0;
    let aspect = width as f32 / height as f32;
    let eye = Vec3::new(0.0, 0.0, d);
    let projection = match kind {
        ProjectionKind::Perspective => Mat4::perspective_rh(40f32.to_radians(), aspect, 0.01, 100.0),
        ProjectionKind::Orthographic => {
            let h = d * 20f32.to_radians().tan();
            Mat4::orthographic_rh(-h * aspect, h * aspect, -h, h, 0.01, 100.0)
        }
    };
    Camera {
        eye,
        view: Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y),
        projection,
        kind,
        infinity_projection_x: Mat4::perspective_rh(40f32.to_radians(), aspect, 0.01, 100.0).x_axis.x,
        zero_depth: d,
        depths: DepthRange { near: 4.0, far: 6.0 },
        width,
        budget: Budget { sunk: 24.0, popped: 12.0 },
    }
}
fn project(view: Mat4, projection: Mat4, p: Vec3) -> Vec3 {
    let clip = projection * view * p.extend(1.0);
    clip.truncate() / clip.w
}
#[test]
fn thousands_of_points_match_the_closed_form_and_keep_parallel_axes() {
    for kind in [ProjectionKind::Perspective, ProjectionKind::Orthographic] {
        for [w, h] in [[320, 180], [640, 360], [360, 640]] {
            let c = camera(w, h, kind);
            let before = c;
            let pair = c.pair().unwrap();
            let f = c.projection.x_axis.x as f64 * f64::from(w) / 2.0;
            for z in [4.0, 4.25, 5.0, 5.5, 6.0] {
                for xi in -8..=8 {
                    for yi in -6..=6 {
                        let point = Vec3::new(xi as f32 * 0.07, yi as f32 * 0.07, 5.0 - z);
                        let a = project(pair.eyes[0].view, pair.eyes[0].projection, point);
                        let b = project(pair.eyes[1].view, pair.eyes[1].projection, point);
                        let predicted = f64::from(pair.baseline)
                            * f
                            * match kind {
                                ProjectionKind::Perspective => 1.0 / f64::from(z) - 0.2,
                                ProjectionKind::Orthographic => 1.0 - f64::from(z) / 5.0,
                            };
                        let measured = f64::from(a.x - b.x) * f64::from(w) / 2.0;
                        assert!((measured - predicted).abs() < 0.0002, "{:?}", ("disparity", kind, z, measured, predicted));
                        assert_eq!(a.y, b.y, "no vertical disparity");
                        assert!((-24.0002..=12.0002).contains(&measured));
                        if z == 5.0 {
                            assert!(measured.abs() < 0.0002);
                        }
                    }
                }
            }
            assert_eq!(c, before, "the central camera is never mutated");
            for e in pair.eyes {
                assert_eq!(e.view.x_axis.truncate(), c.view.x_axis.truncate());
                assert_eq!(e.view.y_axis.truncate(), c.view.y_axis.truncate());
                assert_eq!(e.view.z_axis.truncate(), c.view.z_axis.truncate());
            }
        }
    }
}
#[test]
fn infinity_and_zero_budget_have_their_required_limits() {
    let mut c = camera(640, 360, ProjectionKind::Perspective);
    let pair = c.pair().unwrap();
    let f = c.projection.x_axis.x * 320.0;
    assert!((pair.baseline * f / 5.0 - 24.0).abs() < 0.0001, "the environment at infinity stays within the sunk budget");
    c.budget = Budget { sunk: 0.0, popped: 0.0 };
    for eye in c.pair().unwrap().eyes {
        assert_eq!(eye.view.to_cols_array().map(f32::to_bits), c.view.to_cols_array().map(f32::to_bits));
        assert_eq!(eye.projection.to_cols_array().map(f32::to_bits), c.projection.to_cols_array().map(f32::to_bits));
        assert_eq!(eye.eye, c.eye);
    }
}
#[test]
fn pose_and_model_units_do_not_change_the_pixel_prediction() {
    let c = camera(640, 360, ProjectionKind::Perspective);
    let original = c.pair().unwrap();
    let world = Mat4::from_rotation_y(0.73) * Mat4::from_rotation_x(-0.4);
    let rotated = Camera { eye: world.transform_point3(c.eye), view: c.view * world.inverse(), ..c }.pair().unwrap();
    for p in [Vec3::new(0.4, 0.2, 1.0), Vec3::new(-0.3, -0.1, -1.0)] {
        for i in 0..2 {
            let a = project(original.eyes[i].view, original.eyes[i].projection, p);
            let b = project(rotated.eyes[i].view, rotated.eyes[i].projection, world.transform_point3(p));
            assert!((a - b).length() < 0.00001);
        }
    }
    let scale = 10.0;
    let units = Camera {
        eye: c.eye * scale,
        view: Mat4::look_at_rh(c.eye * scale, Vec3::ZERO, Vec3::Y),
        zero_depth: 50.0,
        depths: DepthRange { near: 40.0, far: 60.0 },
        ..c
    }
    .pair()
    .unwrap();
    assert!((units.baseline - original.baseline * scale).abs() < 0.00001);
}
#[test]
fn invalid_camera_or_budget_is_an_error() {
    let c = camera(640, 360, ProjectionKind::Perspective);
    for bad in [
        Camera { width: 0, ..c },
        Camera { zero_depth: 0.0, ..c },
        Camera { view: Mat4::ZERO, ..c },
        Camera { budget: Budget { sunk: f32::NAN, popped: 1.0 }, ..c },
        Camera { budget: Budget { sunk: 1.0, popped: -1.0 }, ..c },
        Camera { depths: DepthRange { near: 0.0, far: 6.0 }, ..c },
        Camera { projection: Mat4::ZERO, ..c },
    ] {
        assert!(bad.pair().is_err());
    }
}

#[test]
fn bounds_and_an_orthographic_views_perspective_environment_use_the_same_world() {
    let c = camera(640, 360, ProjectionKind::Orthographic);
    assert_eq!(DepthRange::from_aabb(c.view, Vec3::splat(-1.0), Vec3::splat(1.0), 0.01).unwrap(), DepthRange { near: 4.0, far: 6.0 });
    let pair = c.pair().unwrap();
    let perspective = camera(640, 360, ProjectionKind::Perspective).projection;
    let a = pair.eyes[0].background_view_projection(perspective) * Vec3::new(0.0, 0.0, -1.0).extend(0.0);
    let b = pair.eyes[1].background_view_projection(perspective) * Vec3::new(0.0, 0.0, -1.0).extend(0.0);
    assert!(((a.x / a.w - b.x / b.w) * 320.0 + 24.0).abs() < 0.0001);
    assert_eq!(a.y / a.w, b.y / b.w);
    assert!(DepthRange::from_aabb(c.view, Vec3::splat(10.0), Vec3::splat(-10.0), 0.1).is_err());
    assert!(Camera { view: Mat4::from_scale(Vec3::splat(2.0)), ..c }.pair().is_err());
}

#[test]
fn incoherent_eye_and_projection_cross_terms_are_refused() {
    let c = camera(640, 360, ProjectionKind::Perspective);
    assert!(Camera { eye: Vec3::ZERO, ..c }.pair().is_err(), "lighting and projected geometry must have the same physical eye");
    let mut skew = c.projection;
    skew.x_axis.y = 0.2;
    assert!(Camera { projection: skew, ..c }.pair().is_err(), "a cross term would turn horizontal eye motion into vertical disparity");
}

#[test]
fn singular_affine_inputs_are_refused_before_glam_can_assert() {
    let c = camera(640, 360, ProjectionKind::Perspective);
    let singular = Mat4::from_scale(Vec3::new(0.0, 1.0, 1.0));
    assert!(Camera { view: singular, ..c }.pair().is_err());
    let mut projection = c.projection;
    projection.w_axis.z = 0.0;
    assert!(Camera { projection, ..c }.pair().is_err());
}

#[test]
fn a_narrower_environment_uses_its_actual_intrinsics_in_the_budget() {
    for kind in [ProjectionKind::Perspective, ProjectionKind::Orthographic] {
        let mut c = camera(640, 360, kind);
        let background = Mat4::perspective_rh(20f32.to_radians(), 640.0 / 360.0, 0.01, 100.0);
        c.infinity_projection_x = background.x_axis.x;
        let pair = c.pair().unwrap();
        let direction = Vec3::NEG_Z.extend(0.0);
        let a = pair.eyes[0].background_view_projection(background) * direction;
        let b = pair.eyes[1].background_view_projection(background) * direction;
        let disparity = (a.x / a.w - b.x / b.w) * 320.0;
        assert!((disparity + 24.0).abs() < 0.0001, "{kind:?}: {disparity}");
        assert_eq!(a.y / a.w, b.y / b.w);
        for invalid in [-1.0, f32::INFINITY, f32::NAN] {
            assert!(Camera { infinity_projection_x: invalid, ..c }.pair().is_err());
        }
    }
}

#[test]
fn a_solid_background_does_not_introduce_an_infinity_plane() {
    let mut c = camera(640, 360, ProjectionKind::Perspective);
    c.infinity_projection_x = 0.0;
    let pair = c.pair().unwrap();
    let f = c.projection.x_axis.x * 320.0;
    assert!((pair.baseline * f * (1.0 / 4.0 - 1.0 / 5.0) - 12.0).abs() < 0.0001);
    assert!((pair.baseline * f * (1.0 / 6.0 - 1.0 / 5.0) + 8.0).abs() < 0.0001);
    c.depths = DepthRange { near: 5.0, far: 5.0 };
    let flat = c.pair().unwrap();
    assert_eq!(flat.baseline, 0.0);
    for eye in flat.eyes {
        assert_eq!(eye.view, c.view);
        assert_eq!(eye.projection, c.projection);
    }
    assert!(Camera { infinity_projection_x: -1.0, ..c }.pair().is_err());
}
