extern crate cgmath;

use std::collections::HashMap;
use std::f32;
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
    Sphere,
    Plane,
    Mesh(Mesh),
}

#[derive(Debug)]
pub struct Mesh {
    pub vertices: Vec<Vector3<f32>>,
    pub triangles: Vec<(usize, usize, usize)>,
    pub normals: Vec<Vector3<f32>>,
    pub normal_triangles: Vec<(usize, usize, usize)>,
    pub texture_vertices: Vec<Vector3<f32>>,
    pub texture_triangles: Vec<(usize, usize, usize)>,
    pub bounding_box: BoundingBox,
}

#[derive(Debug, Clone, Copy)]
pub struct BoundingBox {
    pub p1: Vector3<f32>,
    pub p2: Vector3<f32>,
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
const EPSILON: f32 = 1.0e-8;

impl Scene {
    pub fn cast(&self, pos: Vector3<f32>, dir: Vector3<f32>, bounces: i32) -> Color {
        let (color, distance) = self.cast_distance(pos, dir, bounces);
        color
    }

    pub fn cast_distance(&self, pos: Vector3<f32>, dir: Vector3<f32>, bounces: i32) -> (Color, f32) {
        if let Some((hit_info, node)) = self.intersect(pos, dir) {
            let material = self.materials.get(&node.object.as_ref().unwrap().material[..])
                .expect("material does not exist for object");

            (self.shade(pos, dir, &hit_info, material, bounces), hit_info.z)
        } else {
            (Vector3::new(0.0, 0.0, 0.0), 0.0)
        }
    }

    pub fn shade(&self, pos: Vector3<f32>, dir: Vector3<f32>, hit_info: &HitInfo, material: &Material, bounces: i32) -> Color {
        /* check if lights are blocked (meaning we're in shadow) */
        let mut color = self.lights.iter().filter(|light| {
            match light.light_type {
                LightType::Ambient => true,
                LightType::Directional(dir) => {
                    let ray = -dir;
                    self.intersect(hit_info.pos + BIAS * ray, ray).is_none()
                },
                LightType::Point(pos) => {
                    let ray = (pos - hit_info.pos).normalize();
                    match self.intersect(hit_info.pos + BIAS * ray, ray) {
                        Some((blocking_hit_info, node)) => blocking_hit_info.z > (pos - hit_info.pos).magnitude(),
                        None => true
                    }
                },
            }
        }).map(|light| {
            let l: Vector3<f32>;
            match light.light_type {
                LightType::Ambient => {
                    return light.intensity * light.color.mul_element_wise(material.diffuse);
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

            let diffuse = material.diffuse;
            let specular = material.specular_value * n_dot_h.powf(material.glossiness) * material.specular;

            (light.intensity * n_dot_l * light.color).mul_element_wise(diffuse + specular)
        }).sum();

        /* reflection and refraction */
        if (material.reflection_value > 0.0 || material.refraction_value > 0.0) && bounces > 0 {
            let normal = match hit_info.side {
                Side::Back => -hit_info.normal,
                Side::Front => hit_info.normal,
            };

            let reflected_ray = reflect_ray(-dir, normal);
            let reflected = self.cast(hit_info.pos + BIAS * reflected_ray, reflected_ray, bounces - 1);

            let (n1, n2) = match hit_info.side {
                Side::Back => (material.refraction_index, 1.0),
                Side::Front => (1.0, material.refraction_index)
            };

            /* Schlick's approximation */
            let r0 = ((n1 - n2) / (n1 + n2)).powi(2);
            let mut ar = r0 + (1.0 - r0) * (1.0 - normal.dot(-dir)).powi(5);

            let refracted = if let Some(refracted_ray) = refract_ray(-dir, normal, n1, n2) {
                let (color, distance) = self.cast_distance(hit_info.pos + BIAS * refracted_ray, refracted_ray, bounces - 1);
                let mut absorb = -distance * material.absorption;
                absorb.x = absorb.x.exp();
                absorb.y = absorb.y.exp();
                absorb.z = absorb.z.exp();
                color.mul_element_wise(absorb)
            } else {
                ar = 1.0;
                Vector3::new(0.0, 0.0, 0.0)
            };

            color = color +
                (material.reflection_value + ar * material.refraction_value) * reflected.mul_element_wise(material.reflection) +
                (1.0 - ar) * material.refraction_value * refracted.mul_element_wise(material.refraction);
        }

        color

        // hit_info.normal / 2.0 + Vector3::new(0.5, 0.5, 0.5)
        // (1.0 - ((hit_info.z - 20.0) / 70.0)) * Vector3::new(1.0, 1.0, 1.0)
        // (1.0 - ((hit_info.pos - pos).magnitude() / 100.0)) * Vector3::new(1.0, 1.0, 1.0)
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
        if !self.bounding_box().intersect(pos, dir) {
            return None;
        }

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
            },
            Geometry::Plane => {
                let t = -(pos.z / dir.z);
                if t > 0.0 {
                    let p = pos + t * dir;
                    if -1.0 < p.x && p.x < 1.0 && -1.0 < p.y && p.y < 1.0 {
                        Some(HitInfo {
                            z: t,
                            pos: p,
                            normal: Vector3::new(0.0, 0.0, 1.0),
                            side: if pos.z > 0.0 { Side::Front } else { Side::Back },
                        })
                    } else {
                        None
                    }
                } else {
                    None
                }
            },
            Geometry::Mesh(ref mesh) => {
                mesh.intersect(pos, dir)
            }
        }
    }

