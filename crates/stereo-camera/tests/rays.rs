use craft_stereo_camera::{Budget, Camera, CameraPair, DepthRange, ProjectionKind};
use glam::{Mat4, Vec3};

fn pair(kind: ProjectionKind, infinite: bool) -> CameraPair {
    let projection = match kind {
        ProjectionKind::Perspective if infinite => Mat4::perspective_infinite_rh(0.7, 1.7, 0.01),
        ProjectionKind::Perspective => Mat4::perspective_rh(0.7, 1.7, 0.01, 100.0),
        ProjectionKind::Orthographic => Mat4::orthographic_rh(-3.0, 3.0, -2.0, 2.0, 0.01, 100.0),
    };
    let eye = Vec3::new(0.0, 0.0, 5.0);
    Camera {
        eye,
        view: Mat4::look_at_rh(eye, Vec3::ZERO, Vec3::Y),
        projection,
        kind,
        infinity_projection_x: projection.x_axis.x,
        zero_depth: 5.0,
        depths: DepthRange { near: 4.0, far: 6.0 },
        width: 640,
        budget: Budget { sunk: 24.0, popped: 12.0 },
    }
    .pair()
    .unwrap()
}

#[test]
fn prepared_rays_intersect_the_independently_predicted_plane() {
    for kind in [ProjectionKind::Perspective, ProjectionKind::Orthographic] {
        for infinite in [false, true] {
            let cameras = pair(kind, infinite);
            for (index, eye) in cameras.eyes.into_iter().enumerate() {
                let rays = eye.rays().unwrap();
                let offset = if index == 0 { -0.5 } else { 0.5 } * cameras.baseline;
                for z in [4.0, 5.0, 6.0] {
                    for ndc in [[0.0, 0.0], [-0.7, 0.3], [0.8, -0.5]] {
                        let ray = rays.ray(ndc).unwrap();
                        let t = (5.0 - z - ray.origin.z) / ray.direction.z;
                        let hit = ray.origin + ray.direction * t;
                        let factor = if kind == ProjectionKind::Perspective { z } else { 1.0 };
                        let x = ndc[0] * factor / eye.projection.x_axis.x + offset * (1.0 - z / 5.0);
                        let y = ndc[1] * factor / eye.projection.y_axis.y;
                        assert!((hit.x - x).abs() < 0.0002, "{kind:?}: {hit:?} vs {x}");
                        assert!((hit.y - y).abs() < 0.0002);
                        assert!((ray.direction.length() - 1.0).abs() < 0.00001);
                    }
                }
                assert!(rays.ray([f32::NAN, 0.0]).is_none());
            }
        }
    }
}

#[test]
fn mutated_singular_or_nonfinite_matrices_are_rejected_before_inverse() {
    let mut eye = pair(ProjectionKind::Perspective, false).eyes[0];
    eye.projection = Mat4::ZERO;
    assert!(eye.rays().is_err());
    eye.projection = Mat4::from_cols_array(&[f32::NAN; 16]);
    assert!(eye.rays().is_err());
}
