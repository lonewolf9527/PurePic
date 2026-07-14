use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

const ICON_RESOURCE_ID: u16 = 1;

fn main() {
    let project_dir = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap());
    let manifest = project_dir.join("app.manifest");

    println!("cargo:rerun-if-changed={}", manifest.display());
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=Assets/icons");

    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
    println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());

    let out_dir = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let icon_path = out_dir.join("PurePic.ico");
    let resource_path = out_dir.join("PurePic.rc");
    write_icon(&icon_path).expect("failed to generate PurePic icon");
    let escaped_icon = icon_path.to_string_lossy().replace('\\', "\\\\");
    fs::write(
        &resource_path,
        format!("{ICON_RESOURCE_ID} ICON \"{escaped_icon}\"\n"),
    )
    .expect("failed to generate PurePic resource script");
    embed_resource::compile(&resource_path, embed_resource::NONE)
        .manifest_required()
        .expect("failed to embed PurePic icon");
}

fn write_icon(path: &Path) -> io::Result<()> {
    let sizes = [16_u32, 32, 48, 256];
    let images: Vec<_> = sizes.into_iter().map(build_icon_image).collect();
    let mut file = Vec::new();

    push_u16(&mut file, 0);
    push_u16(&mut file, 1);
    push_u16(&mut file, images.len() as u16);

    let mut offset = 6 + images.len() as u32 * 16;
    for (size, image) in sizes.into_iter().zip(&images) {
        file.push(if size == 256 { 0 } else { size as u8 });
        file.push(if size == 256 { 0 } else { size as u8 });
        file.push(0);
        file.push(0);
        push_u16(&mut file, 1);
        push_u16(&mut file, 32);
        push_u32(&mut file, image.len() as u32);
        push_u32(&mut file, offset);
        offset += image.len() as u32;
    }

    for image in images {
        file.extend_from_slice(&image);
    }
    fs::write(path, file)
}

fn build_icon_image(size: u32) -> Vec<u8> {
    let pixel_bytes = size * size * 4;
    let mask_stride = size.div_ceil(32) * 4;
    let mut image = Vec::with_capacity((40 + pixel_bytes + mask_stride * size) as usize);

    push_u32(&mut image, 40);
    push_i32(&mut image, size as i32);
    push_i32(&mut image, (size * 2) as i32);
    push_u16(&mut image, 1);
    push_u16(&mut image, 32);
    push_u32(&mut image, 0);
    push_u32(&mut image, pixel_bytes);
    push_i32(&mut image, 0);
    push_i32(&mut image, 0);
    push_u32(&mut image, 0);
    push_u32(&mut image, 0);

    let mut alpha = vec![0_u8; (size * size) as usize];
    for output_y in (0..size).rev() {
        for output_x in 0..size {
            let mut covered = 0_u32;
            let mut red = 0_u32;
            let mut green = 0_u32;
            let mut blue = 0_u32;
            for sample_y in 0..4 {
                for sample_x in 0..4 {
                    let x = (output_x as f32 + (sample_x as f32 + 0.5) / 4.0) / size as f32;
                    let y = (output_y as f32 + (sample_y as f32 + 0.5) / 4.0) / size as f32;
                    if let Some([r, g, b]) = sample_icon(x, y) {
                        covered += 1;
                        red += r as u32;
                        green += g as u32;
                        blue += b as u32;
                    }
                }
            }

            let alpha_value = (covered * 255 / 16) as u8;
            alpha[(output_y * size + output_x) as usize] = alpha_value;
            if covered == 0 {
                image.extend_from_slice(&[0, 0, 0, 0]);
            } else {
                image.extend_from_slice(&[
                    (blue / covered) as u8,
                    (green / covered) as u8,
                    (red / covered) as u8,
                    alpha_value,
                ]);
            }
        }
    }

    for output_y in (0..size).rev() {
        let row_start = image.len();
        image.resize(row_start + mask_stride as usize, 0);
        for output_x in 0..size {
            if alpha[(output_y * size + output_x) as usize] < 128 {
                image[row_start + (output_x / 8) as usize] |= 0x80 >> (output_x % 8);
            }
        }
    }

    image
}

fn sample_icon(x: f32, y: f32) -> Option<[u8; 3]> {
    if !inside_rounded_square(x, y, 0.21) {
        return None;
    }

    let mut color = [
        (24.0 + 32.0 * y) as u8,
        (188.0 + 42.0 * (1.0 - y)) as u8,
        (216.0 + 34.0 * x) as u8,
    ];

    let border =
        inside_rect(x, y, 0.21, 0.23, 0.79, 0.77) && !inside_rect(x, y, 0.27, 0.29, 0.73, 0.71);
    let sun = distance(x, y, 0.63, 0.40) <= 0.065;
    let mountain = distance_to_segment(x, y, 0.27, 0.67, 0.44, 0.50) <= 0.025
        || distance_to_segment(x, y, 0.44, 0.50, 0.57, 0.63) <= 0.025
        || distance_to_segment(x, y, 0.53, 0.64, 0.65, 0.52) <= 0.025
        || distance_to_segment(x, y, 0.65, 0.52, 0.73, 0.62) <= 0.025;
    if border || sun || mountain {
        color = [250, 253, 255];
    }
    Some(color)
}

fn inside_rounded_square(x: f32, y: f32, radius: f32) -> bool {
    let nearest_x = x.clamp(radius, 1.0 - radius);
    let nearest_y = y.clamp(radius, 1.0 - radius);
    distance(x, y, nearest_x, nearest_y) <= radius
}

fn inside_rect(x: f32, y: f32, left: f32, top: f32, right: f32, bottom: f32) -> bool {
    x >= left && x <= right && y >= top && y <= bottom
}

fn distance(x: f32, y: f32, other_x: f32, other_y: f32) -> f32 {
    ((x - other_x).powi(2) + (y - other_y).powi(2)).sqrt()
}

#[allow(clippy::too_many_arguments)]
fn distance_to_segment(x: f32, y: f32, start_x: f32, start_y: f32, end_x: f32, end_y: f32) -> f32 {
    let dx = end_x - start_x;
    let dy = end_y - start_y;
    let length_squared = dx * dx + dy * dy;
    let t = (((x - start_x) * dx + (y - start_y) * dy) / length_squared).clamp(0.0, 1.0);
    distance(x, y, start_x + t * dx, start_y + t * dy)
}

fn push_u16(target: &mut Vec<u8>, value: u16) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(target: &mut Vec<u8>, value: u32) {
    target.extend_from_slice(&value.to_le_bytes());
}

fn push_i32(target: &mut Vec<u8>, value: i32) {
    target.extend_from_slice(&value.to_le_bytes());
}
