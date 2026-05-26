use iced::Task;

use crate::{widget, CoolCooler, Message};

impl CoolCooler {
    pub(crate) fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::SelectFile => self.select_file(),
            Message::FileSelected(path) => self.file_selected(path),
            Message::SourceLoaded { request, result } => {
                self.source_loaded(request, result);
                Task::none()
            }
            Message::AnimationTick => {
                self.animation_tick();
                Task::none()
            }
            Message::WidgetTick => {
                self.widget_tick();
                Task::none()
            }
            Message::Scroll(delta) => {
                self.scroll_canvas(delta);
                Task::none()
            }
            Message::DragStart => {
                self.start_drag();
                Task::none()
            }
            Message::DragMove(pos) => {
                self.drag_canvas(pos);
                Task::none()
            }
            Message::DragEnd => {
                self.end_drag();
                Task::none()
            }
            Message::ResetView => {
                self.reset_active_layer();
                Task::none()
            }
            Message::SelectLayer(option) => {
                self.canvas.select_layer(option.selection);
                Task::none()
            }
            Message::SelectCategory(cat) => {
                self.widgets.select_category(cat);
                Task::none()
            }
            Message::AddWidget(catalog_idx) => self.add_widget(catalog_idx),
            Message::RemoveWidget(id) => {
                self.canvas.remove_widget(id);
                self.commit_frame();
                Task::none()
            }
            Message::SetWidgetOpacity(val) => {
                if self.canvas.set_active_widget_opacity((val * 255.0) as u8) {
                    self.commit_frame();
                }
                Task::none()
            }
            Message::SetWidgetTextColor(color) => {
                self.edit_active_widget(widget::WidgetEdit::Color(color));
                Task::none()
            }
            Message::SetWidgetFont(name) => {
                self.edit_active_widget(widget::WidgetEdit::Font(name));
                Task::none()
            }
            Message::SetWidgetText(text) => {
                self.edit_active_widget(widget::WidgetEdit::Text(text));
                Task::none()
            }
            Message::SetWidgetThickness(thickness) => {
                self.edit_active_widget(widget::WidgetEdit::Thickness(thickness));
                Task::none()
            }
            Message::ShowSaveDialog => {
                self.save_requested();
                Task::none()
            }
            Message::ShowLoadDialog => {
                self.presets.open_load_dialog();
                Task::none()
            }
            Message::CloseSaveDialog => {
                self.presets.show_save_dialog = false;
                Task::none()
            }
            Message::CloseLoadDialog => {
                self.presets.show_load_dialog = false;
                Task::none()
            }
            Message::SaveNameChanged(name) => {
                self.presets.save_name_input = name;
                Task::none()
            }
            Message::SavePreset => {
                self.save_preset();
                Task::none()
            }
            Message::SavePresetAs => {
                self.presets.save_name_input.clear();
                self.presets.show_save_dialog = true;
                Task::none()
            }
            Message::LoadLastPreset(folder) => self.load_preset_folder(folder, true),
            Message::PresetClicked(folder) => self.preset_clicked(folder),
            Message::PresetSourceLoaded {
                request,
                result,
                job,
            } => {
                self.preset_source_loaded(request, result, job);
                Task::none()
            }
            Message::DeletePreset(folder) => {
                self.delete_preset(folder);
                Task::none()
            }
            Message::ToggleTheme => {
                self.ui.dark_mode = !self.ui.dark_mode;
                Task::none()
            }
            Message::TrayPoll => self.tray_poll(),
            Message::DisplayWorkerPoll => {
                self.poll_display_worker();
                Task::none()
            }
            Message::WindowClosed(id) => {
                self.window_closed(id);
                Task::none()
            }
            Message::Quit => self.quit(),
        }
    }
}
