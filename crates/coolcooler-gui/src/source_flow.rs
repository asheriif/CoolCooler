use std::path::PathBuf;
use std::time::Instant;

use iced::Task;

use crate::canvas::Viewport;
use crate::composition::{CanvasPolicy, SourceKind};
use crate::source::{load_source_data, LoadedData};
use crate::windowing::pick_file;
use crate::{CoolCooler, Message};

impl CoolCooler {
    pub(crate) fn is_animated(&self) -> bool {
        self.source_frames.len() > 1
    }

    pub(crate) fn select_file(&mut self) -> Task<Message> {
        Task::perform(pick_file(), Message::FileSelected)
    }

    pub(crate) fn file_selected(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            return Task::none();
        };

        self.selected_path = Some(path.clone());
        self.stop_display();
        self.source_frames.clear();
        self.preview = None;
        self.loading = true;
        self.canvas.set_base_viewport(Viewport::default());
        self.current_frame = 0;
        self.status_message = "Loading...".to_string();

        let filename = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();

        Task::perform(
            async move { load_source_data(&path, filename) },
            Message::SourceLoaded,
        )
    }

    pub(crate) fn source_loaded(&mut self, result: Result<LoadedData, String>) {
        self.loading = false;
        match result {
            Ok(data) => {
                let count = data.frame_count();
                let (frames, filename) = data.into_parts();
                self.filename = filename;
                self.source_frames = frames;

                let policy = CanvasPolicy::for_content(
                    self.display.capability(),
                    SourceKind::from_frame_count(count),
                );
                if !policy.widgets_allowed() && self.canvas.has_widgets() {
                    self.canvas.clear_widgets();
                }

                let detail = if count > 1 {
                    format!(" ({count} frames)")
                } else {
                    String::new()
                };
                self.status_message = format!("{}{detail}", self.filename);
                self.current_frame = 0;
                self.last_advance = Instant::now();
                self.rebuild_preview();
                self.start_display();
            }
            Err(e) => {
                self.status_message = format!("Error: {e}");
            }
        }
    }

    pub(crate) fn animation_tick(&mut self) {
        if self.is_animated() {
            let dur = self.source_frames[self.current_frame].duration;
            if self.last_advance.elapsed() >= dur {
                self.current_frame = (self.current_frame + 1) % self.source_frames.len();
                self.last_advance = Instant::now();
                self.commit_frame();
            }
        }
    }
}
