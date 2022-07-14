use glam::Vec3A;
use rand::{Rng, SeedableRng};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::scene::{ObjectRef, Scene};

mod buffer;
mod ray;

pub use self::buffer::{Buffer, Chunk, Chunks};
pub use self::ray::{Clip, Manifold, Ray};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Config {
    pub max_bounces: usize,
    pub clip_min: f32,
    pub clip_max: f32,
    pub chunks_x: u32,
    pub chunks_y: u32,
}

impl Config {
    const DEFAULT: Self = Self {
        max_bounces: 8,
        clip_min: 0.1,
        clip_max: 1000.0,
        chunks_x: 4,
        chunks_y: 2,
    };
}

impl Default for Config {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    BufferFull,
    Rendered,
}

#[derive(Debug, Default)]
pub struct Tracer {
    pub config: Config,
}

impl Tracer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_config(config: Config) -> Self {
        Self { config }
    }

    pub fn render(&self, scene: &Scene, camera: ObjectRef, buffer: &mut Buffer) -> Status {
        self.render_samples(scene, camera, buffer.max_samples(), buffer)
    }

    pub fn render_samples(
        &self,
        scene: &Scene,
        camera: ObjectRef,
        samples: usize,
        buffer: &mut Buffer,
    ) -> Status {
        let samples = samples.min(buffer.max_samples() - buffer.samples());
        if samples == 0 {
            return Status::BufferFull;
        }

        let chunks = buffer
            .chunks(self.config.chunks_x, self.config.chunks_y)
            .collect::<Vec<_>>();

        chunks.into_par_iter().for_each(|chunk| {
            self.render_samples_to_chunk(scene, camera, samples, chunk);
        });

        buffer.inc_samples(samples);

        Status::Rendered
    }

    fn render_samples_to_chunk<'a>(
        &self,
        scene: &Scene,
        camera: ObjectRef,
        samples: usize,
        chunk: Chunk<'a>,
    ) {
        let camera_obj = scene.get_object(camera);
        let camera = camera_obj.as_camera().expect("expected a camera object");

        let yfov = 2.0 * camera.sensor_size.atan2(2.0 * camera.focal_length);
        let xfov = yfov * camera.aspect_ratio;

        let mut rng = rand::rngs::SmallRng::from_entropy();

        let pixel_width = chunk.pixel_width();
        let pixel_height = chunk.pixel_height();

        let scatter_u = {
            let min = -0.5 * pixel_width;
            let max = 0.5 * pixel_width;
            rand::distributions::Uniform::from(min..max)
        };

        let scatter_v = {
            let min = -0.5 * pixel_height;
            let max = 0.5 * pixel_height;
            rand::distributions::Uniform::from(min..max)
        };

        let scatter = (0..samples)
            .map(|_| {
                let u = rng.sample(scatter_u);
                let v = rng.sample(scatter_v);
                (u, v)
            })
            .collect::<Vec<_>>();

        let mut chunk = chunk;

        for y in chunk.range_y() {
            let v = y as f32 * pixel_height - 1.0;

            for x in chunk.range_x() {
                let u = x as f32 * pixel_width - 1.0;

                let mut sample = Vec3A::ZERO;

                for &(u_offset, v_offset) in &scatter {
                    let u = u + u_offset;
                    let v = v + v_offset;

                    let ray = camera_obj.transform() * Ray::with_frustum(yfov, xfov, u, v);

                    sample += self.sample(&ray, scene, 0);
                }

                chunk.add_samples(x, y, sample.into());
            }
        }
    }

    fn sample(&self, ray: &Ray, scene: &Scene, bounce: usize) -> Vec3A {
        if bounce > self.config.max_bounces {
            return Vec3A::ZERO;
        }

        if let Some(manifold) = self.sample_one(ray, scene) {
            let material = scene
                .get_data(manifold.mat_ref)
                .as_material()
                .expect("expected material data");

            return material.albedo;
        }

        Vec3A::ZERO
    }

    fn sample_one(&self, ray: &Ray, scene: &Scene) -> Option<Manifold> {
        let mut result = None;

        let mut clip = Clip {
            min: self.config.clip_min,
            max: self.config.clip_max,
        };

        for object in scene.iter() {
            if let Some(manifold) = object.hit(ray, &clip) {
                clip.max = manifold.t;
                result = Some(manifold);
            }
        }

        result
    }
}
