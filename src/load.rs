extern crate xmltree;
extern crate cgmath;

use scene::*;

use std::fs::File;
use std::io::prelude::*;
use std::collections::HashMap;
use self::xmltree::Element;
use self::cgmath::{Vector3, Matrix3, InnerSpace, One, Deg};

pub fn load_scene(filename: &str) -> (Scene, Camera) {
    let mut f = File::open(filename).expect("file not found");
    let mut contents = String::new();
    f.read_to_string(&mut contents).expect("could not read file");

    let xml = Element::parse(contents.as_bytes()).expect("could not parse xml");

    let mut scene = Scene {
        nodes: Vec::new(),
        materials: Vec::new(),
        lights: Vec::new(),
    };

    let scene_xml = xml.get_child("scene").expect("no <scene> tag found");
    for child in &scene_xml.children {
        match child.name.as_ref() {
            "object" => {
                scene.nodes.push(load_node(child));
            },
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
                panic!("object type does not exist");
            },
        },
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
                    let scalar: f32 = child.attributes.get("value").expect("no value given for scale")
                        .parse().expect("could not parse value for scale");
                    let mat = scalar * Matrix3::one();
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
