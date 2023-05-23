use image::{ImageBuffer, Luma};
use imageproc::filter;
use rayon::prelude::*;
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
) -> Result<ImageBuffer<Luma<u8>, Vec<u8>>, Box<dyn std::error::Error>> {
    let im1 = imread_as_gray(img1_path)?;
    let im2 = imread_as_gray(img2_path)?;
    let width = im1.width();
    let height = im1.height();

    let mut sub: ImageBuffer<Luma<f32>, Vec<f32>> = ImageBuffer::new(width, height);

    im1.into_iter()
    .zip(im2.into_iter())
    .zip(sub.iter_mut())
    .for_each(|((v1, v2), dst)| {
        *dst = *v1 as f32 - *v2 as f32;
    });

    let sum = sub.par_iter().sum::<f32>();
    let count = (width * height) as f32;
    let mean = sum / count;

    let std = sub
        .iter()
        .cloned()
        .fold(0., |acc, v|  acc + (mean - v).powi(2))
        / count;
    // normalize to 20 times std
    let vmin = -10f32 * std;
    let vmax = 10f32 * std;
    let delta = vmax - vmin;
    let mut sub_norm = ImageBuffer::new(width, height);

    sub_norm
        .iter_mut()
        .zip(sub.into_iter())
        .for_each(|(dst, src)| {
            *dst = ((src - mean) - vmin / delta * 255.0)
                .clamp(0., 255.)
                .round() as u8;
        });


    let im_blur = filter::median_filter(&sub_norm, 5, 5);
    Ok(im_blur)
}
