use std::path::PathBuf;
use std::time::{Duration, Instant};

use iced::Task;

use crate::canvas::Viewport;
use crate::source::{load_source_data, LoadedData};
use crate::{preset, widget, CoolCooler, Message};

impl CoolCooler {
    pub(crate) fn build_preset_data(&self, name: &str) -> preset::PresetData {
        let bg = self.selected_path.as_ref().map(|p| {
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
                config: layer.widget.config(),
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
        self.canvas.set_base_viewport(Viewport {
            zoom: data.viewport.zoom,
            pan: data.viewport.pan,
        });

        self.canvas.clear_widgets();

        let mut skipped_widgets = 0;
        for wd in &data.widgets {
            let Some(spec) = widget::spec_by_type_id(&wd.type_id) else {
                skipped_widgets += 1;
                continue;
            };
            let mut w = spec.create();
            if w.apply_config_value(&wd.config).is_err() {
                skipped_widgets += 1;
                continue;
            }
            self.canvas
                .add_configured_widget(spec, w, wd.position, wd.size, wd.opacity);
        }

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
                    self.status_message = format!("Load failed: {e}");
                }
                return Task::none();
            }
        };

        if let Some(path) = background_path {
            if !path.exists() {
                if !silent {
                    self.status_message = "Load failed: background file missing".to_string();
                }
                return Task::none();
            }

            let filename = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
            let path_clone = path.clone();

            self.loading = true;
            if !silent {
                self.status_message = "Loading preset...".to_string();
            }

            return Task::perform(
                async move { load_source_data(&path_clone, filename) },
                move |result| Message::PresetSourceLoaded {
                    result,
                    data,
                    folder,
                    background_path: Some(path),
                    silent,
                },
            );
        }

        self.apply_loaded_preset(folder, data, None, None, silent);
        Task::none()
    }

    pub(crate) fn apply_loaded_preset(
        &mut self,
        folder: preset::PresetFolder,
        data: preset::PresetData,
        loaded_source: Option<LoadedData>,
        background_path: Option<PathBuf>,
        silent: bool,
    ) {
        let name = data.name.clone();

        if let Some(loaded) = loaded_source {
            let (frames, filename) = loaded.into_parts();
            self.filename = filename;
            self.source_frames = frames;
            self.selected_path = background_path;
        } else {
            self.source_frames.clear();
            self.selected_path = None;
            self.filename.clear();
        }

        self.current_preset_folder = Some(folder.clone());
        self.current_preset_name = Some(name.clone());
        self.current_frame = 0;
        self.last_advance = Instant::now();
        let skipped_widgets = self.apply_preset_config(&data);
        self.start_display();
        preset::remember_last_used(&folder);

        if !silent || self.status_message.is_empty() {
            self.status_message = if skipped_widgets == 0 {
                format!("Loaded preset '{name}'")
            } else {
                format!("Loaded preset '{name}' ({skipped_widgets} incompatible widget(s) skipped)")
            };
        }
    }

    pub(crate) fn save_requested(&mut self) {
        if let Some(name) = self.current_preset_name.clone() {
            let data = self.build_preset_data(&name);
            let composited = self.render_composited();
            match preset::save(
                &name,
                self.current_preset_folder.as_ref(),
                self.selected_path.as_deref(),
                &composited,
                &data,
            ) {
                Ok(folder) => {
                    self.current_preset_folder = Some(folder.clone());
                    self.current_preset_name = Some(name.clone());
                    preset::remember_last_used(&folder);
                    self.status_message = format!("Preset '{name}' saved");
                }
                Err(e) => self.status_message = format!("Save failed: {e}"),
            }
        } else {
            self.save_name_input.clear();
            self.show_save_dialog = true;
        }
    }

    pub(crate) fn save_preset(&mut self) {
        let name = self.save_name_input.trim().to_string();
        if let Err(e) = preset::validate_name(&name) {
            self.status_message = e.to_string();
            return;
        }

        let data = self.build_preset_data(&name);
        let composited = self.render_composited();
        match preset::save(
            &name,
            None,
            self.selected_path.as_deref(),
            &composited,
            &data,
        ) {
            Ok(folder) => {
                preset::remember_last_used(&folder);
                self.current_preset_folder = Some(folder);
                self.current_preset_name = Some(name.clone());
                self.show_save_dialog = false;
                self.status_message = format!("Preset '{name}' saved");
            }
            Err(e) => self.status_message = format!("Save failed: {e}"),
        }
    }

    pub(crate) fn preset_clicked(&mut self, folder: preset::PresetFolder) -> Task<Message> {
        let is_double = self
            .last_preset_click
            .as_ref()
            .map(|(f, t)| f == &folder && t.elapsed() < Duration::from_millis(400))
            .unwrap_or(false);

        if !is_double {
            self.last_preset_click = Some((folder, Instant::now()));
            return Task::none();
        }

        self.last_preset_click = None;
        self.show_load_dialog = false;
        self.load_preset_folder(folder, false)
    }

    pub(crate) fn preset_source_loaded(
        &mut self,
        result: Result<LoadedData, String>,
        data: preset::PresetData,
        folder: preset::PresetFolder,
        background_path: Option<PathBuf>,
        silent: bool,
    ) {
        self.loading = false;
        match result {
            Ok(loaded) => {
                self.apply_loaded_preset(folder, data, Some(loaded), background_path, silent);
            }
            Err(e) => {
                if !silent {
                    self.status_message = format!("Load failed: {e}");
                }
            }
        }
    }

    pub(crate) fn delete_preset(&mut self, folder: preset::PresetFolder) {
        if let Err(e) = preset::delete(&folder) {
            self.status_message = format!("Delete failed: {e}");
            return;
        }

        preset::forget_last_used_if(&folder);
        self.preset_list = preset::list();
        if self.current_preset_folder.as_ref() == Some(&folder) {
            self.current_preset_folder = None;
            self.current_preset_name = None;
            self.selected_path = None;
        }
    }
}
