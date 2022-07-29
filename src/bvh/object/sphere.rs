use std::f32;

use glam::{Affine3A, Vec3A};
use rand::Rng;

use crate::bvh::Aabb;
use crate::material::MaterialRef;
use crate::math::distr::UnitSphere;
use crate::scene;
use crate::tracer::{Clip, Face, Manifold, Ray};

use super::ObjectData;

#[derive(Debug, Clone)]
pub struct Sphere {
    pub material: MaterialRef,
    pub radius: f32,
}

impl Sphere {
    pub fn new(material: MaterialRef, radius: f32) -> Self {
        Self { material, radius }
    }

    pub fn bounding_box(&self, transform: &Affine3A) -> Aabb {
        let half_size = Vec3A::splat(self.radius);
        Aabb::new(
            transform.transform_point3a(-half_size),
            transform.transform_point3a(half_size),
        )
    }

    pub fn random_point<R: Rng + ?Sized>(&self, rng: &mut R, transform: &Affine3A) -> Vec3A {
        transform.transform_point3a(rng.sample::<Vec3A, _>(UnitSphere) * self.radius)
    }

    pub fn pdf(&self, object: &ObjectData, ray: &Ray, clip: &Clip) -> Option<f32> {
        if let Some(manifold) = self.hit(object, ray, clip) {
            let r = self.radius;
            let shadow = f32::consts::PI * r * r;
            let dist_sqr = manifold.t * manifold.t;

            Some(dist_sqr / shadow)
        } else {
            None
        }
    }

    pub fn hit(&self, object: &ObjectData, ray: &Ray, clip: &Clip) -> Option<Manifold> {
        let transform_inv = object.transform.inverse();
        let origin = transform_inv.transform_point3a(ray.origin);
        let direction = transform_inv.transform_vector3a(ray.direction).normalize();

        let oc = origin;
        let half_b = oc.dot(direction);
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

        let position = origin + direction * t;
        let normal = position / self.radius;

        let position = ray.at(t);
        let normal = object.transform.transform_vector3a(normal).normalize();

        let (normal, face) = if ray.direction.dot(normal) < 0.0 {
            (normal, Face::Front)
        } else {
            (-normal, Face::Back)
        };

        Some(Manifold {
            position,
            normal,
            aabb: self.bounding_box(&object.transform),
            face,
            t,
            ray: *ray,
            material: self.material,
        })
    }
}

impl<'a> From<&'a scene::Sphere> for Sphere {
    fn from(sphere: &'a scene::Sphere) -> Self {
        Self::new(sphere.material, sphere.radius)
    }
}
