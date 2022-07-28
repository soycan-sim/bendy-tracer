use glam::{Affine3A, Vec3A};
use rand::Rng;
use rand_distr::Uniform;

use crate::bvh::Aabb;
use crate::material::MaterialRef;
use crate::scene;
use crate::tracer::{Clip, Face, Manifold, Ray};

use super::ObjectData;

#[derive(Debug, Clone)]
pub struct Rect {
    pub material: MaterialRef,
    pub half_width: f32,
    pub half_height: f32,
    x: Vec3A,
    y: Vec3A,
    z: Vec3A,
}

impl Rect {
    pub fn new(material: MaterialRef, x: Vec3A, y: Vec3A) -> Self {
        debug_assert!(x.dot(y).abs() < 1e-5);
        debug_assert!(x.length_squared() > 0.0);
        debug_assert!(y.length_squared() > 0.0);

        let half_width = x.length();
        let half_height = y.length();
        let x = x / half_width;
        let y = y / half_height;
        let z = x.cross(y);
        Self {
            material,
            half_width,
            half_height,
            x,
            y,
            z,
        }
    }

    fn points(&self, transform: &Affine3A) -> impl Iterator<Item = Vec3A> {
        [
            transform.transform_point3a(self.x * self.half_width + self.y * self.half_height),
            transform.transform_point3a(self.x * self.half_width - self.y * self.half_height),
            transform.transform_point3a(-self.x * self.half_width + self.y * self.half_height),
            transform.transform_point3a(-self.x * self.half_width - self.y * self.half_height),
        ]
        .into_iter()
    }

    fn min_x(&self) -> f32 {
        -self.half_width
    }

    fn max_x(&self) -> f32 {
        self.half_width
    }

    fn min_y(&self) -> f32 {
        -self.half_height
    }

    fn max_y(&self) -> f32 {
        self.half_height
    }

    fn contains_point(&self, point: Vec3A) -> bool {
        let x = point.project_onto_normalized(self.x);
        let y = point.project_onto_normalized(self.y);
        let w_sqr = self.half_width * self.half_width;
        let h_sqr = self.half_height * self.half_height;
        x.length_squared() <= w_sqr && y.length_squared() <= h_sqr
    }

    pub fn area(&self) -> f32 {
        4.0 * self.half_width * self.half_height
    }

    pub fn bounding_box(&self, transform: &Affine3A) -> Aabb {
        let min = self
            .points(transform)
            .fold(Vec3A::splat(f32::INFINITY), Vec3A::min);
        let max = self
            .points(transform)
            .fold(Vec3A::splat(f32::NEG_INFINITY), Vec3A::max);
        // give the box some thickness
        let min = min - self.z.abs() * 1e-5;
        let max = max + self.z.abs() * 1e-5;
        Aabb::new(min, max)
    }

    pub fn random_point<R: Rng + ?Sized>(&self, rng: &mut R, transform: &Affine3A) -> Vec3A {
        let x = rng.sample(Uniform::new_inclusive(self.min_x(), self.max_x()));
        let y = rng.sample(Uniform::new_inclusive(self.min_y(), self.max_y()));
        transform.transform_point3a(self.x * x + self.y * y)
    }

    pub(crate) fn pdf_impl(&self, transform: &Affine3A, ray: &Ray, clip: &Clip) -> Option<f32> {
        if let Some(manifold) = self.hit_impl(transform, ray, clip) {
            let shadow = self.area() * ray.direction.dot(manifold.normal).abs();
            let dist_sqr = manifold.t * manifold.t;

            Some(dist_sqr / shadow)
        } else {
            None
        }
    }

    pub fn pdf(&self, object: &ObjectData, ray: &Ray, clip: &Clip) -> Option<f32> {
        self.pdf_impl(&object.transform, ray, clip)
    }

    pub(crate) fn hit_impl(
        &self,
        transform: &Affine3A,
        ray: &Ray,
        clip: &Clip,
    ) -> Option<Manifold> {
        let translation = transform.translation;
        let normal = transform.transform_vector3a(self.z);

        let q = ray.direction.dot(normal);
        if q.abs() <= 1e-5 {
            return None;
        }

        let p = (translation - ray.origin).dot(normal);
        let t = p / q;
        if t < clip.min || t > clip.max {
            return None;
        }

        let position = ray.at(t);

        if !self.contains_point(transform.inverse().transform_point3a(position)) {
            return None;
        }

        let (normal, face) = if p < 0.0 {
            (normal, Face::Front)
        } else {
            (-normal, Face::Back)
        };
        Some(Manifold {
            position,
            normal,
            aabb: self.bounding_box(transform),
            face,
            t,
            ray: *ray,
            material: self.material,
        })
    }

    pub fn hit(&self, object: &ObjectData, ray: &Ray, clip: &Clip) -> Option<Manifold> {
        self.hit_impl(&object.transform, ray, clip)
    }
}

impl<'a> From<&'a scene::Rect> for Rect {
    fn from(rect: &'a scene::Rect) -> Self {
        Self::new(rect.material, rect.x, rect.y)
    }
}
