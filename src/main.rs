use std::error::Error;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

use bendy_tracer::scene::{Camera, Data, Material, Object, Scene, Sphere, Update, UpdateQueue};
use bendy_tracer::tracer::{Buffer, Status, Tracer};
use clap::Parser;
use glam::{Affine3A, Vec3, Vec3A};
use sdl2::event::Event;
use sdl2::keyboard;
use sdl2::pixels::PixelFormatEnum;

const SAMPLES_STEP: usize = 8;

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(long, value_parser, default_value_t = 768)]
    width: u32,

    #[clap(long, value_parser, default_value_t = 512)]
    height: u32,

    #[clap(long, value_parser, default_value_t = 32)]
    max_samples: usize,

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

    let mut buffer = Buffer::new(args.width, args.height, args.max_samples);

    let tracer = Tracer::new();

    let mut update_queue = UpdateQueue::new();

    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, args.width, args.height)
        .unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let first_frame = Instant::now();
    let mut prev_frame = Instant::now();

    let mut speed = 0.0;
    let mut velocity;

    'main: loop {
        if tracer.render_samples(&scene, camera, 1, &mut buffer) == Status::Rendered {
            let preview = buffer.preview();
            let stride = preview.sample_layout().height_stride;

            texture.update(None, preview, stride).unwrap();

            canvas.copy(&texture, None, None).unwrap();
        }

        // time of last render, not the entire loop
        let this_frame = Instant::now();
        let delta = this_frame - prev_frame;
        let time = this_frame - first_frame;

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,
                Event::KeyDown {
                    keycode: Some(keyboard::Keycode::D),
                    repeat: false,
                    ..
                } => speed += 0.1,
                Event::KeyDown {
                    keycode: Some(keyboard::Keycode::A),
                    repeat: false,
                    ..
                } => speed -= 0.1,
                Event::KeyDown {
                    keycode: Some(keyboard::Keycode::Q),
                    ..
                } => {
                    let max_samples = buffer.max_samples().saturating_sub(SAMPLES_STEP);
                    buffer.set_max_samples(max_samples.max(1));
                    buffer.clear();
                }
                Event::KeyDown {
                    keycode: Some(keyboard::Keycode::E),
                    ..
                } => {
                    let max_samples = if buffer.max_samples() == 1 {
                        SAMPLES_STEP
                    } else {
                        buffer.max_samples() + SAMPLES_STEP
                    };
                    buffer.set_max_samples(max_samples);
                    buffer.clear();
                }
                Event::KeyDown {
                    keycode: Some(keyboard::Keycode::P),
                    repeat: false,
                    keymod,
                    ..
                } if keymod.contains(keyboard::Mod::LCTRLMOD) => {
                    buffer.preview_or_update().save(&args.path)?
                }
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

        let samples = buffer.samples();
        let max_samples = buffer.max_samples();
        let seconds = delta.as_secs();
        let millis = delta.as_millis() % 1_000;
        let title = if seconds == 0 {
            format!("bendy tracer; samples: {samples}/{max_samples}; delta t: {millis}ms")
        } else {
            format!(
                "bendy tracer; samples: {samples}/{max_samples}; delta t: {seconds}s {millis}ms"
            )
        };
        canvas.window_mut().set_title(&title).unwrap();

        canvas.present();
        thread::sleep(Duration::new(0, 1_000_000));

        prev_frame = Instant::now();
    }

    Ok(())
}
