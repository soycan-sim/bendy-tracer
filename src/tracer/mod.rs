use glam::{Vec3, Vec3A};
use rand::prelude::*;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::math::UnitDisk;
use crate::scene::{ObjectRef, Scene};

mod buffer;
mod ray;

pub use self::buffer::*;
pub use self::ray::*;

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
            let mut chunk_state = ChunkState {
                config: self.config,
                rng: SmallRng::from_entropy(),
            };
            chunk_state.render_samples(scene, camera, samples, chunk);
        });

        buffer.inc_samples(samples);

        Status::Rendered
    }
}

#[derive(Debug)]
pub struct ChunkState {
    pub config: Config,
    pub rng: SmallRng,
}

impl ChunkState {
    fn render_samples<'a>(
        &mut self,
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

        let scatter_defocus = UnitDisk::new(Vec3::NEG_Z);

        let scatter = (0..samples)
            .map(|_| {
                let u = rng.sample(&scatter_u);
                let v = rng.sample(&scatter_v);
                (u, v)
            })
            .collect::<Vec<_>>();

        let defocus = (0..samples)
            .map(|_| rng.sample(&scatter_defocus))
            .collect::<Vec<Vec3A>>();

        let mut chunk = chunk;

        for y in chunk.range_y() {
            let v = y as f32 * pixel_height - 1.0;

            for x in chunk.range_x() {
                let u = x as f32 * pixel_width - 1.0;

                let mut sample = Vec3A::ZERO;

                for (&(u_offset, v_offset), &defocus) in scatter.iter().zip(&defocus) {
                    let u = u + u_offset;
                    let v = v + v_offset;

                    let mut ray = Ray::with_frustum(yfov, xfov, u, v);
                    if let Some(focus) = camera.focus {
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

                    sample += self.sample(&ray, scene, 0);
                }

                chunk.add_samples(x, y, sample.into());
            }
        }
    }

    fn sample(&mut self, ray: &Ray, scene: &Scene, bounce: usize) -> Vec3A {
        if bounce > self.config.max_bounces {
            return Vec3A::ZERO;
        }

        if let Some(manifold) = self.sample_one(ray, scene) {
            let mat_ref = if let Some(mat_ref) = manifold.mat_ref {
                mat_ref
            } else {
                return Vec3A::ZERO;
            };

            let material = scene
                .get_data(mat_ref)
                .as_material()
                .expect("expected material data");

            let (ray, mut attenuation) = material.shade(&mut self.rng, &manifold);

            if let Some(ray) = ray {
                let reflected = self.sample(&ray, scene, bounce + 1);
                attenuation *= reflected;
            }

            attenuation
        } else {
            let material = scene.root_material();

            let manifold = Manifold {
                position: ray.at(self.config.clip_max),
                normal: -ray.direction,
                face: Face::Back,
                t: self.config.clip_max,
                ray: *ray,
                object_ref: None,
                mat_ref: None,
            };

            let (_, attenuation) = material.shade(&mut self.rng, &manifold);

            attenuation
        }
    }

    fn sample_one(&mut self, ray: &Ray, scene: &Scene) -> Option<Manifold> {
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
