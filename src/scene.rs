extern crate cgmath;

use std::collections::HashMap;
use self::cgmath::{Vector3, Matrix3, SquareMatrix, InnerSpace, Matrix, ElementWise};

#[derive(Debug)]
pub struct Scene {
    pub nodes: Vec<Node>,
    pub materials: HashMap<String, Material>,
    pub lights: Vec<Light>,
}

#[derive(Debug)]
pub struct Node {
    pub object: Option<Object>,
    pub transform: Matrix3<f32>,
    pub translate: Vector3<f32>,
    pub children: Vec<Node>,
    pub name: String,
}

#[derive(Debug)]
pub struct Object {
    pub geometry: Geometry,
    pub material: String,
}

#[derive(Debug)]
pub enum Geometry {
    Sphere
}

#[derive(Debug)]
pub struct Material {
    pub diffuse: Color,
    pub specular: Color,
    pub specular_value: f32,
    pub glossiness: f32,
}

pub type Color = Vector3<f32>;

#[derive(Debug, Clone, Copy)]
pub struct Light {
    pub intensity: f32,
    pub color: Color,
    pub light_type: LightType,
}

#[derive(Debug, Clone, Copy)]
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

pub struct HitInfo {
    pub z: f32,
    pub pos: Vector3<f32>,
    pub normal: Vector3<f32>,
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
    pub fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<(HitInfo, &Node)> {
        let mut nearest: Option<(HitInfo, &Node)> = None;

        for node in self.nodes.iter() {
            if let Some((hit_info, node)) = node.intersect(pos, dir) {
                if let Some((HitInfo { z: nearest_z, pos: _, normal: _ }, _)) = nearest {
                    if hit_info.z < nearest_z {
                        nearest = Some((hit_info, node));
                    }
                } else {
                    nearest = Some((hit_info, node));
                }
            }
        }

        nearest
    }
}

impl Node {
    fn ray_to_local_space(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> (Vector3<f32>, Vector3<f32>) {
        let local_pos = self.to_local_space(pos);
        let local_dir = self.to_local_space(pos + dir) - local_pos;
        (local_pos, local_dir)
    }

    fn ray_from_local_space(&self, local_pos: Vector3<f32>, local_dir: Vector3<f32>) -> (Vector3<f32>, Vector3<f32>) {
        let pos = self.from_local_space(local_pos);
        let dir = self.from_local_space(local_pos + local_dir) - pos;
        (pos, dir)
    }

    fn to_local_space(&self, vec: Vector3<f32>) -> Vector3<f32> {
        self.transform.invert().unwrap() * (vec - self.translate)
    }

    fn from_local_space(&self, vec: Vector3<f32>) -> Vector3<f32> {
        self.transform * vec + self.translate
    }

    fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<(HitInfo, &Node)> {
        let (local_pos, local_dir) = self.ray_to_local_space(pos, dir);

        let nearest = self.object.as_ref().and_then(|object| object.geometry.intersect(local_pos, local_dir) );
        let mut nearest = nearest.map(|hit_info| (hit_info, self));

        for child in self.children.iter() {
            if let Some((hit_info, node)) = child.intersect(local_pos, local_dir) {
                if let Some((HitInfo { z: nearest_z, pos: _, normal: _ }, _)) = nearest {
                    if hit_info.z < nearest_z {
                        nearest = Some((hit_info, node));
                    }
                } else {
                    nearest = Some((hit_info, node));
                }
            }
        }

        /* transform hit info back out of local node space */
        nearest = nearest.map(|(hit_info, node)| {
            (HitInfo {
                z: hit_info.z,
                pos: self.from_local_space(hit_info.pos),
                normal: (self.transform.invert().unwrap().transpose() * hit_info.normal).normalize(),
            }, node)
        });

        nearest
    }
}

impl Geometry {
    fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<HitInfo> {
        match *self {
            Geometry::Sphere => {
                let pos_dot_dir = pos.dot(dir);
                let dir_len_sqr = dir.magnitude2();
                let discriminant = 4.0 * (pos_dot_dir * pos_dot_dir) - 4.0 * dir_len_sqr * (pos.magnitude2() - 1.0);

                if discriminant > 0.0 {
                    let discriminant_sqrt = discriminant.sqrt();
                    let t1 = (-2.0 * pos_dot_dir + discriminant_sqrt) / (2.0 * dir_len_sqr);
                    let t2 = (-2.0 * pos_dot_dir - discriminant_sqrt) / (2.0 * dir_len_sqr);

                    if t1 > 0.0 || t2 > 0.0 {
                        let t = if t1 <= t2 && t1 > 0.0 {
                            t1
                        } else {
                            t2
                        };

                        let hit_pos = pos + t * dir;
                        let normal = hit_pos.normalize();

                        Some(HitInfo {
                            z: t,
                            pos: hit_pos,
                            normal: normal,
                        })
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

impl Material {
    pub fn shade(&self, pos: Vector3<f32>, dir: Vector3<f32>, hit_info: HitInfo, lights: &Vec<&Light>) -> Color {
        lights.iter().map(|light| {
            let l: Vector3<f32>;
            match light.light_type {
                LightType::Ambient => {
                    return light.intensity * light.color.mul_element_wise(self.diffuse);
                },
                LightType::Directional(direction) => {
                    l = (-direction).normalize();
                },
                LightType::Point(location) => {
                    l = (location - hit_info.pos).normalize();
                },
            };
            let v = (pos - hit_info.pos).normalize();
            let half = (l + v).normalize();
            let n_dot_l = hit_info.normal.dot(l).max(0.0).min(1.0);
            let n_dot_h = hit_info.normal.dot(half).max(0.0).min(1.0);

            let diffuse = self.diffuse;
            let specular = self.specular_value * n_dot_h.powf(self.glossiness) * self.specular;

            (light.intensity * n_dot_l * light.color).mul_element_wise(diffuse + specular)
        }).sum()
        // hit_info.normal / 2.0 + Vector3::new(0.5, 0.5, 0.5)
        // (1.0 - ((hit_info.z - 20.0) / 70.0)) * Vector3::new(1.0, 1.0, 1.0)
        // (1.0 - ((hit_info.pos - pos).magnitude() / 100.0)) * Vector3::new(1.0, 1.0, 1.0)

    }
}

pub fn color_as_u8_array(color: Color) -> [u8; 4] {
    [(color.x * 255.0).max(0.0).min(255.0) as u8,
     (color.y * 255.0).max(0.0).min(255.0) as u8,
     (color.z * 255.0).max(0.0).min(255.0) as u8,
     255]
}
