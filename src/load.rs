extern crate xmltree;
extern crate cgmath;
extern crate wavefront_obj;
extern crate png;

use scene::*;
use geometry::*;
use bvh::*;

use std::fs::File;
use std::io::prelude::*;
use std::collections::HashMap;
use self::xmltree::Element;
use self::cgmath::{Vector3, Matrix3, SquareMatrix, InnerSpace, One, Deg};
use self::wavefront_obj::obj;

pub fn load_scene(filename: &str) -> (Scene, Camera) {
    let mut f = File::open(filename).expect("file not found");
    let mut contents = String::new();
    f.read_to_string(&mut contents).expect("could not read file");

    let xml = Element::parse(contents.as_bytes()).expect("could not parse xml");

    let scene_xml = xml.get_child("scene").expect("no <scene> tag found");

    let mut nodes = Vec::new();
    let mut materials = HashMap::new();
    let mut lights = Vec::new();

    let mut background = Texture {
        data: TextureData::Blank,
        color: Vector3::new(0.0, 0.0, 0.0),
        transform: Transform::default(),
    };
    if let Some(background_xml) = scene_xml.get_child("background") {
        background = load_texture(background_xml, Vector3::new(1.0, 1.0, 1.0));
    }

    let mut environment = Texture {
        data: TextureData::Blank,
        color: Vector3::new(0.0, 0.0, 0.0),
        transform: Transform::default(),
    };
    if let Some(environment_xml) = scene_xml.get_child("environment") {
        environment = load_texture(environment_xml, Vector3::new(1.0, 1.0, 1.0));
    }

    for child in &scene_xml.children {
        match child.name.as_ref() {
            "object" => {
                nodes.push(load_node(child));
            },
            "material" => {
                let (name, material) = load_material(child);
                materials.insert(name, material);
            },
            "light" => {
                lights.push(load_light(child));
            }
            _ => {}
        }
    }

    let scene = Scene {
        nodes: nodes,
        materials: materials,
        lights: lights,
        background: background,
        environment: environment,
    };

    let camera_xml = xml.get_child("camera").expect("no <camera> tag found");
    let camera = load_camera(camera_xml);

    (scene, camera)
}

fn load_node(node_xml: &Element) -> Node {
    let object = node_xml.attributes.get("type").map(|object_type| {
        Object {
            geometry: match object_type.as_ref() {
                "sphere" => {
                    Geometry::Sphere
                },
                "plane" => {
                    Geometry::Plane
                },
                "obj" => {
                    load_obj(node_xml.attributes.get("name").expect("no filename given for obj"))
                }
                _ => {
                    panic!("unknown object type");
                },
            },
            material: node_xml.attributes.get("material").expect("no material given for object").clone(),
        }
    });

    let name = node_xml.attributes.get("name").map(|name| name.clone()).unwrap_or("".to_string());

    let mut children: Vec<Node> = Vec::new();
    for child in &node_xml.children {
        if child.name == "object" {
            children.push(load_node(child));
        }
    }

    let transform = load_transform(node_xml);

    Node {
        object: object,
        transform: transform,
        children: children,
        name: name,
    }
}

