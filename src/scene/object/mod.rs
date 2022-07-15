use glam::{Affine3A, Quat, Vec3A};
use serde::{Deserialize, Serialize};

use crate::tracer::{Clip, Manifold, Ray};

use super::{Update, UpdateQueue};

mod camera;
mod sphere;
mod transform;

use self::transform::{Space, Transform};

pub use self::camera::Camera;
pub use self::sphere::Sphere;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ObjectRef(pub(super) u64);

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct Object {
    pub(super) object_ref: Option<ObjectRef>,
    tag: Option<String>,
    transform: Transform,
    inner: ObjectKind,
    children: Option<Vec<ObjectRef>>,
}

impl Object {
    pub fn new<T>(object: T) -> Self
    where
        ObjectKind: From<T>,
    {
        Self {
            object_ref: None,
            tag: None,
            transform: Default::default(),
            inner: ObjectKind::from(object),
            children: None,
        }
    }

    pub fn empty() -> Self {
        Self::new(())
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

    pub fn tag(&self) -> Option<&str> {
        self.tag.as_deref()
    }

    pub fn transform(&self) -> &Affine3A {
        self.transform.get(Space::World)
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

    pub fn hit(&self, ray: &Ray, clip: &Clip) -> Option<Manifold> {
        match self.inner() {
            ObjectKind::Sphere(sphere) => sphere.hit(
                self.object_ref.expect("can't hit-test orphan objects"),
                self.transform().translation,
                ray,
                clip,
            ),
            _ => None,
        }
    }

    fn apply_parent_transform(&mut self, update_queue: &mut UpdateQueue, affine: &Affine3A) {
        self.transform.set_parent(*affine);

        let transform = *self.transform.get(Space::World);
        for child in self.children() {
            update_queue.push(Update::object(child, move |object, update_queue| {
                let affine = transform;
                object.apply_parent_transform(update_queue, &affine);
            }))
        }
    }

    pub fn apply_transform(&mut self, update_queue: &mut UpdateQueue, affine: Affine3A) {
        let transform = self.transform.get(Space::Local);
        self.transform.set(Space::Local, *transform * affine);

        let transform = *self.transform.get(Space::World);
        for child in self.children() {
            update_queue.push(Update::object(child, move |object, update_queue| {
                let affine = transform;
                object.apply_parent_transform(update_queue, &affine);
            }))
        }
    }

    pub fn add(&mut self, update_queue: &mut UpdateQueue, child: ObjectRef) {
        let transform = *self.transform.get(Space::World);
        update_queue.push(Update::object(child, move |object, update_queue| {
            let affine = transform;
            object.apply_parent_transform(update_queue, &affine);
        }));

        match self.children {
            Some(ref mut children) => children.push(child),
            None => self.children = Some(vec![child]),
        }
    }

    pub fn children(&self) -> impl Iterator<Item = ObjectRef> + '_ {
        self.children
            .as_ref()
            .map(|children| children.iter().copied())
            .into_iter()
            .flatten()
    }
}

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize)]
#[non_exhaustive]
pub enum ObjectKind {
    #[default]
    Empty,
    Camera(Camera),
    Sphere(Sphere),
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
