use std::f32::consts::{PI, TAU};

use glam::{Vec3, Vec3A};
use rand::distributions::Uniform;
use rand::prelude::*;

pub trait Vec3Ext {
    fn project(self, normal: Self) -> Self;
    fn reflect(self, normal: Self) -> Self;
    fn refract(self, normal: Self, ior: f32) -> Self;
    fn fresnel(self, normal: Self, ior: f32) -> f32;
    fn lerp(self, other: Self, factor: f32) -> Self;
}

macro_rules! impl_vec3_ext {
    ($vect:ty) => {
        impl Vec3Ext for $vect {
            fn project(self, normal: Self) -> Self {
                normal * self.dot(normal)
            }

            fn reflect(self, normal: Self) -> Self {
                self - 2.0 * self.dot(normal) * normal
            }

            fn refract(self, normal: Self, ior: f32) -> Self {
                let cos_theta = (-self).dot(normal).min(1.0);
                let perp = (normal * cos_theta + self) * ior;
                let parallel = normal * -(1.0 - perp.length_squared()).abs().sqrt();
                perp + parallel
            }

            fn fresnel(self, normal: Self, ior: f32) -> f32 {
                let cos_theta = (-self).dot(normal).min(1.0);
                let r0 = (1.0 - ior) / (1.0 + ior);
                let r0 = r0 * r0;
                r0 + (1.0 - r0) * (1.0 - cos_theta).powi(5)
            }

            fn lerp(self, other: Self, factor: f32) -> Self {
                self + (other - self) * factor
            }
        }
    };
}

impl_vec3_ext!(Vec3);
impl_vec3_ext!(Vec3A);

pub struct UnitSphere;

impl Distribution<Vec3> for UnitSphere {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3 {
        let half_circle = Uniform::new_inclusive(0.0, PI);
        let full_circle = Uniform::new_inclusive(0.0, TAU);

        let theta = half_circle.sample(rng);
        let phi = full_circle.sample(rng);

        let x = phi.cos() * theta.sin();
        let y = phi.sin() * theta.sin();
        let z = theta.cos();

        Vec3::new(x, y, z)
    }
}

impl Distribution<Vec3A> for UnitSphere {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3A {
        <Self as Distribution<Vec3>>::sample(self, rng).into()
    }
}

pub struct UnitHemisphere {
    normal: Vec3,
}

impl UnitHemisphere {
    pub fn new(normal: Vec3) -> Self {
        Self {
            normal: normal.normalize(),
        }
    }
}

impl Distribution<Vec3> for UnitHemisphere {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3 {
        let vec = UnitSphere.sample(rng);
        if self.normal.dot(vec) >= 0.0 {
            vec
        } else {
            vec.reflect(self.normal)
        }
    }
}

impl Distribution<Vec3A> for UnitHemisphere {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3A {
        <Self as Distribution<Vec3>>::sample(self, rng).into()
    }
}
