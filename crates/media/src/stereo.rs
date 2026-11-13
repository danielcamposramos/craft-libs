//! Lossless spatial sample permutations for decoded stereo planes.
//!
//! This is a copy primitive, not a file-format descriptor: callers retain the original
//! metadata (including temporal, anaglyph and depth arrangements) in their media model.
//! Those arrangements require decoding/reconstruction before this primitive can be used.
//! No missing samples are interpolated. Sample shape travels separately from dimensions.

use std::fmt;

/// A view's identity, independent of its position in the stored picture.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Eye {
    Left,
    Right,
}

/// Spatial permutations of two equal-sized rectangular sample grids.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Packing {
    SideBySide {
        gap: usize,
    },
    TopBottom {
        gap: usize,
    },
    Rows,
    Columns,
    /// Alternating columns, with the starting view alternating on every row.
    Checkerboard,
}

/// Positive, reduced ratio of a sample's displayed width to its displayed height.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SampleAspect(u32, u32);

impl SampleAspect {
    pub fn new(numerator: u32, denominator: u32) -> Option<Self> {
        if numerator == 0 || denominator == 0 {
            return None;
        }
        let (mut a, mut b) = (numerator, denominator);
        while b != 0 {
            (a, b) = (b, a % b);
        }
        Some(Self(numerator / a, denominator / a))
    }

    pub fn ratio(self) -> (u32, u32) {
        (self.0, self.1)
    }

    /// A quarter-turn exchanges sample width and height.
    pub fn rotated(self) -> Self {
        Self(self.1, self.0)
    }
}

/// Invalid geometry, mismatched input, or an allocation exceeding the caller's limit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridError {
    Geometry,
    SourceLength,
    Budget,
    Allocation,
}

impl fmt::Display for GridError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Geometry => "invalid stereo sample-grid geometry",
            Self::SourceLength => "stereo sample buffer does not match its dimensions",
            Self::Budget => "stereo sample copy exceeds its output byte limit",
            Self::Allocation => "cannot allocate stereo sample grids",
        })
    }
}

impl std::error::Error for GridError {}

/// A validated address plan; all arithmetic is bounded before copying any samples.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PackedGrid {
    width: usize,
    height: usize,
    eye_width: usize,
    eye_height: usize,
    packing: Packing,
    first: Eye,
    aspect: SampleAspect,
}

impl PackedGrid {
    pub fn new(width: usize, height: usize, packing: Packing, first: Eye, aspect: SampleAspect) -> Result<Self, GridError> {
        let split = |size: usize, gap: usize| size.checked_sub(gap).filter(|n| *n > 0 && n % 2 == 0).map(|n| n / 2).ok_or(GridError::Geometry);
        if width == 0 || height == 0 || width.checked_mul(height).is_none() {
            return Err(GridError::Geometry);
        }
        let (eye_width, eye_height) = match packing {
            Packing::SideBySide { gap } => (split(width, gap)?, height),
            Packing::TopBottom { gap } => (width, split(height, gap)?),
            Packing::Rows => (width, split(height, 0)?),
            Packing::Columns | Packing::Checkerboard => (split(width, 0)?, height),
        };
        Ok(Self { width, height, eye_width, eye_height, packing, first, aspect })
    }

    pub fn eye_size(self) -> (usize, usize) {
        (self.eye_width, self.eye_height)
    }

    pub fn pixel_aspect_ratio(self) -> (u32, u32) {
        self.aspect.ratio()
    }

    /// Source index of one native sample. Coordinates outside a view are rejected.
    pub fn source_index(self, eye: Eye, x: usize, y: usize) -> Option<usize> {
        if x >= self.eye_width || y >= self.eye_height {
            return None;
        }
        let second = usize::from(eye != self.first);
        // Construction proves these addresses lie inside width * height.
        let (x, y) = match self.packing {
            Packing::SideBySide { gap } => (x + second * (self.eye_width + gap), y),
            Packing::TopBottom { gap } => (x, y + second * (self.eye_height + gap)),
            Packing::Rows => (x, y * 2 + second),
            Packing::Columns => (x * 2 + second, y),
            Packing::Checkerboard => (x * 2 + ((y + second) % 2), y),
        };
        Some(y * self.width + x)
    }

    /// Copy whole samples (pixels, alpha or other planes); the budget covers both views.
    pub fn copy_eyes<T: Copy>(self, source: &[T], max_output_bytes: usize) -> Result<NativePair<T>, GridError> {
        if source.len() != self.width * self.height {
            return Err(GridError::SourceLength);
        }
        let count = self.eye_width * self.eye_height;
        let bytes = count.checked_mul(2).and_then(|n| n.checked_mul(std::mem::size_of::<T>())).ok_or(GridError::Budget)?;
        if bytes > max_output_bytes {
            return Err(GridError::Budget);
        }
        let mut left = Vec::new();
        let mut right = Vec::new();
        left.try_reserve_exact(count).map_err(|_| GridError::Allocation)?;
        right.try_reserve_exact(count).map_err(|_| GridError::Allocation)?;
        for y in 0..self.eye_height {
            for x in 0..self.eye_width {
                // get also protects this boundary if a future permutation is added incorrectly.
                let l = self.source_index(Eye::Left, x, y).and_then(|i| source.get(i)).ok_or(GridError::Geometry)?;
                let r = self.source_index(Eye::Right, x, y).and_then(|i| source.get(i)).ok_or(GridError::Geometry)?;
                left.push(*l);
                right.push(*r);
            }
        }
        Ok(NativePair { left, right, width: self.eye_width, height: self.eye_height, aspect: self.aspect })
    }
}

/// Two native sample planes with their dimensions and sample shape retained.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NativePair<T> {
    pub left: Vec<T>,
    pub right: Vec<T>,
    width: usize,
    height: usize,
    aspect: SampleAspect,
}

impl<T> NativePair<T> {
    pub fn eye_size(&self) -> (usize, usize) {
        (self.width, self.height)
    }

    pub fn pixel_aspect_ratio(&self) -> (u32, u32) {
        self.aspect.ratio()
    }
}
