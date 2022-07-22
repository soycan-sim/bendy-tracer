use glam::{IVec3, Vec3A};
use rand::prelude::*;
use rand::Rng;
use rand_distr::Standard;
use serde::{Deserialize, Serialize};

use crate::color::LinearRgb;
use crate::math::{Interpolate, UnitSphere};
use crate::tracer::ColorData;
use crate::tracer::Face;
use crate::tracer::{Manifold, Ray};

#[derive(Debug, Default, Clone, Copy)]
#[non_exhaustive]
pub enum SamplingMode {
    #[default]
    Nearest,
    Trilinear,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Volume {
    DensityMap(DensityMap),
}

impl Volume {
    pub fn shade<R: Rng + ?Sized>(
        &self,
        rng: &mut R,
        manifold: &Manifold,
        step: f32,
    ) -> (Option<Ray>, Option<ColorData>) {
        let offset = manifold.bbox.0;
        let size = manifold.bbox.1 - manifold.bbox.0;
        let coord = (manifold.position - offset) / size;

        let density = step * self.sample(coord, SamplingMode::Trilinear);

        if density >= 1.0 || rng.gen_bool(density as _) {
            let mut origin = manifold.position;
            if manifold.face == Face::Volume {
                origin -= manifold.ray.direction * step * rng.sample::<f32, _>(Standard);
            }
            let direction = UnitSphere.sample(rng);
            let ray = Ray::new(origin, direction);

            let color_data = ColorData {
                color: LinearRgb::splat(0.8),
                albedo: LinearRgb::splat(0.8),
                normal: manifold.normal,
                depth: manifold.t,
            };

            (Some(ray), Some(color_data))
        } else {
            let origin = manifold.position;
            let direction = manifold.ray.direction;
            let ray = Ray::new(origin, direction);
            (Some(ray), None)
        }
    }

    pub fn sample(&self, coord: Vec3A, mode: SamplingMode) -> f32 {
        match self {
            Volume::DensityMap(density_map) => density_map.sample(coord, mode),
        }
    }
}

impl From<DensityMap> for Volume {
    fn from(density_map: DensityMap) -> Self {
        Self::DensityMap(density_map)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DensityMap {
    width: usize,
    height: usize,
    depth: usize,
    size: Vec3A,
    buffer: Vec<f32>,
}

impl DensityMap {
    pub fn new(width: usize, height: usize, depth: usize, buffer: Vec<f32>) -> Self {
        let size = Vec3A::new(width as f32 - 1.0, height as f32 - 1.0, depth as f32 - 1.0);
        Self {
            width,
            height,
            depth,
            size,
            buffer,
        }
    }

    pub fn with_value(width: usize, height: usize, depth: usize, value: f32) -> Self {
        let size = width * height * depth;
        let buffer = vec![value; size];
        Self::new(width, height, depth, buffer)
    }

    pub fn with_func<F>(width: usize, height: usize, depth: usize, mut f: F) -> Self
    where
        F: FnMut(usize, usize, usize) -> f32,
    {
        let size = width * height * depth;
        let buffer = (0..size)
            .map(|i| {
                let x = i % width;
                let yz = i / width;
                let y = yz % height;
                let z = yz / height;
                f(x, y, z)
            })
            .collect();
        Self::new(width, height, depth, buffer)
    }

    pub fn index(&self, coord: IVec3) -> f32 {
        if self.width == 0 || self.height == 0 || self.depth == 0 {
            return 0.0;
        }

        let x = coord.x as usize;
        let y = coord.y as usize;
        let z = coord.z as usize;

        assert!(x < self.width, "volume index out of bounds");
        assert!(y < self.height, "volume index out of bounds");
        assert!(z < self.depth, "volume index out of bounds");

        let index = z * self.height * self.width + y * self.width + x;
        self.buffer[index]
    }

    fn sample_xyz(&self, x: f32, y: f32, z: f32) -> f32 {
        self.index(IVec3::new(x as _, y as _, z as _))
    }

    pub fn sample(&self, coord: Vec3A, mode: SamplingMode) -> f32 {
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