fn load_material(material_xml: &Element) -> (String, Material) {
    let material_type = material_xml.attributes.get("type").expect("no type for material");
    let name = material_xml.attributes.get("name").expect("no name for material");

    match material_type.as_ref() {
        "blinn" => {
            let diffuse_xml = material_xml.get_child("diffuse").expect("no diffuse found for <material>");
            let diffuse = load_texture(diffuse_xml, Vector3::new(1.0, 1.0, 1.0));

            let specular_xml = material_xml.get_child("specular").expect("no specular found for <material>");
            let specular = load_texture(specular_xml, Vector3::new(0.7, 0.7, 0.7));

            let glossiness = if let Some(glossiness_xml) = material_xml.get_child("glossiness") {
                glossiness_xml.attributes
                    .get("value").expect("no value found for <glossiness>")
                    .parse().expect("could not parse glossiness value")
            } else {
                20.0
            };

            let mut reflection = Texture {
                data: TextureData::Blank,
                color: Vector3::new(0.0, 0.0, 0.0),
                transform: Transform::default(),
            };
            let mut reflection_glossiness = 0.0;
            if let Some(reflection_xml) = material_xml.get_child("reflection") {
                reflection = load_texture(reflection_xml, Vector3::new(1.0, 1.0, 1.0));
                reflection_glossiness = reflection_xml.attributes.get("glossiness")
                    .and_then(|s| s.parse().ok()).unwrap_or(reflection_glossiness);
            }

            let mut refraction = Texture {
                data: TextureData::Blank,
                color: Vector3::new(0.0, 0.0, 0.0),
                transform: Transform::default(),
            };
            let mut refraction_index = 1.0;
            let mut refraction_glossiness = 0.0;
            if let Some(refraction_xml) = material_xml.get_child("refraction") {
                refraction = load_texture(refraction_xml, Vector3::new(1.0, 1.0, 1.0));
                refraction_index = refraction_xml.attributes.get("index")
                    .and_then(|s| s.parse().ok()).unwrap_or(refraction_index);
                refraction_glossiness = refraction_xml.attributes.get("glossiness")
                    .and_then(|s| s.parse().ok()).unwrap_or(refraction_glossiness);
            }

            let absorption = material_xml.get_child("absorption").and_then(|absorption_xml| {
                read_color(&absorption_xml.attributes)
            }).unwrap_or(Vector3::new(0.0, 0.0, 0.0));

            (name.clone(), Material {
                diffuse: diffuse,
                specular: specular,
                glossiness: glossiness,
                reflection: reflection,
                reflection_glossiness: reflection_glossiness,
                refraction: refraction,
                refraction_glossiness: refraction_glossiness,
                refraction_index: refraction_index,
                absorption: absorption,
            })
        },
        _ => {
            panic!("unknown material type");
        }
    }
}

fn load_light(light_xml: &Element) -> Light {
    let light_type = light_xml.attributes.get("type").expect("no type for light");

    let intensity_xml = light_xml.get_child("intensity").expect("no intensity given for light");
    let intensity = intensity_xml.attributes.get("value").expect("no value for light intensity")
        .parse().expect("could not parse light intensity value");
    let color = read_color(&intensity_xml.attributes).unwrap_or(Vector3::new(1.0, 1.0, 1.0));

    let light_type = match light_type.as_ref() {
        "ambient" => {
            LightType::Ambient
        },
        "direct" => {
            LightType::Directional(read_vector3(&light_xml.get_child("direction")
                .expect("no direction given for directional light").attributes)
                .normalize())
        },
        "point" => {
            let position = read_vector3(&light_xml.get_child("position")
                .expect("no position given for positional light").attributes);
            let size = light_xml.get_child("size")
                .and_then(|size_xml| size_xml.attributes.get("value"))
                .and_then(|size| size.parse().ok()).unwrap_or(0.0);
            LightType::Point { position: position, size: size }
        },
        _ => {
            panic!("unknown light type");
        }
    };

    Light {
        intensity: intensity,
        color: color,
        light_type: light_type,
    }
}

