use coolcooler_core::Resolution;

/// Viewport state for the background layer.
#[derive(Debug, Clone)]
pub(crate) struct Viewport {
    pub(crate) zoom: f32,
    pub(crate) pan: (f32, f32),
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: (0.0, 0.0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum BaseRenderPlan {
    Crop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    Fit {
        width: u32,
        height: u32,
        offset: (i64, i64),
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ViewportTransform {
    source_size: (u32, u32),
    resolution: Resolution,
    fit_size: (f32, f32),
    visible_size: (f32, f32),
    zoom: f32,
}

impl ViewportTransform {
    pub(crate) fn new(source_size: (u32, u32), resolution: Resolution, zoom: f32) -> Option<Self> {
        if source_size.0 == 0
            || source_size.1 == 0
            || resolution.width == 0
            || resolution.height == 0
        {
            return None;
        }

        let zoom = zoom.max(f32::EPSILON);
        let fit_size = fitted_viewport_size(source_size, resolution);
        let visible_size = (fit_size.0 / zoom, fit_size.1 / zoom);

        Some(Self {
            source_size,
            resolution,
            fit_size,
            visible_size,
            zoom,
        })
    }

    pub(crate) fn clamp_pan(&self, pan: (f32, f32)) -> (f32, f32) {
        let (sw, sh) = (self.source_size.0 as f32, self.source_size.1 as f32);
        let (vis_w, vis_h) = self.visible_size;

        if self.uses_crop() {
            let max_pan_x = ((sw - vis_w) / 2.0).max(0.0);
            let max_pan_y = ((sh - vis_h) / 2.0).max(0.0);
            (
                pan.0.clamp(-max_pan_x, max_pan_x),
                pan.1.clamp(-max_pan_y, max_pan_y),
            )
        } else {
            let margin_x = vis_w * 0.375;
            let margin_y = vis_h * 0.375;
            (
                pan.0.clamp(-margin_x, margin_x),
                pan.1.clamp(-margin_y, margin_y),
            )
        }
    }

    pub(crate) fn pan_after_drag(&self, pan: (f32, f32), delta: (f32, f32)) -> (f32, f32) {
        let (vis_w, vis_h) = self.visible_size;
        let pan = (
            pan.0 - delta.0 * (vis_w / self.resolution.width as f32),
            pan.1 - delta.1 * (vis_h / self.resolution.height as f32),
        );
        self.clamp_pan(pan)
    }

    pub(crate) fn render_plan(&self, pan: (f32, f32)) -> BaseRenderPlan {
        let pan = self.clamp_pan(pan);
        if self.uses_crop() {
            self.crop_plan(pan)
        } else {
            self.fit_plan(pan)
        }
    }

    fn uses_crop(&self) -> bool {
        let (sw, sh) = (self.source_size.0 as f32, self.source_size.1 as f32);
        self.visible_size.0 <= sw && self.visible_size.1 <= sh
    }

    fn crop_plan(&self, pan: (f32, f32)) -> BaseRenderPlan {
        let (sw, sh) = (self.source_size.0 as f32, self.source_size.1 as f32);
        let (vis_w, vis_h) = self.visible_size;
        let cx = (sw / 2.0 + pan.0).clamp(vis_w / 2.0, sw - vis_w / 2.0);
        let cy = (sh / 2.0 + pan.1).clamp(vis_h / 2.0, sh - vis_h / 2.0);
        let x = (cx - vis_w / 2.0).max(0.0) as u32;
        let y = (cy - vis_h / 2.0).max(0.0) as u32;

        BaseRenderPlan::Crop {
            x,
            y,
            width: (vis_w.round() as u32).clamp(1, self.source_size.0 - x),
            height: (vis_h.round() as u32).clamp(1, self.source_size.1 - y),
        }
    }

    fn fit_plan(&self, pan: (f32, f32)) -> BaseRenderPlan {
        let (sw, sh) = (self.source_size.0 as f32, self.source_size.1 as f32);
        let scale = (self.resolution.width as f32 / self.fit_size.0)
            .min(self.resolution.height as f32 / self.fit_size.1)
            * self.zoom;
        let width = ((sw * scale).round() as u32).max(1);
        let height = ((sh * scale).round() as u32).max(1);
        let pan_ox = (pan.0 * scale).round() as i64;
        let pan_oy = (pan.1 * scale).round() as i64;
        let base_ox = self.resolution.width.saturating_sub(width) as i64 / 2;
        let base_oy = self.resolution.height.saturating_sub(height) as i64 / 2;

        BaseRenderPlan::Fit {
            width,
            height,
            offset: (base_ox - pan_ox, base_oy - pan_oy),
        }
    }
}

fn fitted_viewport_size(source_size: (u32, u32), resolution: Resolution) -> (f32, f32) {
    let (sw, sh) = (source_size.0 as f32, source_size.1 as f32);
    let target_ratio = resolution.width as f32 / resolution.height as f32;
    let src_ratio = sw / sh;

    if src_ratio > target_ratio {
        (sh * target_ratio, sh)
    } else {
        (sw, sw / target_ratio)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drag_and_render_share_pan_bounds_for_crop_mode() {
        let transform = ViewportTransform::new((600, 300), Resolution::new(240, 240), 2.0)
            .expect("valid viewport");

        let pan = transform.pan_after_drag((0.0, 0.0), (-10_000.0, 0.0));
        let BaseRenderPlan::Crop { x, width, .. } = transform.render_plan(pan) else {
            panic!("zoomed wide source should crop");
        };

        assert_eq!(x + width, 600);
    }

    #[test]
    fn fit_mode_produces_scaled_overlay_plan() {
        let transform = ViewportTransform::new((120, 120), Resolution::new(240, 320), 0.5)
            .expect("valid viewport");

        let BaseRenderPlan::Fit { width, height, .. } = transform.render_plan((0.0, 0.0)) else {
            panic!("zoomed-out source should fit");
        };

        assert_eq!((width, height), (160, 160));
    }
}