    fn bounding_box(&self) -> BoundingBox {
        match *self {
            Geometry::Sphere => {
                BoundingBox::new(-1.0, -1.0, -1.0, 1.0, 1.0, 1.0)
            },
            Geometry::Plane => {
                BoundingBox::new(-1.0, -1.0, 0.0, 1.0, 1.0, 0.0)
            },
            Geometry::Mesh(ref mesh) => {
                mesh.bounding_box
            }
        }
    }
}

impl Mesh {
    fn get_point(&self, face: usize, u: f32, v: f32) -> Vector3<f32> {
        let points = self.triangles[face];
        (1.0 - u - v) * self.vertices[points.0] + u * self.vertices[points.1] + v * self.vertices[points.2]
    }

    fn get_normal(&self, face: usize, u: f32, v: f32) -> Vector3<f32> {
        let points = self.normal_triangles[face];
        (1.0 - u - v) * self.normals[points.0] + u * self.normals[points.1] + v * self.normals[points.2]
    }

    fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<HitInfo> {
        let mut nearest: Option<HitInfo> = None;

        for (i, triangle) in self.triangles.iter().enumerate() {
            let a = self.vertices[triangle.0];
            let b = self.vertices[triangle.1];
            let c = self.vertices[triangle.2];

            let ab = b - a;
            let ac = c - a;

            let pvec = dir.cross(ac);
            let det = ab.dot(pvec);

            if det.abs() < EPSILON {
                continue;
            }

            let tvec = pos - a;

            let u = tvec.dot(pvec) / det;
            if u < 0.0 || u > 1.0 {
                continue;
            }

            let qvec = tvec.cross(ab);

            let v = dir.dot(qvec) / det;
            if v < 0.0 || u + v > 1.0 {
                continue;
            }

            let t = ac.dot(qvec) / det;
            if t < 0.0 {
                continue;
            }

            /* at this point we know we are intersecting */
            if let Some(ref nearest_hit_info) = nearest {
                if nearest_hit_info.z < t {
                    continue;
                }
            }

            nearest = Some(HitInfo {
                z: t,
                pos: self.get_point(i, u, v),
                normal: self.get_normal(i, u, v),
                side: if det > 0.0 { Side::Front } else { Side::Back },
            })
        }

        nearest
    }
}

impl BoundingBox {
    fn new(x1: f32, y1: f32, z1: f32, x2: f32, y2: f32, z2: f32) -> BoundingBox {
        BoundingBox {
            p1: Vector3::new(x1, y1, z1),
            p2: Vector3::new(x2, y2, z2),
        }
    }

    fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> bool {
        let mut in_x = f32::NEG_INFINITY;
        let mut out_x = f32::INFINITY;
        if dir.x == 0.0 {
            if pos.x < self.p1.x || pos.x > self.p2.x {
                return false;
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
                return false;
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
                return false;
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

        in_x.max(in_y).max(in_z) <= out_x.min(out_y).min(out_z)
    }
}

pub fn color_as_u8_array(color: Color) -> [u8; 4] {
    [(color.x * 255.0).max(0.0).min(255.0) as u8,
     (color.y * 255.0).max(0.0).min(255.0) as u8,
     (color.z * 255.0).max(0.0).min(255.0) as u8,
     255]
}
