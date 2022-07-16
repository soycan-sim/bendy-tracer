use std::borrow::Cow;
use std::fmt::Write as _;
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Write as _};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use anyhow::{ensure, Error};
use bendy_tracer::color::LinearRgb;
use bendy_tracer::scene::{Camera, Data, Material, Object, Scene, Sphere, Update, UpdateQueue};
use bendy_tracer::tracer::{Buffer, Status, Tracer};
use clap::Parser;
use glam::{Affine3A, Quat, Vec3, Vec3A};
use minifb::{Key, KeyRepeat, Window, WindowOptions};

const SAMPLES_STEP: usize = 8;
const SAMPLES_BIG_STEP: usize = 64;
const SAMPLES_VERY_BIG_STEP: usize = 1024;

const DEFAULT_SCENE: &str = "scene.json";
const DEFAULT_SCREENSHOT: &str = "render.png";

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(long, value_parser, default_value_t = 768)]
    width: usize,

    #[clap(long, value_parser, default_value_t = 512)]
    height: usize,

    #[clap(long, value_parser, default_value_t = 64)]
    max_samples: usize,

    #[clap(long, value_parser, default_value_os_t = PathBuf::from("screenshots/render.png"))]
    screenshot: PathBuf,

    #[clap(long, value_parser)]
    scene: Option<PathBuf>,
}

fn main() -> Result<(), Error> {
    let args = Cli::parse();

    let mut window_width = args.width;
    let mut window_height = args.height;
    let mut window = Window::new(
        "bendy tracer",
        window_width,
        window_height,
        WindowOptions {
            resize: true,
            ..Default::default()
        },
    )?;
    // limit to 120fps
    window.limit_update_rate(Some(Duration::from_micros(8333)));

    let mut window_buffer = vec![0_u32; window_width * window_height];

    let mut scene = if let Some(path) = &args.scene {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let scene = serde_json::from_reader(&mut reader)?;

        writeln!(io::stderr(), "loaded scene from {}", path.display())?;

        scene
    } else {
        let mut scene = Scene::default();

        let mat_root = scene.add_data(Data::new(Material::emissive(LinearRgb::WHITE, 0.1)));
        let mat_light = scene.add_data(Data::new(Material::emissive(LinearRgb::WHITE, 10.0)));
        let mat_red = scene.add_data(Data::new(Material::diffuse(
            LinearRgb::new(0.7, 0.1, 0.1),
            0.5,
        )));
        let mat_blue = scene.add_data(Data::new(Material::diffuse(
            LinearRgb::new(0.2, 0.2, 0.5),
            0.8,
        )));

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

    let mut camera = scene.find_by_tag("camera").unwrap();

    let mut update_queue = UpdateQueue::new();
    update_queue.push(Update::object(camera, move |object, _| {
        let aspect_ratio = window_width as f32 / window_height as f32;
        object.as_camera_mut().unwrap().aspect_ratio = aspect_ratio;
    }));
    update_queue.commit(&mut scene);

    let tracer = Tracer::new();

    let mut buffer = Buffer::new(window_width, window_height);
    let mut max_samples = args.max_samples;

    let mut start = None;
    let mut end = None;
    let mut prev_frame;

    while window.is_open() {
        prev_frame = Instant::now();

        let samples = if buffer.samples() < max_samples { 1 } else { 0 };
        let status = tracer.render(&scene, camera, samples, &mut buffer);

        // delta time of the render, not the entire loop
        let this_frame = Instant::now();
        let delta = this_frame - prev_frame;

        if status == Status::InProgress {
            let preview = buffer.preview();

            for (target, source) in window_buffer.iter_mut().zip(preview.pixels()) {
                *target = u32::from_be_bytes([0, source.0[0], source.0[1], source.0[2]]);
            }

            if start.is_none() || end.is_some() {
                start = Some(prev_frame);
                end = None;
            }
        } else if end.is_none() {
            end = Some(this_frame);
        }
        if window.is_key_pressed(Key::Q, KeyRepeat::No) {
            let step = if window.is_key_down(Key::LeftShift) {
                SAMPLES_VERY_BIG_STEP
            } else if window.is_key_down(Key::LeftCtrl) {
                SAMPLES_BIG_STEP
            } else {
                SAMPLES_STEP
            };
            max_samples = max_samples.saturating_sub(step).max(1);
        }
        if window.is_key_pressed(Key::E, KeyRepeat::No) {
            let step = if window.is_key_down(Key::LeftShift) {
                SAMPLES_VERY_BIG_STEP
            } else if window.is_key_down(Key::LeftCtrl) {
                SAMPLES_BIG_STEP
            } else {
                SAMPLES_STEP
            };
            max_samples = if max_samples == 1 {
                step
            } else {
                max_samples + step
            };
        }
        if window.is_key_down(Key::LeftCtrl) && window.is_key_pressed(Key::P, KeyRepeat::No) {
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
        if window.is_key_down(Key::LeftCtrl) && window.is_key_pressed(Key::K, KeyRepeat::No) {
            let path = args
                .scene
                .as_deref()
                .unwrap_or_else(|| Path::new(DEFAULT_SCENE));

            let file = File::create(path)?;
            let mut writer = BufWriter::new(file);
            serde_json::to_writer_pretty(&mut writer, &scene)?;

            writeln!(io::stderr(), "saved scene to {}", path.display())?;
        }
        if window.is_key_down(Key::LeftCtrl) && window.is_key_pressed(Key::L, KeyRepeat::No) {
            let path = args
                .scene
                .as_deref()
                .unwrap_or_else(|| Path::new(DEFAULT_SCENE));

            let file = File::open(path)?;
            let mut reader = BufReader::new(file);
            scene = serde_json::from_reader(&mut reader)?;
            buffer.clear();

            camera = scene.find_by_tag("camera").unwrap();

            update_queue.push(Update::object(camera, move |object, _| {
                let aspect_ratio = window_width as f32 / window_height as f32;
                object.as_camera_mut().unwrap().aspect_ratio = aspect_ratio;
            }));

            writeln!(io::stderr(), "loaded scene from {}", path.display())?;
        }

        let window_size = window.get_size();
        if window_size != (window_width, window_height) {
            window_width = window_size.0;
            window_height = window_size.1;
            buffer.resize(window_width, window_height);
            window_buffer.resize(window_width * window_height, 0);

            update_queue.push(Update::object(camera, move |object, _| {
                let aspect_ratio = window_width as f32 / window_height as f32;
                object.as_camera_mut().unwrap().aspect_ratio = aspect_ratio;
            }));
        }

        update_queue.commit(&mut scene);

        let samples = buffer.samples();
        let seconds = delta.as_secs();
        let millis = delta.as_millis() % 1_000;
        let mut title = format!("bendy tracer; samples: {samples}/{max_samples}");
        if seconds == 0 {
            write!(&mut title, "; delta t: {millis}ms")?;
        } else {
            write!(&mut title, "; delta t: {seconds}s {millis}ms")?;
        }
        if let (Some(start), Some(end)) = (start, end) {
            let total = end - start;
            let seconds = total.as_secs();
            let millis = total.as_millis() % 1_000;
            write!(&mut title, "; total t: {seconds}s {millis}ms")?;
        }

        window.set_title(&title);

        window.update_with_buffer(&window_buffer, window_width, window_height)?;
    }

    Ok(())
}
