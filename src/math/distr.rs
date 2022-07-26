use std::f32::consts::TAU;

use glam::{Vec3, Vec3A};
use rand::distributions::Uniform;
use rand::prelude::*;

#[derive(Debug, Clone, Copy)]
pub struct UnitSphere;

impl Distribution<Vec3> for UnitSphere {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3 {
        let r1 = rng.sample::<f32, _>(Uniform::new_inclusive(0.0, TAU));
        let r2 = rng.sample::<f32, _>(Uniform::new_inclusive(0.0, 1.0));

        let x = r1.cos() * 2.0 * (r2 * (1.0 - r2)).sqrt();
        let y = r1.sin() * 2.0 * (r2 * (1.0 - r2)).sqrt();
        let z = 1.0 - 2.0 * r2;

        Vec3::new(x, y, z)
    }
}

impl Distribution<Vec3A> for UnitSphere {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3A {
        <Self as Distribution<Vec3>>::sample(self, rng).into()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UnitHemisphere {
    x_axis: Vec3,
    y_axis: Vec3,
    z_axis: Vec3,
}

impl UnitHemisphere {
    pub fn new(normal: Vec3) -> Self {
        let z_axis = normal.normalize();
        let (x_axis, y_axis) = z_axis.any_orthonormal_pair();
        Self {
            x_axis,
            y_axis,
            z_axis,
        }
    }
}

impl Distribution<Vec3> for UnitHemisphere {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3 {
        let r1 = rng.sample::<f32, _>(Uniform::new_inclusive(0.0, TAU));
        let r2 = rng.sample::<f32, _>(Uniform::new_inclusive(0.0, 1.0));

        let x = r1.cos() * 2.0 * (r2 * (1.0 - r2)).sqrt();
        let y = r1.sin() * 2.0 * (r2 * (1.0 - r2)).sqrt();
        let z = 1.0 - r2;

        self.x_axis * x + self.y_axis * y + self.z_axis * z
    }
}

impl Distribution<Vec3A> for UnitHemisphere {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3A {
        <Self as Distribution<Vec3>>::sample(self, rng).into()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Cosine {
    x_axis: Vec3,
    y_axis: Vec3,
    z_axis: Vec3,
}

impl Cosine {
    pub fn new(normal: Vec3) -> Self {
        let z_axis = normal.normalize();
        let (x_axis, y_axis) = z_axis.any_orthonormal_pair();
        Self {
            x_axis,
            y_axis,
            z_axis,
        }
    }
}

impl Distribution<Vec3> for Cosine {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3 {
        let r1 = rng.sample::<f32, _>(Uniform::new_inclusive(0.0, TAU));
        let r2 = rng.sample::<f32, _>(Uniform::new_inclusive(0.0, 1.0));

        let x = r1.cos() * r2.sqrt();
        let y = r1.sin() * r2.sqrt();
        let z = (1.0 - r2).sqrt();

        self.x_axis * x + self.y_axis * y + self.z_axis * z
    }
}

impl Distribution<Vec3A> for Cosine {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3A {
        <Self as Distribution<Vec3>>::sample(self, rng).into()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct UnitDisk {
    x_axis: Vec3,
    y_axis: Vec3,
}

impl UnitDisk {
    pub fn new(normal: Vec3) -> Self {
        let normal = normal.normalize();
        let (x_axis, y_axis) = normal.any_orthonormal_pair();
        Self { x_axis, y_axis }
    }
}

impl Distribution<Vec3> for UnitDisk {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3 {
        let full_circle = Uniform::new_inclusive(0.0, TAU);
        let radius = Uniform::new_inclusive(0.0, 1.0);

        let angle = full_circle.sample(rng);
        let r = radius.sample(rng);

        let x = angle.cos();
        let y = angle.sin();

        (self.x_axis * x + self.y_axis * y) * r
    }
}

impl Distribution<Vec3A> for UnitDisk {
    fn sample<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3A {
        <Self as Distribution<Vec3>>::sample(self, rng).into()
    }
}
