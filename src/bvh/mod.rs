use std::mem;

use glam::Vec3A;

use crate::tracer::{Clip, Manifold, Ray};

mod object;

pub use object::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: Vec3A,
    pub max: Vec3A,
}

impl Aabb {
    pub fn new(min: Vec3A, max: Vec3A) -> Self {
        debug_assert!(min.cmplt(max).all());
        Self { min, max }
    }

    pub fn map_into(&self, coord: Vec3A) -> Vec3A {
        (coord - self.min) / self.size()
    }

    pub fn size(&self) -> Vec3A {
        self.max - self.min
    }

    pub fn area(&self) -> f32 {
        let size = self.size();
        size.x * size.y * size.z
    }

    pub fn union(&self, other: &Self) -> Self {
        Self::new(self.min.min(other.min), self.max.max(other.max))
    }

    pub fn intersection(&self, other: &Self) -> Self {
        Self::new(self.min.max(other.min), self.max.min(other.max))
    }

    pub fn overlap(&self, other: &Self) -> f32 {
        self.intersection(other).area()
    }

    pub fn overlaps(&self, other: &Self) -> bool {
        self.overlap(other) > 0.0
    }

    // credit to Andrew Kensler, thanks to Peter Shirley
    // https://raytracing.github.io/books/RayTracingTheNextWeek.html#boundingvolumehierarchies/anoptimizedaabbhitmethod
    pub fn hit(&self, ray: &Ray, clip: &Clip) -> bool {
        let min = self.min.to_array();
        let max = self.max.to_array();
        let origin = ray.origin.to_array();
        let direction = ray.direction.to_array();

        let mut t_min = clip.min;
        let mut t_max = clip.max;

        for i in 0..3 {
            let d_recip = direction[i].recip();
            let mut t0 = (min[i] - origin[i]) * d_recip;
            let mut t1 = (max[i] - origin[i]) * d_recip;
            if d_recip.is_sign_negative() {
                mem::swap(&mut t0, &mut t1);
            }
            t_min = t0.max(t_min);
            t_max = t1.min(t_max);
            if t_max <= t_min {
                return false;
            }
        }

        true
    }
}

#[derive(Debug, Default)]
pub struct Bvh {
    len: usize,
    height: u32,
    root: Option<BvhNode>,
}

impl Bvh {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    pub fn hit(&self, ray: &Ray, clip: &Clip) -> Option<Manifold> {
        self.root.as_ref().and_then(|root| root.hit(ray, clip))
    }

    pub fn iter(&self) -> Iter {
        self.into_iter()
    }
}

// FIXME: naive implementation
impl FromIterator<ObjectData> for Bvh {
    fn from_iter<I: IntoIterator<Item = ObjectData>>(iter: I) -> Self {
        let mut len = 0;
        let mut root: Option<BvhNode> = None;

        for object in iter {
            len += 1;

            let leaf = BvhNode::Leaf {
                aabb: object.bounding_box(),
                child: object,
            };

            if let Some(root) = root.as_mut() {
                root.insert(leaf);
            } else {
                root = Some(leaf);
            }
        }

        let height = root.as_ref().map_or(0, |node| node.height() + 1);

        Self { len, height, root }
    }
}

impl<'a> IntoIterator for &'a Bvh {
    type IntoIter = Iter<'a>;
    type Item = <Iter<'a> as Iterator>::Item;

    fn into_iter(self) -> Self::IntoIter {
        Iter {
            stack: self.root.iter().collect(),
        }
    }
}

#[derive(Debug)]
enum BvhNode {
    // Only used as an intermediate value when inserting or rotating
    Empty,
    Parent {
        aabb: Aabb,
        height: u32,
        left: Box<BvhNode>,
        right: Box<BvhNode>,
    },
    Leaf {
        aabb: Aabb,
        child: ObjectData,
    },
}

impl BvhNode {
    pub fn insert(&mut self, other: Self) {
        match self {
            BvhNode::Parent {
                aabb,
                height,
                left,
                right,
            } => {
                if other.aabb().overlap(left.aabb()) > other.aabb().overlap(right.aabb()) {
                    left.insert(other);
                } else {
                    right.insert(other);
                }
                *aabb = left.aabb().union(right.aabb());
                *height = left.height().max(right.height()) + 1;
            }
            BvhNode::Leaf { .. } => {
                let mut this = Self::Empty;
                mem::swap(self, &mut this);

                let left = this;
                let right = other;

                *self = BvhNode::parent(Box::new(left), Box::new(right));
            }
            BvhNode::Empty => unreachable!(),
        }

        if !self.is_balanced() {
            self.rebalance();
        }
    }

