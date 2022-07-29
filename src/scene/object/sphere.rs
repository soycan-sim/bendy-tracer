use serde::{Deserialize, Serialize};

use crate::material::MaterialRef;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sphere {
    pub material: MaterialRef,
    pub radius: f32,
}

impl Sphere {
    pub fn new(material: MaterialRef, radius: f32) -> Self {
        Self { material, radius }
    }
}
