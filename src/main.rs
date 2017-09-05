extern crate png;
extern crate cgmath;

mod load;
mod scene;

use load::*;

use std::env;

use std::f32::consts;
use self::cgmath::{InnerSpace};

use std::path::Path;
use std::fs::File;
use std::io::BufWriter;
use png::HasParameters;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        panic!("please provide a scene description file and an image output file");
    }
    let filename = &args[1];

    let (scene, camera) = load_scene(filename);

    let distance = 1.0;
    let height = ((camera.fov / 2.0) * (2.0 * consts::PI / 360.0)).tan() * 2.0 * distance;
    let width = height * (camera.img_width as f32) / (camera.img_height as f32);
    let pixel_height = height / (camera.img_height as f32);
    let pixel_width = width / (camera.img_width as f32);

    let right = camera.dir.cross(camera.up);

    /* top middle of the screen */
    let b = camera.pos + distance * camera.dir + (height / 2.0) * camera.up;
    let a = b - (width / 2.0) * right;

    let mut img: Vec<u8> = Vec::with_capacity((camera.img_width * camera.img_height * 4) as usize);

    for row in 0..camera.img_height {
        for col in 0..camera.img_width {
            /* center of the current pixel */
            let p = a + ((col as f32) + 0.5) * pixel_width * right - ((row as f32) + 0.5) * pixel_height * camera.up;
            let dir = (p - camera.pos).normalize();

            let color = if let Some(z) = scene.intersect(camera.pos, dir) {
                // println!("intersected");
                [255, 255, 255, 255]
            } else {
                [0, 0, 0, 255]
            };

            img.extend(color.iter().cloned());
        }
    }

    save_img(&args[2], camera.img_width, camera.img_height, &img);
}

fn save_img(filename: &str, width: u32, height: u32, img: &[u8]) {
    let path = Path::new(filename);
    let file = File::create(path).unwrap();
    let ref mut w = BufWriter::new(file);

    let mut encoder = png::Encoder::new(w, width, height);
    encoder.set(png::ColorType::RGBA).set(png::BitDepth::Eight);
    let mut writer = encoder.write_header().unwrap();

    writer.write_image_data(&img).unwrap();
}
