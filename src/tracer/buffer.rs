use std::mem;
use std::ops::{Deref, Range};

use glam::{Vec3, Vec3A};
use image::{Rgba, Rgba32FImage, RgbaImage};

use crate::color::{LinearRgb, Rgb};

const BLACK_ALPHA_ONE: Rgba<f32> = Rgba([0.0, 0.0, 0.0, 1.0]);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorSpace {
    None,
    Normal,
    Linear,
    SRgb,
}

impl ColorSpace {
    pub fn convert_linear(self, linear: LinearRgb) -> Rgb {
        match self {
            Self::None | Self::Linear => linear.into(),
            Self::Normal => {
                let normal = Vec3::from(Rgb::from(linear)).normalize();
                ((normal + Vec3::ONE) * 0.5).into()
            }
            Self::SRgb => linear.to_srgb().into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Buffer {
    samples: usize,
    buffer: Rgba32FImage,
    preview: Option<RgbaImage>,
    color_space: ColorSpace,
}

impl Buffer {
    pub fn new(width: usize, height: usize, color_space: ColorSpace) -> Self {
        let samples = 0;
        let buffer = Rgba32FImage::from_pixel(width as _, height as _, BLACK_ALPHA_ONE);
        Self {
            samples,
            buffer,
            preview: None,
            color_space,
        }
    }

    pub fn width(&self) -> usize {
        self.buffer.width() as _
    }

    pub fn height(&self) -> usize {
        self.buffer.height() as _
    }

    pub fn dimensions(&self) -> (usize, usize) {
        (self.width(), self.height())
    }

    pub fn samples(&self) -> usize {
        self.samples
    }

    pub fn pixel_width(&self) -> f32 {
        // width of a pixel in a viewport [-1.0; 1.0)
        2.0 * (self.width() as f32).recip()
    }

    pub fn pixel_height(&self) -> f32 {
        // height of a pixel in a viewport [-1.0; 1.0)
        2.0 * (self.height() as f32).recip()
    }

    pub fn into_buffer(self) -> Rgba32FImage {
        self.buffer
    }

    pub fn clear(&mut self) {
        self.buffer
            .pixels_mut()
            .for_each(|pixel| *pixel = BLACK_ALPHA_ONE);
        self.samples = 0;
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        let mut buffer = Rgba32FImage::new(0, 0);
        mem::swap(&mut buffer, &mut self.buffer);

        let mut buffer = buffer.into_raw();
        buffer.resize(4 * width * height, 0.0);

        self.buffer = Rgba32FImage::from_raw(width as _, height as _, buffer).unwrap();
        self.preview = None;

        self.clear();
    }

    pub fn chunks(&mut self, chunks_x: usize, chunks_y: usize) -> Chunks {
        let chunk_width = if self.width() % chunks_x == 0 {
            self.width() / chunks_x
        } else {
            self.width() / chunks_x + 1
        };
        let chunk_height = if self.height() % chunks_y == 0 {
            self.height() / chunks_y
        } else {
            self.height() / chunks_y + 1
        };

        Chunks::new(self, chunk_width, chunk_height)
    }

    pub fn preview(&mut self) -> &RgbaImage {
        let width = self.buffer.width();
        let height = self.buffer.height();
        let preview = self
            .preview
            .get_or_insert_with(|| RgbaImage::new(width, height));

        let samples_recip = (self.samples as f32).recip();

        for (target, source) in preview.pixels_mut().zip(self.buffer.pixels()) {
            let rgb = LinearRgb::from([source.0[0], source.0[1], source.0[2]]) * samples_recip;
            let converted = self.color_space.convert_linear(rgb);
            let alpha = source.0[3];

            let [r, g, b] = converted.to_bytes();
            let a = (alpha * u8::MAX as f32) as u8;

            *target = Rgba([r, g, b, a]);
        }

        preview
    }

    pub fn preview_or_update(&mut self) -> &RgbaImage {
        match self.preview {
            Some(ref preview) => preview,
            None => self.preview(),
        }
    }

    pub fn maybe_preview(&self) -> Option<&RgbaImage> {
        self.preview.as_ref()
    }

    pub fn take_preview(&mut self) -> Option<RgbaImage> {
        self.preview.take()
    }

    pub(super) fn inc_samples(&mut self, samples: usize) {
        self.samples += samples;
    }

    pub(super) fn write_color(&mut self, x: usize, y: usize, pixel: LinearRgb) {
        let Rgba([r, g, b, _]) = self.buffer.get_pixel_mut(x as _, y as _);
        *r += pixel.r;
        *g += pixel.g;
        *b += pixel.b;
    }

    pub(super) fn write_normal(&mut self, x: usize, y: usize, pixel: Vec3A) {
        let Rgba([r, g, b, _]) = self.buffer.get_pixel_mut(x as _, y as _);
        *r += pixel.x;
        *g += pixel.y;
        *b += pixel.z;
    }

    pub(super) fn write_depth(&mut self, x: usize, y: usize, pixel: f32) {
        let Rgba([r, g, b, _]) = self.buffer.get_pixel_mut(x as _, y as _);
        *r += pixel;
        *g += pixel;
        *b += pixel;
    }
}

impl Deref for Buffer {
    type Target = Rgba32FImage;

    fn deref(&self) -> &Self::Target {
        &self.buffer
    }
}

impl AsRef<Rgba32FImage> for Buffer {
    fn as_ref(&self) -> &Rgba32FImage {
        &**self
    }
}

pub struct Chunk<'a> {
    min_x: usize,
    min_y: usize,
    max_x: usize,
    max_y: usize,
    buffer: &'a mut Buffer,
}

impl<'a> Chunk<'a> {
    pub fn chunk_width(&self) -> usize {
        self.max_x - self.min_x
    }

    pub fn chunk_height(&self) -> usize {
        self.max_y - self.min_y
    }

    pub fn range_x(&self) -> Range<usize> {
        self.min_x..self.max_x
    }

    pub fn range_y(&self) -> Range<usize> {
        self.min_y..self.max_y
    }

    // SAFETY: this function must ensure that pixels outside of its bounds are never modified
    //         the bounds are inclusive on the lower bound and exclusive on the upper bound
    pub fn write_color(&mut self, x: usize, y: usize, pixel: LinearRgb) {
        assert!(
            x >= self.min_x && x < self.max_x && y >= self.min_y && y < self.max_y,
            "index ({x}, {y}) out of bounds ({min_x}, {min_y}; {max_x}, {max_y})",
            min_x = self.min_x,
            min_y = self.min_y,
            max_x = self.max_x,
            max_y = self.max_y,
        );
        self.buffer.write_color(x, y, pixel);
    }

    // SAFETY: this function must ensure that pixels outside of its bounds are never modified
    //         the bounds are inclusive on the lower bound and exclusive on the upper bound
    pub fn write_normal(&mut self, x: usize, y: usize, pixel: Vec3A) {
        assert!(
            x >= self.min_x && x < self.max_x && y >= self.min_y && y < self.max_y,
            "index ({x}, {y}) out of bounds ({min_x}, {min_y}; {max_x}, {max_y})",
            min_x = self.min_x,
            min_y = self.min_y,
            max_x = self.max_x,
            max_y = self.max_y,
        );
        self.buffer.write_normal(x, y, pixel);
    }

    // SAFETY: this function must ensure that pixels outside of its bounds are never modified
    //         the bounds are inclusive on the lower bound and exclusive on the upper bound
    pub fn write_depth(&mut self, x: usize, y: usize, pixel: f32) {
        assert!(
            x >= self.min_x && x < self.max_x && y >= self.min_y && y < self.max_y,
            "index ({x}, {y}) out of bounds ({min_x}, {min_y}; {max_x}, {max_y})",
            min_x = self.min_x,
            min_y = self.min_y,
            max_x = self.max_x,
            max_y = self.max_y,
        );
        self.buffer.write_depth(x, y, pixel);
    }
}

impl<'a> Deref for Chunk<'a> {
    type Target = Buffer;

    fn deref(&self) -> &Self::Target {
        &*self.buffer
    }
}

pub struct Chunks<'a> {
    done: bool,
    offset_x: usize,
    offset_y: usize,
    chunk_width: usize,
    chunk_height: usize,
    buffer: &'a mut Buffer,
}

impl<'a> Chunks<'a> {
    pub fn new(buffer: &'a mut Buffer, chunk_width: usize, chunk_height: usize) -> Self {
        Self {
            done: false,
            offset_x: 0,
            offset_y: 0,
            chunk_width,
            chunk_height,
            buffer,
        }
    }
}

impl<'a> Iterator for Chunks<'a> {
    type Item = Chunk<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }

        let remaining_width = self.buffer.width() - self.offset_x;
        let remaining_height = self.buffer.height() - self.offset_y;
        let chunk_width = self.chunk_width.min(remaining_width);
        let chunk_height = self.chunk_height.min(remaining_height);

        let chunk = Chunk {
            min_x: self.offset_x,
            min_y: self.offset_y,
            max_x: self.offset_x + chunk_width,
            max_y: self.offset_y + chunk_height,
            // SAFETY: chunks never overlap and all mutable operations on `Chunk` ensure they only modify their content
            //         the original buffer cannot be modified as long as any chunks exist
            buffer: unsafe { &mut *(self.buffer as *mut _) },
        };

        self.offset_x += chunk_width;
        if self.offset_x == self.buffer.width() {
            self.offset_x = 0;
            self.offset_y += chunk_height;
        }
        if self.offset_y == self.buffer.height() {
            self.done = true;
        }

        Some(chunk)
    }
}
