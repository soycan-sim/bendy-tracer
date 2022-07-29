use std::ops::{Add, AddAssign, Div, DivAssign, Mul, MulAssign, Sub, SubAssign};

use glam::Vec3;
use serde::{Deserialize, Serialize};

use crate::math::Interpolate;

fn srgb_to_linear(x: f32) -> f32 {
    if x <= 0.04045 {
        x / 12.92
    } else {
        ((x + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(x: f32) -> f32 {
    if x <= 0.0031308 {
        12.92 * x
    } else {
        1.055 * x.powf(1.0 / 2.4) - 0.055
    }
}

fn f32_to_u8(x: f32) -> u8 {
    (x * u8::MAX as f32) as u8
}

fn u8_to_f32(x: u8) -> f32 {
    x as f32 / u8::MAX as f32
}

macro_rules! impl_rgb {
    ($t:ty) => {
        impl $t {
            pub const BLACK: Self = Self::splat(0.0);
            pub const WHITE: Self = Self::splat(1.0);

            pub const fn new(r: f32, g: f32, b: f32) -> Self {
                Self { r, g, b }
            }

            pub const fn splat(x: f32) -> Self {
                Self::new(x, x, x)
            }

            pub fn to_bytes(self) -> [u8; 3] {
                <[u8; 3]>::from(self)
            }
        }

        impl Interpolate for $t {
            fn lerp(self, other: Self, factor: f32) -> Self {
                Self::new(
                    self.r.lerp(other.r, factor),
                    self.g.lerp(other.g, factor),
                    self.b.lerp(other.b, factor),
                )
            }
        }

        impl From<[f32; 3]> for $t {
            fn from([r, g, b]: [f32; 3]) -> Self {
                Self { r, g, b }
            }
        }

        impl From<$t> for [f32; 3] {
            fn from(x: $t) -> Self {
                [x.r, x.g, x.b]
            }
        }

        impl From<(f32, f32, f32)> for $t {
            fn from((r, g, b): (f32, f32, f32)) -> Self {
                Self { r, g, b }
            }
        }

        impl From<$t> for (f32, f32, f32) {
            fn from(x: $t) -> Self {
                (x.r, x.g, x.b)
            }
        }

        impl From<$t> for [u8; 3] {
            fn from(x: $t) -> Self {
                let r = f32_to_u8(x.r);
                let g = f32_to_u8(x.g);
                let b = f32_to_u8(x.b);
                [r, g, b]
            }
        }

        impl From<$t> for (u8, u8, u8) {
            fn from(x: $t) -> Self {
                let r = f32_to_u8(x.r);
                let g = f32_to_u8(x.g);
                let b = f32_to_u8(x.b);
                (r, g, b)
            }
        }

        impl From<[u8; 3]> for $t {
            fn from([r, g, b]: [u8; 3]) -> Self {
                let r = u8_to_f32(r);
                let g = u8_to_f32(g);
                let b = u8_to_f32(b);
                Self { r, g, b }
            }
        }

        impl From<(u8, u8, u8)> for $t {
            fn from((r, g, b): (u8, u8, u8)) -> Self {
                let r = u8_to_f32(r);
                let g = u8_to_f32(g);
                let b = u8_to_f32(b);
                Self { r, g, b }
            }
        }
    };
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Rgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl_rgb!(Rgb);

impl From<SRgb> for Rgb {
    fn from(c: SRgb) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
        }
    }
}

impl From<Vec3> for Rgb {
    fn from(v: Vec3) -> Self {
        Self {
            r: v.x,
            g: v.y,
            b: v.z,
        }
    }
}

impl From<Rgb> for Vec3 {
    fn from(c: Rgb) -> Self {
        Self {
            x: c.r,
            y: c.g,
            z: c.b,
        }
    }
}

impl From<LinearRgb> for Rgb {
    fn from(c: LinearRgb) -> Self {
        Self {
            r: c.r,
            g: c.g,
            b: c.b,
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SRgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl_rgb!(SRgb);

impl SRgb {
    pub fn from_linear(r: f32, g: f32, b: f32) -> Self {
        let r = linear_to_srgb(r);
        let g = linear_to_srgb(g);
        let b = linear_to_srgb(b);

        Self { r, g, b }
    }

    pub fn to_linear(self) -> LinearRgb {
        LinearRgb::from(self)
    }
}

impl From<LinearRgb> for SRgb {
    fn from(c: LinearRgb) -> Self {
        Self::from_linear(c.r, c.g, c.b)
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LinearRgb {
    pub r: f32,
    pub g: f32,
    pub b: f32,
}

impl_rgb!(LinearRgb);

impl LinearRgb {
    pub fn from_srgb(r: f32, g: f32, b: f32) -> Self {
        let r = srgb_to_linear(r);
        let g = srgb_to_linear(g);
        let b = srgb_to_linear(b);

        Self { r, g, b }
    }

    pub fn to_srgb(self) -> SRgb {
        SRgb::from(self)
    }
}

impl From<SRgb> for LinearRgb {
    fn from(c: SRgb) -> Self {
        Self::from_srgb(c.r, c.g, c.b)
    }
}

macro_rules! impl_rgb_op {
    ($trait:ty, $func:ident) => {
        impl $trait for LinearRgb {
            type Output = Self;

            fn $func(self, rhs: Self) -> Self::Output {
                Self {
                    r: self.r.$func(rhs.r),
                    g: self.g.$func(rhs.g),
                    b: self.b.$func(rhs.b),
                }
            }
        }
    };
}

macro_rules! impl_scalar_op {
    ($trait:ty, $func:ident) => {
        impl $trait for LinearRgb {
            type Output = Self;

            fn $func(self, rhs: f32) -> Self::Output {
                Self {
                    r: self.r.$func(rhs),
                    g: self.g.$func(rhs),
                    b: self.b.$func(rhs),
                }
            }
        }
    };
}

macro_rules! impl_rgb_op_assign {
    ($trait:ty, $assign:ident, $func:ident) => {
        impl $trait for LinearRgb {
            fn $assign(&mut self, rhs: Self) {
                self.r = self.r.$func(rhs.r);
                self.g = self.g.$func(rhs.g);
                self.b = self.b.$func(rhs.b);
            }
        }
    };
}

macro_rules! impl_scalar_op_assign {
    ($trait:ty, $assign:ident, $func:ident) => {
        impl $trait for LinearRgb {
            fn $assign(&mut self, rhs: f32) {
                self.r = self.r.$func(rhs);
                self.g = self.g.$func(rhs);
                self.b = self.b.$func(rhs);
            }
        }
    };
}

impl_rgb_op!(Add, add);
impl_rgb_op_assign!(AddAssign, add_assign, add);

impl_rgb_op!(Sub, sub);
impl_rgb_op_assign!(SubAssign, sub_assign, sub);

impl_rgb_op!(Mul, mul);
impl_scalar_op!(Mul<f32>, mul);
impl_rgb_op_assign!(MulAssign, mul_assign, mul);
impl_scalar_op_assign!(MulAssign<f32>, mul_assign, mul);

impl_rgb_op!(Div, div);
impl_scalar_op!(Div<f32>, div);
impl_rgb_op_assign!(DivAssign, div_assign, div);
impl_scalar_op_assign!(DivAssign<f32>, div_assign, div);
