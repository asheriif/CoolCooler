use coolcooler_core::Resolution;
use iced::{mouse, Point, Task};
use image::{Rgba, RgbaImage};

use crate::canvas::LayerSelection;
use crate::composition::{CanvasPolicy, SourceKind};
use crate::rendering::{circular_preview_from_rgba, render_base_rgba};
use crate::widget::WidgetEdit;
use crate::{CoolCooler, Message};

impl CoolCooler {
    pub(crate) fn source_kind(&self) -> SourceKind {
        SourceKind::from_frame_count(self.source.frame_count())
    }

    pub(crate) fn current_source_size(&self) -> Option<(u32, u32)> {
        self.source.current_size()
    }

    pub(crate) fn canvas_policy(&self) -> CanvasPolicy {
        CanvasPolicy::for_content(self.display.capability(), self.source_kind())
    }

    pub(crate) fn lcd_resolution(&self) -> Resolution {
        self.display.resolution()
    }

    pub(crate) fn render_composited(&self) -> RgbaImage {
        let resolution = self.lcd_resolution();
        let base = if let Some(src) = self.source.current_frame() {
            let vp = self.canvas.base_viewport();
            render_base_rgba(&src.rgba, resolution, vp.zoom, vp.pan)
        } else {
            RgbaImage::from_pixel(resolution.width, resolution.height, Rgba([0, 0, 0, 255]))
        };
        self.canvas.composite(base, &self.widget_ctx)
    }

    pub(crate) fn rebuild_preview(&mut self) {
        let composited = self.render_composited();
        self.preview = Some(circular_preview_from_rgba(composited));
    }

    pub(crate) fn commit_frame(&mut self) {
        let composited = self.render_composited();
        self.display.submit_frame(&composited);
        self.preview = Some(circular_preview_from_rgba(composited));
    }

    pub(crate) fn edit_active_widget(&mut self, edit: WidgetEdit) {
        if self.canvas.edit_active_widget(edit) {
            self.commit_frame();
        }
    }

    pub(crate) fn widget_tick(&mut self) {
        let has_sysinfo_widgets = self.canvas.has_widgets_in_category("System Metrics");
        if has_sysinfo_widgets {
            self.sysinfo_backend.refresh();
            self.widget_ctx.sysinfo = self.sysinfo_backend.data().clone();
        }

        if self.canvas.tick_widgets(&self.widget_ctx) {
            self.commit_frame();
        }
    }

    pub(crate) fn scroll_canvas(&mut self, delta: mouse::ScrollDelta) {
        let y = match delta {
            mouse::ScrollDelta::Lines { y, .. } => y,
            mouse::ScrollDelta::Pixels { y, .. } => y / 28.0,
        };
        let factor = 1.1_f32.powf(y);

        if self
            .canvas
            .zoom_active_layer(factor, self.lcd_resolution(), self.current_source_size())
        {
            self.commit_frame();
        }
    }

    pub(crate) fn start_drag(&mut self) {
        self.dragging = true;
        self.last_cursor = None;
    }

    pub(crate) fn drag_canvas(&mut self, pos: Point) {
        if self.dragging {
            if let Some(last) = self.last_cursor {
                let dx = pos.x - last.x;
                let dy = pos.y - last.y;
                if self.canvas.drag_active_layer(
                    (dx, dy),
                    self.lcd_resolution(),
                    self.current_source_size(),
                ) {
                    self.commit_frame();
                }
            }
            self.last_cursor = Some(pos);
        }
    }

    pub(crate) fn end_drag(&mut self) {
        self.dragging = false;
        self.last_cursor = None;
    }

    pub(crate) fn reset_active_layer(&mut self) {
        if self.canvas.reset_active_layer(self.lcd_resolution()) {
            self.commit_frame();
        }
    }

    pub(crate) fn add_widget(&mut self, catalog_idx: usize) -> Task<Message> {
        if !self.canvas_policy().widgets_allowed() {
            return Task::none();
        }
        if let Some(spec) = self.widget_catalog.get(catalog_idx) {
            let id = self.canvas.add_widget(spec, self.lcd_resolution());
            self.canvas.select_layer(LayerSelection::Widget(id));
            self.commit_frame();
        }
        Task::none()
    }
}
