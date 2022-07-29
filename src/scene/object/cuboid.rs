use glam::Vec3A;
use serde::{Deserialize, Serialize};

use crate::material::MaterialRef;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cuboid {
    pub material: MaterialRef,
    pub(crate) x: Vec3A,
    pub(crate) y: Vec3A,
    pub(crate) z: Vec3A,
}

impl Cuboid {
    pub fn new(material: MaterialRef, x: Vec3A, y: Vec3A, z: Vec3A) -> Self {
        debug_assert!(x.dot(y).abs() < 1e-5);
        debug_assert!(x.dot(z).abs() < 1e-5);
        debug_assert!(x.length_squared() > 0.0);
        debug_assert!(y.length_squared() > 0.0);
        debug_assert!(z.length_squared() > 0.0);

        Self { material, x, y, z }
    }
}
