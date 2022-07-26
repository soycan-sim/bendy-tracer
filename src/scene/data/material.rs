use std::f32;

use approx::AbsDiffEq;
use glam::Vec3A;
use rand::prelude::*;
use rand_distr::Uniform;
use serde::{Deserialize, Serialize};

use crate::color::LinearRgb;
use crate::math::distr::{Cosine, UnitHemisphere};
use crate::math::{Interpolate, Vec3Ext};
use crate::scene::{ObjectFlags, ObjectRef};
use crate::tracer::{Clip, ColorData, Manifold, Ray};

#[derive(Debug, Clone, Copy)]
pub struct ShaderData {
    pub scatter: Option<Ray>,
    pub albedo: Option<ColorData>,
    pub pdf: f32,
}

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

    pub fn emitted<R: Rng + ?Sized>(&self, _rng: &mut R, _manifold: &Manifold) -> LinearRgb {
        match *self {
            Material::Diffuse { .. } | Material::Metallic { .. } | Material::Glass { .. } => {
                LinearRgb::BLACK
            }
            Material::Flat { albedo } => albedo,
            Material::Emissive { albedo, intensity } => albedo * intensity,
        }
    }

    pub fn shade<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        manifold: &Manifold,
        clip: &Clip,
    ) -> ShaderData {
        match *self {
            Material::Flat { .. } => ShaderData {
                scatter: None,
                albedo: Some(ColorData {
                    color: LinearRgb::BLACK,
                    albedo: LinearRgb::BLACK,
                    normal: manifold.normal,
                    depth: manifold.t,
                }),
                pdf: 1.0,
            },
            Material::Diffuse { albedo, .. } => {
                let color_data = ColorData {
                    color: albedo,
                    albedo,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                let count = manifold
                    .scene
                    .iter()
                    .filter(|object| object.has_flags(ObjectFlags::LIGHT))
                    .count();

                let index = rng.sample::<usize, _>(Uniform::new(0, count));

                let (light, _) = manifold
                    .scene
                    .pairs()
                    .filter(|(_, object)| object.has_flags(ObjectFlags::LIGHT))
                    .nth(index)
                    .unwrap();

                // TODO: optimize this allocation
                let pdf = Pdf::Mix(Box::new(Pdf::Diffuse), Box::new(Pdf::Light(light)), 0.5);
                let ray = pdf.scatter(rng, manifold);

                if let Some(pdf) = pdf.pdf(&ray, manifold, clip) {
                    ShaderData {
                        scatter: Some(ray),
                        albedo: Some(color_data),
                        pdf,
                    }
                } else {
                    ShaderData {
                        scatter: None,
                        albedo: Some(color_data),
                        pdf: 1.0,
                    }
                }
            }
            Material::Metallic { albedo, roughness } => {
                let color_data = ColorData {
                    color: albedo,
                    albedo,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                let pdf = Pdf::Metallic(roughness);
                let ray = pdf.scatter(rng, manifold);

                if let Some(pdf) = pdf.pdf(&ray, manifold, clip) {
                    ShaderData {
                        scatter: Some(ray),
                        albedo: Some(color_data),
                        pdf,
                    }
                } else {
                    ShaderData {
                        scatter: None,
                        albedo: Some(color_data),
                        pdf: 1.0,
                    }
                }
            }
            Material::Glass {
                albedo,
                roughness,
                ior,
            } => {
                let color_data = ColorData {
                    color: albedo,
                    albedo,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                let pdf = Pdf::Glass(roughness, ior);
                let ray = pdf.scatter(rng, manifold);

                if let Some(pdf) = pdf.pdf(&ray, manifold, clip) {
                    ShaderData {
                        scatter: Some(ray),
                        albedo: Some(color_data),
                        pdf,
                    }
                } else {
                    ShaderData {
                        scatter: None,
                        albedo: Some(color_data),
                        pdf: 1.0,
                    }
                }
            }
            Material::Emissive { .. } => ShaderData {
                scatter: None,
                albedo: None,
                pdf: 1.0,
            },
        }
    }

    pub fn pdf(&self, manifold: &Manifold, ray: &Ray) -> f32 {
        match *self {
            Material::Flat { .. } => 1.0,
            Material::Diffuse { .. } => diffuse_pdf(ray, manifold),
            Material::Metallic { roughness, .. } => metallic_pdf(ray, manifold, roughness),
            Material::Glass { roughness, ior, .. } => glass_pdf(ray, manifold, roughness, ior),
            Material::Emissive { .. } => 1.0,
        }
    }
}

#[derive(Debug)]
enum Pdf {
    Diffuse,
    Metallic(f32),
    Glass(f32, f32),
    Light(ObjectRef),
    Mix(Box<Pdf>, Box<Pdf>, f32),
}

impl Pdf {
    pub fn scatter<R: Rng + ?Sized>(&self, rng: &mut R, manifold: &Manifold) -> Ray {
        match *self {
            Self::Diffuse => {
                let cosine = Cosine::new(manifold.normal.into());

                let origin = manifold.position;
                let direction = rng.sample::<Vec3A, _>(&cosine);
                Ray::new(origin, direction)
            }
            Self::Metallic(roughness) => {
                let hemisphere = UnitHemisphere::new(manifold.normal.into());

                let origin = manifold.position;
                let direction = manifold.ray.direction.reflect(manifold.normal);
                let fuzz: Vec3A = hemisphere.sample(rng);
                let fuzz = fuzz * roughness;
                Ray::new(origin, direction + fuzz)
            }
            Self::Glass(roughness, ior) => {
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
                Ray::new(origin, direction + fuzz)
            }
            Self::Light(object) => {
                let light = manifold.scene.get_object(object);

                let origin = manifold.position;
                let direction = light.random_point(rng) - origin;
                Ray::new(origin, direction)
            }
            Self::Mix(ref a, ref b, x) => {
                if rng.gen_bool(x as _) {
                    b.scatter(rng, manifold)
                } else {
                    a.scatter(rng, manifold)
                }
            }
        }
    }

    pub fn pdf(&self, ray: &Ray, manifold: &Manifold, clip: &Clip) -> Option<f32> {
        let p = self.pdf_impl(ray, manifold, clip);
        if p.abs_diff_eq(&0.0, 1e-5) {
            None
        } else {
            Some(p)
        }
    }

    fn pdf_impl(&self, ray: &Ray, manifold: &Manifold, clip: &Clip) -> f32 {
        match *self {
            Self::Diffuse => diffuse_pdf(ray, manifold),
            Self::Metallic(roughness) => metallic_pdf(ray, manifold, roughness),
            Self::Glass(roughness, ior) => glass_pdf(ray, manifold, roughness, ior),
            Self::Light(object) => light_pdf(object, ray, manifold, clip),
            Self::Mix(ref a, ref b, x) => a
                .pdf_impl(ray, manifold, clip)
                .lerp(b.pdf_impl(ray, manifold, clip), x),
        }
    }
}

fn diffuse_pdf(ray: &Ray, manifold: &Manifold) -> f32 {
    manifold.normal.dot(ray.direction) * f32::consts::FRAC_1_PI
}

fn metallic_pdf(_ray: &Ray, _manifold: &Manifold, _roughness: f32) -> f32 {
    1.0
}

fn glass_pdf(_ray: &Ray, _manifold: &Manifold, _roughness: f32, _ior: f32) -> f32 {
    1.0
}

fn light_pdf(object: ObjectRef, ray: &Ray, manifold: &Manifold, clip: &Clip) -> f32 {
    let light = manifold.scene.get_object(object);
    light.pdf(ray, clip, manifold.scene).unwrap_or_default()
}
