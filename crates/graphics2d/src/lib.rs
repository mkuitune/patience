//! Ordered instances in document space, independent of any rendering backend.

pub use glam::{Mat3, Vec2};
use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub enum GeometryError {
    #[error("placement requires finite coordinates, a nonzero direction, and positive scale")]
    InvalidPlacement,
    #[error("handedness must be +1 or -1")]
    InvalidHandedness,
    #[error("bounds require finite coordinates and positive width and height")]
    InvalidBounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(into = "i8", try_from = "i8")]
pub enum Handedness {
    Positive,
    Negative,
}

impl From<Handedness> for i8 {
    fn from(value: Handedness) -> Self {
        match value {
            Handedness::Positive => 1,
            Handedness::Negative => -1,
        }
    }
}

impl TryFrom<i8> for Handedness {
    type Error = GeometryError;
    fn try_from(value: i8) -> Result<Self, Self::Error> {
        match value {
            1 => Ok(Self::Positive),
            -1 => Ok(Self::Negative),
            _ => Err(GeometryError::InvalidHandedness),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "StoredPlacement", into = "StoredPlacement")]
pub struct SimplePlacement {
    position: Vec2,
    x_direction: Vec2,
    scale: f32,
    handedness: Handedness,
}

#[derive(Serialize, Deserialize)]
struct StoredPlacement {
    position: Vec2,
    x_direction: Vec2,
    scale: f32,
    handedness: Handedness,
}

impl SimplePlacement {
    /// Places the local origin at `position`; normalizes the supplied X direction.
    pub fn new(
        position: Vec2,
        x_direction: Vec2,
        scale: f32,
        handedness: Handedness,
    ) -> Result<Self, GeometryError> {
        if !position.is_finite() || !scale.is_finite() || scale <= 0.0 {
            return Err(GeometryError::InvalidPlacement);
        }
        let x_direction = x_direction
            .try_normalize()
            .ok_or(GeometryError::InvalidPlacement)?;
        Ok(Self {
            position,
            x_direction,
            scale,
            handedness,
        })
    }

    pub const IDENTITY: Self = Self {
        position: Vec2::ZERO,
        x_direction: Vec2::X,
        scale: 1.0,
        handedness: Handedness::Positive,
    };

    pub fn position(self) -> Vec2 {
        self.position
    }
    pub fn x_direction(self) -> Vec2 {
        self.x_direction
    }
    pub fn scale(self) -> f32 {
        self.scale
    }
    pub fn handedness(self) -> Handedness {
        self.handedness
    }

