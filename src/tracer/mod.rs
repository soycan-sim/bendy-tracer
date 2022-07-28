use glam::{Vec3, Vec3A};
use rand::prelude::*;
use rand_distr::Uniform;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::bvh::Aabb;
use crate::bvh::Bvh;
use crate::material::MaterialRef;
use crate::material::Materials;
use crate::math::distr::UnitDisk;
use crate::scene::Object;

mod buffer;
mod ray;

pub use self::buffer::*;
pub use self::ray::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub max_bounces: usize,
    pub max_volume_bounces: usize,
    pub clip_min: f32,
    pub clip_max: f32,
    pub volume_step: f32,
    pub chunks_x: usize,
    pub chunks_y: usize,
    pub output: Output,
}

impl Config {
    const DEFAULT: Self = Self {
        max_bounces: 8,
        max_volume_bounces: 32,
        clip_min: 0.01,
        clip_max: 1000.0,
        volume_step: 0.1,
        chunks_x: 4,
        chunks_y: 2,
        output: Output::Full,
    };
}

impl Default for Config {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub enum Subsample {
    #[default]
    None,
    Subpixel(usize),
}

impl Subsample {
    pub fn subpixel_size(&self) -> f32 {
        match *self {
            Subsample::None => 1.0,
            Subsample::Subpixel(count) => (count as f32).recip(),
        }
    }

    pub fn subpixel_count(&self) -> usize {
        match *self {
            Subsample::None => 1,
            Subsample::Subpixel(count) => count * count,
        }
    }
}

impl IntoIterator for Subsample {
    type Item = <SubsampleIntoIter as Iterator>::Item;
    type IntoIter = SubsampleIntoIter;

    fn into_iter(self) -> Self::IntoIter {
        SubsampleIntoIter {
            subsample: self,
            count: 0,
        }
    }
}

pub struct SubsampleIntoIter {
    subsample: Subsample,
    count: usize,
}

impl Iterator for SubsampleIntoIter {
    type Item = (f32, f32);

