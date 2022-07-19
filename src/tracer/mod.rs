use glam::{Vec3, Vec3A};
use rand::prelude::*;
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::color::LinearRgb;
use crate::math::UnitDisk;
use crate::scene::{DataRef, ObjectRef, Scene};

mod buffer;
mod ray;

pub use self::buffer::*;
pub use self::ray::*;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Config {
    pub max_bounces: usize,
    pub max_volume_bounces: usize,
    pub clip_min: f32,
    pub clip_max: f32,
    pub volume_step: f32,
    pub chunks_x: usize,
    pub chunks_y: usize,
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
    };
}

impl Default for Config {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Done,
    InProgress,
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

    pub fn render(
        &self,
        scene: &Scene,
        camera: ObjectRef,
        samples: usize,
        buffer: &mut Buffer,
    ) -> Status {
        if samples == 0 {
            return Status::Done;
        }

        let chunks = buffer
            .chunks(self.config.chunks_x, self.config.chunks_y)
            .collect::<Vec<_>>();

        chunks.into_par_iter().for_each(|chunk| {
            let mut chunk_state = ChunkState::new(self.config);
            chunk_state.render_samples(scene, camera, samples, chunk);
        });

        buffer.inc_samples(samples);

        Status::InProgress
    }
}

#[derive(Debug)]
pub struct ChunkState {
    pub config: Config,
    pub rng: SmallRng,
}

impl ChunkState {
    fn new(config: Config) -> Self {
        let rng = SmallRng::from_entropy();
        Self { config, rng }
    }

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
                let u = self.rng.sample(&scatter_u);
                let v = self.rng.sample(&scatter_v);
                (u, v)
            })
            .collect::<Vec<_>>();

        let defocus = (0..samples)
            .map(|_| self.rng.sample(&scatter_defocus))
            .collect::<Vec<Vec3A>>();

        let mut chunk = chunk;

        for y in chunk.range_y() {
            let v = y as f32 * pixel_height - 1.0;

            for x in chunk.range_x() {
                let u = x as f32 * pixel_width - 1.0;

                let mut sample = LinearRgb::BLACK;

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

                chunk.add_samples(x, y, sample);
            }
        }
    }

    fn sample(&mut self, ray: &Ray, scene: &Scene, bounce: usize) -> LinearRgb {
        if bounce > self.config.max_bounces {
            return LinearRgb::BLACK;
        }

        if let Some(manifold) = self.try_hit(ray, scene) {
            if manifold.face.is_surface() {
                match manifold.mat_ref {
                    Some(mat_ref) => self.sample_surface(scene, &manifold, mat_ref, bounce),
                    None => LinearRgb::BLACK,
                }
            } else {
                match manifold.vol_ref {
                    Some(vol_ref) => self.sample_volume(scene, &manifold, vol_ref, bounce, 0),
                    None => LinearRgb::BLACK,
                }
            }
        } else {
            self.sample_root(ray, scene)
        }
    }

    fn sample_volumetric(
        &mut self,
        ray: &Ray,
        scene: &Scene,
        last_object: ObjectRef,
        bounce: usize,
        volume_bounce: usize,
    ) -> LinearRgb {
        if volume_bounce > self.config.max_volume_bounces {
            return LinearRgb::BLACK;
        }

        if let Some(manifold) = self.try_hit_volume(ray, scene, last_object) {
            if manifold.face.is_surface() {
                match manifold.mat_ref {
                    Some(mat_ref) => self.sample_surface(scene, &manifold, mat_ref, bounce),
                    None => LinearRgb::BLACK,
                }
            } else {
                match manifold.vol_ref {
                    Some(vol_ref) => {
                        self.sample_volume(scene, &manifold, vol_ref, bounce, volume_bounce)
                    }
                    None => LinearRgb::BLACK,
                }
            }
        } else {
            self.sample_root(ray, scene)
        }
    }

    fn try_hit(&mut self, ray: &Ray, scene: &Scene) -> Option<Manifold> {
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

    fn try_hit_volume(
        &mut self,
        ray: &Ray,
        scene: &Scene,
        last_object: ObjectRef,
    ) -> Option<Manifold> {
        let mut result = None;

        let mut clip = Clip {
            min: 0.0,
            max: self.config.volume_step,
        };

        for (object_ref, object) in scene.pairs() {
            let manifold = if object_ref == last_object {
                object.hit_volumetric(ray, &clip)
            } else {
                object.hit(ray, &clip)
            };
            if let Some(manifold) = manifold {
                clip.max = manifold.t;
                result = Some(manifold);
            }
        }

        result
    }

    fn sample_root(&mut self, ray: &Ray, scene: &Scene) -> LinearRgb {
        let material = scene.root_material();

        let manifold = Manifold {
            position: ray.at(self.config.clip_max),
            normal: -ray.direction,
            bbox: (Vec3A::splat(f32::NEG_INFINITY), Vec3A::splat(f32::INFINITY)),
            face: Face::Volume,
            t: self.config.clip_max,
            ray: *ray,
            object_ref: None,
            mat_ref: None,
            vol_ref: None,
        };

        let (_, attenuation) = material.shade(&mut self.rng, &manifold);

        attenuation.unwrap_or_default()
    }

    fn sample_surface(
        &mut self,
        scene: &Scene,
        manifold: &Manifold,
        mat_ref: DataRef,
        bounce: usize,
    ) -> LinearRgb {
        let material = scene
            .get_data(mat_ref)
            .as_material()
            .expect("expected material data");

        let (ray, mut attenuation) = material.shade(&mut self.rng, manifold);

        if let Some(ray) = ray {
            let reflected = self.sample(&ray, scene, bounce + 1);
            if let Some(attenuation) = &mut attenuation {
                *attenuation *= reflected;
            } else {
                attenuation = Some(reflected);
            }
        }

        attenuation.unwrap_or_default()
    }

    fn sample_volume(
        &mut self,
        scene: &Scene,
        manifold: &Manifold,
        vol_ref: DataRef,
        bounce: usize,
        volume_bounce: usize,
    ) -> LinearRgb {
        let volume = scene
            .get_data(vol_ref)
            .as_volume()
            .expect("expected volume data");

        let (ray, mut attenuation) = volume.shade(&mut self.rng, manifold, self.config.volume_step);

        if let Some(ray) = ray {
            let reflected = if manifold.face == Face::VolumeBack {
                self.sample(&ray, scene, bounce + 1)
            } else {
                self.sample_volumetric(
                    &ray,
                    scene,
                    manifold.object_ref.unwrap(),
                    bounce,
                    volume_bounce + 1,
                )
            };
            if let Some(attenuation) = &mut attenuation {
                *attenuation *= reflected;
            } else {
                attenuation = Some(reflected);
            }
        }

        attenuation.unwrap_or_default()
    }
}
