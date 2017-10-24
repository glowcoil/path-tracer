extern crate cgmath;

use std::f32;
use std::ops::IndexMut;
use self::cgmath::Vector3;

#[derive(Debug)]
pub struct BVH {
    nodes: Vec<BVHNode>
}

#[derive(Debug)]
pub enum BVHNode {
    Node {
        left_child: usize,
        right_child: usize,
        bounding_box: BoundingBox,
    },
    Leaf {
        index: usize,
        bounding_box: BoundingBox,
    }
}

impl BVHNode {
    fn bounding_box(&self) -> BoundingBox {
        match *self {
            BVHNode::Node { bounding_box, .. } => bounding_box,
            BVHNode::Leaf { bounding_box, .. } => bounding_box,
        }
    }
}

impl BVH {
    pub fn build(root_box: BoundingBox, boxes: &[BoundingBox]) -> BVH {
        if boxes.len() == 0 {
            panic!("Cannot construct empty BVH");
        }

        let mut nodes: Vec<BVHNode> = Vec::new();
        let mut elems: Vec<usize> = (0..boxes.len()).collect();

        Self::split(&mut nodes, &mut elems, &root_box, boxes);

        BVH { nodes: nodes }
    }

    fn split(nodes: &mut Vec<BVHNode>, elems: &mut [usize], root_box: &BoundingBox, boxes: &[BoundingBox]) {
        if elems.len() == 1 {
            nodes.push(BVHNode::Leaf {
                index: elems[0],
                bounding_box: boxes[elems[0]],
            });
            return;
        }

        let x_size = root_box.p2.x - root_box.p1.x;
        let y_size = root_box.p2.y - root_box.p1.y;
        let z_size = root_box.p2.z - root_box.p1.z;

        /* partition indices */
        let mut j = 0;
        if x_size > y_size && x_size > z_size {
            let pivot = root_box.p1.x + x_size / 2.0;

            for i in 0..elems.len() {
                let elem_box = &boxes[elems[i]];
                let center = (elem_box.p2.x - elem_box.p1.x) / 2.0;

                if center < pivot {
                    let tmp = elems[i];
                    elems[i] = elems[j];
                    elems[j] = tmp;
                    j += 1;
                }
            }
        } else if y_size > z_size {
            let pivot = root_box.p1.y + y_size / 2.0;

            for i in 0..elems.len() {
                let elem_box = &boxes[elems[i]];
                let center = (elem_box.p2.y - elem_box.p1.y) / 2.0;

                if center < pivot {
                    let tmp = elems[i];
                    elems[i] = elems[j];
                    elems[j] = tmp;
                    j += 1;
                }
            }
        } else {
            let pivot = root_box.p1.z + z_size / 2.0;

            for i in 0..elems.len() {
                let elem_box = &boxes[elems[i]];
                let center = (elem_box.p2.z - elem_box.p1.z) / 2.0;

                if center < pivot {
                    let tmp = elems[i];
                    elems[i] = elems[j];
                    elems[j] = tmp;
                    j += 1;
                }
            }
        }

        if j == 0 || j == elems.len() {
            j = elems.len() / 2;
        }

        let node_index = nodes.len();
        nodes.push(BVHNode::Node {
            left_child: node_index + 1,
            right_child: 0,
            bounding_box: *root_box,
        });

        {
            let left_elems = &mut elems[0..j];
            let mut left_box = boxes[left_elems[0]];
            for elem in &left_elems[1..] {
                left_box.union(&boxes[*elem]);
            }
            Self::split(nodes, left_elems, &left_box, boxes);
        }

        {
            let right_index = nodes.len();
            let node = nodes.index_mut(node_index);
            match *node {
                BVHNode::Node { ref mut right_child, .. } => {
                    *right_child = right_index;
                },
                _ => {},
            }
        }

        {
            let right_elems = &mut elems[j..];
            let mut right_box = boxes[right_elems[0]];
            for elem in &right_elems[1..] {
                right_box.union(&boxes[*elem]);
            }
            Self::split(nodes, right_elems, root_box, boxes);
        }
    }

