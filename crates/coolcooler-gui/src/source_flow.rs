use std::path::PathBuf;

use iced::Task;

use crate::canvas::Viewport;
use crate::composition::{CanvasPolicy, SourceKind};
use crate::source::{load_source_data, LoadedData, SourceLoadRequest};
use crate::windowing::pick_file;
use crate::{CoolCooler, Message};

impl CoolCooler {
    pub(crate) fn select_file(&mut self) -> Task<Message> {
        Task::perform(pick_file(), Message::FileSelected)
    }

    pub(crate) fn file_selected(&mut self, path: Option<PathBuf>) -> Task<Message> {
        let Some(path) = path else {
            return Task::none();
        };

        let request = self.source.begin_loading(path);
        let task_request = request.clone();
        self.stop_display();
        self.ui.preview = None;
        self.canvas.set_base_viewport(Viewport::default());
        self.ui.status_message = "Loading...".to_string();

        Task::perform(
            async move { load_source_data(task_request.path(), task_request.filename().to_string()) },
            move |result| Message::SourceLoaded { request, result },
        )
    }

    pub(crate) fn source_loaded(
        &mut self,
        request: SourceLoadRequest,
        result: Result<LoadedData, String>,
    ) {
        match result {
            Ok(data) => {
                let Some(summary) = self.source.complete_loading(&request, data, None) else {
                    return;
                };

                let policy = CanvasPolicy::for_content(
                    self.display.capability(),
                    SourceKind::from_frame_count(summary.frame_count),
                );
                if !policy.widgets_allowed() && self.canvas.has_widgets() {
                    self.canvas.clear_widgets();
                }

                let detail = if summary.frame_count > 1 {
                    format!(" ({} frames)", summary.frame_count)
                } else {
                    String::new()
                };
                self.ui.status_message = format!("{}{detail}", summary.filename);
                self.rebuild_preview();
                self.start_display();
            }
            Err(e) => {
                if self.source.fail_loading(&request) {
                    self.ui.status_message = format!("Error: {e}");
                }
            }
        }
    }

    pub(crate) fn animation_tick(&mut self) {
        if self.source.advance_frame_if_due() {
            self.commit_frame();
        }
    }
}
