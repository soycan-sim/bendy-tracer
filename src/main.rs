use std::error::Error;
use std::fs::File;
use std::io::BufWriter;

use clap::Parser;

use bendy_tracer::scene::{Camera, Data, Material, Object, Scene, Sphere, Update, UpdateQueue};
use bendy_tracer::tracer::{Config, Tracer};
use glam::{Affine3A, Vec3, Vec3A};

#[derive(Parser)]
struct Cli {}

fn main() -> Result<(), Box<dyn Error>> {
    let mut scene = Scene::default();

    let mat_red = scene.add_data(Data::new(Material::flat(Vec3A::new(1.0, 0.0, 0.0))));

    let camera = scene.add_object(Object::new(Camera::default()));
    let sphere = scene.add_object(
        Object::new(Sphere::new(mat_red, 1.0)).with_translation(Vec3A::new(0.0, 0.0, -8.0)),
    );

    let tracer = Tracer::with_config(Config {
        threads: 8,
        samples: 16,
        ..Default::default()
    });

    let mut update_queue = UpdateQueue::new();

    for i in 0..4 {
        let image = tracer.render(&scene, camera).into_rgba8();

        let file = File::create(format!("render-{i}.png"))?;
        let mut file = BufWriter::new(file);
        image.write_to(&mut file, image::ImageOutputFormat::Png)?;

        update_queue.push(Update::object(sphere, |sphere, update_queue| {
            sphere.apply_transform(update_queue, Affine3A::from_translation(Vec3::X));
        }));

        update_queue.commit(&mut scene);
    }

    Ok(())
}
