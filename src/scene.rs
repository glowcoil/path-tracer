extern crate cgmath;

use self::cgmath::{Vector3, Matrix3, SquareMatrix, InnerSpace};

#[derive(Debug)]
pub struct Scene {
    pub nodes: Vec<Node>,
    pub materials: Vec<Material>,
    pub lights: Vec<Light>,
}

#[derive(Debug)]
pub struct Node {
    pub object: Object,
    pub transform: Matrix3<f32>,
    pub translate: Vector3<f32>,
    pub children: Vec<Node>,
}

#[derive(Debug)]
pub struct Object {
    pub geometry: Geometry,
    pub name: String,
}

#[derive(Debug)]
pub enum Geometry {
    Sphere
}

#[derive(Debug)]
pub struct Material {

}

#[derive(Debug)]
pub struct Light {
    intensity: f32,
    light_type: LightType,
}

#[derive(Debug)]
pub enum LightType {
    Ambient,
    Directional(Vector3<f32>),
    Point(Vector3<f32>),
}

pub struct Camera {
    pub pos: Vector3<f32>,
    pub dir: Vector3<f32>,
    pub up: Vector3<f32>,
    pub fov: f32,
    pub img_width: u32,
    pub img_height: u32,
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            pos: Vector3::new(0.0, 0.0, 0.0),
            dir: Vector3::new(0.0, 0.0, -1.0),
            up: Vector3::new(0.0, 1.0, 0.0),
            fov: 40.0,
            img_width: 800,
            img_height: 600,
        }
    }
}

impl Scene {
    pub fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<f32> {
        let mut nearest = None;

        for node in self.nodes.iter() {
            if let Some(z) = node.intersect(pos, dir) {
                if let Some(nearest_z) = nearest {
                    if z < nearest_z {
                        nearest = Some(z);
                    }
                } else {
                    nearest = Some(z);
                }
            }
        }

        nearest
    }
}

impl Node {
    fn transform_ray(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> (Vector3<f32>, Vector3<f32>) {
        let local_pos = self.to_local_space(pos);
        let local_dir = self.to_local_space(pos + dir) - local_pos;
        (local_pos, local_dir)
    }

    fn to_local_space(&self, vec: Vector3<f32>) -> Vector3<f32> {
        self.transform.invert().unwrap() * (vec - self.translate)
    }

    fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<f32> {
        let (local_pos, local_dir) = self.transform_ray(pos, dir);

        let mut nearest = self.object.geometry.intersect(local_pos, local_dir);

        for child in self.children.iter() {
            if let Some(z) = child.intersect(local_pos, local_dir) {
                if let Some(nearest_z) = nearest {
                    if z < nearest_z {
                        nearest = Some(z);
                    }
                } else {
                    nearest = Some(z);
                }
            }
        }

        nearest
    }
}

impl Geometry {
    fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<f32> {
        match *self {
            Geometry::Sphere => {
                let pos_dot_dir = pos.dot(dir);
                let dir_len_sqr = dir.magnitude2();
                let discriminant = 4.0 * (pos_dot_dir * pos_dot_dir) - 4.0 * dir_len_sqr * (pos.magnitude2() - 1.0);

                if discriminant > 0.0 {
                    let discriminant_sqrt = discriminant.sqrt();
                    let t1 = (-2.0 * pos_dot_dir + discriminant_sqrt) / (2.0 * dir_len_sqr);
                    let t2 = (-2.0 * pos_dot_dir - discriminant_sqrt) / (2.0 * dir_len_sqr);

                    if t1 <= t2 && t1 > 0.0 {
                        Some(t1)
                    } else if t2 > 0.0 {
                        Some(t2)
                    } else {
                        None
                    }
                } else {
                    None
                }
            }
        }
    }
}
