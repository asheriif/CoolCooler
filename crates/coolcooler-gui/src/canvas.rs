use image::{imageops, RgbaImage};

use coolcooler_core::Resolution;

pub(crate) use crate::viewport::Viewport;
use crate::viewport::ViewportTransform;
use crate::widget::{LcdWidget, WidgetContext, WidgetEdit, WidgetId, WidgetSpec};

/// Which layer the user is currently controlling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayerSelection {
    Base,
    Widget(WidgetId),
}

/// A widget instance placed on the canvas.
pub struct WidgetLayer {
    pub id: WidgetId,
    pub type_id: &'static str,
    pub widget: Box<dyn LcdWidget>,
    pub position: (i32, i32),
    pub size: (u32, u32),
    pub visible: bool,
    /// Layer opacity: 0 = fully transparent, 255 = fully opaque.
    pub opacity: u8,
}

impl std::fmt::Debug for WidgetLayer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WidgetLayer")
            .field("id", &self.id)
            .field("type_id", &self.type_id)
            .field("position", &self.position)
            .field("size", &self.size)
            .finish()
    }
}

/// The canvas model: base layer + ordered widget layers.
pub struct Canvas {
    base_viewport: Viewport,
    layers: Vec<WidgetLayer>,
    active_layer: LayerSelection,
    next_id: usize,
}

impl Canvas {
    pub fn new() -> Self {
        Self {
            base_viewport: Viewport::default(),
            layers: Vec::new(),
            active_layer: LayerSelection::Base,
            next_id: 0,
        }
    }

    pub fn base_viewport(&self) -> &Viewport {
        &self.base_viewport
    }

    pub fn set_base_viewport(&mut self, viewport: Viewport) {
        self.base_viewport = viewport;
    }

    pub fn layers(&self) -> &[WidgetLayer] {
        &self.layers
    }

    pub fn has_widgets(&self) -> bool {
        !self.layers.is_empty()
    }

    pub fn has_widgets_in_category(&self, category: &str) -> bool {
        self.layers
            .iter()
            .any(|layer| layer.widget.descriptor().category == category)
    }

    pub fn active_layer(&self) -> LayerSelection {
        self.active_layer
    }

    pub fn select_layer(&mut self, selection: LayerSelection) {
        self.active_layer = selection;
    }

    pub fn active_widget_layer(&self) -> Option<&WidgetLayer> {
        let LayerSelection::Widget(id) = self.active_layer else {
            return None;
        };
        self.layers.iter().find(|layer| layer.id == id)
    }

    fn active_widget_layer_mut(&mut self) -> Option<&mut WidgetLayer> {
        let LayerSelection::Widget(id) = self.active_layer else {
            return None;
        };
        self.layers.iter_mut().find(|layer| layer.id == id)
    }

    pub fn clear_widgets(&mut self) {
        self.layers.clear();
        self.active_layer = LayerSelection::Base;
    }

    /// Get the next unique widget ID and advance the counter.
    fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Add a widget instance to the canvas, centered by default.
    pub fn add_widget(&mut self, spec: &WidgetSpec, resolution: Resolution) -> WidgetId {
        let id = WidgetId(self.next_id);
        self.next_id += 1;
        let size = spec.descriptor.default_size;
        let position = default_widget_position(resolution, size);
        self.layers.push(WidgetLayer {
            id,
            type_id: spec.type_id,
            widget: spec.create(),
            position,
            size,
            visible: true,
            opacity: 255,
        });
        id
    }

    pub fn add_configured_widget(
        &mut self,
        spec: &WidgetSpec,
        widget: Box<dyn LcdWidget>,
        position: (i32, i32),
        size: (u32, u32),
        opacity: u8,
    ) -> WidgetId {
        let id = WidgetId(self.next_id());
        self.layers.push(WidgetLayer {
            id,
            type_id: spec.type_id,
            widget,
            position,
            size,
            visible: true,
            opacity,
        });
        id
    }

    pub fn remove_widget(&mut self, id: WidgetId) {
        self.layers.retain(|l| l.id != id);
        if self.active_layer == LayerSelection::Widget(id) {
            self.active_layer = LayerSelection::Base;
        }
    }

    pub fn edit_active_widget(&mut self, edit: WidgetEdit) -> bool {
        let Some(layer) = self.active_widget_layer_mut() else {
            return false;
        };
        layer.widget.apply_edit(edit);
        true
    }

    pub fn set_active_widget_opacity(&mut self, opacity: u8) -> bool {
        let Some(layer) = self.active_widget_layer_mut() else {
            return false;
        };
        layer.opacity = opacity;
        true
    }

    pub fn zoom_active_layer(
        &mut self,
        factor: f32,
        resolution: Resolution,
        source_size: Option<(u32, u32)>,
    ) -> bool {
        match self.active_layer {
            LayerSelection::Base => {
                let Some(source_size) = source_size else {
                    return false;
                };
                self.base_viewport.zoom = (self.base_viewport.zoom * factor).clamp(0.25, 10.0);
                self.clamp_base_pan(source_size, resolution);
                true
            }
            LayerSelection::Widget(_) => {
                let Some(layer) = self.active_widget_layer_mut() else {
                    return false;
                };
                let new_w = ((layer.size.0 as f32) * factor).round() as u32;
                let new_h = ((layer.size.1 as f32) * factor).round() as u32;
                layer.size = (
                    new_w.clamp(10, resolution.width.max(10)),
                    new_h.clamp(10, resolution.height.max(10)),
                );
                true
            }
        }
    }

