extern crate cgmath;
extern crate rand;

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
    pub emission: Color,
    pub reflection: Texture,
    pub reflection_glossiness: f32,
    pub refraction: Texture,
    pub refraction_glossiness: f32,
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
    Point { position: Vector3<f32>, size: f32 },
}

pub struct Camera {
    pub pos: Vector3<f32>,
    pub dir: Vector3<f32>,
    pub up: Vector3<f32>,
    pub fov: f32,
    pub img_width: u32,
    pub img_height: u32,
    pub focaldist: f32,
    pub dof: f32,
}

pub struct HitInfo {
    pub z: f32,
    pub pos: Vector3<f32>,
    pub uv: Vector3<f32>,
    pub normal: Vector3<f32>,
    pub side: Side,
}

#[derive(PartialEq, Debug)]
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
            focaldist: 1.0,
            dof: 0.0,
        }
    }
}

pub const BIAS: f32 = 0.01;
pub const EPSILON: f32 = 1.0e-8;

impl Scene {
    pub fn sample(&self, pos: Vector3<f32>, dir: Vector3<f32>, x: f32, y: f32) -> Color {
        self.cast(pos, dir, 1.0).unwrap_or_else(|| self.background.sample(Vector3::new(x, y, 0.0)))
    }

    pub fn cast(&self, pos: Vector3<f32>, dir: Vector3<f32>, weight: f32) -> Option<Color> {
        let result = self.intersect(pos, dir).map(|(hit_info, node)| {
            let material = self.materials.get(&node.object.as_ref().unwrap().material[..])
                .expect("material does not exist for object");

            let diffuse = material.diffuse.sample(hit_info.uv);
            let reflection = material.reflection.sample(hit_info.uv);
            let refraction = material.refraction.sample(hit_info.uv);

            let normal = match hit_info.side {
                Side::Back => -hit_info.normal,
                Side::Front => hit_info.normal,
            };

            /* Schlick's approximation for Fresnel reflectance */
            let (n1, n2) = match hit_info.side {
                Side::Back => (material.refraction_index, 1.0),
                Side::Front => (1.0, material.refraction_index)
            };
            let r0 = ((n1 - n2) / (n1 + n2)).powi(2);
            let ar = r0 + (1.0 - r0) * (1.0 - normal.dot(-dir)).powi(5);

            let p_diffuse = (diffuse.x + diffuse.y + diffuse.z) / 3.0;
            let p_reflection = (1.0 + ar) * (reflection.x + reflection.y + reflection.z) / 3.0;
            let p_refraction = (1.0 - ar) * (refraction.x + refraction.y + refraction.z) / 3.0;

            let p_range = p_diffuse + p_reflection + p_refraction;

            let mut color = material.emission;

            /* Russian Roulette */
            if p_range == 0.0 || rand::random::<f32>() > weight {
                return color;
            }

            let rnd = rand::random::<f32>() * p_range;
            if rnd < p_diffuse {
                let new_dir = random_rotation(normal, consts::PI / 2.0);
                color += normal.dot(new_dir) * diffuse.mul_element_wise(self.cast(hit_info.pos + BIAS * new_dir, new_dir, weight * p_diffuse)
                    .unwrap_or_else(|| self.environment.sample_environment(new_dir))) / p_diffuse;
            } else if rnd < p_diffuse + p_reflection {
                let new_dir = random_rotation(reflect_ray(-dir, normal), material.reflection_glossiness);
                color += normal.dot(new_dir) * reflection.mul_element_wise(self.cast(hit_info.pos + BIAS * new_dir, new_dir, weight * p_reflection)
                    .unwrap_or_else(|| self.environment.sample_environment(new_dir))) / p_reflection;
            } else if rnd < p_diffuse + p_reflection + p_refraction {
                let new_dir = random_rotation(refract_ray(-dir, normal, n1, n2).unwrap_or_else(|| reflect_ray(-dir, normal)), material.refraction_glossiness);
                color += normal.dot(new_dir) * refraction.mul_element_wise(self.cast(hit_info.pos + BIAS * new_dir, new_dir, weight * p_refraction)
                    .unwrap_or_else(|| self.environment.sample_environment(new_dir))) / p_refraction;
            }

            color
        });

        result
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
    (-vec + 2.0 * normal.dot(vec) * normal).normalize()
}

fn refract_ray(vec: Vector3<f32>, normal: Vector3<f32>, n1: f32, n2: f32) -> Option<Vector3<f32>> {
    let n = n1 / n2;
    let normal_dot_vec = normal.dot(vec);
    let s = n * (normal_dot_vec * normal - vec);
    let cos_sqr = 1.0 - n.powi(2) * (1.0 - normal_dot_vec.powi(2));
    if cos_sqr >= 0.0 {
        Some((s - cos_sqr.sqrt() * normal).normalize())
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

const GAMMA: f32 = 1.0/2.2;

pub fn color_as_u8_array(color: Color) -> [u8; 4] {
    [(color.x.powf(GAMMA) * 255.0).max(0.0).min(255.0) as u8,
     (color.y.powf(GAMMA) * 255.0).max(0.0).min(255.0) as u8,
     (color.z.powf(GAMMA) * 255.0).max(0.0).min(255.0) as u8,
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

fn random_rotation(vec: Vector3<f32>, max_angle: f32) -> Vector3<f32> {
    let x_abs = vec.x.abs(); let y_abs = vec.y.abs(); let z_abs = vec.z.abs();
    let smallest_axis = if x_abs < y_abs && x_abs < z_abs {
        Vector3::unit_x()
    } else if y_abs < z_abs {
        Vector3::unit_y()
    } else {
        Vector3::unit_z()
    };
    let u = vec.cross(smallest_axis).normalize();
    let v = vec.cross(u).normalize();

    let z_min = max_angle.cos();
    let z = z_min + rand::random::<f32>() * (1.0 - z_min);
    let theta = rand::random::<f32>() * 2.0 * consts::PI;
    let output = vec * z + z.asin().cos() * (theta.cos() * u + theta.sin() * v);
    output.normalize()
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
