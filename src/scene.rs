extern crate cgmath;

use std::collections::HashMap;
use std::f32::consts;
use self::cgmath::{Vector3, Matrix3, SquareMatrix, InnerSpace, Matrix, ElementWise, Zero, One};
use geometry::*;

#[derive(Debug)]
pub struct Scene {
    pub nodes: Vec<Node>,
    pub materials: HashMap<String, Material>,
    pub lights: Vec<Light>,
    pub background: Texture,
    pub environment: Texture,
}

#[derive(Debug)]
pub struct Node {
    pub object: Option<Object>,
    pub transform: Transform,
    pub children: Vec<Node>,
    pub name: String,
}

#[derive(Debug)]
pub struct Object {
    pub geometry: Geometry,
    pub material: String,
}

#[derive(Debug)]
pub struct Material {
    pub diffuse: Texture,
    pub specular: Texture,
    pub glossiness: f32,
    pub reflection: Texture,
    pub refraction: Texture,
    pub refraction_index: f32,
    pub absorption: Color,
}

#[derive(Debug)]
pub struct Texture {
    pub data: TextureData,
    pub color: Color,
    pub transform: Transform,
}

#[derive(Debug)]
pub enum TextureData {
    Blank,
    Image { pixels: Vec<u8>, width: usize, height: usize },
    Checkerboard { color1: Color, color2: Color },
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
    pub uv: Vector3<f32>,
    pub normal: Vector3<f32>,
    pub side: Side,
}

#[derive(PartialEq)]
pub enum Side {
    Back,
    Front,
}

#[derive(Debug)]
pub struct Transform {
    pub transform: Matrix3<f32>,
    pub translate: Vector3<f32>,
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

pub const BIAS: f32 = 0.1;
pub const EPSILON: f32 = 1.0e-8;

impl Scene {
    pub fn cast(&self, pos: Vector3<f32>, dir: Vector3<f32>, x: f32, y: f32, bounces: i32) -> Color {
        if let Some((color, _)) = self.cast_distance(pos, dir, bounces) {
            color
        } else {
            self.background.sample(Vector3::new(x, y, 0.0))
        }
    }

    pub fn cast_distance(&self, pos: Vector3<f32>, dir: Vector3<f32>, bounces: i32) -> Option<(Color, f32)> {
        if let Some((hit_info, node)) = self.intersect(pos, dir) {
            let material = self.materials.get(&node.object.as_ref().unwrap().material[..])
                .expect("material does not exist for object");

            Some((self.shade(pos, dir, &hit_info, material, bounces), hit_info.z))
        } else {
            None
        }
    }

    pub fn shade(&self, pos: Vector3<f32>, dir: Vector3<f32>, hit_info: &HitInfo, material: &Material, bounces: i32) -> Color {
        let diffuse = material.diffuse.sample(hit_info.uv);
        let specular = material.specular.sample(hit_info.uv);
        let reflection = material.reflection.sample(hit_info.uv);
        let refraction = material.refraction.sample(hit_info.uv);

        let mut diffuse_color = Vector3::zero();
        let mut specular_color = Vector3::zero();

        /* check if lights are blocked (meaning we're in shadow) */
        let visible_lights = self.lights.iter().filter(|light| {
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
        });

        for light in visible_lights {
            let l: Vector3<f32>;
            match light.light_type {
                LightType::Ambient => {
                    diffuse_color += light.intensity * light.color.mul_element_wise(diffuse);
                    continue;
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

            let specular = n_dot_h.powf(material.glossiness) * specular;

            let light_color = light.intensity * n_dot_l * light.color;

            diffuse_color += light_color.mul_element_wise(diffuse);
            specular_color += light_color.mul_element_wise(specular);
        }

        /* reflection and refraction */
        if (!reflection.is_zero() || !refraction.is_zero()) && bounces > 0 {
            let normal = match hit_info.side {
                Side::Back => -hit_info.normal,
                Side::Front => hit_info.normal,
            };

            let reflected_ray = reflect_ray(-dir, normal);
            let reflected = if let Some((color, _)) = self.cast_distance(hit_info.pos + BIAS * reflected_ray, reflected_ray, bounces - 1) {
                color
            } else {
                self.environment.sample_environment(reflected_ray)
            };

            let (n1, n2) = match hit_info.side {
                Side::Back => (material.refraction_index, 1.0),
                Side::Front => (1.0, material.refraction_index)
            };

            /* Schlick's approximation */
            let r0 = ((n1 - n2) / (n1 + n2)).powi(2);
            let mut ar = r0 + (1.0 - r0) * (1.0 - normal.dot(-dir)).powi(5);

            let refracted = if let Some(refracted_ray) = refract_ray(-dir, normal, n1, n2) {
                if let Some((color, distance)) = self.cast_distance(hit_info.pos + BIAS * refracted_ray, refracted_ray, bounces - 1) {
                    if hit_info.side == Side::Front {
                        let mut absorb = -distance * material.absorption;
                        absorb.x = absorb.x.exp();
                        absorb.y = absorb.y.exp();
                        absorb.z = absorb.z.exp();
                        color.mul_element_wise(absorb)
                    } else {
                        color
                    }
                } else {
                    self.environment.sample_environment(refracted_ray.normalize())
                }
            } else {
                ar = 1.0;
                Vector3::new(0.0, 0.0, 0.0)
            };

            diffuse_color + specular_color + (1.0 + ar) * reflected.mul_element_wise(reflection) + (1.0 - ar) * refracted.mul_element_wise(refraction)
        } else {
            diffuse_color + specular_color
        }

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

    fn to_local_space(&self, vec: Vector3<f32>) -> Vector3<f32> {
        self.transform.to_local_space(vec)
    }

    fn from_local_space(&self, vec: Vector3<f32>) -> Vector3<f32> {
        self.transform.from_local_space(vec)
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
                uv: hit_info.uv,
                normal: self.transform.normal_from_local_space(hit_info.normal),
                side: hit_info.side,
            }, node)
        });

