use glam::{Vec3, Vec3A};

pub mod distr;

pub trait Interpolate {
    fn lerp(self, other: Self, factor: f32) -> Self;
}

impl<T: Interpolate> Interpolate for Option<T> {
    fn lerp(self, other: Self, factor: f32) -> Self {
        match (self, other) {
            (None, None) => None,
            (Some(x), None) => Some(x),
            (None, Some(y)) => Some(y),
            (Some(x), Some(y)) => Some(x.lerp(y, factor)),
        }
    }
}

impl Interpolate for f32 {
    fn lerp(self, other: Self, factor: f32) -> Self {
        self + (other - self) * factor
    }
}

impl Interpolate for Vec3 {
    fn lerp(self, other: Self, factor: f32) -> Self {
        self + (other - self) * factor
    }
}

impl Interpolate for Vec3A {
    fn lerp(self, other: Self, factor: f32) -> Self {
        self + (other - self) * factor
    }
}

pub trait Vec3Ext {
    fn project(self, normal: Self) -> Self;
    fn reflect(self, normal: Self) -> Self;
    fn refract(self, normal: Self, ior: f32) -> Self;
    fn fresnel(self, normal: Self, ior: f32) -> f32;
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
        }
    };
}

impl_vec3_ext!(Vec3);
impl_vec3_ext!(Vec3A);