fn load_camera(camera_xml: &Element) -> Camera {
    let mut camera: Camera = Default::default();

    camera.pos = read_vector3_default(&camera_xml.get_child("position").expect("no <position> tag found in <camera>").attributes, camera.pos);
    camera.dir = (read_vector3_default(&camera_xml.get_child("target").expect("no <target> tag found in <camera>").attributes, camera.pos + camera.dir)
        - camera.pos).normalize();
    camera.up = read_vector3_default(&camera_xml.get_child("up").expect("no <up> tag found in <camera>").attributes, camera.up);
    camera.fov = camera_xml.get_child("fov").expect("no <fov> tag found in <camera>")
        .attributes.get("value").expect("no value attribute found on <fov> tag")
        .parse().expect("could not parse camera fov");
    camera.img_width = camera_xml.get_child("width").expect("no <width> tag found in <camera>")
        .attributes.get("value").expect("no value attribute found on <width> tag")
        .parse().expect("could not parse camera width");
    camera.img_height = camera_xml.get_child("height").expect("no <height> tag found in <camera>")
        .attributes.get("value").expect("no value attribute found on <height> tag")
        .parse().expect("could not parse camera height");
    camera.focaldist = camera_xml.get_child("focaldist")
        .and_then(|focaldist_xml| focaldist_xml.attributes.get("value"))
        .and_then(|focaldist| focaldist.parse().ok()).unwrap_or(camera.focaldist);
    camera.dof = camera_xml.get_child("dof")
        .and_then(|dof_xml| dof_xml.attributes.get("value"))
        .and_then(|dof| dof.parse().ok()).unwrap_or(camera.dof);

    /* make sure camera.up is orthogonal to camera.dir */
    camera.up = (camera.dir.cross(camera.up)).cross(camera.dir);

    camera
}

fn load_obj(filename: &str) -> Geometry {
    let mut f = File::open(filename).expect("file not found");
    let mut contents = String::new();
    f.read_to_string(&mut contents).expect("could not read file");

    match obj::parse(contents) {
        Ok(obj_set) => {
            if obj_set.objects.len() < 1 {
                panic!("no objects found in file");
            } else {
                let object = &obj_set.objects[0];

                let vertices: Vec<Vector3<f32>> = object.vertices.iter().map(|v| Vector3::new(v.x as f32, v.y as f32, v.z as f32)).collect();

                let mut p1 = vertices[0];
                let mut p2 = vertices[0];
                for vertex in &vertices {
                    if vertex.x < p1.x { p1.x = vertex.x; }
                    if vertex.y < p1.y { p1.y = vertex.y; }
                    if vertex.z < p1.z { p1.z = vertex.z; }
                    if vertex.x > p2.x { p2.x = vertex.x; }
                    if vertex.y > p2.y { p2.y = vertex.y; }
                    if vertex.z > p2.z { p2.z = vertex.z; }
                }

                let mut triangles = Vec::new();
                let mut normal_triangles = Vec::new();
                let mut texture_triangles = Vec::new();

                for geometry in &object.geometry {
                    for shape in &geometry.shapes {
                        if let obj::Primitive::Triangle(v1, v2, v3) = shape.primitive {
                            triangles.push((v1.0, v2.0, v3.0));
                            texture_triangles.push((v1.1.unwrap(), v2.1.unwrap(), v3.1.unwrap()));
                            normal_triangles.push((v1.2.unwrap(), v2.2.unwrap(), v3.2.unwrap()));
                        }
                    }
                }

                let bounding_box = BoundingBox { p1: p1, p2: p2 };
                let bvh = Mesh::build_bvh(&vertices, &triangles, bounding_box);

                Geometry::Mesh(Mesh {
                    vertices: vertices,
                    triangles: triangles,
                    normals: object.normals.iter().map(|v| Vector3::new(v.x as f32, v.y as f32, v.z as f32)).collect(),
                    normal_triangles: normal_triangles,
                    texture_vertices: object.tex_vertices.iter().map(|v| Vector3::new(v.u as f32, v.v as f32, v.w as f32)).collect(),
                    texture_triangles: texture_triangles,
                    bounding_box: bounding_box,
                    bvh: bvh,
                })
            }
        },
        Err(parse_error) => {
            panic!(parse_error.message);
        },
    }
}

