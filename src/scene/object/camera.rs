use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Camera {
    pub sensor_size: f32,
    pub focal_length: f32,
    pub aspect_ratio: f32,
    pub fstop: f32,
    pub focus: Option<f32>,
}

impl Camera {
    const DEFAULT: Self = Self {
        sensor_size: 0.024,
        focal_length: 0.05,
        aspect_ratio: 1.5,
        fstop: 2.0,
        focus: None,
    };
}

impl Default for Camera {
    fn default() -> Self {
        Self::DEFAULT
    }
}
