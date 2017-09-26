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
    pub reflection: Color,
    pub reflection_value: f32,
    pub refraction: Color,
    pub refraction_value: f32,
    pub refraction_index: f32,
    pub absorption: Color,
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
    pub side: Side,
}

pub enum Side {
    Back,
    Front,
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

const BIAS: f32 = 0.1;

impl Scene {
    pub fn cast(&self, pos: Vector3<f32>, dir: Vector3<f32>, bounces: i32) -> Color {
        if let Some((hit_info, node)) = self.intersect(pos, dir) {
            let material = self.materials.get(&node.object.as_ref().unwrap().material[..])
                .expect("material does not exist for object");

            /* check if lights are blocked (meaning we're in shadow) */
            let lights: Vec<&Light> = {
                self.lights.iter().filter(|light| {
                    match light.light_type {
                        LightType::Ambient => true,
                        LightType::Directional(dir) => {
                            let ray = -dir;
                            self.intersect(hit_info.pos + BIAS * ray, ray).is_none()
                        },
                        LightType::Point(pos) => {
                            let ray = (pos - hit_info.pos).normalize();
                            match self.intersect(hit_info.pos + BIAS * ray, ray) {
                                Some((blocking_hit_info, _)) => blocking_hit_info.z > (pos - hit_info.pos).magnitude(),
                                None => true
                            }
                        },
                    }
                })
            }.collect();

            let mut color = material.shade(pos, dir, &hit_info, &lights);

            if (material.reflection_value > 0.0 || material.refraction_value > 0.0) && bounces > 0 {
                let normal = match hit_info.side {
                    Side::Back => -hit_info.normal,
                    Side::Front => hit_info.normal,
                };

                let reflected_ray = reflect_ray(-dir, normal);
                let reflected = material.reflection_value * material.reflection
                    .mul_element_wise(self.cast(hit_info.pos + BIAS * reflected_ray, reflected_ray, bounces - 1));


                match hit_info.side {
                    Side::Back => {

                    },
                    Side::Front => {

                    }
                }
                let (n1, n2) = match hit_info.side {
                    Side::Back => (material.refraction_index, 1.0),
                    Side::Front => (1.0, material.refraction_index)
                };

                let refracted = if let Some(refracted_ray) = refract_ray(-dir, normal, n1, n2) {
                    material.refraction_value * material.refraction
                        .mul_element_wise(self.cast(hit_info.pos + BIAS * refracted_ray, refracted_ray, bounces - 1))
                } else {
                    Vector3::new(0.0, 0.0, 0.0)
                };

                let cos = match hit_info.side {
                    Side::Back => 0.0,
                    Side::Front => hit_info.normal.dot(-dir),
                };

                let r0 = ((n1 - n2) / (n1 + n2)).powi(2);
                let ar = r0 + (1.0 - r0) * (1.0 - normal.dot(-dir)).powi(1);

                // println!("{:?}", ar * material.reflection_value * material.reflection.mul_element_wise(reflected));

                color += ar * reflected +
                    (1.0 - ar) * refracted;
            }

            color
        } else {
            Vector3::new(0.0, 0.0, 0.0)
        }
    }

    pub fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<(HitInfo, &Node)> {
        let mut nearest: Option<(HitInfo, &Node)> = None;

        for node in self.nodes.iter() {
            if let Some((hit_info, node)) = node.intersect(pos, dir) {
                if let Some((nearest_hit_info, nearest_node)) = nearest {
                    nearest = if hit_info.z < nearest_hit_info.z {
                        Some((hit_info, node))
                    } else {
                        Some((nearest_hit_info, nearest_node))
                    };
                } else {
                    nearest = Some((hit_info, node));
                }
            }
        }

        nearest
    }
}

fn reflect_ray(vec: Vector3<f32>, normal: Vector3<f32>) -> Vector3<f32> {
    -vec + 2.0 * normal.dot(vec) * normal
}

fn refract_ray(vec: Vector3<f32>, normal: Vector3<f32>, n1: f32, n2: f32) -> Option<Vector3<f32>> {
    let n = n1 / n2;
    let normal_dot_vec = normal.dot(vec);
    let s = n * (normal_dot_vec * normal - vec);
    let cos_sqr = 1.0 - n.powi(2) * (1.0 - normal_dot_vec.powi(2));
    if cos_sqr >= 0.0 {
        Some(s - cos_sqr.sqrt() * normal)
    } else {
        None
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
                if let Some((nearest_hit_info, nearest_node)) = nearest {
                    nearest = if hit_info.z < nearest_hit_info.z {
                        Some((hit_info, node))
                    } else {
                        Some((nearest_hit_info, nearest_node))
                    };
                } else {
                    nearest = Some((hit_info, node));
                };
            }
        }

        /* transform hit info back out of local node space */
        nearest = nearest.map(|(hit_info, node)| {
            (HitInfo {
                z: hit_info.z,
                pos: self.from_local_space(hit_info.pos),
                normal: (self.transform.invert().unwrap().transpose() * hit_info.normal).normalize(),
                side: hit_info.side,
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
                        let t;
                        let side;
                        if t1 > 0.0 && t2 > 0.0 {
                            side = Side::Front;
                            t = t1.min(t2);
                        } else {
                            side = Side::Back;
                            t = t1.max(t2);
                        }

                        let hit_pos = pos + t * dir;
                        let normal = hit_pos.normalize();

                        Some(HitInfo {
                            z: t,
                            pos: hit_pos,
                            normal: normal,
                            side: side,
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
    pub fn shade(&self, pos: Vector3<f32>, dir: Vector3<f32>, hit_info: &HitInfo, lights: &Vec<&Light>) -> Color {
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
