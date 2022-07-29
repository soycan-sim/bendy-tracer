use std::borrow::Cow;
use std::fmt::{self, Display, Write as _};
use std::fs;
use std::io::{self, Write as _};
use std::path::PathBuf;
use std::time::{Duration, Instant};

use anyhow::{ensure, Error};
use bendy_tracer::color::LinearRgb;
use bendy_tracer::material::{Material, Materials};
use bendy_tracer::scene::{Camera, Cuboid, Object, ObjectFlags, Rect, Scene};
use bendy_tracer::tracer::{Buffer, ColorSpace, Config, RenderConfig, Status, Subsample, Tracer};
use clap::{Parser, ValueEnum};
use glam::{Affine3A, EulerRot, Quat, Vec3, Vec3A};
use minifb::{Key, KeyRepeat, Window, WindowOptions};

const DEFAULT_SCREENSHOT: &str = "render.png";

#[derive(Debug, Default, Clone, Copy, ValueEnum)]
enum Output {
    #[default]
    Full,
    Albedo,
    Normal,
}

impl Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Full => write!(f, "full"),
            Self::Albedo => write!(f, "albedo"),
            Self::Normal => write!(f, "normal"),
        }
    }
}

impl Output {
    fn into_output(self) -> bendy_tracer::tracer::Output {
        match self {
            Self::Full => bendy_tracer::tracer::Output::Full,
            Self::Albedo => bendy_tracer::tracer::Output::Albedo,
            Self::Normal => bendy_tracer::tracer::Output::Normal,
        }
    }

    fn color_space(self) -> ColorSpace {
        match self {
            Self::Full => ColorSpace::SRgb,
            Self::Albedo => ColorSpace::SRgb,
            Self::Normal => ColorSpace::Normal,
        }
    }
}

#[derive(Debug, Parser)]
#[clap(author, version, about)]
struct Cli {
    #[clap(long, value_parser, default_value_t = 768)]
    width: usize,

    #[clap(long, value_parser, default_value_t = 512)]
    height: usize,

    #[clap(long, value_parser, default_value_t = Default::default())]
    output: Output,

    #[clap(long, value_parser, default_value_t = 64)]
    samples: usize,

    #[clap(long, value_parser, default_value_t = 2)]
    subsample: usize,

    #[clap(long, value_parser, default_value_os_t = PathBuf::from("screenshots/render.png"))]
    screenshot: PathBuf,

    #[clap(long, value_parser, default_value_os_t = PathBuf::from("scene.json"))]
    scene: PathBuf,
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

    let (mut scene, materials) = if args.scene.exists() {
        let path = &args.scene;
        let scene = bendy_tracer::load(path)?;

        writeln!(io::stderr(), "loaded scene from {}", path.display())?;

        scene
    } else {
        let mut scene = Scene::new();
        let mut materials = Materials::new();

        let mat_light = materials.add(Material::emissive(LinearRgb::WHITE, 20.0));
        let mat_white = materials.add(Material::diffuse(LinearRgb::splat(0.73)));
        let mat_metal = materials.add(Material::metallic(LinearRgb::splat(0.55), 0.01));
        let mat_red = materials.add(Material::diffuse(LinearRgb::new(0.7, 0.1, 0.1)));
        let mat_green = materials.add(Material::diffuse(LinearRgb::new(0.2, 0.7, 0.4)));

        scene.add(
            Object::new(Camera {
                focal_length: 0.05,
                fstop: 1.4,
                focus: Some(12.5),
                ..Default::default()
            })
            .with_tag("camera".to_string())
            .with_translation(Vec3A::new(0.0, 2.5, 10.0)),
        );
        // left
        scene.add(
            Object::new(Rect::new(
                mat_green,
                Vec3A::new(0.0, 0.0, -2.5),
                Vec3A::new(0.0, 2.5, 0.0),
            ))
            .with_translation(Vec3A::new(-2.5, 2.5, -2.5)),
        );
        // right
        scene.add(
            Object::new(Rect::new(
                mat_red,
                Vec3A::new(0.0, 0.0, 2.5),
                Vec3A::new(0.0, 2.5, 0.0),
            ))
            .with_translation(Vec3A::new(2.5, 2.5, -2.5)),
        );
        // back
        scene.add(
            Object::new(Rect::new(
                mat_white,
                Vec3A::new(2.5, 0.0, 0.0),
                Vec3A::new(0.0, 2.5, 0.0),
            ))
            .with_translation(Vec3A::new(0.0, 2.5, -5.0)),
        );
        // floor
        scene.add(
            Object::new(Rect::new(
                mat_white,
                Vec3A::new(2.5, 0.0, 0.0),
                Vec3A::new(0.0, 0.0, -2.5),
            ))
            .with_translation(Vec3A::new(0.0, 0.0, -2.5)),
        );
        // ceiling
        scene.add(
            Object::new(Rect::new(
                mat_white,
                Vec3A::new(2.5, 0.0, 0.0),
                Vec3A::new(0.0, 0.0, 2.5),
            ))
            .with_translation(Vec3A::new(0.0, 5.0, -2.5)),
        );
        scene.add(
            Object::new(Rect::new(
                mat_light,
                Vec3A::new(0.5, 0.0, 0.0),
                Vec3A::new(0.0, 0.0, 0.5),
            ))
            .with_translation(Vec3A::new(0.0, 4.999, -2.5))
            .with_flags(ObjectFlags::LIGHT),
        );

        // tall box
        let angle = 20_f32.to_radians();
        scene.add(
            Object::new(Cuboid::new(
                mat_metal,
                Vec3A::new(0.5, 0.0, 0.0),
                Vec3A::new(0.0, 1.0, 0.0),
                Vec3A::new(0.0, 0.0, 0.4),
            ))
            .with_transform(Affine3A::from_rotation_translation(
                Quat::from_euler(EulerRot::YXZ, angle, 0.0, 0.0),
                Vec3::new(-1.2, 1.0, -3.2),
            )),
        );

        // short box
        scene.add(
            Object::new(Cuboid::new(
                mat_white,
                Vec3A::new(0.5, 0.0, 0.0),
                Vec3A::new(0.0, 0.6, 0.0),
                Vec3A::new(0.0, 0.0, 0.5),
            ))
            .with_translation(Vec3A::new(1.0, 0.6, -1.4)),
        );

        (scene, materials)
    };

