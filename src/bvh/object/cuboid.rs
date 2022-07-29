use std::{f32, mem};

use glam::{Affine3A, BVec3A, Quat, Vec3, Vec3A};
use rand::distributions::{Standard, WeightedIndex};
use rand::Rng;

use crate::bvh::Aabb;
use crate::material::MaterialRef;
use crate::scene;
use crate::tracer::{Clip, Face, Manifold, Ray};

use super::ObjectData;

fn area(points: &[Vec3A; 4]) -> f32 {
    let a = points[0].distance(points[1]);
    let b = points[0].distance(points[2]);
    a * b
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    PosX,
    PosY,
    PosZ,
    NegX,
    NegY,
    NegZ,
}

impl Axis {
    pub fn from_mask(mask: BVec3A) -> Self {
        let mask = mask.bitmask();
        if mask & 0x1 != 0 {
            Axis::PosX
        } else if mask & 0x2 != 0 {
            Axis::PosY
        } else {
            Axis::PosZ
        }
    }

    pub fn as_vec(&self) -> Vec3A {
        match self {
            Axis::PosX => Vec3A::X,
            Axis::PosY => Vec3A::Y,
            Axis::PosZ => Vec3A::Z,
            Axis::NegX => Vec3A::NEG_X,
            Axis::NegY => Vec3A::NEG_Y,
            Axis::NegZ => Vec3A::NEG_Z,
        }
    }

    pub fn flip_with(&mut self, mask: BVec3A) {
        let mask = mask.bitmask();
        match *self {
            Axis::PosX if mask & 0x1 != 0 => *self = Axis::NegX,
            Axis::PosY if mask & 0x2 != 0 => *self = Axis::NegY,
            Axis::PosZ if mask & 0x4 != 0 => *self = Axis::NegZ,
            Axis::NegX if mask & 0x1 != 0 => *self = Axis::PosX,
            Axis::NegY if mask & 0x2 != 0 => *self = Axis::PosY,
            Axis::NegZ if mask & 0x4 != 0 => *self = Axis::PosZ,
            _ => {}
        }
    }
}

#[derive(Debug, Clone)]
pub struct Cuboid {
    material: MaterialRef,
    x: Vec3A,
    y: Vec3A,
    z: Vec3A,
    rot: Quat,
}

impl Cuboid {
    pub fn new(material: MaterialRef, x: Vec3A, y: Vec3A, z: Vec3A) -> Self {
        debug_assert!(x.dot(y).abs() < 1e-5);
        debug_assert!(x.dot(z).abs() < 1e-5);
        debug_assert!(x.length_squared() > 0.0);
        debug_assert!(y.length_squared() > 0.0);
        debug_assert!(z.length_squared() > 0.0);

        let rot = Quat::from_rotation_arc(Vec3::X, x.normalize().into());

        Self {
            material,
            x,
            y,
            z,
            rot,
        }
    }

    fn min(&self) -> Vec3A {
        -self.x - self.y - self.z
    }

    fn max(&self) -> Vec3A {
        self.x + self.y + self.z
    }

    fn half_size(&self) -> Vec3A {
        self.max().abs()
    }

    fn axes(&self, transform: &Affine3A) -> [Vec3A; 3] {
        let x = transform.transform_point3a(self.rot * self.x);
        let y = transform.transform_point3a(self.rot * self.y);
        let z = transform.transform_point3a(self.rot * self.z);
        [x, y, z]
    }

    fn points(&self, transform: &Affine3A) -> impl Iterator<Item = Vec3A> {
        let [x, y, z] = self.axes(transform);
        [
            x + y + z,
            x + y - z,
            x - y + z,
            x - y - z,
            -x + y + z,
            -x + y - z,
            -x - y + z,
            -x - y - z,
        ]
        .into_iter()
    }

    fn faces(&self, transform: &Affine3A) -> [[Vec3A; 4]; 6] {
        let [x, y, z] = self.axes(transform);
        [
            [x + y + z, x + y - z, x - y + z, x - y - z],
            [-x + y + z, -x + y - z, -x - y + z, -x - y - z],
            [z + x + y, z + x - y, z - x + y, z - x - y],
            [-z + x + y, -z + x - y, -z - x + y, -z - x - y],
            [y + z + x, y + z - x, y - z + x, y - z - x],
            [-y + z + x, -y + z - x, -y - z + x, -y - z - x],
        ]
    }

