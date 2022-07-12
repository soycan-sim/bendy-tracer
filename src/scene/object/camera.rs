use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Camera {
    pub sensor_size: f32,
    pub focal_length: f32,
    pub aspect_ratio: f32,
    pub dof: bool,
    pub fstop: f32,
    pub focus: f32,
}

impl Camera {
    const DEFAULT: Self = Self {
        sensor_size: 0.036,
        focal_length: 0.05,
        aspect_ratio: 1.5,
        dof: true,
        fstop: 2.0,
        focus: 1.0,
    };
}

impl Default for Camera {
    fn default() -> Self {
        Self::DEFAULT
    }
}
