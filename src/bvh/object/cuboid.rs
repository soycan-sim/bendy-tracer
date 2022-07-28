use std::f32;

use glam::{Affine3A, Vec3A};
use rand::distributions::WeightedIndex;
use rand::Rng;

use crate::bvh::Aabb;
use crate::material::MaterialRef;
use crate::scene;
use crate::tracer::{Clip, Manifold, Ray};

use super::{ObjectData, Rect};

#[derive(Debug, Clone)]
pub struct Cuboid {
    pub faces: Box<[(Vec3A, Rect); 6]>,
}

impl Cuboid {
    pub fn new(material: MaterialRef, x: Vec3A, y: Vec3A, z: Vec3A) -> Self {
        debug_assert!(x.dot(y).abs() < 1e-5);
        debug_assert!(x.dot(z).abs() < 1e-5);
        debug_assert!(x.length_squared() > 0.0);
        debug_assert!(y.length_squared() > 0.0);
        debug_assert!(z.length_squared() > 0.0);

        Self {
            faces: Box::new([
                (-z, Rect::new(material, x, y)),
                (z, Rect::new(material, -x, y)),
                (-x, Rect::new(material, z, y)),
                (x, Rect::new(material, -z, y)),
                (-y, Rect::new(material, x, z)),
                (y, Rect::new(material, x, -z)),
            ]),
        }
    }

    fn faces(&self) -> impl Iterator<Item = (Vec3A, &Rect)> {
        self.faces.iter().map(|(offset, rect)| (*offset, rect))
    }

    pub fn bounding_box(&self, transform: &Affine3A) -> Aabb {
        let (min, max) = self
            .faces()
            .map(|(offset, rect)| {
                let transform = *transform * Affine3A::from_translation(offset.into());
                let Aabb { min, max } = rect.bounding_box(&transform);
                (min, max)
            })
            .fold(
                (Vec3A::splat(f32::INFINITY), Vec3A::splat(f32::NEG_INFINITY)),
                |(mina, maxa), (minb, maxb)| (mina.min(minb), maxa.max(maxb)),
            );
        Aabb::new(min, max)
    }

    pub fn random_point<R: Rng + ?Sized>(&self, rng: &mut R, transform: &Affine3A) -> Vec3A {
        let dist = WeightedIndex::new(self.faces().map(|(_, rect)| rect.area())).unwrap();
        let index = rng.sample(dist);
        let (offset, rect) = &self.faces[index];
        let transform = *transform * Affine3A::from_translation((*offset).into());
        rect.random_point(rng, &transform)
    }

    pub fn pdf(&self, object: &ObjectData, ray: &Ray, clip: &Clip) -> Option<f32> {
        let mut t = clip.max;
        let mut result = None;

        for (offset, rect) in self.faces() {
            let transform = object.transform * Affine3A::from_translation(offset.into());
            if let Some(manifold) = rect.hit_impl(&transform, ray, clip) {
                if manifold.t < t {
                    t = manifold.t;
                    result = Some((offset, rect));
                }
            }
        }

        result.and_then(|(offset, rect)| {
            let transform = object.transform * Affine3A::from_translation(offset.into());
            rect.pdf_impl(&transform, ray, clip)
        })
    }

    pub fn hit(&self, object: &ObjectData, ray: &Ray, clip: &Clip) -> Option<Manifold> {
        let mut t = clip.max;
        let mut result = None;

        for (offset, rect) in self.faces() {
            let transform = object.transform * Affine3A::from_translation(offset.into());
            if let Some(manifold) = rect.hit_impl(&transform, ray, clip) {
                if manifold.t < t {
                    t = manifold.t;
                    result = Some(manifold);
                }
            }
        }

        result
    }
}

impl<'a> From<&'a scene::Cuboid> for Cuboid {
    fn from(cuboid: &'a scene::Cuboid) -> Self {
        Self::new(cuboid.material, cuboid.x, cuboid.y, cuboid.z)
    }
}