    pub fn matrix(self) -> Mat3 {
        let x = self.x_direction * self.scale;
        let y = Vec2::new(-x.y, x.x) * f32::from(i8::from(self.handedness));
        Mat3::from_cols(x.extend(0.0), y.extend(0.0), self.position.extend(1.0))
    }
}

impl TryFrom<StoredPlacement> for SimplePlacement {
    type Error = GeometryError;
    fn try_from(value: StoredPlacement) -> Result<Self, Self::Error> {
        Self::new(
            value.position,
            value.x_direction,
            value.scale,
            value.handedness,
        )
    }
}

impl From<SimplePlacement> for StoredPlacement {
    fn from(value: SimplePlacement) -> Self {
        Self {
            position: value.position,
            x_direction: value.x_direction,
            scale: value.scale,
            handedness: value.handedness,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum Placement {
    Simple(SimplePlacement),
}

impl Placement {
    pub fn matrix(self) -> Mat3 {
        match self {
            Self::Simple(value) => value.matrix(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "[Vec2; 2]", into = "[Vec2; 2]")]
pub struct Bounds {
    min: Vec2,
    max: Vec2,
}

impl Bounds {
    pub fn new(min: Vec2, max: Vec2) -> Result<Self, GeometryError> {
        let size = max - min;
        if !min.is_finite() || !max.is_finite() || !size.is_finite() || size.min_element() <= 0.0 {
            return Err(GeometryError::InvalidBounds);
        }
        Ok(Self { min, max })
    }

    /// Defines positive, finite bounds starting at the local origin.
    pub const fn from_size(width: f32, height: f32) -> Self {
        assert!(width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0);
        Self {
            min: Vec2::ZERO,
            max: Vec2::new(width, height),
        }
    }
    pub fn min(self) -> Vec2 {
        self.min
    }
    pub fn max(self) -> Vec2 {
        self.max
    }
    pub fn size(self) -> Vec2 {
        self.max - self.min
    }
    pub fn center(self) -> Vec2 {
        self.min + self.size() * 0.5
    }
    pub fn corners(self) -> [Vec2; 4] {
        [
            self.min,
            Vec2::new(self.max.x, self.min.y),
            self.max,
            Vec2::new(self.min.x, self.max.y),
        ]
    }
}

impl TryFrom<[Vec2; 2]> for Bounds {
    type Error = GeometryError;
    fn try_from([min, max]: [Vec2; 2]) -> Result<Self, Self::Error> {
        Self::new(min, max)
    }
}
impl From<Bounds> for [Vec2; 2] {
    fn from(value: Bounds) -> Self {
        [value.min, value.max]
    }
}

pub struct SvgAsset {
    pub name: &'static str,
    pub svg: &'static str,
    pub bounds: Bounds,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct AssetId(pub usize);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Paint {
    Original,
    /// Uses vector coverage as a mask for a straight-alpha RGBA color.
    Solid([u8; 4]),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Instance {
    pub asset: AssetId,
    pub placement: Placement,
    pub paint: Paint,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Document {
    /// First instance is painted first. Later instances paint over earlier ones.
    pub instances: Vec<Instance>,
}

#[derive(Clone, Copy)]
pub struct View<'a> {
    pub document: &'a Document,
    pub bounds: Bounds,
    pub clip: bool,
}

impl<'a> View<'a> {
    pub fn new(document: &'a Document, bounds: Bounds) -> Self {
        Self {
            document,
            bounds,
            clip: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rotated_and_mirrored_local_axes_map_into_document_space() {
        for (handedness, expected_y) in [
            (Handedness::Positive, Vec2::new(4.0, 20.0)),
            (Handedness::Negative, Vec2::new(16.0, 20.0)),
        ] {
            let placement =
                SimplePlacement::new(Vec2::new(10.0, 20.0), Vec2::new(0.0, 5.0), 2.0, handedness)
                    .unwrap();
            assert_eq!(placement.x_direction(), Vec2::Y);
            assert_eq!(
                placement.matrix().transform_point2(Vec2::ZERO),
                Vec2::new(10.0, 20.0)
            );
            assert_eq!(
                placement.matrix().transform_point2(Vec2::new(3.0, 0.0)),
                Vec2::new(10.0, 26.0)
            );
            assert_eq!(
                placement.matrix().transform_point2(Vec2::new(0.0, 3.0)),
                expected_y
            );
        }
    }

    #[test]
    fn storage_preserves_placements_and_rejects_invalid_geometry() {
        let placement =
            SimplePlacement::new(Vec2::new(12.0, 23.0), -Vec2::X, 0.5, Handedness::Negative)
                .unwrap();
        let document = Document {
            instances: vec![Instance {
                asset: AssetId(3),
                placement: Placement::Simple(placement),
                paint: Paint::Solid([20, 30, 40, 255]),
            }],
        };
        let stored = serde_json::to_string(&document).unwrap();
        assert_eq!(serde_json::from_str::<Document>(&stored).unwrap(), document);
        for (field, value) in [
            ("x_direction", serde_json::json!([0.0, 0.0])),
            ("scale", serde_json::json!(0.0)),
            ("handedness", serde_json::json!(0)),
        ] {
            let mut bad: serde_json::Value = serde_json::from_str(&stored).unwrap();
            bad["instances"][0]["placement"]["Simple"][field] = value;
            assert!(serde_json::from_value::<Document>(bad).is_err());
        }
        assert!(SimplePlacement::new(Vec2::ZERO, Vec2::ZERO, 1.0, Handedness::Positive).is_err());
        assert!(
            SimplePlacement::new(Vec2::splat(f32::NAN), Vec2::X, 1.0, Handedness::Positive)
                .is_err()
        );
        assert!(Bounds::new(Vec2::ZERO, Vec2::new(0.0, 2.0)).is_err());
    }
}