    fn parent(left: Box<BvhNode>, right: Box<BvhNode>) -> Self {
        let aabb = left.aabb().union(right.aabb());
        let height = left.height().max(right.height()) + 1;
        Self::Parent {
            aabb,
            height,
            left,
            right,
        }
    }

    fn rebalance(&mut self) {
        if self.is_balanced() {
            return;
        }

        let balance = self.balance();

        if let Self::Parent { left, right, .. } = self {
            if balance < 0 && left.balance() <= 0 {
                self.rotate_right()
            } else if balance > 0 && right.balance() >= 0 {
                self.rotate_left()
            } else if balance < 0 && left.balance() > 0 {
                self.rotate_left_right()
            } else if balance > 0 && right.balance() < 0 {
                self.rotate_right_left()
            }
        }
    }

    fn rotate_right(&mut self) {
        let mut this = Self::Empty;
        mem::swap(self, &mut this);

        if let Self::Parent {
            left: root_left,
            right: root_right,
            ..
        } = this
        {
            if let Self::Parent {
                left: pivot_left,
                right: pivot_right,
                ..
            } = *root_left
            {
                let right = Self::parent(pivot_right, root_right);
                let left = pivot_left;
                *self = Self::parent(left, Box::new(right));
            }
        }
    }

    fn rotate_left(&mut self) {
        let mut this = Self::Empty;
        mem::swap(self, &mut this);

        if let Self::Parent {
            left: root_left,
            right: root_right,
            ..
        } = this
        {
            if let Self::Parent {
                left: pivot_left,
                right: pivot_right,
                ..
            } = *root_right
            {
                let left = Self::parent(root_left, pivot_left);
                let right = pivot_right;
                *self = Self::parent(Box::new(left), right);
            }
        }
    }

    fn rotate_right_left(&mut self) {
        if let Self::Parent { right, .. } = self {
            right.rotate_right();
            self.rotate_left();
        }
    }

    fn rotate_left_right(&mut self) {
        if let Self::Parent { left, .. } = self {
            left.rotate_left();
            self.rotate_right();
        }
    }

    pub fn height(&self) -> u32 {
        match *self {
            Self::Parent { height, .. } => height,
            Self::Leaf { .. } => 0,
            BvhNode::Empty => unreachable!(),
        }
    }

    fn balance(&self) -> i32 {
        match self {
            Self::Parent { left, right, .. } => right.height() as i32 - left.height() as i32,
            Self::Leaf { .. } => 0,
            BvhNode::Empty => unreachable!(),
        }
    }

    fn is_balanced(&self) -> bool {
        self.balance().abs() <= 1
    }

    pub fn aabb(&self) -> &Aabb {
        match self {
            Self::Parent { aabb, .. } | Self::Leaf { aabb, .. } => aabb,
            BvhNode::Empty => unreachable!(),
        }
    }

    pub fn hit(&self, ray: &Ray, clip: &Clip) -> Option<Manifold> {
        match self {
            Self::Leaf { aabb, child } if aabb.hit(ray, clip) => child.hit(ray, clip),
            Self::Parent {
                aabb, left, right, ..
            } if aabb.hit(ray, clip) => {
                let left = left.hit(ray, clip);
                let right = right.hit(ray, clip);
                match (left, right) {
                    (None, None) => None,
                    (Some(manifold), None) => Some(manifold),
                    (None, Some(manifold)) => Some(manifold),
                    (Some(left), Some(right)) if left.t < right.t => Some(left),
                    (Some(_), Some(right)) => Some(right),
                }
            }
            _ => None,
        }
    }
}

#[derive(Debug)]
pub struct Iter<'a> {
    stack: Vec<&'a BvhNode>,
}

impl<'a> Iterator for Iter<'a> {
    type Item = &'a ObjectData;

    fn next(&mut self) -> Option<Self::Item> {
        let mut node = self.stack.pop()?;
        loop {
            match node {
                BvhNode::Parent { left, right, .. } => {
                    if left.height() < right.height() {
                        node = &**left;
                        self.stack.push(right);
                    } else {
                        node = &**right;
                        self.stack.push(left);
                    }
                }
                BvhNode::Leaf { child, .. } => return Some(child),
                BvhNode::Empty => unreachable!(),
            }
        }
    }
}
