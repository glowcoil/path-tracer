extern crate png;
extern crate cgmath;
extern crate rayon;

mod load;
mod scene;
mod geometry;
mod bvh;

use load::*;
use scene::*;

use std::env;

use std::f32::consts;
use self::cgmath::{InnerSpace};

use std::path::Path;
use std::fs::File;
use std::io::BufWriter;
use png::HasParameters;

use rayon::prelude::*;

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

    let mut img: Vec<u8> = vec![0; (camera.img_width * camera.img_height * 4) as usize];

    img.par_chunks_mut(4).enumerate().for_each(|(i, pixel)| {
        let x = (i as u32) % camera.img_width;
        let y = (i as u32) / camera.img_width;

        /* center of the current pixel */
        let p = a + ((x as f32) + 0.5) * pixel_width * right - ((y as f32) + 0.5) * pixel_height * camera.up;
        let dir = (p - camera.pos).normalize();

        let color: [u8; 4] = color_as_u8_array(scene.cast(camera.pos, dir, x as f32 / camera.img_width as f32, y as f32 / camera.img_height as f32, 3));

        pixel.copy_from_slice(&color);
    });

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
