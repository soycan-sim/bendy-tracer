use glam::Vec3A;
use image::{DynamicImage, ImageBuffer, Rgba, Rgba32FImage};
use rand::{Rng, SeedableRng};
use rayon::iter::{IntoParallelIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::scene::{ObjectRef, Scene};

mod ray;

pub use self::ray::{Clip, Manifold, Ray};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Config {
    pub width: u32,
    pub height: u32,
    pub samples: usize,
    pub max_bounces: usize,
    pub clip_min: f32,
    pub clip_max: f32,
    pub threads: usize,
}

impl Config {
    const DEFAULT: Self = Self {
        width: 768,
        height: 512,
        samples: 32,
        max_bounces: 8,
        clip_min: 0.1,
        clip_max: 1000.0,
        threads: 8,
    };
}

impl Default for Config {
    fn default() -> Self {
        Self::DEFAULT
    }
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

    pub fn render(&self, scene: &Scene, camera: ObjectRef) -> DynamicImage {
        let buffer_width = self.config.width as usize;
        let buffer_height = self.config.height as usize;
        let buffer_size = buffer_width * buffer_height;

        let mut buffer = vec![0.0_f32; 4 * buffer_size];

        self.render_to_buffer(&mut buffer, scene, camera);

        Rgba32FImage::from_vec(buffer_width as _, buffer_height as _, buffer)
            .unwrap()
            .into()
    }

    pub fn render_to_buffer<'buf>(
        &self,
        buffer: &'buf mut [f32],
        scene: &Scene,
        camera: ObjectRef,
    ) -> ImageBuffer<Rgba<f32>, &'buf mut [f32]> {
        let buffer_width = self.config.width as usize;
        let buffer_height = self.config.height as usize;

        let chunk_width = buffer_width;
        let chunk_height = if buffer_height % self.config.threads == 0 {
            buffer_height / self.config.threads
        } else {
            buffer_height / self.config.threads + 1
        };
        let chunk_size = chunk_width * chunk_height;

        let (chunk_x, mut chunk_y) = (0_usize, 0_usize);

        let chunks = buffer
            .chunks_mut(4 * chunk_size)
            .map(|chunk| {
                let x = chunk_x as u32;
                let y = chunk_y as u32;
                let w = chunk_width as u32;
                let h = chunk_height.min(buffer_height - chunk_y) as u32;
                chunk_y += chunk_height;
                (chunk, (x, y), (w, h))
            })
            .collect::<Vec<_>>();

        chunks
            .into_par_iter()
            .for_each(|(chunk, offset, dimensions)| {
                let image =
                    ImageBuffer::<Rgba<f32>, _>::from_raw(dimensions.0, dimensions.1, chunk)
                        .unwrap();

                let dimensions = (buffer_width as _, buffer_height as _);
                self.render_to_chunk(image, offset, dimensions, scene, camera)
            });

        ImageBuffer::<Rgba<f32>, _>::from_raw(buffer_width as _, buffer_height as _, buffer)
            .unwrap()
    }

    fn render_to_chunk(
        &self,
        image: ImageBuffer<Rgba<f32>, &mut [f32]>,
        offset: (u32, u32),
        dimensions: (u32, u32),
        scene: &Scene,
        camera: ObjectRef,
    ) {
        let mut image = image;

        let camera_obj = scene.get_object(camera);
        let camera = camera_obj.as_camera().expect("expected a camera object");

        let yfov = 2.0 * camera.sensor_size.atan2(2.0 * camera.focal_length);
        let xfov = yfov * camera.aspect_ratio;

        let mut rng = rand::rngs::SmallRng::from_entropy();

        let samples_recip = (self.config.samples as f32).recip();
        let pixel_width = 2.0 * (dimensions.0 as f32).recip();
        let pixel_height = 2.0 * (dimensions.1 as f32).recip();

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

        let scatter = (0..self.config.samples)
            .map(|_| {
                let u = rng.sample(scatter_u);
                let v = rng.sample(scatter_v);
                (u, v)
            })
            .collect::<Vec<_>>();

        for y in 0..image.height() {
            let v = (y + offset.1) as f32 * pixel_height - 1.0;

            for x in 0..image.width() {
                let u = (x + offset.0) as f32 * pixel_width - 1.0;

                let mut color = Vec3A::ZERO;

                for &(u_offset, v_offset) in &scatter {
                    let u = u + u_offset;
                    let v = v + v_offset;

                    let ray = camera_obj.transform() * Ray::with_frustum(yfov, xfov, u, v);
                    let sample = self.sample(&ray, scene, 0);

                    color += sample;
                }

                let color = (color * samples_recip).extend(1.0);
                let pixel = Rgba(color.into());
                image.put_pixel(x, y, pixel);
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
