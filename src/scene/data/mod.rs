use serde::{Deserialize, Serialize};

mod material;
mod volume;

pub use self::material::*;
pub use self::volume::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DataRef(pub(super) u64);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Data {
    inner: DataKind,
}

impl Data {
    pub fn new<T>(data: T) -> Self
    where
        DataKind: From<T>,
    {
        Self {
            inner: DataKind::from(data),
        }
    }

    pub fn inner(&self) -> &DataKind {
        &self.inner
    }

    pub fn as_material(&self) -> Option<&Material> {
        match self.inner() {
            DataKind::Material(material) => Some(material),
            _ => None,
        }
    }

    pub fn as_volume(&self) -> Option<&Volume> {
        match self.inner() {
            DataKind::Volume(volume) => Some(volume),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DataKind {
    Material(Material),
    Volume(Volume),
}

impl From<Material> for DataKind {
    fn from(material: Material) -> Self {
        Self::Material(material)
    }
}

impl From<Volume> for DataKind {
    fn from(volume: Volume) -> Self {
        Self::Volume(volume)
    }
}
