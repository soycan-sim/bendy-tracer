use std::ops::Mul;

use glam::{Affine3A, Quat, Vec3A};

use crate::bvh::Aabb;
use crate::color::LinearRgb;
use crate::material::MaterialRef;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Face {
    Front,
    Back,
    Volume,
    VolumeFront,
    VolumeBack,
}

impl Face {
    pub fn is_front(&self) -> bool {
        matches!(self, Self::Front | Self::VolumeFront)
    }

    pub fn is_back(&self) -> bool {
        matches!(self, Self::Back | Self::VolumeBack)
    }

    pub fn is_surface(&self) -> bool {
        matches!(self, Self::Front | Self::Back)
    }

    pub fn is_volume(&self) -> bool {
        matches!(self, Self::Volume | Self::VolumeFront | Self::VolumeBack)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Manifold {
    pub position: Vec3A,
    pub normal: Vec3A,
    pub aabb: Aabb,
    pub face: Face,
    pub t: f32,
    pub ray: Ray,
    pub material: MaterialRef,
}

#[derive(Debug, Clone, Copy)]
pub struct ColorData {
    pub color: LinearRgb,
    pub albedo: LinearRgb,
    pub emitted: LinearRgb,
    pub normal: Vec3A,
    pub depth: f32,
}

impl Default for ColorData {
    fn default() -> Self {
        Self {
            color: LinearRgb::BLACK,
            albedo: LinearRgb::BLACK,
            emitted: LinearRgb::BLACK,
            normal: Vec3A::ZERO,
            depth: f32::INFINITY,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Clip {
    pub min: f32,
    pub max: f32,
}

#[derive(Debug, Clone, Copy)]
pub struct Ray {
    pub origin: Vec3A,
    pub direction: Vec3A,
}

impl Ray {
    const DEFAULT: Self = Ray {
        origin: Vec3A::ZERO,
        direction: Vec3A::NEG_Z,
    };

    pub fn new(origin: Vec3A, direction: Vec3A) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    pub fn with_frustum(yfov: f32, xfov: f32, u: f32, v: f32) -> Self {
        let direction = Vec3A::NEG_Z;
        let yrot = xfov * 0.5 * -u;
        let xrot = yfov * 0.5 * -v;
        let rotation = Quat::from_euler(glam::EulerRot::YXZ, yrot, xrot, 0.0);
        let direction = rotation * direction;
        Self {
            direction,
            ..Default::default()
        }
    }

    pub fn at(&self, t: f32) -> Vec3A {
        self.origin + t * self.direction
    }
}

impl Default for Ray {
    fn default() -> Self {
        Self::DEFAULT
    }
}

macro_rules! impl_mul_fn {
    ($ray:ty) => {
        fn mul(self, ray: $ray) -> Self::Output {
            #[allow(clippy::suspicious_arithmetic_impl)]
            let origin = self.translation + ray.origin;
            let direction = self
                .transform_vector3(ray.direction.into())
                .normalize_or_zero()
                .into();
            Ray::new(origin, direction)
        }
    };
}

impl Mul<Ray> for Affine3A {
    type Output = Ray;

    impl_mul_fn!(Ray);
}

impl<'a> Mul<&'a Ray> for Affine3A {
    type Output = Ray;

    impl_mul_fn!(&'a Ray);
}

impl<'a> Mul<Ray> for &'a Affine3A {
    type Output = Ray;

    impl_mul_fn!(Ray);
}

impl<'a, 'b> Mul<&'a Ray> for &'b Affine3A {
    type Output = Ray;

    impl_mul_fn!(&'a Ray);
}
