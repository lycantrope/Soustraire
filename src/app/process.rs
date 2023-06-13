use image::{ImageBuffer, Luma};
use imageproc::filter;
use std::path::Path;

pub fn imread_as_gray<P: AsRef<Path>>(
    path: P,
) -> Result<image::ImageBuffer<image::Luma<u8>, Vec<u8>>, image::ImageError> {
    Ok(image::open(path)?.grayscale().to_luma8())
}

fn _imread<P: AsRef<Path>>(
    path: P,
) -> Result<image::ImageBuffer<image::Rgba<u8>, Vec<u8>>, image::ImageError> {
    Ok(image::open(path)?.to_rgba8())
}

// /// Formats the sum of two numbers as string.
pub fn subtract<P: AsRef<Path>>(
    img1_path: P,
    img2_path: P,
) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>, image::ImageError> {
    let im1 = imread_as_gray(img1_path)?;
    let im2 = imread_as_gray(img2_path)?;
    let width = im1.width();
    let height = im1.height();

    let mut sub: ImageBuffer<Luma<i16>, Vec<i16>> = ImageBuffer::new(width, height);

    let sum = im1
        .iter()
        .zip(im2.iter())
        .zip(sub.iter_mut())
        .fold(0_i64, |acc, ((v1, v2), dst)| {
            let delta = *v1 as i16 - *v2 as i16;
            *dst = delta;
            acc + delta as i64
        }) as f64;

    let count = (width * height) as f64;
    let mean = sum / count;

    let std = (sub
        .iter()
        .cloned()
        .fold(0., |acc, v| acc + (mean - v as f64).powi(2))
        / count)
        .sqrt();
    // normalize to 20 times std
    let vmin = -10f64 * std;
    let vmax = 10f64 * std;
    let delta = vmax - vmin;

    let mut lut: [u8; 511] = [0; 511];
    // (0usize..512).for_each(|v|  (v as f64-255.0 - mean) - vmin  )
    if delta.is_normal() {
        lut.iter_mut().enumerate().for_each(|(val, lut)| {
            *lut = ((val as f64 - 255.0 - vmin) / delta * 255.)
                .clamp(0., 255.)
                .round() as u8;
        });
    }

    let mut sub_norm = ImageBuffer::new(width, height);
    sub_norm
        .iter_mut()
        .zip(sub.iter())
        .for_each(|(dst, src)| *dst = lut[(src + 255) as usize]);

    // radius = 2 is equivalent to k_size = 5,
    let im_blur = filter::median_filter(&sub_norm, 2, 2);
    Ok(im_blur)
}
