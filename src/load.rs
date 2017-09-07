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
    let object = Object {
        geometry: match node_xml.attributes.get("type").expect("no type given for object").as_ref() {
            "sphere" => {
                Geometry::Sphere
            },
            _ => {
                panic!("unknown object type");
            },
        },
        material: node_xml.attributes.get("material").expect("no material given for object").clone(),
        name: node_xml.attributes.get("name").expect("no name given for object").clone(),
    };

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
                        let diagonal = read_vector3(&child.attributes);
                        Matrix3::from_diagonal(diagonal)
                    };
                    transform = mat * transform;
                    translate = mat * translate;
                },
                "translate" => {
                    translate += read_vector3(&child.attributes);
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
    }
}

fn load_material(material_xml: &Element) -> (String, Material) {
    let material_type = material_xml.attributes.get("type").expect("no type for material");
    let name = material_xml.attributes.get("name").expect("no name for material");

    match material_type.as_ref() {
        "blinn" => {
            let diffuse = read_color(&material_xml.get_child("diffuse").expect("no diffuse found for <material>").attributes);
            let specular_xml = material_xml.get_child("specular").expect("no specular found for <material>");
            let specular = read_color(&specular_xml.attributes);
            let specular_value = specular_xml.attributes.get("value").expect("no value found for <specular>")
                .parse().expect("could not parse glossiness value");
            let glossiness = material_xml.get_child("glossiness").expect("no glossiness found for <material>").attributes
                .get("value").expect("no value found for <glossiness>")
                .parse().expect("could not parse glossiness value");

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

    let intensity = light_xml.get_child("intensity").expect("no intensity given for light")
        .attributes.get("value").expect("no value for light intensity")
        .parse().expect("could not parse light intensity value");

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
        light_type: light_type,
    }
}

fn load_camera(camera_xml: &Element) -> Camera {
    let mut camera: Camera = Default::default();

    camera.pos = read_vector3(&camera_xml.get_child("position").expect("no <position> tag found in <camera>").attributes);
    camera.dir = (read_vector3(&camera_xml.get_child("target").expect("no <target> tag found in <camera>").attributes)
        - camera.pos).normalize();
    camera.up = read_vector3(&camera_xml.get_child("up").expect("no <up> tag found in <camera>").attributes);
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
    Vector3::new(
        attrs.get("x").unwrap().parse().unwrap(),
        attrs.get("y").unwrap().parse().unwrap(),
        attrs.get("z").unwrap().parse().unwrap(),
    )
}

fn read_color(attrs: &HashMap<String, String>) -> Color {
    Vector3::new(
        attrs.get("r").unwrap().parse().unwrap(),
        attrs.get("g").unwrap().parse().unwrap(),
        attrs.get("b").unwrap().parse().unwrap(),
    )
}
