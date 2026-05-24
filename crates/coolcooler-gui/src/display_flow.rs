use crate::canvas_editor::render_composited;
use crate::display_session::DisplayNotice;
use crate::CoolCooler;

impl CoolCooler {
    pub(crate) fn start_display(&mut self) {
        let source = &self.source;
        let canvas = &self.canvas;
        let widget_ctx = self.widgets.context();
        self.display
            .restart_with(|resolution| render_composited(source, canvas, widget_ctx, resolution));
    }

    pub(crate) fn stop_display(&mut self) {
        self.display.stop();
    }

    pub(crate) fn reap_display_session(&mut self) {
        for notice in self.display.poll_lifecycle() {
            match notice {
                DisplayNotice::Started(name) => {
                    self.ui.status_message = format!("Displaying on {name}");
                }
                DisplayNotice::Reconnecting(message) => {
                    self.ui.status_message = format!("Display reconnecting: {message}");
                }
                DisplayNotice::TransferFailed(message) => {
                    self.ui.status_message = format!("Display transfer failed: {message}");
                }
                DisplayNotice::Failed(message) => {
                    self.ui.status_message = format!("Display failed: {message}");
                }
                DisplayNotice::Stopped => {}
            }
        }
    }
}
