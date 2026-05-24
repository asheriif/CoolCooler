use std::time::{Duration, Instant};

use iced::Task;

use crate::canvas::{Canvas, Viewport};
use crate::source::{load_source_data, LoadedData, SourceLoadRequest};
use crate::{preset, widget, CoolCooler, Message};

#[derive(Debug, Clone)]
pub(crate) struct CurrentPreset {
    pub(crate) folder: preset::PresetFolder,
    pub(crate) name: String,
}

pub(crate) struct PresetState {
    pub(crate) current: Option<CurrentPreset>,
    pub(crate) show_save_dialog: bool,
    pub(crate) show_load_dialog: bool,
    pub(crate) save_name_input: String,
    pub(crate) list: Vec<preset::PresetEntry>,
    last_click: Option<(preset::PresetFolder, Instant)>,
}

impl PresetState {
    pub(crate) fn new() -> Self {
        Self {
            current: None,
            show_save_dialog: false,
            show_load_dialog: false,
            save_name_input: String::new(),
            list: Vec::new(),
            last_click: None,
        }
    }

    pub(crate) fn has_current(&self) -> bool {
        self.current.is_some()
    }

    pub(crate) fn open_load_dialog(&mut self) {
        self.list = preset::list();
        self.show_load_dialog = true;
    }

    pub(crate) fn register_click(&mut self, folder: preset::PresetFolder) -> bool {
        let is_double = self
            .last_click
            .as_ref()
            .map(|(f, t)| f == &folder && t.elapsed() < Duration::from_millis(400))
            .unwrap_or(false);

        if is_double {
            self.last_click = None;
        } else {
            self.last_click = Some((folder, Instant::now()));
        }

        is_double
    }
}

impl CoolCooler {
    pub(crate) fn build_preset_data(&self, name: &str) -> preset::PresetData {
        let bg = self.source.path().map(|p| {
            let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("png");
            preset::BackgroundData {
                file: format!("background.{ext}"),
            }
        });

        let widgets = self
            .canvas
            .layers()
            .iter()
            .map(|layer| preset::WidgetLayerData {
                type_id: layer.type_id.to_string(),
                position: layer.position,
                size: layer.size,
                opacity: layer.opacity,
                config: layer.widget.preset_config(),
            })
            .collect();

        preset::PresetData {
            version: 1,
            name: name.to_string(),
            background: bg,
            viewport: preset::ViewportData {
                zoom: self.canvas.base_viewport().zoom,
                pan: self.canvas.base_viewport().pan,
            },
            widgets,
        }
    }

    pub(crate) fn apply_preset_config(&mut self, data: &preset::PresetData) -> usize {
        let widgets_allowed = self.canvas_policy().widgets_allowed();

        self.canvas.set_base_viewport(Viewport {
            zoom: data.viewport.zoom,
            pan: data.viewport.pan,
        });

        self.canvas.clear_widgets();

        let skipped_widgets =
            restore_preset_widgets(&mut self.canvas, &data.widgets, widgets_allowed);

        self.rebuild_preview();
        skipped_widgets
    }

    pub(crate) fn load_preset_folder(
        &mut self,
        folder: preset::PresetFolder,
        silent: bool,
    ) -> Task<Message> {
        let preset::LoadedPreset {
            data,
            background_path,
        } = match preset::load(&folder) {
            Ok(loaded) => loaded,
            Err(e) => {
                if !silent {
                    self.ui.status_message = format!("Load failed: {e}");
                }
                return Task::none();
            }
        };

        if let Some(path) = background_path {
            if !path.exists() {
                if !silent {
                    self.ui.status_message = "Load failed: background file missing".to_string();
                }
                return Task::none();
            }

            let request = self.source.begin_background_loading(path.clone());
            let task_request = request.clone();

            if !silent {
                self.ui.status_message = "Loading preset...".to_string();
            }

            return Task::perform(
                async move {
                    load_source_data(task_request.path(), task_request.filename().to_string())
                },
                move |result| Message::PresetSourceLoaded {
                    request,
                    result,
                    data,
                    folder,
                    background_path: Some(path),
                    silent,
                },
            );
        }

        self.source.clear();
        self.apply_loaded_preset(folder, data, silent);
        Task::none()
    }

    pub(crate) fn apply_loaded_preset(
        &mut self,
        folder: preset::PresetFolder,
        data: preset::PresetData,
        silent: bool,
    ) {
        let name = data.name.clone();

        self.presets.current = Some(CurrentPreset {
            folder: folder.clone(),
            name: name.clone(),
        });
        let skipped_widgets = self.apply_preset_config(&data);
        self.start_display();
        preset::remember_last_used(&folder);

        if !silent || self.ui.status_message.is_empty() {
            self.ui.status_message = if skipped_widgets == 0 {
                format!("Loaded preset '{name}'")
            } else {
                format!("Loaded preset '{name}' ({skipped_widgets} incompatible widget(s) skipped)")
            };
        }
    }

