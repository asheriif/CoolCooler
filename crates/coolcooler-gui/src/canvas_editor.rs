use coolcooler_core::Resolution;
use iced::{mouse, Point, Task};
use image::{Rgba, RgbaImage};

use crate::canvas::LayerSelection;
use crate::composition::{CanvasPolicy, SourceKind};
use crate::rendering::{circular_preview_from_rgba, render_base_rgba};
use crate::source::SourceState;
use crate::widget::WidgetContext;
use crate::widget::WidgetEdit;
use crate::{CoolCooler, Message};

pub(crate) struct CanvasInteraction {
    dragging: bool,
    last_cursor: Option<Point>,
}

impl CanvasInteraction {
    pub(crate) fn new() -> Self {
        Self {
            dragging: false,
            last_cursor: None,
        }
    }

    pub(crate) fn is_dragging(&self) -> bool {
        self.dragging
    }

    fn start_drag(&mut self) {
        self.dragging = true;
        self.last_cursor = None;
    }

    fn drag_delta(&mut self, pos: Point) -> Option<(f32, f32)> {
        if !self.dragging {
            return None;
        }
        let delta = self
            .last_cursor
            .map(|last| (pos.x - last.x, pos.y - last.y));
        self.last_cursor = Some(pos);
        delta
    }

    fn end_drag(&mut self) {
        self.dragging = false;
        self.last_cursor = None;
    }
}

pub(crate) fn render_composited(
    source: &SourceState,
    canvas: &crate::canvas::Canvas,
    widget_ctx: &WidgetContext,
    resolution: Resolution,
) -> RgbaImage {
    let base = if let Some(src) = source.current_frame() {
        let vp = canvas.base_viewport();
        render_base_rgba(&src.rgba, resolution, vp.zoom, vp.pan)
    } else {
        RgbaImage::from_pixel(resolution.width, resolution.height, Rgba([0, 0, 0, 255]))
    };
    canvas.composite(base, widget_ctx)
}

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

    pub(crate) fn render_composited_at(&self, resolution: Resolution) -> RgbaImage {
        render_composited(
            &self.source,
            &self.canvas,
            self.widgets.context(),
            resolution,
        )
    }

    pub(crate) fn render_composited(&self) -> RgbaImage {
        self.render_composited_at(self.lcd_resolution())
    }

    pub(crate) fn rebuild_preview(&mut self) {
        let composited = self.render_composited();
        self.ui.preview = Some(circular_preview_from_rgba(composited));
    }

    pub(crate) fn commit_frame(&mut self) {
        let composited = self.render_composited();
        self.display.submit_frame(&composited);
        self.ui.preview = Some(circular_preview_from_rgba(composited));
    }

    pub(crate) fn edit_active_widget(&mut self, edit: WidgetEdit) {
        if self.canvas.edit_active_widget(edit) {
            self.commit_frame();
        }
    }

    pub(crate) fn widget_tick(&mut self) {
        self.widgets
            .refresh_if_needed(self.canvas.has_widgets_in_category("System Metrics"));

        if self.canvas.tick_widgets(self.widgets.context()) {
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
        self.interaction.start_drag();
    }

    pub(crate) fn drag_canvas(&mut self, pos: Point) {
        if let Some(delta) = self.interaction.drag_delta(pos) {
            if self.canvas.drag_active_layer(
                delta,
                self.lcd_resolution(),
                self.current_source_size(),
            ) {
                self.commit_frame();
            }
        }
    }

    pub(crate) fn end_drag(&mut self) {
        self.interaction.end_drag();
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
        if let Some(spec) = self.widgets.catalog().get(catalog_idx) {
            let id = self.canvas.add_widget(spec, self.lcd_resolution());
            self.canvas.select_layer(LayerSelection::Widget(id));
            self.commit_frame();
        }
        Task::none()
    }
}