    pub fn drag_active_layer(
        &mut self,
        delta: (f32, f32),
        resolution: Resolution,
        source_size: Option<(u32, u32)>,
    ) -> bool {
        match self.active_layer {
            LayerSelection::Base => {
                let Some(transform) = source_size.and_then(|source_size| {
                    ViewportTransform::new(source_size, resolution, self.base_viewport.zoom)
                }) else {
                    return false;
                };
                self.base_viewport.pan = transform.pan_after_drag(self.base_viewport.pan, delta);
                true
            }
            LayerSelection::Widget(_) => {
                let Some(layer) = self.active_widget_layer_mut() else {
                    return false;
                };
                let size = layer.size;
                let mut new_pos = (
                    layer.position.0 + delta.0 as i32,
                    layer.position.1 + delta.1 as i32,
                );
                let min_visible = 10i32;
                let lcd_w = resolution.width as i32;
                let lcd_h = resolution.height as i32;
                new_pos.0 = new_pos
                    .0
                    .clamp(-(size.0 as i32) + min_visible, lcd_w - min_visible);
                new_pos.1 = new_pos
                    .1
                    .clamp(-(size.1 as i32) + min_visible, lcd_h - min_visible);
                layer.position = new_pos;
                true
            }
        }
    }

    pub fn reset_active_layer(&mut self, resolution: Resolution) -> bool {
        match self.active_layer {
            LayerSelection::Base => {
                if self.base_viewport.zoom == 1.0 && self.base_viewport.pan == (0.0, 0.0) {
                    return false;
                }
                self.base_viewport = Viewport::default();
                true
            }
            LayerSelection::Widget(_) => {
                let Some(layer) = self.active_widget_layer_mut() else {
                    return false;
                };
                let default_size = layer.widget.descriptor().default_size;
                let default_position = default_widget_position(resolution, default_size);
                if layer.size == default_size && layer.position == default_position {
                    return false;
                }
                layer.size = default_size;
                layer.position = default_position;
                true
            }
        }
    }

    pub fn active_layer_can_reset(&self, resolution: Resolution) -> bool {
        match self.active_layer {
            LayerSelection::Base => {
                (self.base_viewport.zoom - 1.0).abs() > 0.01 || self.base_viewport.pan != (0.0, 0.0)
            }
            LayerSelection::Widget(_) => self
                .active_widget_layer()
                .map(|layer| {
                    let default_size = layer.widget.descriptor().default_size;
                    layer.size != default_size
                        || layer.position != default_widget_position(resolution, default_size)
                })
                .unwrap_or(false),
        }
    }

    pub fn active_layer_reset_status(&self) -> String {
        match self.active_layer {
            LayerSelection::Base => format!("{}%", (self.base_viewport.zoom * 100.0) as u32),
            LayerSelection::Widget(_) => self
                .active_widget_layer()
                .map(|layer| format!("{}×{}", layer.size.0, layer.size.1))
                .unwrap_or_default(),
        }
    }

    /// Build display labels for the layer dropdown.
    pub fn layer_options(&self) -> Vec<(LayerSelection, String)> {
        let mut opts = vec![(LayerSelection::Base, "Background".to_string())];

        // Count widget types for numbering duplicates
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for layer in &self.layers {
            *counts.entry(layer.widget.descriptor().name).or_default() += 1;
        }

        let mut seen: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for layer in &self.layers {
            let name = layer.widget.descriptor().name;
            let n = seen.entry(name).or_default();
            *n += 1;
            let label = if counts[name] > 1 {
                format!("{name} #{n}")
            } else {
                name.to_string()
            };
            opts.push((LayerSelection::Widget(layer.id), label));
        }

        opts
    }

    /// Composite all visible widget layers onto a base RGBA image.
    pub fn composite(&self, mut base: RgbaImage, ctx: &WidgetContext) -> RgbaImage {
        for layer in &self.layers {
            if !layer.visible || layer.opacity == 0 {
                continue;
            }
            let mut rendered = layer.widget.render(layer.size.0, layer.size.1, ctx);

            // Apply layer opacity by scaling each pixel's alpha
            if layer.opacity < 255 {
                let factor = layer.opacity as f32 / 255.0;
                for pixel in rendered.pixels_mut() {
                    pixel.0[3] = (pixel.0[3] as f32 * factor) as u8;
                }
            }

            imageops::overlay(
                &mut base,
                &rendered,
                layer.position.0 as i64,
                layer.position.1 as i64,
            );
        }
        base
    }

    /// Tick all dynamic widgets. Returns true if any content changed.
    pub fn tick_widgets(&mut self, ctx: &WidgetContext) -> bool {
        let mut changed = false;
        for layer in &mut self.layers {
            if layer.widget.is_dynamic() && layer.widget.tick(ctx) {
                changed = true;
            }
        }
        changed
    }

    fn clamp_base_pan(&mut self, source_size: (u32, u32), resolution: Resolution) {
        if let Some(transform) =
            ViewportTransform::new(source_size, resolution, self.base_viewport.zoom)
        {
            self.base_viewport.pan = transform.clamp_pan(self.base_viewport.pan);
        }
    }
}

fn default_widget_position(resolution: Resolution, widget_size: (u32, u32)) -> (i32, i32) {
    (
        (resolution.width as i32 - widget_size.0 as i32) / 2,
        (resolution.height as i32 - widget_size.1 as i32) / 2,
    )
}