    fn next(&mut self) -> Option<Self::Item> {
        match self.subsample {
            Subsample::None if self.count == 0 => {
                self.count += 1;
                Some((0.0, 0.0))
            }
            Subsample::Subpixel(count) if self.count < count * count => {
                let width = (count as f32).recip();
                let i = self.count % count;
                let j = self.count / count;
                self.count += 1;
                Some((i as f32 * width, j as f32 * width))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub enum Output {
    #[default]
    Full,
    Albedo,
    Normal,
    Depth,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderConfig {
    pub subsample: Subsample,
    pub samples: usize,
    pub output: Option<Output>,
    pub max_bounces: Option<usize>,
    pub max_volume_bounces: Option<usize>,
    pub volume_step: Option<f32>,
}

impl RenderConfig {
    const DEFAULT: Self = Self {
        subsample: Subsample::None,
        samples: 64,
        output: None,
        max_bounces: None,
        max_volume_bounces: None,
        volume_step: None,
    };

    pub fn with_samples(samples: usize) -> Self {
        Self {
            samples,
            ..Default::default()
        }
    }

    pub fn with_samples_subsample(samples: usize, subsample: Subsample) -> Self {
        Self {
            samples,
            subsample,
            ..Default::default()
        }
    }
}

impl Default for RenderConfig {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Done,
    InProgress,
}

#[derive(Debug)]
pub struct Tracer {
    pub config: Config,
    pub materials: Materials,
}

impl Tracer {
    pub fn new(materials: Materials) -> Self {
        Self {
            config: Default::default(),
            materials,
        }
    }

    pub fn with_config(materials: Materials, config: Config) -> Self {
        Self { config, materials }
    }

    pub fn render(
        &self,
        bvh: &Bvh,
        camera: &Object,
        config: &RenderConfig,
        buffer: &mut Buffer,
    ) -> Status {
        if config.samples == 0 {
            return Status::Done;
        }

        let chunks = buffer
            .chunks(self.config.chunks_x, self.config.chunks_y)
            .collect::<Vec<_>>();

        chunks.into_par_iter().for_each(|chunk| {
            let mut chunk_state = ChunkState::new(
                ChunkConfig::with_configs(&self.config, config),
                &self.materials,
            );
            chunk_state.render_samples(bvh, camera, chunk);
        });

        buffer.inc_samples(config.samples * config.subsample.subpixel_count());

        Status::InProgress
    }
}

#[derive(Debug, Clone)]
struct ChunkConfig {
    pub output: Output,
    pub subsample: Subsample,
    pub samples: usize,
    pub max_bounces: usize,
    pub max_volume_bounces: usize,
    pub clip_min: f32,
    pub clip_max: f32,
    pub volume_step: f32,
}

impl ChunkConfig {
    fn with_configs(main: &Config, render: &RenderConfig) -> Self {
        Self {
            output: render.output.unwrap_or(main.output),
            subsample: render.subsample,
            samples: render.samples,
            max_bounces: render.max_bounces.unwrap_or(main.max_bounces),
            max_volume_bounces: render.max_bounces.unwrap_or(main.max_volume_bounces),
            clip_min: main.clip_min,
            clip_max: main.clip_max,
            volume_step: render.volume_step.unwrap_or(main.volume_step),
        }
    }
}

#[derive(Debug)]
pub struct ChunkState<'mat> {
    config: ChunkConfig,
    materials: &'mat Materials,
    rng: SmallRng,
}

impl<'mat> ChunkState<'mat> {
    fn new(config: ChunkConfig, materials: &'mat Materials) -> Self {
        let rng = SmallRng::from_entropy();
        Self {
            config,
            materials,
            rng,
        }
    }

    fn render_samples<'a>(&mut self, bvh: &Bvh, camera: &Object, chunk: Chunk<'a>) {
        let camera_obj = camera;
        let camera = camera_obj.as_camera().expect("expected a camera object");

        let yfov = 2.0 * camera.sensor_size.atan2(2.0 * camera.focal_length);
        let xfov = yfov * camera.aspect_ratio;

        let pixel_width = chunk.pixel_width();
        let pixel_height = chunk.pixel_height();
        let subpixel_scale = self.config.subsample.subpixel_size();

        let scatter_u = {
            let min = -0.5 * pixel_width * subpixel_scale;
            let max = 0.5 * pixel_width * subpixel_scale;
            Uniform::from(min..max)
        };

        let scatter_v = {
            let min = -0.5 * pixel_height * subpixel_scale;
            let max = 0.5 * pixel_height * subpixel_scale;
            Uniform::from(min..max)
        };

        let scatter_defocus = UnitDisk::new(Vec3::NEG_Z);

        let mut chunk = chunk;

        for y in chunk.range_y() {
            let v = y as f32 * pixel_height - 1.0;

            for x in chunk.range_x() {
                let u = x as f32 * pixel_width - 1.0;

                for _ in 0..self.config.samples {
                    for (u_sub, v_sub) in self.config.subsample {
                        let u_offset = u_sub * pixel_width + self.rng.sample(&scatter_u);
                        let v_offset = v_sub * pixel_height + self.rng.sample(&scatter_v);

                        let u = u + u_offset;
                        let v = v + v_offset;

                        let mut ray = Ray::with_frustum(yfov, xfov, u, v);
                        if let Some(focus) = camera.focus {
                            let defocus = self.rng.sample::<Vec3A, _>(&scatter_defocus);

                            let aperture = 0.5 * camera.focal_length / camera.fstop;
                            let defocus_offset = camera_obj
                                .transform()
                                .transform_vector3a(defocus * aperture);

                            let frac_f_z = focus / ray.direction.z.abs();

                            ray = camera_obj.transform() * ray;

                            ray.origin += defocus_offset;
                            ray.direction = (ray.direction * frac_f_z - defocus_offset).normalize();
                        } else {
                            ray = camera_obj.transform() * ray;
                        }

                        let sample = self.sample(&ray, bvh, 0);

                        let depth = (sample.depth - self.config.clip_min)
                            / (self.config.clip_max - self.config.clip_min);
                        let depth = depth.clamp(0.0, 1.0);

                        match self.config.output {
                            Output::Full => chunk.write_color(x, y, sample.color),
                            Output::Albedo => chunk.write_color(x, y, sample.albedo),
                            Output::Normal => chunk.write_normal(x, y, sample.normal),
                            Output::Depth => chunk.write_depth(x, y, depth),
                        }
                    }
                }
            }
        }
    }

    fn sample(&mut self, ray: &Ray, bvh: &Bvh, bounce: usize) -> ColorData {
        if bounce > self.config.max_bounces {
            return Default::default();
        }

        if let Some(manifold) = bvh.hit(ray, &self.clip()) {
            let material = self.materials.get(manifold.material);

            let clip = self.clip();
            let data = material.shade(
                &mut self.rng,
                &manifold,
                &clip,
                self.config.volume_step,
                bvh,
            );

            if let Some(ray) = data.scatter {
                let reflected = self.sample(&ray, bvh, bounce + 1);
                let mut attenuation = if let Some(mut attenuation) = data.color {
                    attenuation.color *= material.pdf(&manifold, &ray);
                    attenuation.color *= reflected.color / data.pdf;
                    attenuation
                } else {
                    reflected
                };

                attenuation.color += attenuation.emitted;
                attenuation
            } else {
                let mut attenuation = data.color.unwrap_or_default();
                attenuation.color += attenuation.emitted;
                attenuation
            }
        } else {
            self.sample_root(ray, bvh)
        }
    }

    fn clip(&self) -> Clip {
        Clip {
            min: self.config.clip_min,
            max: self.config.clip_max,
        }
    }

    fn clip_volumetric(&self) -> Clip {
        Clip {
            min: 0.0,
            max: self.config.volume_step,
        }
    }

    fn sample_root(&mut self, ray: &Ray, bvh: &Bvh) -> ColorData {
        let material = self.materials.root();

        let manifold = Manifold {
            position: ray.at(self.config.clip_max),
            normal: -ray.direction,
            aabb: Aabb::new(Vec3A::splat(f32::NEG_INFINITY), Vec3A::splat(f32::INFINITY)),
            face: Face::Volume,
            t: self.config.clip_max,
            ray: *ray,
            material: MaterialRef::root(),
        };

        let clip = self.clip();
        let data = material.shade(
            &mut self.rng,
            &manifold,
            &clip,
            self.config.volume_step,
            bvh,
        );

        let mut color_data = data.color.unwrap_or_default();
        color_data.color += color_data.emitted;
        color_data
    }
}