fn load_texture(texture_xml: &Element, default_color: Color) -> Texture {
    let maybe_value = texture_xml.attributes.get("value");
    let maybe_color = read_color(&texture_xml.attributes);
    let color = if maybe_value.is_some() || maybe_color.is_some() {
        maybe_value.and_then(|s| s.parse().ok()).unwrap_or(1.0) * maybe_color.unwrap_or(Vector3::new(1.0, 1.0, 1.0))
    } else {
        default_color
    };

    let texture_data = if let Some(texture) = texture_xml.attributes.get("texture") {
        if texture == "checkerboard" {
            let mut color1 = Vector3::new(0.0, 0.0, 0.0);
            let mut color2 = Vector3::new(1.0, 1.0, 1.0);
            if let Some(color1_xml) = texture_xml.get_child("color1") {
                color1 = read_color(&color1_xml.attributes).unwrap_or(color1);
            }
            if let Some(color2_xml) = texture_xml.get_child("color2") {
                color2 = read_color(&color2_xml.attributes).unwrap_or(color2);
            }

            TextureData::Checkerboard { color1, color2 }
        } else {
            load_img(texture)
        }
    } else {
        TextureData::Blank
    };

    let transform = load_transform(texture_xml);

    Texture {
        data: texture_data,
        color: color,
        transform: transform,
    }
}

fn load_transform(transform_xml: &Element) -> Transform {
    let mut transform = Transform::default();

    for child in &transform_xml.children {
        match child.name.as_ref() {
            "scale" => {
                let mat = if let Some(scalar) = child.attributes.get("value") {
                    let scalar: f32 = scalar.parse().expect("could not parse value for scale");
                    scalar * Matrix3::one()
                } else {
                    let diagonal = read_vector3_default(&child.attributes, Vector3::new(1.0, 1.0, 1.0));
                    Matrix3::from_diagonal(diagonal)
                };
                transform.transform = mat * transform.transform;
                transform.translate = mat * transform.translate;
            },
            "translate" => {
                transform.translate += read_vector3_default(&child.attributes, Vector3::new(0.0, 0.0, 0.0));
            },
            "rotate" => {
                let angle = Deg(
                    child.attributes.get("angle").expect("no angle given for rotate")
                    .parse().expect("could not parse angle for rotate")
                );

                let rotate: Matrix3<f32>;
                if let Some(_) = child.attributes.get("x") {
                    rotate = Matrix3::from_angle_x(angle);
                } else if let Some(_) = child.attributes.get("y") {
                    rotate = Matrix3::from_angle_y(angle);
                } else if let Some(_) = child.attributes.get("z") {
                    rotate = Matrix3::from_angle_z(angle);
                } else {
                    rotate = Matrix3::from_angle_z(angle);
                }

                transform.transform = rotate * transform.transform;
                transform.translate = rotate * transform.translate;
            },
            _ => {}
        }
    }

    transform
}

fn read_vector3(attrs: &HashMap<String, String>) -> Vector3<f32> {
    read_vector3_default(attrs, Vector3::new(0.0, 0.0, 0.0))
}

fn read_vector3_default(attrs: &HashMap<String, String>, default: Vector3<f32>) -> Vector3<f32> {
    Vector3::new(
        attrs.get("x").and_then(|s| s.parse().ok()).unwrap_or(default.x),
        attrs.get("y").and_then(|s| s.parse().ok()).unwrap_or(default.y),
        attrs.get("z").and_then(|s| s.parse().ok()).unwrap_or(default.z),
    )
}

fn read_color(attrs: &HashMap<String, String>) -> Option<Color> {
    let r = attrs.get("r").and_then(|s| s.parse().ok());
    let g = attrs.get("g").and_then(|s| s.parse().ok());
    let b = attrs.get("b").and_then(|s| s.parse().ok());

    if let (Some(r), Some(g), Some(b)) = (r, g, b) {
        Some(Vector3::new(r, g, b))
    } else {
        None
    }
}

fn load_img(filename: &str) -> TextureData {
    let decoder = png::Decoder::new(File::open(filename).unwrap());
    let (info, mut reader) = decoder.read_info().unwrap();
    let mut buf = vec![0; info.buffer_size()];
    reader.next_frame(&mut buf).unwrap();
    TextureData::Image { pixels: buf, width: info.width as usize, height: info.height as usize }
}
