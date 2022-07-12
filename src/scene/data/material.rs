use glam::Vec3A;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Material {
    pub albedo: Vec3A,
}

impl Material {
    pub fn flat(albedo: Vec3A) -> Self {
        Self { albedo }
    }
}
