use iced::{window, Task};

use crate::windowing::app_window_settings;
use crate::{tray, CoolCooler, Message};

impl CoolCooler {
    pub(crate) fn tray_poll(&mut self) -> Task<Message> {
        let got_show = matches!(
            self.tray_rx.lock().unwrap().try_recv(),
            Ok(tray::TrayEvent::ShowWindow)
        );
        if got_show {
            self.show_window()
        } else {
            Task::none()
        }
    }

    pub(crate) fn show_window(&mut self) -> Task<Message> {
        if let Some(window_id) = self.window_id {
            return window::gain_focus(window_id);
        }
        let (id, task) = window::open(app_window_settings());
        self.window_id = Some(id);
        task.discard()
    }

    pub(crate) fn quit(&mut self) -> Task<Message> {
        self.stop_display();
        iced::exit()
    }
}
