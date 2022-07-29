use std::ops::Mul;

use glam::{IVec3, Vec3A};
use rand::Rng;
use rand_distr::Standard;
use serde::{Deserialize, Serialize};

use crate::color::LinearRgb;
use crate::math::{distr::UnitSphere, Interpolate};
use crate::tracer::{ColorData, Face, Manifold, Ray};

use super::ShaderData;

#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub enum SamplingMode {
    #[default]
    Nearest,
    Trilinear,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Volume {
    VoxelMap(VoxelMap),
}

impl Volume {
    pub fn shade<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        manifold: &Manifold,
        step: f32,
    ) -> ShaderData {
        let coord = manifold.aabb.map_into(manifold.position);

        let voxel = self.sample(coord, SamplingMode::Trilinear) * step;

        if voxel.density >= 1.0 || rng.gen_bool(voxel.density as _) {
            let mut origin = manifold.position;
            if manifold.face == Face::Volume {
                origin -= manifold.ray.direction * step * rng.sample::<f32, _>(Standard);
            }
            let direction = rng.sample(UnitSphere);
            let ray = Ray::new(origin, direction);

            let color_data = ColorData {
                color: voxel.albedo,
                albedo: voxel.albedo,
                emitted: voxel.emissive,
                normal: manifold.normal,
                depth: manifold.t,
            };

            ShaderData {
                is_volume: true,
                scatter: Some(ray),
                color: Some(color_data),
                pdf: 1.0,
            }
        } else {
            let origin = manifold.position;
            let direction = manifold.ray.direction;
            let ray = Ray::new(origin, direction);

            ShaderData {
                is_volume: true,
                scatter: Some(ray),
                color: None,
                pdf: 1.0,
            }
        }
    }

    fn sample(&self, coord: Vec3A, mode: SamplingMode) -> Voxel {
        match self {
            Volume::VoxelMap(voxel_map) => voxel_map.sample(coord, mode).unwrap_or_default(),
        }
    }
}

impl From<VoxelMap> for Volume {
    fn from(voxel_map: VoxelMap) -> Self {
        Self::VoxelMap(voxel_map)
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub struct Voxel {
    pub density: f32,
    pub albedo: LinearRgb,
    pub emissive: LinearRgb,
}

impl Voxel {
    pub fn new(density: f32, albedo: LinearRgb) -> Self {
        Self {
            density,
            albedo,
            emissive: LinearRgb::BLACK,
        }
    }

    pub fn with_emissive(density: f32, albedo: LinearRgb, emissive: LinearRgb) -> Self {
        Self {
            density,
            albedo,
            emissive,
        }
    }
}

impl Interpolate for Voxel {
    fn lerp(self, other: Self, factor: f32) -> Self {
        Self::with_emissive(
            self.density.lerp(other.density, factor),
            self.albedo.lerp(other.albedo, factor),
            self.emissive.lerp(other.emissive, factor),
        )
    }
}

impl Mul<f32> for Voxel {
    type Output = Self;

    fn mul(self, rhs: f32) -> Self::Output {
        Self::with_emissive(self.density * rhs, self.albedo, self.emissive)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VoxelMap {
    width: usize,
    height: usize,
    depth: usize,
    size: Vec3A,
    buffer: Vec<Voxel>,
}

impl VoxelMap {
    pub fn new(width: usize, height: usize, depth: usize, buffer: Vec<Voxel>) -> Self {
        let size = Vec3A::new(width as f32 - 1.0, height as f32 - 1.0, depth as f32 - 1.0);
        Self {
            width,
            height,
            depth,
            size,
            buffer,
        }
    }

    pub fn with_voxel(width: usize, height: usize, depth: usize, voxel: Voxel) -> Self {
        let size = width * height * depth;
        let buffer = vec![voxel; size];
        Self::new(width, height, depth, buffer)
    }

    pub fn with_func<F>(width: usize, height: usize, depth: usize, mut f: F) -> Self
    where
        F: FnMut(IVec3) -> Voxel,
    {
        let size = width * height * depth;
        let buffer = (0..size)
            .map(|i| {
                let x = i % width;
                let yz = i / width;
                let y = yz % height;
                let z = yz / height;
                f(IVec3::new(x as _, y as _, z as _))
            })
            .collect();
        Self::new(width, height, depth, buffer)
    }

    fn index(&self, coord: IVec3) -> Option<Voxel> {
        if self.width == 0 || self.height == 0 || self.depth == 0 {
            return None;
        }

        let x = coord.x as usize;
        let y = coord.y as usize;
        let z = coord.z as usize;

        assert!(x < self.width, "volume index out of bounds");
        assert!(y < self.height, "volume index out of bounds");
        assert!(z < self.depth, "volume index out of bounds");

        let index = z * self.height * self.width + y * self.width + x;
        Some(self.buffer[index])
    }

    fn sample_xyz(&self, x: f32, y: f32, z: f32) -> Option<Voxel> {
        self.index(IVec3::new(x as _, y as _, z as _))
    }

    pub fn sample(&self, coord: Vec3A, mode: SamplingMode) -> Option<Voxel> {
        let coord = coord.clamp(Vec3A::ZERO, Vec3A::ONE);
        let icoord = coord * self.size;
        match mode {
            SamplingMode::Nearest => {
                self.sample_xyz(icoord.x.round(), icoord.y.round(), icoord.z.round())
            }
            SamplingMode::Trilinear => {
                let x0 = self.sample_xyz(icoord.x.floor(), icoord.y.floor(), icoord.z.floor());
                let x1 = self.sample_xyz(icoord.x.ceil(), icoord.y.floor(), icoord.z.floor());
                let y0 = x0.lerp(x1, icoord.x.fract());
                let x0 = self.sample_xyz(icoord.x.floor(), icoord.y.ceil(), icoord.z.floor());
                let x1 = self.sample_xyz(icoord.x.ceil(), icoord.y.ceil(), icoord.z.floor());
                let y1 = x0.lerp(x1, icoord.x.fract());
                let z0 = y0.lerp(y1, icoord.y.fract());

                let x0 = self.sample_xyz(icoord.x.floor(), icoord.y.floor(), icoord.z.ceil());
                let x1 = self.sample_xyz(icoord.x.ceil(), icoord.y.floor(), icoord.z.ceil());
                let y0 = x0.lerp(x1, icoord.x.fract());
                let x0 = self.sample_xyz(icoord.x.floor(), icoord.y.ceil(), icoord.z.ceil());
                let x1 = self.sample_xyz(icoord.x.ceil(), icoord.y.ceil(), icoord.z.ceil());
                let y1 = x0.lerp(x1, icoord.x.fract());
                let z1 = y0.lerp(y1, icoord.y.fract());

                z0.lerp(z1, icoord.z.fract())
            }
        }
    }
}
