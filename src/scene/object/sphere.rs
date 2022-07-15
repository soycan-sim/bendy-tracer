use glam::Vec3A;
use serde::{Deserialize, Serialize};

use crate::scene::{DataRef, ObjectRef};
use crate::tracer::{Clip, Face, Manifold, Ray};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Sphere {
    pub material: DataRef,
    pub radius: f32,
}

impl Sphere {
    pub fn new(material: DataRef, radius: f32) -> Self {
        Self { material, radius }
    }

    pub fn hit(
        &self,
        object_ref: ObjectRef,
        translation: Vec3A,
        ray: &Ray,
        clip: &Clip,
    ) -> Option<Manifold> {
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

        let position = ray.at(t);
        let normal = (position - translation) / self.radius;
        let (normal, face) = if ray.direction.dot(normal) < 0.0 {
            (normal, Face::Front)
        } else {
            (-normal, Face::Back)
        };
        Some(Manifold {
            position,
            normal,
            face,
            t,
            ray: *ray,
            object_ref: Some(object_ref),
            mat_ref: Some(self.material),
        })
    }
}