    fn face(&self, transform: &Affine3A, axis: Axis) -> [Vec3A; 4] {
        let [x, y, z] = self.axes(transform);
        match axis {
            Axis::PosX => [x + y + z, x + y - z, x - y + z, x - y - z],
            Axis::PosY => [y + z + x, y + z - x, y - z + x, y - z - x],
            Axis::PosZ => [z + x + y, z + x - y, z - x + y, z - x - y],
            Axis::NegX => [-x + y + z, -x + y - z, -x - y + z, -x - y - z],
            Axis::NegY => [-y + z + x, -y + z - x, -y - z + x, -y - z - x],
            Axis::NegZ => [-z + x + y, -z + x - y, -z - x + y, -z - x - y],
        }
    }

    pub fn bounding_box(&self, transform: &Affine3A) -> Aabb {
        let min = self
            .points(transform)
            .fold(Vec3A::splat(f32::INFINITY), Vec3A::min);
        let max = self
            .points(transform)
            .fold(Vec3A::splat(f32::NEG_INFINITY), Vec3A::max);
        Aabb::new(min, max)
    }

    pub fn random_point<R: Rng + ?Sized>(&self, rng: &mut R, transform: &Affine3A) -> Vec3A {
        let faces = self.faces(transform);
        let dist = WeightedIndex::new(faces.iter().map(area)).unwrap();
        let index = rng.sample(dist);
        let points = &faces[index];
        let o = points[0];
        let a = points[1] - points[0];
        let b = points[2] - points[0];
        let x = rng.sample::<f32, _>(Standard);
        let y = rng.sample::<f32, _>(Standard);
        o + x * a + y * b
    }

    pub fn pdf(&self, object: &ObjectData, ray: &Ray, clip: &Clip) -> Option<f32> {
        if let Some((manifold, axis)) = self.hit_impl(object, ray, clip) {
            let face = self.face(&object.transform, axis);
            let shadow = area(&face) * ray.direction.dot(manifold.normal).abs();
            let dist_sqr = manifold.t * manifold.t;

            Some(dist_sqr / shadow)
        } else {
            None
        }
    }

    fn hit_impl(&self, object: &ObjectData, ray: &Ray, clip: &Clip) -> Option<(Manifold, Axis)> {
        let transform_inv = object.transform.inverse();
        let origin = transform_inv.transform_point3a(ray.origin);
        let direction = transform_inv.transform_vector3a(ray.direction).normalize();

        let min = self.min().to_array();
        let max = self.max().to_array();
        let origin = origin.to_array();
        let direction = direction.to_array();

        let mut t_min = clip.min;
        let mut t_max = clip.max;

        for i in 0..3 {
            let d_recip = direction[i].recip();
            let mut t0 = (min[i] - origin[i]) * d_recip;
            let mut t1 = (max[i] - origin[i]) * d_recip;
            if d_recip.is_sign_negative() {
                mem::swap(&mut t0, &mut t1);
            }
            t_min = t0.max(t_min);
            t_max = t1.min(t_max);
            if t_max <= t_min {
                return None;
            }
        }

        let t = t_min;

        // first find the normal axis
        let p = Vec3A::from(origin) + Vec3A::from(direction) * t;
        let axes = (p.abs() - self.half_size()).abs().cmple(Vec3A::splat(1e-5));
        let mut axis = Axis::from_mask(axes);
        // then correct sign
        axis.flip_with(p.cmple(Vec3A::ZERO));
        let normal = axis.as_vec();

        let position = ray.at(t);
        let normal = object
            .transform
            .transform_vector3a(self.rot * normal)
            .normalize();

        let (normal, face) = if ray.direction.dot(normal) < 0.0 {
            (normal, Face::Front)
        } else {
            (-normal, Face::Back)
        };

        Some((
            Manifold {
                position,
                normal,
                aabb: self.bounding_box(&object.transform),
                face,
                t,
                ray: *ray,
                material: self.material,
            },
            axis,
        ))
    }

    pub fn hit(&self, object: &ObjectData, ray: &Ray, clip: &Clip) -> Option<Manifold> {
        self.hit_impl(object, ray, clip)
            .map(|(manifold, _)| manifold)
    }
}

impl<'a> From<&'a scene::Cuboid> for Cuboid {
    fn from(cuboid: &'a scene::Cuboid) -> Self {
        Self::new(cuboid.material, cuboid.x, cuboid.y, cuboid.z)
    }
}