    pub(crate) fn save_requested(&mut self) {
        if let Some(current) = self.presets.current.clone() {
            let name = current.name;
            let data = self.build_preset_data(&name);
            let composited = self.render_composited();
            match preset::save(
                &name,
                Some(&current.folder),
                self.source.path(),
                &composited,
                &data,
            ) {
                Ok(folder) => {
                    self.presets.current = Some(CurrentPreset {
                        folder: folder.clone(),
                        name: name.clone(),
                    });
                    preset::remember_last_used(&folder);
                    self.ui.status_message = format!("Preset '{name}' saved");
                }
                Err(e) => self.ui.status_message = format!("Save failed: {e}"),
            }
        } else {
            self.presets.save_name_input.clear();
            self.presets.show_save_dialog = true;
        }
    }

    pub(crate) fn save_preset(&mut self) {
        let name = self.presets.save_name_input.trim().to_string();
        if let Err(e) = preset::validate_name(&name) {
            self.ui.status_message = e.to_string();
            return;
        }

        let data = self.build_preset_data(&name);
        let composited = self.render_composited();
        match preset::save(&name, None, self.source.path(), &composited, &data) {
            Ok(folder) => {
                preset::remember_last_used(&folder);
                self.presets.current = Some(CurrentPreset {
                    folder,
                    name: name.clone(),
                });
                self.presets.show_save_dialog = false;
                self.ui.status_message = format!("Preset '{name}' saved");
            }
            Err(e) => self.ui.status_message = format!("Save failed: {e}"),
        }
    }

    pub(crate) fn preset_clicked(&mut self, folder: preset::PresetFolder) -> Task<Message> {
        if !self.presets.register_click(folder.clone()) {
            return Task::none();
        }

        self.presets.show_load_dialog = false;
        self.load_preset_folder(folder, false)
    }

    pub(crate) fn preset_source_loaded(
        &mut self,
        request: SourceLoadRequest,
        result: Result<LoadedData, String>,
        data: preset::PresetData,
        folder: preset::PresetFolder,
        background_path: Option<std::path::PathBuf>,
        silent: bool,
    ) {
        match result {
            Ok(loaded) => {
                if self
                    .source
                    .complete_loading(&request, loaded, background_path)
                    .is_some()
                {
                    self.apply_loaded_preset(folder, data, silent);
                }
            }
            Err(e) => {
                if self.source.fail_loading(&request) && !silent {
                    self.ui.status_message = format!("Load failed: {e}");
                }
            }
        }
    }

    pub(crate) fn delete_preset(&mut self, folder: preset::PresetFolder) {
        if let Err(e) = preset::delete(&folder) {
            self.ui.status_message = format!("Delete failed: {e}");
            return;
        }

        preset::forget_last_used_if(&folder);
        self.presets.list = preset::list();
        if self
            .presets
            .current
            .as_ref()
            .is_some_and(|current| current.folder == folder)
        {
            self.presets.current = None;
            self.source.clear();
            self.commit_frame();
        }
    }
}

fn restore_preset_widgets(
    canvas: &mut Canvas,
    widgets: &[preset::WidgetLayerData],
    widgets_allowed: bool,
) -> usize {
    let mut skipped_widgets = 0;
    for wd in widgets {
        if !widgets_allowed {
            skipped_widgets += 1;
            continue;
        }

        let Some(spec) = widget::spec_by_type_id(&wd.type_id) else {
            skipped_widgets += 1;
            continue;
        };
        let mut w = spec.create();
        if w.apply_preset_config(&wd.config).is_err() {
            skipped_widgets += 1;
            continue;
        }
        canvas.add_configured_widget(spec, w, wd.position, wd.size, wd.opacity);
    }

    skipped_widgets
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn text_widget_layer() -> preset::WidgetLayerData {
        preset::WidgetLayerData {
            type_id: "free_text".to_string(),
            position: (10, 20),
            size: (80, 28),
            opacity: 255,
            config: json!({
                "text": "Label",
                "color": [255, 255, 255, 255],
                "font": "Inter"
            }),
        }
    }

    #[test]
    fn preset_widgets_are_skipped_when_policy_disallows_them() {
        let mut canvas = Canvas::new();

        let skipped = restore_preset_widgets(&mut canvas, &[text_widget_layer()], false);

        assert_eq!(skipped, 1);
        assert!(canvas.layers().is_empty());
    }

    #[test]
    fn preset_widgets_restore_when_policy_allows_them() {
        let mut canvas = Canvas::new();

        let skipped = restore_preset_widgets(&mut canvas, &[text_widget_layer()], true);

        assert_eq!(skipped, 0);
        assert_eq!(canvas.layers().len(), 1);
    }
}