    pub fn traverse<'a>(&'a self, pos: Vector3<f32>, dir: Vector3<f32>) -> BVHIterator<'a> {
        let stack = if self.nodes[0].bounding_box().intersect(pos, dir).is_some() {
            vec![0]
        } else {
            vec![]
        };

        BVHIterator {
            bvh: &self,
            stack: stack,
            pos: pos,
            dir: dir,
        }
    }
}

pub struct BVHIterator<'a> {
    bvh: &'a BVH,
    stack: Vec<usize>,
    pos: Vector3<f32>,
    dir: Vector3<f32>,
}

impl<'a> Iterator for BVHIterator<'a> {
    type Item = usize;

    fn next(&mut self) -> Option<usize> {
        while let Some(i) = self.stack.pop() {
            match self.bvh.nodes[i] {
                BVHNode::Node { left_child, right_child, bounding_box: _ } => {
                    let left = self.bvh.nodes[left_child].bounding_box().intersect(self.pos, self.dir);
                    let right = self.bvh.nodes[right_child].bounding_box().intersect(self.pos, self.dir);

                    if let Some(t_left) = left {
                        if let Some(t_right) = right {
                            let (i_first, i_second) = if t_left < t_right {
                                (left_child, right_child)
                            } else {
                                (right_child, left_child)
                            };

                            self.stack.push(i_second);
                            self.stack.push(i_first);
                        } else {
                            self.stack.push(left_child);
                        }
                    } else if let Some(_) = right {
                        self.stack.push(right_child);
                    }
                },
                BVHNode::Leaf { index, bounding_box: _ } => {
                    return Some(index);
                },
            }
        }
        None
    }
}

pub trait Bounded {
    fn bounding_box(&self) -> BoundingBox;
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub p1: Vector3<f32>,
    pub p2: Vector3<f32>,
}

impl BoundingBox {
    pub fn new(x1: f32, y1: f32, z1: f32, x2: f32, y2: f32, z2: f32) -> BoundingBox {
        BoundingBox {
            p1: Vector3::new(x1, y1, z1),
            p2: Vector3::new(x2, y2, z2),
        }
    }

    pub fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<f32> {
        let mut in_x = f32::NEG_INFINITY;
        let mut out_x = f32::INFINITY;
        if dir.x == 0.0 {
            if pos.x < self.p1.x || pos.x > self.p2.x {
                return None;
            }
        } else {
            in_x = (self.p1.x - pos.x) / dir.x;
            out_x = (self.p2.x - pos.x) / dir.x;
            if out_x < in_x {
                let tmp = in_x;
                in_x = out_x;
                out_x = tmp;
            }
        }

        let mut in_y = f32::NEG_INFINITY;
        let mut out_y = f32::INFINITY;
        if dir.y == 0.0 {
            if pos.y < self.p1.y || pos.y > self.p2.y {
                return None;
            }
        } else {
            in_y = (self.p1.y - pos.y) / dir.y;
            out_y = (self.p2.y - pos.y) / dir.y;
            if out_y < in_y {
                let tmp = in_y;
                in_y = out_y;
                out_y = tmp;
            }
        }

        let mut in_z = f32::NEG_INFINITY;
        let mut out_z = f32::INFINITY;
        if dir.z == 0.0 {
            if pos.z < self.p1.z || pos.z > self.p2.z {
                return None;
            }
        } else {
            in_z = (self.p1.z - pos.z) / dir.z;
            out_z = (self.p2.z - pos.z) / dir.z;
            if out_z < in_z {
                let tmp = in_z;
                in_z = out_z;
                out_z = tmp;
            }
        }

        let t_in = in_x.max(in_y).max(in_z);
        let t_out = out_x.min(out_y).min(out_z);
        if t_in <= t_out {
            Some(t_in)
        } else {
            None
        }
    }

    pub fn union(&mut self, other: &BoundingBox) {
        self.p1.x = self.p1.x.min(other.p1.x);
        self.p1.y = self.p1.y.min(other.p1.y);
        self.p1.z = self.p1.z.min(other.p1.z);

        self.p2.x = self.p2.x.max(other.p2.x);
        self.p2.y = self.p2.y.max(other.p2.y);
        self.p2.z = self.p2.z.max(other.p2.z);
    }
}
