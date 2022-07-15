use std::borrow::Cow;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{ensure, Error};
use bendy_tracer::scene::{Camera, Data, Material, Object, Scene, Sphere};
use bendy_tracer::tracer::{Buffer, Status, Tracer};
use clap::Parser;
use glam::{Affine3A, Quat, Vec3, Vec3A};
use sdl2::event::Event;
use sdl2::keyboard;
use sdl2::pixels::PixelFormatEnum;

const SAMPLES_STEP: usize = 8;
const SAMPLES_BIG_STEP: usize = 64;
const SAMPLES_VERY_BIG_STEP: usize = 1024;

const DEFAULT_SCENE: &str = "scene.json";
const DEFAULT_SCREENSHOT: &str = "render.png";

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(long, value_parser, default_value_t = 768)]
    width: u32,

    #[clap(long, value_parser, default_value_t = 512)]
    height: u32,

    #[clap(long, value_parser, default_value_t = 64)]
    max_samples: usize,

    #[clap(long, value_parser, default_value_t = 2.0)]
    gamma: f32,

    #[clap(long, value_parser, default_value_os_t = PathBuf::from("screenshots/render.png"))]
    screenshot: PathBuf,

    #[clap(long, value_parser)]
    scene: Option<PathBuf>,
}

fn main() -> Result<(), Error> {
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

    let mut scene = if let Some(path) = &args.scene {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let scene = serde_json::from_reader(&mut reader)?;

        writeln!(io::stderr(), "loaded scene from {}", path.display())?;

        scene
    } else {
        let mut scene = Scene::default();

        let mat_root = scene.add_data(Data::new(Material::emissive(Vec3A::ONE, 0.1)));
        let mat_light = scene.add_data(Data::new(Material::emissive(Vec3A::ONE, 10.0)));
        let mat_red = scene.add_data(Data::new(Material::diffuse(Vec3A::new(0.7, 0.1, 0.1), 0.5)));
        let mat_blue = scene.add_data(Data::new(Material::diffuse(Vec3A::new(0.2, 0.2, 0.5), 0.8)));

        scene.set_root_material(mat_root);

        scene.add_object(
            Object::new(Camera::default())
                .with_tag("camera".to_string())
                .with_transform(Affine3A::from_rotation_translation(
                    Quat::from_euler(
                        glam::EulerRot::YXZ,
                        10_f32.to_radians(),
                        -5_f32.to_radians(),
                        0.0,
                    ),
                    Vec3::new(1.6, 2.1, 8.0),
                )),
        );
        scene.add_object(
            Object::new(Sphere::new(mat_blue, 100.0))
                .with_translation(Vec3A::new(0.0, -101.0, 0.0)),
        );
        scene.add_object(
            Object::new(Sphere::new(mat_red, 1.0)).with_translation(Vec3A::new(0.0, 0.0, 0.0)),
        );
        scene.add_object(
            Object::new(Sphere::new(mat_light, 0.5)).with_translation(Vec3A::new(3.0, 3.0, 2.0)),
        );

        scene
    };

    let camera = scene.find_by_tag("camera").unwrap();

    let tracer = Tracer::new();

    let mut buffer = Buffer::new(args.width, args.height, args.gamma, args.max_samples);

    let texture_creator = canvas.texture_creator();
    let mut texture = texture_creator
        .create_texture_streaming(PixelFormatEnum::RGBA32, args.width, args.height)
        .unwrap();

    let mut event_pump = sdl_context.event_pump().unwrap();
    let mut prev_frame = Instant::now();

    'main: loop {
        if tracer.render_samples(&scene, camera, 1, &mut buffer) == Status::Rendered {
            let preview = buffer.preview();
            let stride = preview.sample_layout().height_stride;

            texture.update(None, preview, stride).unwrap();

            canvas.copy(&texture, None, None).unwrap();
        }

        // delta time of the render, not the entire loop
        let this_frame = Instant::now();
        let delta = this_frame - prev_frame;

        for event in event_pump.poll_iter() {
            match event {
                Event::Quit { .. } => break 'main,
                Event::KeyDown {
                    keycode: Some(keyboard::Keycode::Q),
                    keymod,
                    ..
                } => {
                    let step = if keymod.contains(keyboard::Mod::LSHIFTMOD) {
                        SAMPLES_VERY_BIG_STEP
                    } else if keymod.contains(keyboard::Mod::LCTRLMOD) {
                        SAMPLES_BIG_STEP
                    } else {
                        SAMPLES_STEP
                    };
                    let max_samples = buffer.max_samples().saturating_sub(step);
                    buffer.set_max_samples(max_samples.max(1));
                    buffer.clear();
                }
                Event::KeyDown {
                    keycode: Some(keyboard::Keycode::E),
                    keymod,
                    ..
                } => {
                    let step = if keymod.contains(keyboard::Mod::LSHIFTMOD) {
                        SAMPLES_VERY_BIG_STEP
                    } else if keymod.contains(keyboard::Mod::LCTRLMOD) {
                        SAMPLES_BIG_STEP
                    } else {
                        SAMPLES_STEP
                    };
                    let max_samples = if buffer.max_samples() == 1 {
                        step
                    } else {
                        buffer.max_samples() + step
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
                    let path = &args.screenshot;
                    let path = if path.extension().is_none() {
                        Cow::from(path.with_file_name(DEFAULT_SCREENSHOT))
                    } else {
                        Cow::from(path)
                    };

                    if let Some(dir) = path.parent() {
                        if dir.exists() {
                            ensure!(
                                dir.is_dir(),
                                "{dir} is not a directory",
                                dir = dir.display(),
                            );
                        } else {
                            fs::create_dir_all(dir)?;
                        }
                    }

                    buffer.preview_or_update().save(&path)?;

                    writeln!(io::stderr(), "saved screenshot to {}", path.display())?;
                }
                Event::KeyDown {
                    keycode: Some(keyboard::Keycode::K),
                    repeat: false,
                    keymod,
                    ..
                } if keymod.contains(keyboard::Mod::LCTRLMOD) => {
                    let path = args
                        .scene
                        .as_deref()
                        .unwrap_or_else(|| Path::new(DEFAULT_SCENE));

                    let file = File::create(path)?;
                    let mut writer = BufWriter::new(file);
                    serde_json::to_writer_pretty(&mut writer, &scene)?;

                    writeln!(io::stderr(), "saved scene to {}", path.display())?;
                }
                Event::KeyDown {
                    keycode: Some(keyboard::Keycode::L),
                    repeat: false,
                    keymod,
                    ..
                } if keymod.contains(keyboard::Mod::LCTRLMOD) => {
                    let path = args
                        .scene
                        .as_deref()
                        .unwrap_or_else(|| Path::new(DEFAULT_SCENE));

                    let file = File::open(path)?;
                    let mut reader = BufReader::new(file);
                    scene = serde_json::from_reader(&mut reader)?;
                    buffer.clear();

                    writeln!(io::stderr(), "loaded scene from {}", path.display())?;
                }
                _ => {}
            }
        }

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
