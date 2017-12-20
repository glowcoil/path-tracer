extern crate png;
extern crate cgmath;
extern crate rayon;
extern crate rand;

mod load;
mod scene;
mod geometry;
mod bvh;

use load::*;
use scene::*;

use std::env;

use std::f32;
use std::f32::consts;
use self::cgmath::{Vector3, InnerSpace};

use std::path::Path;
use std::fs::File;
use std::io::BufWriter;
use png::HasParameters;

use rayon::prelude::*;

const R_THRESHOLD: f32 = 0.4;
const G_THRESHOLD: f32 = 0.3;
const B_THRESHOLD: f32 = 0.6;

const INITIAL_SAMPLES: i32 = 4;
const MAX_SAMPLES: i32 = 4;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 3 {
        panic!("please provide a scene description file and an image output file");
    }
    let filename = &args[1];

    let (scene, camera) = load_scene(filename);

    let height = ((camera.fov / 2.0) * (2.0 * consts::PI / 360.0)).tan() * 2.0 * camera.focaldist;
    let width = height * (camera.img_width as f32) / (camera.img_height as f32);
    let pixel_height = height / (camera.img_height as f32);
    let pixel_width = width / (camera.img_width as f32);

    let right = camera.dir.cross(camera.up);

    /* top-middle of the screen */
    let b = camera.pos + camera.focaldist * camera.dir + (height / 2.0) * camera.up;
    /* top-left corner of the screen */
    let a = b - (width / 2.0) * right;

    let mut img: Vec<u8> = vec![0; (camera.img_width * camera.img_height * 4) as usize];

    img.par_chunks_mut(4).enumerate().for_each(|(i, pixel)| {
        let x = (i as u32) % camera.img_width;
        let y = (i as u32) / camera.img_width;
        if x == 0 {
            println!("{}", y);
        }

        /* top-left corner of the current pixel */
        let top_left = a + x as f32 * pixel_width * right - y as f32 * pixel_height * camera.up;

        let mut samples = Vec::new();
        let mut num_samples = INITIAL_SAMPLES;
        // let mut iters = 0;

        loop {
            let mut new_samples: Vec<Color> = ((samples.len() as i32 + 1)..(num_samples + 1)).into_par_iter().map(|i| {
                let x_offset = halton(i, 2);
                let y_offset = halton(i, 3);

                let r1: f32 = rand::random();
                let r2: f32 = rand::random();
                let eye_x_offset: f32 = 2.0 * r1 - 1.0;//2.0 * halton(i, 5) - 1.0;
                let eye_y_offset: f32 = 2.0 * r2 - 1.0;//2.0 * halton(i, 7) - 1.0;

                let p: Vector3<f32> = top_left + x_offset * pixel_width * right - y_offset * pixel_height * camera.up;
                let eye: Vector3<f32> = camera.pos + eye_x_offset * camera.dof * right + eye_y_offset * camera.dof * camera.up;

                let dir = (p - eye).normalize();
                scene.cast(eye, dir, (x as f32 + x_offset) / camera.img_width as f32, (y as f32 + y_offset) / camera.img_height as f32, 3)
            }).collect();

            samples.append(&mut new_samples);

            let mut r_min = f32::INFINITY; let mut r_max = 0.0;
            let mut g_min = f32::INFINITY; let mut g_max = 0.0;
            let mut b_min = f32::INFINITY; let mut b_max = 0.0;
            for sample in &samples {
                if sample.x < r_min { r_min = sample.x; }
                if sample.x > r_max { r_max = sample.x; }
                if sample.y < g_min { g_min = sample.y; }
                if sample.y > g_max { g_max = sample.y; }
                if sample.z < b_min { b_min = sample.z; }
                if sample.z > b_max { b_max = sample.z; }
            }
            if num_samples < MAX_SAMPLES && ((r_max - r_min) / (r_min + r_max) > R_THRESHOLD || (g_max - g_min) / (g_min + g_max) > G_THRESHOLD || (b_max - b_min) / (b_min + b_max) > B_THRESHOLD) {
                num_samples *= 2;
                // iters += 1;
            } else {
                break;
            }
        }

        let total: Color = samples.iter().sum();
        let color: [u8; 4] = color_as_u8_array(total / num_samples as f32);
        // let brightness = 255.0 * iters as f32 / 2.0 as f32;
        // let color: [u8; 4] = color_as_u8_array(Vector3::new(brightness, brightness, brightness));

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
