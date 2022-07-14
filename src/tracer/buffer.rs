use std::ops::{Deref, Range};

use image::{Rgba, Rgba32FImage, RgbaImage};

const BLACK_ALPHA_ONE: Rgba<f32> = Rgba([0.0, 0.0, 0.0, 1.0]);

#[derive(Debug, Clone)]
pub struct Buffer {
    samples: usize,
    max_samples: usize,
    weight: f32,
    buffer: Rgba32FImage,
    preview: Option<RgbaImage>,
}

impl Buffer {
    pub fn new(width: u32, height: u32, max_samples: usize) -> Self {
        let samples = 0;
        let weight = (max_samples as f32).recip();
        let buffer = Rgba32FImage::from_pixel(width, height, BLACK_ALPHA_ONE);
        Self {
            samples,
            max_samples,
            weight,
            buffer,
            preview: None,
        }
    }

    pub fn samples(&self) -> usize {
        self.samples
    }

    pub fn max_samples(&self) -> usize {
        self.max_samples
    }

    pub fn set_max_samples(&mut self, max_samples: usize) {
        self.max_samples = max_samples;
        self.weight = (max_samples as f32).recip();
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

    pub fn chunks(&mut self, chunks_x: u32, chunks_y: u32) -> Chunks {
        let chunk_width = if self.buffer.width() % chunks_x == 0 {
            self.buffer.width() / chunks_x
        } else {
            self.buffer.width() / chunks_x + 1
        };
        let chunk_height = if self.buffer.height() % chunks_y == 0 {
            self.buffer.height() / chunks_y
        } else {
            self.buffer.height() / chunks_y + 1
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
        let weight = samples_recip * self.max_samples as f32;

        for (target, source) in preview.pixels_mut().zip(self.buffer.pixels()) {
            let r = (source.0[0] * weight * 255.0) as u8;
            let g = (source.0[1] * weight * 255.0) as u8;
            let b = (source.0[2] * weight * 255.0) as u8;
            let a = (source.0[3] * 255.0) as u8;
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

    pub(super) fn add_samples(&mut self, x: u32, y: u32, pixel: [f32; 3]) {
        let Rgba([r, g, b, _]) = self.buffer.get_pixel_mut(x, y);
        *r += pixel[0] * self.weight;
        *g += pixel[1] * self.weight;
        *b += pixel[2] * self.weight;
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
    min_x: u32,
    min_y: u32,
    max_x: u32,
    max_y: u32,
    buffer: &'a mut Buffer,
}

impl<'a> Chunk<'a> {
    pub fn chunk_width(&self) -> u32 {
        self.max_x - self.min_x
    }

    pub fn chunk_height(&self) -> u32 {
        self.max_y - self.min_y
    }

    pub fn range_x(&self) -> Range<u32> {
        self.min_x..self.max_x
    }

    pub fn range_y(&self) -> Range<u32> {
        self.min_y..self.max_y
    }

    // SAFETY: this function must ensure that pixels outside of its bounds are never modified
    //         the bounds are inclusive on the lower bound and exclusive on the upper bound
    pub fn add_samples(&mut self, x: u32, y: u32, pixel: [f32; 3]) {
        assert!(
            x >= self.min_x && x < self.max_x && y >= self.min_y && y < self.max_y,
            "index ({x}, {y}) out of bounds ({min_x}, {min_y}; {max_x}, {max_y})",
            min_x = self.min_x,
            min_y = self.min_y,
            max_x = self.max_x,
            max_y = self.max_y,
        );
        self.buffer.add_samples(x, y, pixel);
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
    offset_x: u32,
    offset_y: u32,
    chunk_width: u32,
    chunk_height: u32,
    buffer: &'a mut Buffer,
}

impl<'a> Chunks<'a> {
    pub fn new(buffer: &'a mut Buffer, chunk_width: u32, chunk_height: u32) -> Self {
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
