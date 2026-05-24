use std::sync::{Arc, Mutex};

use iced::{window, Task};

use crate::windowing::app_window_settings;
use crate::{tray, CoolCooler, Message};

pub(crate) struct WindowState {
    _tray_handle: tray::TrayHandle,
    tray_rx: Arc<Mutex<std::sync::mpsc::Receiver<tray::TrayEvent>>>,
    window_id: Option<window::Id>,
}

impl WindowState {
    pub(crate) fn new(
        window_id: window::Id,
        tray_handle: tray::TrayHandle,
        tray_rx: std::sync::mpsc::Receiver<tray::TrayEvent>,
    ) -> Self {
        Self {
            _tray_handle: tray_handle,
            tray_rx: Arc::new(Mutex::new(tray_rx)),
            window_id: Some(window_id),
        }
    }

    fn poll_tray(&self) -> Option<tray::TrayEvent> {
        self.tray_rx.lock().ok()?.try_recv().ok()
    }

    fn window_id(&self) -> Option<window::Id> {
        self.window_id
    }

    fn set_window_id(&mut self, window_id: Option<window::Id>) {
        self.window_id = window_id;
    }
}

impl CoolCooler {
    pub(crate) fn tray_poll(&mut self) -> Task<Message> {
        let got_show = matches!(self.windows.poll_tray(), Some(tray::TrayEvent::ShowWindow));
        if got_show {
            self.show_window()
        } else {
            Task::none()
        }
    }

    pub(crate) fn show_window(&mut self) -> Task<Message> {
        if let Some(window_id) = self.windows.window_id() {
            return window::gain_focus(window_id);
        }
        let (id, task) = window::open(app_window_settings());
        self.windows.set_window_id(Some(id));
        task.discard()
    }

    pub(crate) fn window_closed(&mut self, id: window::Id) {
        if self.windows.window_id() == Some(id) {
            self.windows.set_window_id(None);
        }
    }

    pub(crate) fn quit(&mut self) -> Task<Message> {
        self.stop_display();
        iced::exit()
    }
}
