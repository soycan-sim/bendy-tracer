use std::f32;

use glam::{Affine3A, Vec3A};
use rand::distributions::WeightedIndex;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::scene::{DataRef, ObjectRef, Scene};
use crate::tracer::{Clip, Manifold, Ray};

use super::Rect;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cuboid {
    pub faces: Box<[(Vec3A, Rect); 6]>,
}

impl Cuboid {
    pub fn new(material: DataRef, x: Vec3A, y: Vec3A, z: Vec3A) -> Self {
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

    pub fn bounding_box(&self, transform: &Affine3A) -> (Vec3A, Vec3A) {
        self.faces()
            .map(|(offset, rect)| {
                let transform = *transform * Affine3A::from_translation(offset.into());
                rect.bounding_box(&transform)
            })
            .fold(
                (Vec3A::splat(f32::INFINITY), Vec3A::splat(f32::NEG_INFINITY)),
                |(mina, maxa), (minb, maxb)| (mina.min(minb), maxa.max(maxb)),
            )
    }

    pub fn random_point<R: Rng + ?Sized>(&self, rng: &mut R, transform: &Affine3A) -> Vec3A {
        let dist = WeightedIndex::new(self.faces().map(|(_, rect)| rect.area())).unwrap();
        let index = rng.sample(dist);
        let (offset, rect) = &self.faces[index];
        let transform = *transform * Affine3A::from_translation((*offset).into());
        rect.random_point(rng, &transform)
    }

    pub fn pdf(
        &self,
        object_ref: ObjectRef,
        transform: &Affine3A,
        ray: &Ray,
        clip: &Clip,
        scene: &Scene,
    ) -> Option<f32> {
        let mut t = clip.max;
        let mut result = None;

        for (offset, rect) in self.faces() {
            let transform = *transform * Affine3A::from_translation(offset.into());
            if let Some(manifold) = rect.hit(object_ref, &transform, ray, clip, scene) {
                if manifold.t < t {
                    t = manifold.t;
                    result = Some((offset, rect));
                }
            }
        }

        result.and_then(|(offset, rect)| {
            let transform = *transform * Affine3A::from_translation(offset.into());
            rect.pdf(object_ref, &transform, ray, clip, scene)
        })
    }

    pub fn hit<'a>(
        &self,
        object_ref: ObjectRef,
        transform: &Affine3A,
        ray: &Ray,
        clip: &Clip,
        scene: &'a Scene,
    ) -> Option<Manifold<'a>> {
        let mut t = clip.max;
        let mut result = None;

        for (offset, rect) in self.faces() {
            let transform = *transform * Affine3A::from_translation(offset.into());
            if let Some(manifold) = rect.hit(object_ref, &transform, ray, clip, scene) {
                if manifold.t < t {
                    t = manifold.t;
                    result = Some(manifold);
                }
            }
        }

        result
    }
}
