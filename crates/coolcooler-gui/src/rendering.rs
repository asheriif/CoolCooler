use coolcooler_core::DeviceInfo;
use fast_image_resize as fir;
use iced::widget::image::Handle;
use image::{imageops, Rgba, RgbaImage};

/// Render the base layer at its viewport.
pub(crate) fn render_base_rgba(
    source: &RgbaImage,
    info: &DeviceInfo,
    zoom: f32,
    pan: (f32, f32),
) -> RgbaImage {
    let (sw, sh) = (source.width() as f32, source.height() as f32);
    let res = info.resolution;
    let short = sw.min(sh);
    let vis = short / zoom;

    if vis <= sw && vis <= sh {
        let cx = (sw / 2.0 + pan.0).clamp(vis / 2.0, sw - vis / 2.0);
        let cy = (sh / 2.0 + pan.1).clamp(vis / 2.0, sh - vis / 2.0);

        let x0 = (cx - vis / 2.0).max(0.0) as u32;
        let y0 = (cy - vis / 2.0).max(0.0) as u32;
        let side = (vis as u32)
            .min(source.width() - x0)
            .min(source.height() - y0);

        let cropped = imageops::crop_imm(source, x0, y0, side, side).to_image();
        resize_rgba(&cropped, res.width, res.height)
    } else {
        let scale = res.width as f32 * zoom / short;
        let scaled_w = (sw * scale).round() as u32;
        let scaled_h = (sh * scale).round() as u32;

        let scaled = resize_rgba(source, scaled_w.max(1), scaled_h.max(1));

        let pan_scale = res.width as f32 / short;
        let pan_ox = (pan.0 * pan_scale).round() as i64;
        let pan_oy = (pan.1 * pan_scale).round() as i64;

        let mut canvas = RgbaImage::from_pixel(res.width, res.height, Rgba([0, 0, 0, 255]));
        let base_ox = (res.width.saturating_sub(scaled_w)) as i64 / 2;
        let base_oy = (res.height.saturating_sub(scaled_h)) as i64 / 2;
        imageops::overlay(&mut canvas, &scaled, base_ox - pan_ox, base_oy - pan_oy);
        canvas
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
