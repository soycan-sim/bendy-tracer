use glam::Vec3A;
use rand::prelude::*;
use serde::{Deserialize, Serialize};

use crate::color::LinearRgb;
use crate::math::{UnitHemisphere, Vec3Ext};
use crate::tracer::{ColorData, Manifold, Ray};

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
    ) -> (Option<Ray>, Option<ColorData>) {
        match *self {
            Material::Flat { albedo } => {
                let color_data = ColorData {
                    color: albedo,
                    albedo,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                (None, Some(color_data))
            }
            Material::Diffuse { albedo, roughness } => {
                let hemisphere = UnitHemisphere::new(manifold.normal.into());

                let origin = manifold.position;
                let direction = manifold.normal;
                let fuzz: Vec3A = hemisphere.sample(rng);
                let fuzz = fuzz * roughness;
                let ray = Ray::new(origin, direction + fuzz);

                let color_data = ColorData {
                    color: albedo,
                    albedo,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                (Some(ray), Some(color_data))
            }
            Material::Metallic { albedo, roughness } => {
                let hemisphere = UnitHemisphere::new(manifold.normal.into());

                let origin = manifold.position;
                let direction = manifold.ray.direction.reflect(manifold.normal);
                let fuzz: Vec3A = hemisphere.sample(rng);
                let fuzz = fuzz * roughness;
                let ray = Ray::new(origin, direction + fuzz);

                let color_data = ColorData {
                    color: albedo,
                    albedo,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                (Some(ray), Some(color_data))
            }
            Material::Glass {
                albedo,
                roughness,
                ior,
            } => {
                let hemisphere = UnitHemisphere::new(manifold.normal.into());

                let ior = if manifold.face.is_front() {
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

                let color_data = ColorData {
                    color: albedo,
                    albedo,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                (Some(ray), Some(color_data))
            }
            Material::Emissive { albedo, intensity } => {
                let color_data = ColorData {
                    color: albedo * intensity,
                    albedo: albedo * intensity,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                (None, Some(color_data))
            }
        }
    }
}
