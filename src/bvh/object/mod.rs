use glam::{Affine3A, Vec3A};
use rand::Rng;

use crate::scene::ObjectFlags;
use crate::tracer::{Clip, Manifold, Ray};

mod cuboid;
mod rect;
mod sphere;

pub use cuboid::*;
pub use rect::*;
pub use sphere::*;

use super::Aabb;

#[derive(Debug, Clone)]
pub struct ObjectData {
    pub flags: ObjectFlags,
    pub tag: Option<String>,
    pub transform: Affine3A,
    pub shape: Shape,
}

impl ObjectData {
    pub fn flags(&self) -> ObjectFlags {
        self.flags
    }

    pub fn has_flags(&self, flags: ObjectFlags) -> bool {
        self.flags.contains(flags)
    }

    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub fn bounding_box(&self) -> Aabb {
        match &self.shape {
            Shape::Sphere(sphere) => sphere.bounding_box(&self.transform),
            Shape::Rect(rect) => rect.bounding_box(&self.transform),
            Shape::Cuboid(cuboid) => cuboid.bounding_box(&self.transform),
        }
    }

    pub fn random_point<R: Rng + ?Sized>(&self, rng: &mut R) -> Vec3A {
        match &self.shape {
            Shape::Sphere(sphere) => sphere.random_point(rng, &self.transform),
            Shape::Rect(rect) => rect.random_point(rng, &self.transform),
            Shape::Cuboid(cuboid) => cuboid.random_point(rng, &self.transform),
        }
    }

    pub fn pdf(&self, ray: &Ray, clip: &Clip) -> Option<f32> {
        match &self.shape {
            Shape::Sphere(sphere) => sphere.pdf(self, ray, clip),
            Shape::Rect(rect) => rect.pdf(self, ray, clip),
            Shape::Cuboid(cuboid) => cuboid.pdf(self, ray, clip),
        }
    }

    pub fn hit(&self, ray: &Ray, clip: &Clip) -> Option<Manifold> {
        match &self.shape {
            Shape::Sphere(sphere) => sphere.hit(self, ray, clip),
            Shape::Rect(rect) => rect.hit(self, ray, clip),
            Shape::Cuboid(cuboid) => cuboid.hit(self, ray, clip),
        }
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum Shape {
    Sphere(Sphere),
    Rect(Rect),
    Cuboid(Cuboid),
}
