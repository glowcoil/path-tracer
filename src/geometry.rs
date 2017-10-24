extern crate cgmath;

use scene::*;
use bvh::*;

use self::cgmath::{Vector3, InnerSpace};

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
    pub bvh: BVH,
}

impl Geometry {
    pub fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<HitInfo> {
        if self.bounding_box().intersect(pos, dir).is_none() {
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
}

impl Bounded for Geometry {
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

        for i in self.bvh.traverse(pos, dir) {
            let triangle = self.triangles[i];
            let a = self.vertices[triangle.0];
            let b = self.vertices[triangle.1];
            let c = self.vertices[triangle.2];

            if let Some((t, u, v, side)) = Self::intersect_triangle(a, b, c, pos, dir) {
                if let Some(ref nearest_hit_info) = nearest {
                    if nearest_hit_info.z < t {
                        continue;
                    }
                }
                nearest = Some(HitInfo {
                    z: t,
                    pos: self.get_point(i, u, v),
                    normal: self.get_normal(i, u, v),
                    side: side,
                })
            }
        }

        nearest
    }

    // fn intersect(&self, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<HitInfo> {
    //     let mut nearest: Option<HitInfo> = None;

    //     for (i, triangle) in self.triangles.iter().enumerate() {
    //         let a = self.vertices[triangle.0];
    //         let b = self.vertices[triangle.1];
    //         let c = self.vertices[triangle.2];

    //         if let Some((t, u, v, side)) = Self::intersect_triangle(a, b, c, pos, dir) {
    //             if let Some(ref nearest_hit_info) = nearest {
    //                 if nearest_hit_info.z < t {
    //                     continue;
    //                 }
    //             }

    //             nearest = Some(HitInfo {
    //                 z: t,
    //                 pos: self.get_point(i, u, v),
    //                 normal: self.get_normal(i, u, v),
    //                 side: side,
    //             })
    //         }
    //     }

    //     nearest
    // }

    /* returns t, u, v, side */
    fn intersect_triangle(a: Vector3<f32>, b: Vector3<f32>, c: Vector3<f32>, pos: Vector3<f32>, dir: Vector3<f32>) -> Option<(f32, f32, f32, Side)> {
        let ab = b - a;
        let ac = c - a;

        let pvec = dir.cross(ac);
        let det = ab.dot(pvec);

        if det.abs() < EPSILON {
            return None;
        }

        let tvec = pos - a;

        let u = tvec.dot(pvec) / det;
        if u < 0.0 || u > 1.0 {
            return None;
        }

        let qvec = tvec.cross(ab);

        let v = dir.dot(qvec) / det;
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        let t = ac.dot(qvec) / det;
        if t < 0.0 {
            return None;
        }

        let side = if det > 0.0 { Side::Front } else { Side::Back };

        Some((t, u, v, side))
    }

    pub fn build_bvh(vertices: &[Vector3<f32>], triangles: &[(usize, usize, usize)], bounding_box: BoundingBox) -> BVH {
        let mut boxes = Vec::with_capacity(triangles.len());

        for triangle in triangles.iter() {
            let a = vertices[triangle.0];
            let b = vertices[triangle.1];
            let c = vertices[triangle.2];

            let mut bounding_box = BoundingBox { p1: a, p2: a };
            bounding_box.union(&BoundingBox { p1: b, p2: b });
            bounding_box.union(&BoundingBox { p1: c, p2: c });

            boxes.push(bounding_box);
        }

        BVH::build(bounding_box, &boxes)
    }
}