        nearest
    }
}

impl Transform {
    fn to_local_space(&self, vec: Vector3<f32>) -> Vector3<f32> {
        self.transform.invert().unwrap() * (vec - self.translate)
    }

    fn from_local_space(&self, vec: Vector3<f32>) -> Vector3<f32> {
        self.transform * vec + self.translate
    }

    fn normal_from_local_space(&self, normal: Vector3<f32>) -> Vector3<f32> {
        (self.transform.invert().unwrap().transpose() * normal).normalize()
    }

    pub fn default() -> Transform {
        Transform {
            transform: Matrix3::one(),
            translate: Vector3::zero(),
        }
    }
}

impl Texture {
    fn sample(&self, point: Vector3<f32>) -> Color {
        self.color.mul_element_wise(self.data.sample(self.to_local_space(point)))
    }

    fn sample_environment(&self, dir: Vector3<f32>) -> Color {
        // let z = (-dir.z).asin() / consts::PI + 0.5;
        // let mut y = dir.x;
        // let mut x = dir.y;
        // let x_plus_y = dir.x.abs() + dir.y.abs();
        // if x_plus_y > 0.0 {
        //     x /= x_plus_y;
        //     y /= x_plus_y;
        // }
        // self.sample(Vector3::new(0.5, 0.5, 0.0) + z * (x * Vector3::new(-0.5, 0.5, 0.0) + y * Vector3::new(0.5, 0.5, 0.0)))
        self.sample(Vector3::new(0.5 + (dir.y).atan2(dir.x) / (2.0 * consts::PI), 0.5 - (-dir.z).asin() / consts::PI, 0.0))
    }

    fn to_local_space(&self, vec: Vector3<f32>) -> Vector3<f32> {
        self.transform.to_local_space(vec)
    }
}

impl TextureData {
    fn sample(&self, point: Vector3<f32>) -> Color {
        match *self {
            TextureData::Blank => Vector3::new(1.0, 1.0, 1.0),
            TextureData::Image { ref pixels, width, height } => {
                if width + height == 0 {
                    return Vector3::new(0.0, 0.0, 0.0);
                }

                let clamped = unit_clamp(point);

                let x = clamped.x * width as f32;
                let y = clamped.y * height as f32;
                let mut x1 = x as usize;
                let mut y1 = y as usize;
                let xt = x - x1 as f32;
                let yt = y - y1 as f32;

                x1 = x1 % width;
                // if x1 < 0 { x1 += width; }
                y1 = y1 % height;
                // if y1 < 0 { y1 += height; }
                let x2 = (x1 + 1) % width;
                let y2 = (y1 + 1) % height;

                let i00 = 3 * (y1 * width + x1);
                let i10 = 3 * (y1 * width + x2);
                let i01 = 3 * (y2 * width + x1);
                let i11 = 3 * (y2 * width + x2);
                let p00 = u8_array_as_color(&pixels[i00..i00+3]);
                let p10 = u8_array_as_color(&pixels[i10..i10+3]);
                let p01 = u8_array_as_color(&pixels[i01..i01+3]);
                let p11 = u8_array_as_color(&pixels[i11..i11+3]);

                (1.0-xt)*(1.0-yt) as f32 * p00 + xt*(1.0-yt) as f32 * p10 + (1.0-xt)*yt as f32 * p01 + xt*yt as f32 * p11
            },
            TextureData::Checkerboard { color1, color2 } => {
                let clamped = unit_clamp(point);
                if (clamped.x < 0.5 && clamped.y < 0.5) || (clamped.x > 0.5 && clamped.y > 0.5) {
                    color1
                } else {
                    color2
                }
            }
        }
    }
}

pub fn color_as_u8_array(color: Color) -> [u8; 4] {
    [(color.x * 255.0).max(0.0).min(255.0) as u8,
     (color.y * 255.0).max(0.0).min(255.0) as u8,
     (color.z * 255.0).max(0.0).min(255.0) as u8,
     255]
}

pub fn u8_array_as_color(color: &[u8]) -> Color {
    // let alpha = (color[3] as f32 / 255.0).max(0.0).min(1.0);
    Vector3::new(
        /*alpha * */(color[0] as f32 / 255.0).max(0.0).min(1.0),
        /*alpha * */(color[1] as f32 / 255.0).max(0.0).min(1.0),
        /*alpha * */(color[2] as f32 / 255.0).max(0.0).min(1.0),
    )
}

pub fn unit_clamp(point: Vector3<f32>) -> Vector3<f32> {
    let mut x = point.x - (point.x as i32) as f32;
    if x < 0.0 {
        x += 1.0;
    }
    let mut y = point.y - (point.y as i32) as f32;
    if y < 0.0 {
        y += 1.0;
    }
    let mut z = point.z - (point.z as i32) as f32;
    if z < 0.0 {
        z += 1.0;
    }
    Vector3::new(x, y, z)
}

pub fn halton(index: i32, base: i32) -> f32 {
    let mut r = 0.0;
    let mut f = 1.0;

    let mut i = index;
    while i > 0 {
        f /= base as f32;
        r += f * (i % base) as f32;
        i /= base;
    }

    r
}
