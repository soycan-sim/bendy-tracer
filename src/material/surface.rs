use std::f32;

use glam::Vec3A;
use rand::prelude::*;
use rand_distr::Uniform;
use serde::{Deserialize, Serialize};

use crate::bvh::{Bvh, ObjectData};
use crate::color::LinearRgb;
use crate::math::distr::{Cosine, UnitHemisphere};
use crate::math::{Interpolate, Vec3Ext};
use crate::scene::ObjectFlags;
use crate::tracer::{Clip, ColorData, Manifold, Ray};

use super::ShaderData;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum Surface {
    Diffuse {
        albedo: LinearRgb,
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

impl Surface {
    pub fn shade<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        manifold: &Manifold,
        clip: &Clip,
        bvh: &Bvh,
    ) -> ShaderData {
        match *self {
            Surface::Diffuse { albedo } => {
                let color_data = ColorData {
                    color: albedo,
                    albedo,
                    emitted: LinearRgb::BLACK,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                let count = bvh
                    .iter()
                    .filter(|object| object.has_flags(ObjectFlags::LIGHT))
                    .count();

                let index = rng.sample::<usize, _>(Uniform::new(0, count));

                let light = bvh
                    .iter()
                    .filter(|object| object.has_flags(ObjectFlags::LIGHT))
                    .nth(index)
                    .unwrap();

                let light = Pdf::Light(light);
                let pdf = Pdf::Mix(&Pdf::Diffuse, &light, 0.5);
                let ray = pdf.scatter(rng, manifold);

                if let Some(pdf) = pdf.pdf(&ray, manifold, clip) {
                    ShaderData {
                        is_volume: false,
                        scatter: Some(ray),
                        color: Some(color_data),
                        pdf,
                    }
                } else {
                    ShaderData {
                        is_volume: false,
                        scatter: None,
                        color: Some(color_data),
                        pdf: 1.0,
                    }
                }
            }
            Surface::Metallic { albedo, roughness } => {
                let color_data = ColorData {
                    color: albedo,
                    albedo,
                    emitted: LinearRgb::BLACK,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                let pdf = Pdf::Metallic(roughness);
                let ray = pdf.scatter(rng, manifold);

                if let Some(pdf) = pdf.pdf(&ray, manifold, clip) {
                    ShaderData {
                        is_volume: false,
                        scatter: Some(ray),
                        color: Some(color_data),
                        pdf,
                    }
                } else {
                    ShaderData {
                        is_volume: false,
                        scatter: None,
                        color: Some(color_data),
                        pdf: 1.0,
                    }
                }
            }
            Surface::Glass {
                albedo,
                roughness,
                ior,
            } => {
                let color_data = ColorData {
                    color: albedo,
                    albedo,
                    emitted: LinearRgb::BLACK,
                    normal: manifold.normal,
                    depth: manifold.t,
                };

                let pdf = Pdf::Glass(roughness, ior);
                let ray = pdf.scatter(rng, manifold);

                if let Some(pdf) = pdf.pdf(&ray, manifold, clip) {
                    ShaderData {
                        is_volume: false,
                        scatter: Some(ray),
                        color: Some(color_data),
                        pdf,
                    }
                } else {
                    ShaderData {
                        is_volume: false,
                        scatter: None,
                        color: Some(color_data),
                        pdf: 1.0,
                    }
                }
            }
            Surface::Emissive { albedo, intensity } => ShaderData {
                is_volume: false,
                scatter: None,
                color: Some(ColorData {
                    color: LinearRgb::BLACK,
                    albedo: LinearRgb::BLACK,
                    emitted: albedo * intensity,
                    normal: manifold.normal,
                    depth: manifold.t,
                }),
                pdf: 1.0,
            },
        }
    }

    pub fn pdf(&self, manifold: &Manifold, ray: &Ray) -> f32 {
        match *self {
            Surface::Diffuse { .. } => diffuse_pdf(ray, manifold),
            Surface::Metallic { roughness, .. } => metallic_pdf(ray, manifold, roughness),
            Surface::Glass { roughness, ior, .. } => glass_pdf(ray, manifold, roughness, ior),
            Surface::Emissive { .. } => 1.0,
        }
    }
}

#[derive(Debug)]
enum Pdf<'pdf, 'a> {
    Diffuse,
    Metallic(f32),
    Glass(f32, f32),
    Light(&'a ObjectData),
    Mix(&'pdf Pdf<'pdf, 'a>, &'pdf Pdf<'pdf, 'a>, f32),
}

impl<'pdf, 'a> Pdf<'pdf, 'a> {
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
            Self::Light(light) => {
                let origin = manifold.position;
                let direction = light.random_point(rng) - origin;
                Ray::new(origin, direction)
            }
            Self::Mix(a, b, x) => {
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
        if p.abs() < 1e-5 {
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
            Self::Light(object) => light_pdf(ray, manifold, clip, object),
            Self::Mix(a, b, x) => a
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

fn light_pdf(ray: &Ray, _manifold: &Manifold, clip: &Clip, object: &ObjectData) -> f32 {
    object.pdf(ray, clip).unwrap_or_default()
}
