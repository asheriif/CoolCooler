use std::borrow::Cow;
use fast_image_resize as fir;
use image::codecs::jpeg::JpegEncoder;
use image::imageops;
use image::{DynamicImage, ImageEncoder, RgbImage};

use crate::{DeviceInfo, Error, Resolution, Result, Rotation};

/// Default JPEG encoding quality (0-100).
pub const DEFAULT_JPEG_QUALITY: u8 = 85;

/// Center-crop and resize an image to the target resolution.
///
/// Crops to match the target aspect ratio (removing excess width or height),
/// then resizes to the exact target dimensions using SIMD-accelerated bicubic
/// filtering via `fast_image_resize`.
/// Does NOT apply rotation or JPEG encoding.
pub fn crop_and_resize(img: &DynamicImage, resolution: Resolution) -> RgbImage {
    let mut rgb = img.to_rgb8();
    let (src_w, src_h) = (rgb.width(), rgb.height());

    let target_ratio = resolution.aspect_ratio();
    let src_ratio = src_w as f64 / src_h as f64;

    if (src_ratio - target_ratio).abs() > 0.01 {
        let (crop_w, crop_h) = if src_ratio > target_ratio {
            ((src_h as f64 * target_ratio) as u32, src_h)
        } else {
            (src_w, (src_w as f64 / target_ratio) as u32)
        };
        let left = src_w.saturating_sub(crop_w) / 2;
        let top = src_h.saturating_sub(crop_h) / 2;
        rgb = imageops::crop_imm(&rgb, left, top, crop_w, crop_h).to_image();
    }

    // Fast SIMD-accelerated resize
    let (cw, ch) = (rgb.width(), rgb.height());
    if cw == resolution.width && ch == resolution.height {
        return rgb;
    }

    let src_image = fir::images::Image::from_vec_u8(cw, ch, rgb.into_raw(), fir::PixelType::U8x3)
        .unwrap();

    let mut dst_image = fir::images::Image::new(resolution.width, resolution.height, fir::PixelType::U8x3);

    let mut resizer = fir::Resizer::new();
    resizer
        .resize(
            &src_image,
            &mut dst_image,
            &fir::ResizeOptions::new().resize_alg(fir::ResizeAlg::Convolution(
                fir::FilterType::CatmullRom,
            )),
        )
        .unwrap();

    RgbImage::from_raw(resolution.width, resolution.height, dst_image.into_vec())
        .expect("resize produced wrong buffer size")
}

/// Apply rotation and JPEG-encode an already-resized image.
///
/// Use this with [`crop_and_resize`] when you need the resized image for
/// multiple purposes (e.g., both device JPEG and GUI preview).
pub fn encode_resized(rgb: &RgbImage, rotation: Rotation, quality: u8) -> Result<Vec<u8>> {
    let final_rgb: Cow<'_, RgbImage> = match rotation {
        Rotation::None => Cow::Borrowed(rgb),
        Rotation::Deg90 => Cow::Owned(imageops::rotate90(rgb)),
        Rotation::Deg180 => Cow::Owned(imageops::rotate180(rgb)),
        Rotation::Deg270 => Cow::Owned(imageops::rotate270(rgb)),
    };

    let mut buf = Vec::new();
    let encoder = JpegEncoder::new_with_quality(&mut buf, quality);
    encoder
        .write_image(
            final_rgb.as_raw(),
            final_rgb.width(),
            final_rgb.height(),
            image::ExtendedColorType::Rgb8,
        )
        .map_err(|e| Error::Image(e.to_string()))?;

    Ok(buf)
}

/// Prepare an image for display on a cooler LCD.
///
/// Performs center-crop, resize, rotation, and JPEG encoding in one step.
/// If you also need the resized image for other purposes, use
/// [`crop_and_resize`] + [`encode_resized`] instead.
pub fn prepare(img: &DynamicImage, info: &DeviceInfo, quality: u8) -> Result<Vec<u8>> {
    let resized = crop_and_resize(img, info.resolution);
    encode_resized(&resized, info.rotation, quality)
}
