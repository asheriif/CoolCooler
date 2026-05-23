use crate::CoolCooler;

impl CoolCooler {
    pub(crate) fn start_display(&mut self) {
        let composited = self.render_composited();
        self.display.restart(&composited);
    }

    pub(crate) fn stop_display(&mut self) {
        self.display.stop();
    }

    pub(crate) fn reap_display_session(&mut self) {
        self.display.join_finished();
    }
}
