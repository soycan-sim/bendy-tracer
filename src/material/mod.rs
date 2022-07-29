use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::bvh::Bvh;
use crate::color::LinearRgb;
use crate::tracer::Clip;
use crate::tracer::ColorData;
use crate::tracer::Manifold;
use crate::tracer::Ray;

mod surface;
mod volume;

pub use self::surface::*;
pub use self::volume::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub struct MaterialRef(usize);

impl MaterialRef {
    pub fn root() -> Self {
        Self(0)
    }

    pub fn to_index(self) -> usize {
        self.0 - 1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Materials {
    root: Material,
    materials: Vec<Material>,
}

impl Materials {
    pub fn new() -> Self {
        Self::with_root(Material::Surface(Surface::Emissive {
            albedo: LinearRgb::BLACK,
            intensity: 0.0,
        }))
    }

    pub fn with_root(root: Material) -> Self {
        Self {
            root,
            materials: Vec::new(),
        }
    }

    pub fn add(&mut self, material: Material) -> MaterialRef {
        self.materials.push(material);
        MaterialRef(self.materials.len())
    }

    pub fn get(&self, index: MaterialRef) -> &Material {
        if index.0 == 0 {
            self.root()
        } else {
            &self.materials[index.to_index()]
        }
    }

    pub fn root(&self) -> &Material {
        &self.root
    }
}

impl Default for Materials {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ShaderData {
    pub is_volume: bool,
    pub scatter: Option<Ray>,
    pub color: Option<ColorData>,
    pub pdf: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Material {
    Surface(Surface),
    Volume(Volume),
}

impl Material {
    pub const fn flat(albedo: LinearRgb) -> Self {
        Self::Surface(Surface::Emissive {
            albedo,
            intensity: 1.0,
        })
    }

    pub const fn diffuse(albedo: LinearRgb) -> Self {
        Self::Surface(Surface::Diffuse { albedo })
    }

    pub const fn metallic(albedo: LinearRgb, roughness: f32) -> Self {
        Self::Surface(Surface::Metallic { albedo, roughness })
    }

    pub const fn glass(albedo: LinearRgb, roughness: f32, ior: f32) -> Self {
        Self::Surface(Surface::Glass {
            albedo,
            roughness,
            ior,
        })
    }

    pub const fn emissive(albedo: LinearRgb, intensity: f32) -> Self {
        Self::Surface(Surface::Emissive { albedo, intensity })
    }

    pub fn shade<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        manifold: &Manifold,
        clip: &Clip,
        step: f32,
        bvh: &Bvh,
    ) -> ShaderData {
        match self {
            Self::Surface(surface) => surface.shade(rng, manifold, clip, bvh),
            Self::Volume(volume) => volume.shade(rng, manifold, step),
        }
    }

    pub fn pdf(&self, manifold: &Manifold, ray: &Ray) -> f32 {
        match self {
            Self::Surface(surface) => surface.pdf(manifold, ray),
            Self::Volume(_) => 1.0,
        }
    }
}

impl From<Surface> for Material {
    fn from(surface: Surface) -> Self {
        Self::Surface(surface)
    }
}

impl From<Volume> for Material {
    fn from(volume: Volume) -> Self {
        Self::Volume(volume)
    }
}
