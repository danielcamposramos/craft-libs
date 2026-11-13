//! One world, two parallel cameras, within a compositor's native-pixel depth budget.
//!
//! Right-handed views look along -Z. Matrices and distances retain the caller's units.
//! The central camera is never modified; independent-light work remains shared by its renderer.
#![forbid(unsafe_code)]
#![deny(clippy::unwrap_used, clippy::expect_used, clippy::panic, clippy::unimplemented, clippy::todo, clippy::unreachable)]

use glam::{Mat4, Vec3, Vec4};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum ProjectionKind {
    Perspective,
    Orthographic,
}

/// Nonnegative disparity magnitudes, in samples of the native eye picture.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Budget {
    pub sunk: f32,
    pub popped: f32,
}

/// Positive central-view depths of the visible world, including displacement.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DepthRange {
    pub near: f32,
    pub far: f32,
}

impl DepthRange {
    /// Project all eight world-space bounds corners and clip to the central near plane.
    /// Expand the bounds by the renderer's displacement envelope before calling this.
    pub fn from_aabb(view: Mat4, lo: Vec3, hi: Vec3, clip_near: f32) -> Result<Self, Error> {
        if !view.is_finite() || !lo.is_finite() || !hi.is_finite() || !lo.cmple(hi).all() || !clip_near.is_finite() || clip_near <= 0.0 {
            return Err(Error("invalid scene bounds or near plane"));
        }
        let mut near = f32::INFINITY;
        let mut far = f32::NEG_INFINITY;
        for x in [lo.x, hi.x] {
            for y in [lo.y, hi.y] {
                for z in [lo.z, hi.z] {
                    let depth = -view.transform_point3(Vec3::new(x, y, z)).z;
                    near = near.min(depth);
                    far = far.max(depth);
                }
            }
        }
        if !near.is_finite() || !far.is_finite() || far < clip_near {
            return Err(Error("scene bounds are outside the camera's positive depth range"));
        }
        Ok(Self { near: near.max(clip_near), far })
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub eye: Vec3,
    pub view: Mat4,
    pub projection: Mat4,
    pub kind: ProjectionKind,
    /// Horizontal focal coefficient of the actual perspective environment projection.
    pub infinity_projection_x: f32,
    /// Positive view depth of the existing camera's point of interest.
    pub zero_depth: f32,
    pub depths: DepthRange,
    pub width: u32,
    pub budget: Budget,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraPair {
    /// Distance between the physical eyes, in the caller's world units.
    pub baseline: f32,
    /// Left first. Neither view rotates relative to the central camera.
    pub eyes: [EyeCamera; 2],
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EyeCamera {
    pub eye: Vec3,
    pub view: Mat4,
    pub projection: Mat4,
    offset: f32,
    zero_depth: f32,
}

impl EyeCamera {
    /// Prepare one checked inverse per eye, rather than inverting for every sample.
    pub fn rays(self) -> Result<RayGrid, Error> {
        let matrix = self.view_projection();
        let determinant = matrix.determinant();
        // Consumers may enable glam-assert: reject singular input before inverse().
        if !matrix.is_finite() || !determinant.is_finite() || determinant == 0.0 {
            return Err(Error("the eye view projection must be finite and invertible"));
        }
        let inverse = matrix.inverse();
        if !inverse.is_finite() {
            return Err(Error("the inverse eye view projection is outside the finite world range"));
        }
        Ok(RayGrid { inverse })
    }

    pub fn view_projection(self) -> Mat4 {
        self.projection * self.view
    }

    /// Apply the same physical offset and target plane to another central projection.
    /// An orthographic model view can use its existing perspective environment this way.
    pub fn projection_for(self, mut central: Mat4) -> Mat4 {
        if self.offset != 0.0 {
            central.z_axis.x -= central.x_axis.x * self.offset / self.zero_depth;
        }
        central
    }

    pub fn background_view_projection(self, central_projection: Mat4) -> Mat4 {
        self.projection_for(central_projection) * self.view
    }
}

/// A world-space ray beginning at the camera's near clipping plane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    pub direction: Vec3,
}

/// Prepared unprojection of perspective or orthographic eye samples.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RayGrid {
    inverse: Mat4,
}

impl RayGrid {
    /// NDC uses +Y up and depth 0..1. Samples outside the image support overscan.
    /// A homogeneous far point (infinite projection) is a direction, not a divide by zero.
    pub fn ray(self, ndc: [f32; 2]) -> Option<Ray> {
        if !ndc.iter().all(|v| v.is_finite()) {
            return None;
        }
        let near = self.inverse * Vec4::new(ndc[0], ndc[1], 0.0, 1.0);
        let far = self.inverse * Vec4::new(ndc[0], ndc[1], 1.0, 1.0);
        if !near.is_finite() || !far.is_finite() || near.w == 0.0 {
            return None;
        }
        let origin = near.truncate() / near.w;
        let direction = (far.truncate() * near.w - near.truncate() * far.w).try_normalize()?;
        origin.is_finite().then_some(Ray { origin, direction })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Error(&'static str);

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}
impl std::error::Error for Error {}

impl Camera {
    /// Solve both disparity inequalities, including the environment at infinity.
    /// No device, output format, separation preference or eye-order policy lives here.
    pub fn pair(self) -> Result<CameraPair, Error> {
        let Self { eye, view, projection, kind, infinity_projection_x, zero_depth: d, depths, width, budget } = self;
        if !infinity_projection_x.is_finite()
            || infinity_projection_x <= 0.0
            || width == 0
            || !eye.is_finite()
            || !view.is_finite()
            || !projection.is_finite()
            || !d.is_finite()
            || d <= 0.0
            || !depths.near.is_finite()
            || !depths.far.is_finite()
            || depths.near <= 0.0
            || depths.far < depths.near
            || !budget.sunk.is_finite()
            || !budget.popped.is_finite()
            || budget.sunk < 0.0
            || budget.popped < 0.0
            || projection.x_axis.x <= 0.0
            || projection.y_axis.y <= 0.0
        {
            return Err(Error("invalid camera, scene depths or native-pixel budget"));
        }
        if view.x_axis.w != 0.0 || view.y_axis.w != 0.0 || view.z_axis.w != 0.0 || view.w_axis.w != 1.0 {
            return Err(Error("the central view must be affine"));
        }
        let determinant = view.determinant();
        let projection_determinant = projection.determinant();
        if !determinant.is_finite() || (determinant - 1.0).abs() > 0.0001 || !projection_determinant.is_finite() || projection_determinant == 0.0 {
            return Err(Error("the camera matrices must be finite and invertible"));
        }
        if !projection.inverse().is_finite() || !(projection * view).is_finite() {
            return Err(Error("the camera projection is outside the finite world range"));
        }
        let inverse = view.inverse();
        let basis = [inverse.x_axis.truncate(), inverse.y_axis.truncate(), inverse.z_axis.truncate()];
        if !inverse.is_finite()
            || basis.iter().any(|v| (v.length_squared() - 1.0).abs() > 0.0001)
            || basis[0].dot(basis[1]).abs() > 0.0001
            || basis[0].dot(basis[2]).abs() > 0.0001
            || basis[1].dot(basis[2]).abs() > 0.0001
        {
            return Err(Error("the central view must be a rigid right-handed camera"));
        }
        if (inverse.w_axis.truncate() - eye).abs().max_element() > 0.0001 * (1.0 + eye.abs().max_element()) {
            return Err(Error("physical eye does not match the central view"));
        }
        if projection.x_axis.y != 0.0 || projection.x_axis.z != 0.0 || projection.x_axis.w != 0.0 || projection.y_axis.w != 0.0 {
            return Err(Error("projection cross terms do not preserve horizontal stereo"));
        }
        let valid_projection = match kind {
            ProjectionKind::Perspective => projection.z_axis.w == -1.0 && projection.w_axis.w == 0.0,
            ProjectionKind::Orthographic => projection.z_axis.w == 0.0 && projection.w_axis.w == 1.0,
        };
        if !valid_projection {
            return Err(Error("projection kind does not match its homogeneous matrix"));
        }
        let d64 = f64::from(d);
        let profile = |z: f32| match kind {
            ProjectionKind::Perspective => 1.0 / f64::from(z) - 1.0 / d64,
            ProjectionKind::Orthographic => 1.0 - f64::from(z) / d64,
        };
        let infinity = f64::from(infinity_projection_x) / f64::from(projection.x_axis.x) / d64;
        let positive = profile(depths.near).max(0.0);
        let negative = (-profile(depths.far)).max(infinity);
        let f = f64::from(projection.x_axis.x) * f64::from(width) / 2.0;
        let mut e = f64::from(budget.sunk) / (f * negative);
        if positive > 0.0 {
            e = e.min(f64::from(budget.popped) / (f * positive));
        }
        let e = e as f32;
        if !e.is_finite() || e < 0.0 {
            return Err(Error("the camera's baseline is outside the finite world range"));
        }
        let make = |offset: f32| {
            let mut camera = EyeCamera { eye, view, projection, offset, zero_depth: d };
            if offset != 0.0 {
                camera.eye += basis[0] * offset;
                camera.view = Mat4::from_translation(Vec3::new(-offset, 0.0, 0.0)) * view;
                camera.projection = camera.projection_for(projection);
            }
            camera
        };
        let eyes = [make(-e / 2.0), make(e / 2.0)];
        if eyes.iter().any(|eye| !eye.eye.is_finite() || !eye.view.is_finite() || !eye.projection.is_finite() || !eye.view_projection().is_finite()) {
            return Err(Error("the stereo projections are outside the finite world range"));
        }
        Ok(CameraPair { baseline: e, eyes })
    }
}