    let camera = scene.find_by_tag_mut("camera").unwrap();

    let aspect_ratio = window_width as f32 / window_height as f32;
    camera.as_camera_mut().unwrap().aspect_ratio = aspect_ratio;

    let mut tracer = Tracer::with_config(
        materials,
        Config {
            output: args.output.into_output(),
            chunks_x: 8,
            chunks_y: 4,
            ..Default::default()
        },
    );

    let mut bvh = scene.build_bvh();

    let mut buffer = Buffer::new(window_width, window_height, args.output.color_space());
    let max_samples = args.samples;
    let subsample = match args.subsample {
        0 | 1 => Subsample::None,
        n => Subsample::Subpixel(n),
    };
    let mut last_samples = 0;
    let mut sum_delta = Duration::ZERO;

    let mut start = None;
    let mut end = None;
    let mut prev_frame;

    while window.is_open() {
        prev_frame = Instant::now();

        let camera = scene.find_by_tag("camera").unwrap();

        let samples = if buffer.samples() < max_samples { 1 } else { 0 };
        let status = tracer.render(
            &bvh,
            camera,
            &RenderConfig::with_samples_subsample(samples, subsample),
            &mut buffer,
        );

        // delta time of the render, not the entire loop
        let this_frame = Instant::now();
        let delta = this_frame - prev_frame;
        sum_delta += delta;

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
            let path = &args.scene;

            bendy_tracer::save(path, &scene, &tracer.materials)?;

            writeln!(io::stderr(), "saved scene to {}", path.display())?;
        }
        if window.is_key_down(Key::LeftCtrl) && window.is_key_pressed(Key::L, KeyRepeat::No) {
            let path = &args.scene;
            let (mut new_scene, materials) = bendy_tracer::load(path)?;

            let new_camera = new_scene.find_by_tag_mut("camera").unwrap();

            let aspect_ratio = window_width as f32 / window_height as f32;
            new_camera.as_camera_mut().unwrap().aspect_ratio = aspect_ratio;

            writeln!(io::stderr(), "loaded scene from {}", path.display())?;

            scene = new_scene;
            bvh = scene.build_bvh();
            tracer.materials = materials;

            buffer.clear();
        }

        let window_size = window.get_size();
        if window_size != (window_width, window_height) {
            window_width = window_size.0;
            window_height = window_size.1;
            buffer.resize(window_width, window_height);
            window_buffer.resize(window_width * window_height, 0);

            let camera = scene.find_by_tag_mut("camera").unwrap();

            let aspect_ratio = window_width as f32 / window_height as f32;
            camera.as_camera_mut().unwrap().aspect_ratio = aspect_ratio;
        }

        let total_samples = buffer.samples();
        let samples = total_samples - last_samples;
        last_samples = total_samples;

        let delta = if samples != 0 {
            Some(delta / (samples as u32))
        } else {
            None
        };

        let mut title = format!("bendy tracer; samples: {total_samples}/{max_samples}");
        if let Some(delta) = delta {
            let seconds = delta.as_secs();
            let millis = delta.as_millis() % 1_000;
            if seconds == 0 {
                write!(&mut title, "; delta t: {millis}ms")?;
            } else {
                write!(&mut title, "; delta t: {seconds}s {millis}ms")?;
            }
        } else {
            let avg_delta = sum_delta / (total_samples as u32);
            let seconds = avg_delta.as_secs();
            let millis = avg_delta.as_millis() % 1_000;
            if seconds == 0 {
                write!(&mut title, "; avg t per sample: {millis}ms")?;
            } else {
                write!(&mut title, "; avg t per sample: {seconds}s {millis}ms")?;
            }
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
