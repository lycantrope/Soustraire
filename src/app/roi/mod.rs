use std::error::Error;

use super::font::ROBOTO_FNT;
use image::{GenericImageView, ImageBuffer};
use imageproc::drawing::{draw_hollow_rect_mut, draw_text_mut};
use itertools::iproduct;
use rusttype::{Font, Scale};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Debug, Serialize, Deserialize, Default)]
struct Roi {
    x: u32,
    y: u32,
    width: u32,
    height: u32,
    index: usize,
}

impl Roi {
    fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
            index: 0,
        }
    }
    fn set_index(mut self, idx: usize) -> Self {
        self.index = idx;
        self
    }
    fn measure(&self, im: &ImageBuffer<image::Luma<u8>, Vec<u8>>) -> u32 {
        let x2 = std::cmp::min(self.width + self.x, im.width());
        let y2 = std::cmp::min(self.height + self.y, im.height());
        let width = x2.saturating_sub(self.x);
        let height = y2.saturating_sub(self.y);
        im.view(self.x, self.y, width, height)
            .pixels()
            .fold(0u32, |acc, pix| {
                let (_, _, image::Luma([pix])) = pix;
                acc + (pix as u32 & 1)
            })
    }

    fn draw_roi(&self, gray: &mut ImageBuffer<image::Rgba<u8>, Vec<u8>>, font: &Font<'_>) {
        let white = image::Rgba([255, 255, 0, 128]);
        draw_hollow_rect_mut(
            gray,
            imageproc::rect::Rect::at(self.x as i32, self.y as i32)
                .of_size(self.width, self.height),
            white,
        );
        draw_hollow_rect_mut(
            gray,
            imageproc::rect::Rect::at(self.x as i32 - 1, self.y as i32 - 1)
                .of_size(self.width + 2, self.height + 2),
            white,
        );
        draw_text_mut(
            gray,
            white,
            self.x as i32 - 15,
            self.y as i32 - 15,
            Scale { x: 16.0, y: 16.0 },
            font,
            &format!("{}", self.index),
        )
    }
}
#[derive(Debug, Serialize, Deserialize)]
pub struct RoiCollection {
    pub nrow: u32,
    pub ncol: u32,
    pub x: u32,
    pub y: u32,
    pub xinterval: u32,
    pub yinterval: u32,
    pub width: u32,
    pub height: u32,
    pub rotate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    rois: Option<Vec<Roi>>,
}

impl std::default::Default for RoiCollection {
    fn default() -> Self {
        Self {
            nrow: 6,
            ncol: 8,
            x: 18,
            y: 20,
            xinterval: 128,
            yinterval: 125,
            width: 78,
            height: 78,
            rotate: 0.0,
            rois: None,
        }
    }
}

impl RoiCollection {
    pub fn measure_all(
        &self,
        subimg: &ImageBuffer<image::Luma<u8>, Vec<u8>>,
        threshold: f64,
    ) -> Option<Vec<u32>> {
        let thresh = (127.0f64 - threshold * 12.8f64).clamp(0f64, 255f64).round() as u8;
        let mut thres_im = imageproc::contrast::threshold(subimg, thresh);
        // invert the byte;
        thres_im.iter_mut().for_each(|pix| {
            *pix = !*pix;
        });

        self.rois
            .as_ref()
            .map(|rois| rois.iter().map(|roi| roi.measure(&thres_im)).collect())
    }

    pub fn update_rois(&mut self) {
        let rot = self.rotate / 180. * std::f64::consts::PI;
        let rot_cos = rot.cos();
        let rot_sin = rot.sin();

        let rois = iproduct!(0..self.nrow, 0..self.ncol)
            .map(|(i, j)| {
                let fx = (self.x + j * self.xinterval) as f64;
                let fy = (self.y + i * self.yinterval) as f64;
                let fx = fx * rot_cos - fy * rot_sin;
                let fy = fx * rot_sin + fy * rot_cos;

                Roi::new(
                    f64::max(fx.round(), 0.) as u32,
                    f64::max(fy.round(), 0.) as u32,
                    self.width,
                    self.height,
                )
            })
            .enumerate()
            .map(|(idx, roi)| roi.set_index(idx))
            .collect::<Vec<Roi>>();
        self.rois = Some(rois);
    }

    pub fn draw_rois(&self, gray: &mut ImageBuffer<image::Rgba<u8>, Vec<u8>>) {
        let font = Font::try_from_bytes(ROBOTO_FNT).unwrap();

        if let Some(rois) = self.rois.as_ref() {
            rois.iter().for_each(|roi| {
                roi.draw_roi(gray, &font);
            })
        };
    }

    pub fn to_json<P: AsRef<Path>>(&self, path: P) -> Result<(), Box<dyn Error>> {
        let contents = serde_json::to_string_pretty(self)?;
        std::fs::write(path, contents)?;
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.rois.as_ref().map(|rois| rois.len()).unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_new_roi() {
        let mut roicol = RoiCollection::default();
        roicol.update_rois();
        roicol
            .to_json("./test.json")
            .expect("fail to write to json");
    }
}
