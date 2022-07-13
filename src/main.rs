use std::error::Error;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use bendy_tracer::scene::{Camera, Data, Material, Object, Scene, Sphere, Update, UpdateQueue};
use bendy_tracer::tracer::{Config, Tracer};
use clap::Parser;
use glam::{Affine3A, Vec3, Vec3A};
use sdl2::event::Event;
use sdl2::keyboard::Keycode;
use sdl2::pixels::PixelFormatEnum;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(long, value_parser, default_value_t = 768)]
    width: u32,

    #[clap(long, value_parser, default_value_t = 512)]
    height: u32,

    #[clap(long, value_parser, default_value_os_t = PathBuf::from("render.png"))]
    path: PathBuf,
}

fn main() -> Result<(), Box<dyn Error>> {
    let args = Cli::parse();

    let sdl_context = sdl2::init().unwrap();
    let video_subsystem = sdl_context.video().unwrap();

    let window = video_subsystem
        .window("bendy tracer", args.width, args.height)
        .position_centered()
        .build()
        .unwrap();

    let mut canvas = window.into_canvas().build().unwrap();
    canvas.clear();
    canvas.present();

    let mut scene = Scene::default();

    let mat_red = scene.add_data(Data::new(Material::flat(Vec3A::new(1.0, 0.0, 0.0))));

    let camera = scene.add_object(Object::new(Camera::default()));
    let sphere = scene.add_object(
        Object::new(Sphere::new(mat_red, 1.0)).with_translation(Vec3A::new(0.0, 0.0, -8.0)),
    );

    let mut tracer = Tracer::with_config(Config {
        threads: 8,
        samples: 4,
        ..Default::default()
    });

    let mut update_queue = UpdateQueue::new();

    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, args.width, args.height)
        .unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let first_frame = Instant::now();
    let mut prev_frame = Instant::now();

    let mut speed = 1.0;
    let mut velocity;

    'main: loop {
        let image = tracer.render(&scene, camera).into_rgba8();

        texture
            .update(None, &image, 4 * image.width() as usize)
            .unwrap();

        canvas.copy(&texture, None, None).unwrap();

        // time of last render, not the entire loop
        let this_frame = Instant::now();
        let delta = this_frame - prev_frame;
        let time = this_frame - first_frame;

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,
                Event::KeyDown {
                    keycode: Some(Keycode::D),
                    repeat: false,
                    ..
                } => speed += 0.1,
                Event::KeyDown {
                    keycode: Some(Keycode::A),
                    repeat: false,
                    ..
                } => speed -= 0.1,
                Event::KeyDown {
                    keycode: Some(Keycode::Q),
                    ..
                } => tracer.config.samples = (tracer.config.samples - 1).max(1),
                Event::KeyDown {
                    keycode: Some(Keycode::E),
                    ..
                } => tracer.config.samples += 1,
                Event::KeyDown {
                    keycode: Some(Keycode::PrintScreen),
                    repeat: false,
                    ..
                } => image.save(&args.path)?,
                _ => {}
            }
        }

        velocity = time.as_secs_f32().cos();

        update_queue.push(Update::object(sphere, move |sphere, update_queue| {
            sphere.apply_transform(
                update_queue,
                Affine3A::from_translation(Vec3::X * velocity * speed * delta.as_secs_f32()),
            );
        }));

        update_queue.commit(&mut scene);

        let samples = tracer.config.samples;
        let seconds = delta.as_secs();
        let millis = delta.as_millis() % 1_000;
        let title = if seconds == 0 {
            format!("bendy tracer; samples: {samples}; delta t: {millis}ms")
        } else {
            format!("bendy tracer; samples: {samples}; delta t: {seconds}s {millis}ms")
        };
        canvas.window_mut().set_title(&title).unwrap();

        canvas.present();
        thread::sleep(Duration::new(0, 1_000_000));

        prev_frame = Instant::now();
    }

    Ok(())
}
