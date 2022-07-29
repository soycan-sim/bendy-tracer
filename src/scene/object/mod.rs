use bitflags::bitflags;
use glam::{Affine3A, Quat, Vec3A};
use serde::{Deserialize, Serialize};

mod camera;
mod cuboid;
mod rect;
mod sphere;
mod transform;

use crate::bvh::{ObjectData, Shape};

use self::transform::{Space, Transform};

pub use self::camera::Camera;
pub use self::cuboid::Cuboid;
pub use self::rect::Rect;
pub use self::sphere::Sphere;

bitflags! {
    #[derive(Serialize, Deserialize)]
    pub struct ObjectFlags: u32 {
        const LIGHT = 0x1;
        const VISIBLE = 0x2;
    }
}

impl Default for ObjectFlags {
    fn default() -> Self {
        ObjectFlags::VISIBLE
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Object {
    tag: Option<String>,
    flags: ObjectFlags,
    transform: Transform,
    inner: ObjectKind,
    children: Option<Vec<Object>>,
}

impl Object {
    pub fn new<T>(object: T) -> Self
    where
        ObjectKind: From<T>,
    {
        Self {
            tag: None,
            flags: ObjectFlags::default(),
            transform: Default::default(),
            inner: ObjectKind::from(object),
            children: None,
        }
    }

    pub fn empty() -> Self {
        Self::new(())
    }

    pub fn with_flags(self, flags: ObjectFlags) -> Self {
        Self {
            flags: self.flags.union(flags),
            ..self
        }
    }

    pub fn with_tag(self, tag: String) -> Self {
        Self {
            tag: Some(tag),
            ..self
        }
    }

    pub fn with_transform(self, affine: Affine3A) -> Self {
        Self {
            transform: Transform::from(affine),
            ..self
        }
    }

    pub fn with_translation(self, translation: Vec3A) -> Self {
        self.with_transform(Affine3A::from_translation(translation.into()))
    }

    pub fn with_rotation(self, translation: Vec3A, rotation: Quat) -> Self {
        self.with_transform(Affine3A::from_rotation_translation(
            rotation,
            translation.into(),
        ))
    }

    pub fn inner(&self) -> &ObjectKind {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut ObjectKind {
        &mut self.inner
    }

    pub fn flags(&self) -> ObjectFlags {
        self.flags
    }

    pub fn has_flags(&self, flags: ObjectFlags) -> bool {
        self.flags.contains(flags)
    }

    pub fn set_flags(&mut self, flags: ObjectFlags) {
        self.flags.insert(flags);
    }

    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub fn transform(&self) -> &Affine3A {
        self.transform.get(Space::World)
    }

    pub fn find_by_tag(&self, tag: &str) -> Option<&Self> {
        if self.tag.as_deref() == Some(tag) {
            Some(self)
        } else {
            self.iter().find_map(|object| object.find_by_tag(tag))
        }
    }

    pub fn find_by_tag_mut(&mut self, tag: &str) -> Option<&mut Self> {
        if self.tag.as_deref() == Some(tag) {
            Some(self)
        } else {
            self.iter_mut()
                .find_map(|object| object.find_by_tag_mut(tag))
        }
    }

    pub fn as_camera(&self) -> Option<&Camera> {
        match self.inner() {
            ObjectKind::Camera(camera) => Some(camera),
            _ => None,
        }
    }

    pub fn as_camera_mut(&mut self) -> Option<&mut Camera> {
        match self.inner_mut() {
            ObjectKind::Camera(camera) => Some(camera),
            _ => None,
        }
    }

    fn apply_parent_transform(&mut self, affine: &Affine3A) {
        self.transform.set_parent(*affine);

        let world = *self.transform.get(Space::World);

        for child in self.iter_mut() {
            child.apply_parent_transform(&world);
        }
    }

    pub fn apply_transform(&mut self, affine: Affine3A) {
        let transform = self.transform.get(Space::Local);
        self.transform.set(Space::Local, *transform * affine);

        let world = *self.transform.get(Space::World);

        for child in self.iter_mut() {
            child.apply_parent_transform(&world);
        }
    }

    pub fn add(&mut self, child: Object) {
        self.children.get_or_insert_with(Vec::new).push(child);
    }

    pub fn iter(&self) -> impl Iterator<Item = &'_ Object> {
        self.children.iter().flat_map(|children| children.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = &'_ mut Object> {
        self.children
            .iter_mut()
            .flat_map(|children| children.iter_mut())
    }

    pub fn build(&self) -> Option<ObjectData> {
        if !self.has_flags(ObjectFlags::VISIBLE) {
            return None;
        }

        let flags = self.flags;
        let tag = self.tag.clone();
        let transform = *self.transform.get(Space::World);

        let shape = match self.inner() {
            ObjectKind::Sphere(sphere) => Shape::Sphere(From::from(sphere)),
            ObjectKind::Rect(rect) => Shape::Rect(From::from(rect)),
            ObjectKind::Cuboid(cuboid) => Shape::Cuboid(From::from(cuboid)),
            _ => return None,
        };

        Some(ObjectData {
            flags,
            tag,
            transform,
            shape,
        })
    }
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ObjectKind {
    #[default]
    Empty,
    Camera(Camera),
    Sphere(Sphere),
    Rect(Rect),
    Cuboid(Cuboid),
}

impl From<()> for ObjectKind {
    fn from(_: ()) -> Self {
        Self::Empty
    }
}

impl From<Camera> for ObjectKind {
    fn from(camera: Camera) -> Self {
        Self::Camera(camera)
    }
}

impl From<Sphere> for ObjectKind {
    fn from(sphere: Sphere) -> Self {
        Self::Sphere(sphere)
    }
}

impl From<Rect> for ObjectKind {
    fn from(rect: Rect) -> Self {
        Self::Rect(rect)
    }
}

impl From<Cuboid> for ObjectKind {
    fn from(cuboid: Cuboid) -> Self {
        Self::Cuboid(cuboid)
    }
}
