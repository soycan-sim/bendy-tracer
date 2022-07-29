use serde::{Deserialize, Serialize};

mod object;

use crate::bvh::Bvh;

pub use self::object::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    roots: Vec<Object>,
}

impl Scene {
    pub fn new() -> Self {
        Self { roots: Vec::new() }
    }

    pub fn add(&mut self, object: Object) {
        self.roots.push(object)
    }

    pub fn find_by_tag(&self, tag: &str) -> Option<&Object> {
        self.roots.iter().find_map(|object| object.find_by_tag(tag))
    }

    pub fn find_by_tag_mut(&mut self, tag: &str) -> Option<&mut Object> {
        self.roots
            .iter_mut()
            .find_map(|object| object.find_by_tag_mut(tag))
    }

    pub fn build_bvh(&self) -> Bvh {
        self.iter().filter_map(Object::build).collect()
    }

    pub fn iter(&self) -> Iter {
        self.into_iter()
    }
}

impl Default for Scene {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a Scene {
    type IntoIter = Iter<'a>;
    type Item = <Iter<'a> as Iterator>::Item;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            stack: self.roots.iter().collect(),
        }
    }
}

pub struct Iter<'a> {
    stack: Vec<&'a Object>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a Object;

    fn next(&mut self) -> Option<Self::Item> {
        let next = self.stack.pop()?;
        self.stack.extend(next.iter());
        Some(next)
    }
}
