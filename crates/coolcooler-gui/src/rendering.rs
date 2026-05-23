use coolcooler_core::Resolution;
use fast_image_resize as fir;
use iced::widget::image::Handle;
use image::{imageops, Rgba, RgbaImage};

use crate::viewport::{BaseRenderPlan, ViewportTransform};

/// Render the base layer at its viewport.
pub(crate) fn render_base_rgba(
    source: &RgbaImage,
    resolution: Resolution,
    zoom: f32,
    pan: (f32, f32),
) -> RgbaImage {
    let Some(transform) =
        ViewportTransform::new((source.width(), source.height()), resolution, zoom)
    else {
        return RgbaImage::new(resolution.width, resolution.height);
    };

    match transform.render_plan(pan) {
        BaseRenderPlan::Crop {
            x,
            y,
            width,
            height,
        } => {
            let cropped = imageops::crop_imm(source, x, y, width, height).to_image();
            resize_rgba(&cropped, resolution.width, resolution.height)
        }
        BaseRenderPlan::Fit {
            width,
            height,
            offset,
        } => {
            let scaled = resize_rgba(source, width, height);
            let mut canvas =
                RgbaImage::from_pixel(resolution.width, resolution.height, Rgba([0, 0, 0, 255]));
            imageops::overlay(&mut canvas, &scaled, offset.0, offset.1);
            canvas
        }
    }
}

fn resize_rgba(source: &RgbaImage, width: u32, height: u32) -> RgbaImage {
    let (sw, sh) = (source.width(), source.height());
    if sw == width && sh == height {
        return source.clone();
    }
    let src_img =
        fir::images::Image::from_vec_u8(sw, sh, source.as_raw().clone(), fir::PixelType::U8x4)
            .unwrap();
    let mut dst_img = fir::images::Image::new(width, height, fir::PixelType::U8x4);
    let mut resizer = fir::Resizer::new();
    resizer
        .resize(
            &src_img,
            &mut dst_img,
            &fir::ResizeOptions::new()
                .resize_alg(fir::ResizeAlg::Convolution(fir::FilterType::CatmullRom)),
        )
        .unwrap();
    RgbaImage::from_raw(width, height, dst_img.into_vec()).unwrap()
}

pub(crate) fn circular_preview_from_rgba(mut rgba: RgbaImage) -> Handle {
    let (w, h) = (rgba.width(), rgba.height());
    let cx = w as f32 / 2.0;
    let cy = h as f32 / 2.0;
    let r = cx.min(cy);
    let aa_width = 1.5;

    for y in 0..h {
        for x in 0..w {
            let dx = x as f32 - cx + 0.5;
            let dy = y as f32 - cy + 0.5;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist > r {
                rgba.put_pixel(x, y, Rgba([0, 0, 0, 0]));
            } else if dist > r - aa_width {
                let alpha = ((r - dist) / aa_width * 255.0).clamp(0.0, 255.0) as u8;
                let p = *rgba.get_pixel(x, y);
                rgba.put_pixel(x, y, Rgba([p[0], p[1], p[2], alpha]));
            }
        }
    }

    Handle::from_rgba(w, h, rgba.into_raw())
}

#[cfg(test)]
mod tests {
    use coolcooler_core::Resolution;

    use super::*;

    #[test]
    fn render_base_preserves_rectangular_target_resolution() {
        let source = RgbaImage::from_pixel(120, 120, Rgba([255, 0, 0, 255]));

        let rendered = render_base_rgba(&source, Resolution::new(240, 320), 1.0, (0.0, 0.0));

        assert_eq!(rendered.width(), 240);
        assert_eq!(rendered.height(), 320);
    }
}
