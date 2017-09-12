extern crate xmltree;
extern crate cgmath;

use scene::*;

use std::fs::File;
use std::io::prelude::*;
use std::collections::HashMap;
use self::xmltree::Element;
use self::cgmath::{Vector3, Matrix3, SquareMatrix, InnerSpace, One, Deg};

pub fn load_scene(filename: &str) -> (Scene, Camera) {
    let mut f = File::open(filename).expect("file not found");
    let mut contents = String::new();
    f.read_to_string(&mut contents).expect("could not read file");

    let xml = Element::parse(contents.as_bytes()).expect("could not parse xml");

    let mut scene = Scene {
        nodes: Vec::new(),
        materials: HashMap::new(),
        lights: Vec::new(),
    };

    let scene_xml = xml.get_child("scene").expect("no <scene> tag found");
    for child in &scene_xml.children {
        match child.name.as_ref() {
            "object" => {
                scene.nodes.push(load_node(child));
            },
            "material" => {
                let (name, material) = load_material(child);
                scene.materials.insert(name, material);
            },
            "light" => {
                scene.lights.push(load_light(child));
            }
            _ => {}
        }
    }

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
                _ => {
                    panic!("unknown object type");
                },
            },
            material: node_xml.attributes.get("material").expect("no material given for object").clone(),
        }
    });

    let name = node_xml.attributes.get("name").expect("no name given for object").clone();

    let mut transform = Matrix3::one();
    let mut translate = Vector3::new(0.0, 0.0, 0.0);

    let mut children: Vec<Node> = Vec::new();
    for child in &node_xml.children {
        if child.name == "object" {
            children.push(load_node(child));
        } else {
            match child.name.as_ref() {
                "scale" => {
                    let mat = if let Some(scalar) = child.attributes.get("value") {
                        let scalar: f32 = scalar.parse().expect("could not parse value for scale");
                        scalar * Matrix3::one()
                    } else {
                        let diagonal = read_vector3_default(&child.attributes, Vector3::new(1.0, 1.0, 1.0));
                        Matrix3::from_diagonal(diagonal)
                    };
                    transform = mat * transform;
                    translate = mat * translate;
                },
                "translate" => {
                    translate += read_vector3_default(&child.attributes, Vector3::new(0.0, 0.0, 0.0));
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
                        panic!("no axis given for rotate");
                    }

                    transform = rotate * transform;
                    translate = rotate * translate;
                },
                _ => {}
            }
        }
    }

    Node {
        object: object,
        transform: transform,
        translate: translate,
        children: children,
        name: name,
    }
}

fn load_material(material_xml: &Element) -> (String, Material) {
    let material_type = material_xml.attributes.get("type").expect("no type for material");
    let name = material_xml.attributes.get("name").expect("no name for material");

    match material_type.as_ref() {
        "blinn" => {
            let diffuse = material_xml.get_child("diffuse").and_then(|diffuse_xml| {
                read_color(&diffuse_xml.attributes)
            }).expect("no diffuse found for <material>");

            let specular_xml = material_xml.get_child("specular").expect("no specular found for <material>");
            let specular = read_color(&specular_xml.attributes).unwrap_or(Vector3::new(0.0, 0.0, 0.0));
            let specular_value = specular_xml.attributes.get("value").expect("no value found for <specular>")
                .parse().expect("could not parse glossiness value");

            let glossiness = if let Some(glossiness_xml) = material_xml.get_child("glossiness") {
                glossiness_xml.attributes
                    .get("value").expect("no value found for <glossiness>")
                    .parse().expect("could not parse glossiness value")
            } else {
                1.0
            };

            (name.clone(), Material {
                diffuse: diffuse,
                specular: specular,
                specular_value: specular_value,
                glossiness: glossiness,
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
            LightType::Point(read_vector3(&light_xml.get_child("position")
                .expect("no position given for positional light").attributes))
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

    /* make sure camera.up is orthogonal to camera.dir */
    camera.up = (camera.dir.cross(camera.up)).cross(camera.dir);

    camera
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
