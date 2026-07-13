use super::DecodedImage;

const WIDTH: u32 = 1280;
const HEIGHT: u32 = 720;

pub fn create_demo_image() -> DecodedImage {
    let stride = WIDTH * 4;
    let mut pixels = vec![0_u8; (stride * HEIGHT) as usize];

    for y in 0..HEIGHT {
        for x in 0..WIDTH {
            let nx = x as f32 / (WIDTH - 1) as f32;
            let ny = y as f32 / (HEIGHT - 1) as f32;
            let horizon = 0.64 + (nx * 7.0).sin() * 0.035 + (nx * 17.0).sin() * 0.012;

            let mut red = 20.0 + 42.0 * (1.0 - ny);
            let mut green = 33.0 + 58.0 * (1.0 - ny);
            let mut blue = 55.0 + 92.0 * (1.0 - ny);
            add_glow(
                &mut red,
                &mut green,
                &mut blue,
                nx,
                ny,
                0.27,
                0.28,
                0.34,
                [38.0, 118.0, 150.0],
            );
            add_glow(
                &mut red,
                &mut green,
                &mut blue,
                nx,
                ny,
                0.76,
                0.36,
                0.42,
                [128.0, 48.0, 118.0],
            );

            let sun_distance = ((nx - 0.72).powi(2) + (ny - 0.28).powi(2)).sqrt();
            if sun_distance < 0.075 {
                let strength = (1.0 - sun_distance / 0.075).powf(0.45);
                red += 176.0 * strength;
                green += 116.0 * strength;
                blue += 48.0 * strength;
            }

            if ny > horizon {
                let depth = ((ny - horizon) / (1.0 - horizon)).clamp(0.0, 1.0);
                red = 18.0 + 18.0 * depth;
                green = 39.0 + 35.0 * depth;
                blue = 48.0 + 25.0 * depth;
            }

            let reflection = ((nx - 0.72).abs() * 8.0 + (ny - 0.70).abs() * 2.0).max(0.05);
            if ny > horizon && reflection < 0.55 {
                let strength = (0.55 - reflection) / 0.55;
                red += 80.0 * strength;
                green += 62.0 * strength;
                blue += 35.0 * strength;
            }

            let offset = (y * stride + x * 4) as usize;
            pixels[offset] = blue.clamp(0.0, 255.0) as u8;
            pixels[offset + 1] = green.clamp(0.0, 255.0) as u8;
            pixels[offset + 2] = red.clamp(0.0, 255.0) as u8;
            pixels[offset + 3] = 255;
        }
    }

    DecodedImage {
        file_name: "PurePic 演示图".to_owned(),
        original_width: WIDTH,
        original_height: HEIGHT,
        width: WIDTH,
        height: HEIGHT,
        stride,
        file_size: 0,
        pixels,
    }
}

#[allow(clippy::too_many_arguments)]
fn add_glow(
    red: &mut f32,
    green: &mut f32,
    blue: &mut f32,
    x: f32,
    y: f32,
    center_x: f32,
    center_y: f32,
    radius: f32,
    color: [f32; 3],
) {
    let distance = ((x - center_x).powi(2) + (y - center_y).powi(2)).sqrt();
    let strength = (1.0 - distance / radius).clamp(0.0, 1.0).powi(2);
    *red += color[0] * strength;
    *green += color[1] * strength;
    *blue += color[2] * strength;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn demo_image_has_valid_bgra_storage() {
        let image = create_demo_image();
        assert_eq!((image.width, image.height), (WIDTH, HEIGHT));
        assert_eq!(image.stride, WIDTH * 4);
        assert_eq!(image.pixels.len(), (image.stride * HEIGHT) as usize);
        assert!(image.pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
    }
}
