use std::collections::VecDeque;
use std::hash::Hash;
use std::mem;

use hashbrown::HashMap;
use serde::{Deserialize, Serialize};

mod data;
mod object;

use crate::color::LinearRgb;

pub use self::data::*;
pub use self::object::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Collection<K: Eq + Hash, V> {
    collection: HashMap<K, V>,
    next_key: u64,
}

impl<K: Eq + Hash, V> Collection<K, V> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn get(&self, key: &K) -> Option<&V> {
        self.collection.get(key)
    }

    pub fn get_mut(&mut self, key: &K) -> Option<&mut V> {
        self.collection.get_mut(key)
    }

    pub fn iter(&self) -> impl Iterator<Item = &'_ V> {
        self.collection.values()
    }

    fn iter_mut(&mut self) -> impl Iterator<Item = &'_ mut V> {
        self.collection.values_mut()
    }

    pub fn pairs(&self) -> impl Iterator<Item = (&'_ K, &'_ V)> {
        self.collection.iter()
    }
}

pub type ObjectCollection = Collection<ObjectRef, Object>;
pub type DataCollection = Collection<DataRef, Data>;

impl ObjectCollection {
    pub fn add(&mut self, mut value: Object) -> ObjectRef {
        let key = ObjectRef(self.next_key);
        value.object_ref = Some(key);

        self.collection.insert(key, value);

        self.next_key += 1;

        key
    }
}

impl DataCollection {
    pub fn add(&mut self, value: Data) -> DataRef {
        let key = DataRef(self.next_key);
        self.collection.insert(key, value);

        self.next_key += 1;

        key
    }
}

impl<K: Eq + Hash, V> Default for Collection<K, V> {
    fn default() -> Self {
        Self {
            collection: Default::default(),
            next_key: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    roots: Vec<ObjectRef>,
    root_material: DataRef,
    objects: ObjectCollection,
    data: DataCollection,
}

impl Scene {
    pub fn new() -> Self {
        let roots = Vec::new();
        let objects = ObjectCollection::new();
        let mut data = DataCollection::new();
        let root_mat = data.add(Data::new(Material::flat(LinearRgb::BLACK)));
        Self {
            roots,
            root_material: root_mat,
            objects,
            data,
        }
    }

    pub fn add_object(&mut self, object: Object) -> ObjectRef {
        self.objects.add(object)
    }

    pub fn add_data(&mut self, data: Data) -> DataRef {
        self.data.add(data)
    }

    pub fn root_material(&self) -> &Material {
        self.get_data(self.root_material)
            .as_material()
            .expect("expected root material to be a material")
    }

    pub fn set_root_material(&mut self, data: DataRef) {
        self.root_material = data;
    }

    pub fn find_by_tag(&self, tag: &str) -> Option<ObjectRef> {
        self.objects
            .pairs()
            .find(|(_, object)| object.tag() == Some(tag))
            .map(|(&object_ref, _)| object_ref)
    }

    pub fn get_object(&self, object: ObjectRef) -> &Object {
        self.objects.get(&object).expect("invalid object ref")
    }

    pub fn get_data(&self, data: DataRef) -> &Data {
        self.data.get(&data).expect("invalid data ref")
    }

    pub fn iter(&self) -> impl Iterator<Item = &'_ Object> {
        self.objects.iter()
    }

    pub fn pairs(&self) -> impl Iterator<Item = (ObjectRef, &'_ Object)> {
        self.objects.pairs().map(|(k, v)| (*k, v))
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct UpdateQueue {
    queue: VecDeque<Update>,
}

impl UpdateQueue {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn len(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn push(&mut self, update: Update) {
        self.queue.push_back(update);
    }

    fn commit_one(self, scene: &mut Scene) -> Option<UpdateQueue> {
        let mut update_queue = Self::default();

        for update in self.queue {
            match update {
                Update::Object(object_ref, func) => {
                    let object = scene
                        .objects
                        .get_mut(&object_ref)
                        .expect("invalid object ref");
                    func(object, &mut update_queue, &scene.data);
                }
                Update::AllObjects(mut func) => scene
                    .objects
                    .iter_mut()
                    .for_each(|object| func(object, &mut update_queue, &scene.data)),
            }
        }

        if update_queue.is_empty() {
            None
        } else {
            Some(update_queue)
        }
    }

    pub fn commit(&mut self, scene: &mut Scene) {
        let mut update_queue = Self::new();
        mem::swap(self, &mut update_queue);

        let mut update_queue = Some(update_queue);

        while let Some(queue) = update_queue.take() {
            update_queue = queue.commit_one(scene);
        }
    }
}

pub type ObjectUpdate =
    dyn FnOnce(&mut Object, &mut UpdateQueue, &DataCollection) + Send + Sync + 'static;
pub type ObjectUpdateAll =
    dyn FnMut(&mut Object, &mut UpdateQueue, &DataCollection) + Send + Sync + 'static;

pub enum Update {
    Object(ObjectRef, Box<ObjectUpdate>),
    AllObjects(Box<ObjectUpdateAll>),
}

impl Update {
    pub fn object<F>(object_ref: ObjectRef, f: F) -> Self
    where
        F: FnOnce(&mut Object, &mut UpdateQueue, &DataCollection) + Send + Sync + 'static,
    {
        Self::Object(object_ref, Box::new(f))
    }

    pub fn all_objects<F>(f: F) -> Self
    where
        F: FnMut(&mut Object, &mut UpdateQueue, &DataCollection) + Send + Sync + 'static,
    {
        Self::AllObjects(Box::new(f))
    }
}

#[cfg(test)]
mod tests {
    use glam::{Affine3A, Vec3};

    use super::*;

    #[test]
    fn update() {
        let mut scene = Scene::default();
        let object_ref = scene.add_object(Object::empty());

        let mut update_queue = UpdateQueue::default();

        update_queue.push(Update::object(object_ref, |object, update_queue, _| {
            object.apply_transform(
                update_queue,
                Affine3A::from_translation(Vec3::new(1.0, 2.0, 3.0)),
            );
        }));
    }
}
