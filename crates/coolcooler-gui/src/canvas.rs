use image::{imageops, RgbaImage};

use crate::widget::{LcdWidget, WidgetContext, WidgetId};

/// Viewport state for a single layer.
#[derive(Debug, Clone)]
pub struct Viewport {
    pub zoom: f32,
    pub pan: (f32, f32),
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            zoom: 1.0,
            pan: (0.0, 0.0),
        }
    }
}

/// Which layer the user is currently controlling.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayerSelection {
    Base,
    Widget(WidgetId),
}

/// A widget instance placed on the canvas.
pub struct WidgetLayer {
    pub id: WidgetId,
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
            .field("position", &self.position)
            .field("size", &self.size)
            .finish()
    }
}

/// The canvas model: base layer + ordered widget layers.
pub struct Canvas {
    pub base_viewport: Viewport,
    pub layers: Vec<WidgetLayer>,
    pub active_layer: LayerSelection,
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

    /// Get the next unique widget ID and advance the counter.
    pub fn next_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    /// Add a widget instance to the canvas, centered by default.
    pub fn add_widget(&mut self, widget: Box<dyn LcdWidget>, lcd_size: u32) -> WidgetId {
        let id = WidgetId(self.next_id);
        self.next_id += 1;
        let size = widget.descriptor().default_size;
        let position = (
            (lcd_size as i32 - size.0 as i32) / 2,
            (lcd_size as i32 - size.1 as i32) / 2,
        );
        self.layers.push(WidgetLayer {
            id,
            widget,
            position,
            size,
            visible: true,
            opacity: 255,
        });
        id
    }

    pub fn remove_widget(&mut self, id: WidgetId) {
        self.layers.retain(|l| l.id != id);
        if self.active_layer == LayerSelection::Widget(id) {
            self.active_layer = LayerSelection::Base;
        }
    }

    /// Build display labels for the layer dropdown.
    pub fn layer_options(&self) -> Vec<(LayerSelection, String)> {
        let mut opts = vec![(LayerSelection::Base, "Background".to_string())];

        // Count widget types for numbering duplicates
        let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
        for layer in &self.layers {
            *counts
                .entry(layer.widget.descriptor().name)
                .or_default() += 1;
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
}
