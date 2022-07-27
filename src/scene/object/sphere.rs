use std::f32;

use glam::Vec3A;
use rand::Rng;
use serde::{Deserialize, Serialize};

use crate::math::distr::UnitSphere;
use crate::scene::{DataRef, ObjectRef, Scene};
use crate::tracer::{Clip, Face, Manifold, Ray};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sphere {
    pub material: DataRef,
    pub volume: Option<DataRef>,
    pub radius: f32,
}

impl Sphere {
    pub fn new(material: DataRef, radius: f32) -> Self {
        Self {
            material,
            volume: None,
            radius,
        }
    }

    pub fn new_volumetric(material: DataRef, volume: DataRef, radius: f32) -> Self {
        Self {
            material,
            volume: Some(volume),
            radius,
        }
    }

    pub fn bounding_box(&self, translation: Vec3A) -> (Vec3A, Vec3A) {
        let half_size = Vec3A::splat(self.radius);
        (translation - half_size, translation + half_size)
    }

    pub fn random_point<R: Rng + ?Sized>(&self, rng: &mut R, translation: Vec3A) -> Vec3A {
        translation + rng.sample::<Vec3A, _>(UnitSphere) * self.radius
    }

    pub fn pdf(
        &self,
        object_ref: ObjectRef,
        translation: Vec3A,
        ray: &Ray,
        clip: &Clip,
        scene: &Scene,
    ) -> Option<f32> {
        if let Some(manifold) = self.hit(object_ref, translation, ray, clip, scene) {
            let r = self.radius;
            let shadow = f32::consts::PI * r * r;
            let dist_sqr = manifold.t * manifold.t;

            Some(dist_sqr / shadow)
        } else {
            None
        }
    }

    fn generate_volume_manifold<'a>(
        &self,
        scene: &'a Scene,
        object_ref: ObjectRef,
        translation: Vec3A,
        ray: Ray,
        t: f32,
    ) -> Manifold<'a> {
        Manifold {
            position: ray.at(t),
            normal: Vec3A::ZERO,
            bbox: self.bounding_box(translation),
            face: Face::Volume,
            t,
            ray,
            object_ref: Some(object_ref),
            mat_ref: Some(self.material),
            vol_ref: self.volume,
            scene,
        }
    }

    fn generate_surface_manifold<'a>(
        &self,
        scene: &'a Scene,
        object_ref: ObjectRef,
        translation: Vec3A,
        ray: Ray,
        t: f32,
    ) -> Manifold<'a> {
        let (front_face, back_face) = if self.volume.is_some() {
            (Face::VolumeFront, Face::VolumeBack)
        } else {
            (Face::Front, Face::Back)
        };

        let position = ray.at(t);
        let normal = (position - translation) / self.radius;

        let (normal, face) = if ray.direction.dot(normal) < 0.0 {
            (normal, front_face)
        } else {
            (-normal, back_face)
        };
        Manifold {
            position,
            normal,
            bbox: self.bounding_box(translation),
            face,
            t,
            ray,
            object_ref: Some(object_ref),
            mat_ref: Some(self.material),
            vol_ref: self.volume,
            scene,
        }
    }

    pub fn hit<'a>(
        &self,
        object_ref: ObjectRef,
        translation: Vec3A,
        ray: &Ray,
        clip: &Clip,
        scene: &'a Scene,
    ) -> Option<Manifold<'a>> {
        let oc = ray.origin - translation;
        let half_b = oc.dot(ray.direction);
        let c = oc.length_squared() - self.radius * self.radius;

        let discriminant = half_b * half_b - c;
        if discriminant.is_sign_negative() {
            return None;
        }

        let sqrtd = discriminant.sqrt();
        let mut t = -half_b - sqrtd;
        if t < clip.min || t > clip.max {
            t = -half_b + sqrtd;
            if t < clip.min || t > clip.max {
                return None;
            }
        }

        Some(self.generate_surface_manifold(scene, object_ref, translation, *ray, t))
    }

    pub fn hit_volumetric<'a>(
        &self,
        object_ref: ObjectRef,
        translation: Vec3A,
        ray: &Ray,
        clip: &Clip,
        scene: &'a Scene,
    ) -> Option<Manifold<'a>> {
        let t = clip.max;
        let dist_sqr = ray.at(t).distance_squared(translation);
        let r_sqr = self.radius * self.radius;
        if dist_sqr <= r_sqr {
            return Some(self.generate_volume_manifold(scene, object_ref, translation, *ray, t));
        }

        self.hit(object_ref, translation, ray, clip, scene)
    }
}
