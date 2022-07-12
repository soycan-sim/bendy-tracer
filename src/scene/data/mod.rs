use serde::{Deserialize, Serialize};

mod material;

pub use self::material::Material;

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
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum DataKind {
    Material(Material),
}

impl From<Material> for DataKind {
    fn from(material: Material) -> Self {
        Self::Material(material)
    }
}
