use glam::Vec3A;
use rand::prelude::*;
use serde::{Deserialize, Serialize};

use crate::color::LinearRgb;
use crate::math::{UnitHemisphere, Vec3Ext};
use crate::tracer::{Face, Manifold, Ray};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Material {
    Flat {
        albedo: LinearRgb,
    },
    Diffuse {
        albedo: LinearRgb,
        roughness: f32,
    },
    Metallic {
        albedo: LinearRgb,
        roughness: f32,
    },
    Glass {
        albedo: LinearRgb,
        roughness: f32,
        ior: f32,
    },
    Emissive {
        albedo: LinearRgb,
        intensity: f32,
    },
}

impl Material {
    pub const fn flat(albedo: LinearRgb) -> Self {
        Self::Flat { albedo }
    }

    pub const fn diffuse(albedo: LinearRgb, roughness: f32) -> Self {
        Self::Diffuse { albedo, roughness }
    }

    pub const fn glass(albedo: LinearRgb, roughness: f32, ior: f32) -> Self {
        Self::Glass {
            albedo,
            roughness,
            ior,
        }
    }

    pub const fn emissive(albedo: LinearRgb, intensity: f32) -> Self {
        Self::Emissive { albedo, intensity }
    }

    pub fn shade<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        manifold: &Manifold,
    ) -> (Option<Ray>, LinearRgb) {
        match *self {
            Material::Flat { albedo } => (None, albedo),
            Material::Diffuse { albedo, roughness } => {
                let hemisphere = UnitHemisphere::new(manifold.normal.into());

                let origin = manifold.position;
                let direction = manifold.normal;
                let fuzz: Vec3A = hemisphere.sample(rng);
                let fuzz = fuzz * roughness;
                let ray = Ray::new(origin, direction + fuzz);

                (Some(ray), albedo)
            }
            Material::Metallic { albedo, roughness } => {
                let hemisphere = UnitHemisphere::new(manifold.normal.into());

                let origin = manifold.position;
                let direction = manifold.ray.direction.reflect(manifold.normal);
                let fuzz: Vec3A = hemisphere.sample(rng);
                let fuzz = fuzz * roughness;
                let ray = Ray::new(origin, direction + fuzz);

                (Some(ray), albedo)
            }
            Material::Glass {
                albedo,
                roughness,
                ior,
            } => {
                let hemisphere = UnitHemisphere::new(manifold.normal.into());

                let ior = if manifold.face == Face::Front {
                    ior.recip()
                } else {
                    ior
                };
                let cos_theta = (-manifold.ray.direction).dot(manifold.normal).min(1.0);
                let sin_theta = (1.0 - cos_theta * cos_theta).sqrt();
                let fresnel = manifold.ray.direction.fresnel(manifold.normal, ior);

                let origin = manifold.position;
                let direction = if ior * sin_theta > 1.0 || rng.gen_bool(fresnel as _) {
                    manifold.ray.direction.reflect(manifold.normal)
                } else {
                    manifold.ray.direction.refract(manifold.normal, ior)
                };
                let fuzz: Vec3A = hemisphere.sample(rng);
                let fuzz = fuzz * roughness;
                let ray = Ray::new(origin, direction + fuzz);

                (Some(ray), albedo)
            }
            Material::Emissive { albedo, intensity } => (None, albedo * intensity),
        }
    }
}
