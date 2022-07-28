use std::fs::File;
use std::io::{BufReader, BufWriter};
use std::path::Path;

use anyhow::Result;
use flate2::bufread::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;

use self::material::Materials;
use self::scene::Scene;

pub mod bvh;
pub mod color;
pub mod material;
pub mod math;
pub mod scene;
pub mod tracer;

pub fn load(path: impl AsRef<Path>) -> Result<(Scene, Materials)> {
    let path = path.as_ref();
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
    let scene = if path.extension() == Some("gz".as_ref()) {
        let mut decoder = GzDecoder::new(reader);
        serde_json::from_reader(&mut decoder)?
    } else {
        serde_json::from_reader(&mut reader)?
    };
    Ok(scene)
}

pub fn save(path: impl AsRef<Path>, scene: &Scene, materials: &Materials) -> Result<()> {
    let scene = (scene, materials);

    let path = path.as_ref();
    let file = File::create(path)?;
    let mut writer = BufWriter::new(file);

    if path.extension() == Some("gz".as_ref()) {
        let mut encoder = GzEncoder::new(writer, Compression::default());
        serde_json::to_writer(&mut encoder, &scene)?;
    } else {
        serde_json::to_writer_pretty(&mut writer, &scene)?;
    }

    Ok(())
}
