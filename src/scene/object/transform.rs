use glam::Affine3A;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy)]
pub(super) enum Space {
    World,
    Local,
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
pub(super) struct Transform {
    transform_world: Affine3A,
    transform_local: Affine3A,
    transform_parent: Option<Affine3A>,
}

impl Transform {
    pub fn get(&self, space: Space) -> &Affine3A {
        match space {
            Space::World => &self.transform_world,
            Space::Local => &self.transform_local,
        }
    }

    pub fn set(&mut self, space: Space, transform: Affine3A) {
        match space {
            Space::World => {
                self.transform_world = transform;
                match self.transform_parent {
                    Some(parent) => self.transform_local = parent.inverse() * self.transform_world,
                    None => self.transform_local = self.transform_world,
                }
            }
            Space::Local => {
                self.transform_local = transform;
                match self.transform_parent {
                    Some(parent) => self.transform_world = parent * self.transform_local,
                    None => self.transform_world = self.transform_local,
                }
            }
        }
    }

    pub fn set_parent(&mut self, transform: Affine3A) {
        self.transform_parent = Some(transform);
        self.transform_world = transform * self.transform_local;
    }
}

impl From<Affine3A> for Transform {
    fn from(affine: Affine3A) -> Self {
        Self {
            transform_world: affine,
            transform_local: affine,
            transform_parent: None,
        }
    }
}
