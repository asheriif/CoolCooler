use iced::widget::{
    button, column, container, mouse_area, pick_list, row, scrollable, slider, text, text_input,
};
use iced::{mouse, window, Background, Border, Color, Element, Length, Theme};
use image::{Rgba, RgbaImage};

use crate::rendering::circular_preview_from_rgba;
use crate::style::AppColors;
use crate::{widget, CoolCooler, LayerOption, Message};

pub(crate) fn view(app: &CoolCooler, _window_id: window::Id) -> Element<'_, Message> {
    let c = app.colors();

    if app.show_save_dialog {
        return save_dialog(app, c);
    }

    if app.show_load_dialog {
        return load_dialog(app, c);
    }

    column![
        title_bar(app, c),
        row![left_panel(app, c), widget_catalog_panel(app, c)]
            .spacing(16)
            .height(Length::Fill),
        footer(c),
    ]
    .spacing(16)
    .padding(24)
    .height(Length::Fill)
    .into()
}

fn title_bar<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    row![
        text("CoolCooler").size(28).color(c.accent),
        iced::widget::space().width(Length::Fill),
        iced::widget::toggler(app.dark_mode)
            .label("Dark")
            .on_toggle(|_| Message::ToggleTheme)
            .size(16)
            .text_size(12),
    ]
    .align_y(iced::Alignment::Center)
    .into()
}

fn left_panel<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    let mut panel = column![device_card(app, c), preview_card(app, c)]
        .spacing(16)
        .width(Length::FillPortion(7))
        .height(Length::Fill);

    if let Some(config) = widget_config_card(app, c) {
        panel = panel.push(config);
    }

    panel
        .push(text(&app.status_message).size(12).color(c.text_dim))
        .push(iced::widget::space().height(Length::Fill))
        .push(select_image_button(app, c))
        .push(preset_buttons(app, c))
        .push(styled_button("Quit", ButtonKind::Danger, c).on_press(Message::Quit))
        .into()
}

fn device_card<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    let content: Element<Message> = if let Some(info) = app.display.device_info() {
        let (w, h) = (info.resolution.width, info.resolution.height);
        column![
            section_label("DEVICE", c),
            row![
                text("●").size(10).color(c.green),
                text(info.name.as_str()).size(15),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
            text(format!("{w}×{h} LCD")).size(12).color(c.text_dim),
        ]
        .spacing(6)
        .into()
    } else {
        column![
            section_label("DEVICE", c),
            row![
                text("●").size(10).color(c.red),
                text("No cooler detected").size(15),
            ]
            .spacing(8)
            .align_y(iced::Alignment::Center),
        ]
        .spacing(6)
        .into()
    };

    card(content, c).into()
}

fn preview_card<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    card(
        column![
            canvas_header(app, c),
            preview_canvas(app),
            layer_controls(app, c),
        ]
        .spacing(12),
        c,
    )
    .into()
}

fn preview_canvas(app: &CoolCooler) -> Element<'_, Message> {
    let resolution = app.lcd_resolution();
    let handle = app.preview.clone().unwrap_or_else(|| {
        circular_preview_from_rgba(RgbaImage::from_pixel(
            resolution.width,
            resolution.height,
            Rgba([0, 0, 0, 255]),
        ))
    });
    let is_widget_selected = app.canvas.active_widget_layer().is_some();
    let cursor_style = if app.dragging {
        mouse::Interaction::Grabbing
    } else if is_widget_selected {
        mouse::Interaction::Move
    } else {
        mouse::Interaction::Grab
    };

    row![
        iced::widget::space().width(Length::Fill),
        mouse_area(
            iced::widget::image(handle)
                .width(resolution.width as f32)
                .height(resolution.height as f32),
        )
        .on_scroll(Message::Scroll)
        .on_press(Message::DragStart)
        .on_release(Message::DragEnd)
        .on_move(Message::DragMove)
        .interaction(cursor_style),
        iced::widget::space().width(Length::Fill),
    ]
    .into()
}

fn canvas_header<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    if !canvas_can_reset(app) {
        return row![section_label("CANVAS", c)].into();
    }

    row![
        section_label("CANVAS", c),
        iced::widget::space().width(Length::Fill),
        text(canvas_reset_status(app)).size(11).color(c.text_dim),
        button(text("Reset").size(10).color(c.accent))
            .padding([2, 8])
            .style(|_: &Theme, _| button::Style {
                background: None,
                text_color: c.accent,
                ..Default::default()
            })
            .on_press(Message::ResetView),
    ]
    .spacing(6)
    .align_y(iced::Alignment::Center)
    .into()
}

fn canvas_can_reset(app: &CoolCooler) -> bool {
    app.canvas.active_layer_can_reset(app.lcd_resolution())
}

fn canvas_reset_status(app: &CoolCooler) -> String {
    app.canvas.active_layer_reset_status()
}

fn layer_controls<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    let layer_options: Vec<LayerOption> = app
        .canvas
        .layer_options()
        .into_iter()
        .map(|(sel, label)| LayerOption {
            selection: sel,
            label,
        })
        .collect();
    let active_option = layer_options
        .iter()
        .find(|o| o.selection == app.canvas.active_layer())
        .cloned();
    let layer_picker = pick_list(layer_options, active_option, Message::SelectLayer)
        .width(Length::Fill)
        .text_size(13);

    if let Some(layer) = app.canvas.active_widget_layer() {
        let id = layer.id;
        row![
            text("Layer").size(12).color(c.text_dim),
            layer_picker,
            button(text("Remove").size(11).color(c.red))
                .padding([6, 12])
                .style({
                    let danger_bg = c.danger_bg;
                    let red = c.red;
                    move |_: &Theme, _| button::Style {
                        background: Some(Background::Color(danger_bg)),
                        text_color: red,
                        border: Border {
                            radius: 6.0.into(),
                            ..Default::default()
                        },
                        ..Default::default()
                    }
                })
                .on_press(Message::RemoveWidget(id)),
        ]
        .spacing(8)
        .align_y(iced::Alignment::Center)
        .into()
    } else {
        row![text("Layer").size(12).color(c.text_dim), layer_picker]
            .spacing(8)
            .align_y(iced::Alignment::Center)
            .into()
    }
}

fn widget_config_card<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Option<Element<'a, Message>> {
    app.canvas.active_widget_layer().map(|layer| {
        let opacity_val = layer.opacity as f32 / 255.0;
        let controls = layer.widget.controls();
        let mut config_items = column![opacity_control(opacity_val, c)].spacing(8);

        if let Some(color) = controls.color {
            config_items = config_items.push(color_controls(color, c));
        }
        if let Some(font_name) = controls.font_name {
            config_items = config_items.push(font_control(font_name.to_string(), c));
        }
        if let Some(text) = controls.editable_text {
            config_items = config_items.push(text_control(text.to_string(), c));
        }

        card(config_items, c).into()
    })
}

fn opacity_control(opacity_val: f32, c: &AppColors) -> Element<'_, Message> {
    row![
        text("Opacity").size(12).color(c.text_dim).width(60),
        slider(0.0..=1.0, opacity_val, Message::SetWidgetOpacity)
            .step(0.01)
            .width(Length::Fill),
        text(format!("{}%", (opacity_val * 100.0) as u32))
            .size(11)
            .color(c.text_dim)
            .width(35),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

fn color_controls(current_color: [u8; 4], c: &AppColors) -> Element<'_, Message> {
    let colors: Vec<[u8; 4]> = vec![
        [255, 255, 255, 255],
        [220, 220, 220, 255],
        [80, 255, 80, 255],
        [255, 80, 60, 255],
        [0, 180, 255, 255],
        [255, 200, 50, 255],
        [255, 140, 0, 255],
        [200, 80, 255, 255],
        [255, 105, 180, 255],
    ];

    let mut swatches = row![text("Color").size(12).color(c.text_dim).width(60)]
        .spacing(4)
        .align_y(iced::Alignment::Center);

    for color in colors {
        let is_selected = current_color[0..3] == color[0..3];
        let border_color = if is_selected {
            Color::WHITE
        } else {
            Color::TRANSPARENT
        };
        let swatch_color = Color::from_rgb8(color[0], color[1], color[2]);

        swatches = swatches.push(
            button(text("").width(14).height(14))
                .padding(0)
                .width(18)
                .height(18)
                .style(move |_: &Theme, _| button::Style {
                    background: Some(Background::Color(swatch_color)),
                    border: Border {
                        radius: 3.0.into(),
                        width: 2.0,
                        color: border_color,
                    },
                    ..Default::default()
                })
                .on_press(Message::SetWidgetTextColor(color)),
        );
    }

    swatches.into()
}

fn font_control(current_font: String, c: &AppColors) -> Element<'_, Message> {
    let font_names: Vec<String> = widget::fonts::font_names()
        .into_iter()
        .map(|s| s.to_string())
        .collect();

    row![
        text("Font").size(12).color(c.text_dim).width(60),
        pick_list(font_names, Some(current_font), Message::SetWidgetFont)
            .width(Length::Fill)
            .text_size(12),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

fn text_control(current_text: String, c: &AppColors) -> Element<'_, Message> {
    row![
        text("Text").size(12).color(c.text_dim).width(60),
        iced::widget::text_input("Enter text...", &current_text)
            .on_input(Message::SetWidgetText)
            .size(12)
            .width(Length::Fill),
    ]
    .spacing(8)
    .align_y(iced::Alignment::Center)
    .into()
}

fn select_image_button<'a>(app: &CoolCooler, c: &'a AppColors) -> button::Button<'a, Message> {
    if app.loading {
        styled_button("Loading...", ButtonKind::Disabled, c)
    } else {
        styled_button("Select Image", ButtonKind::Default, c).on_press(Message::SelectFile)
    }
}

fn preset_buttons<'a>(app: &CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    let save_label = if app.current_preset_name.is_some() {
        "Save"
    } else {
        "Save Preset"
    };
    let save_btn =
        styled_button(save_label, ButtonKind::Default, c).on_press(Message::ShowSaveDialog);
    let mut buttons = row![save_btn].spacing(8);

    if app.current_preset_name.is_some() {
        buttons = buttons
            .push(styled_button("Save As", ButtonKind::Default, c).on_press(Message::SavePresetAs));
    }

    buttons
        .push(
            styled_button("Load Preset", ButtonKind::Default, c).on_press(Message::ShowLoadDialog),
        )
        .into()
}

fn widget_catalog_panel<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    let policy = app.canvas_policy();
    if let Some(message) = policy.widget_block_message() {
        return card(
            column![
                text("Widgets").size(18).color(c.text_primary),
                text(message).size(13).color(c.text_dim),
            ]
            .spacing(12),
            c,
        )
        .width(Length::FillPortion(3))
        .height(Length::Fill)
        .into();
    }

    let categories: Vec<String> = widget::categories(app.widget_catalog)
        .into_iter()
        .map(|s| s.to_string())
        .collect();
    let category_picker = pick_list(
        categories,
        Some(app.selected_category.clone()),
        Message::SelectCategory,
    )
    .width(Length::Fill)
    .text_size(13);

    card(
        column![
            text("Widgets").size(18).color(c.text_primary),
            category_picker,
            catalog_items(app, c),
        ]
        .spacing(12),
        c,
    )
    .width(Length::FillPortion(3))
    .height(Length::Fill)
    .into()
}

fn catalog_items<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    let mut items = column![].spacing(6);
    for (i, spec) in app.widget_catalog.iter().enumerate() {
        let desc = spec.descriptor;
        if desc.category != app.selected_category {
            continue;
        }
        items = items.push(
            button(text(desc.name).size(13).color(c.text_primary).center())
                .padding([8, 12])
                .width(Length::Fill)
                .style(|_: &Theme, _| button::Style {
                    background: Some(Background::Color(c.surface)),
                    text_color: c.text_primary,
                    border: Border {
                        radius: 8.0.into(),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .on_press(Message::AddWidget(i)),
        );
    }
    items.into()
}

fn save_dialog<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    container(card(
        column![
            text("Save Preset").size(22).color(c.text_primary),
            text_input("Preset name...", &app.save_name_input)
                .on_input(Message::SaveNameChanged)
                .on_submit(Message::SavePreset)
                .size(14)
                .padding(10),
            row![
                styled_button("Save", ButtonKind::Default, c).on_press(Message::SavePreset),
                styled_button("Cancel", ButtonKind::Danger, c).on_press(Message::CloseSaveDialog),
            ]
            .spacing(8),
        ]
        .spacing(16)
        .width(350),
        c,
    ))
    .width(Length::Fill)
    .height(Length::Fill)
    .center_x(Length::Shrink)
    .center_y(Length::Shrink)
    .padding(24)
    .into()
}

fn load_dialog<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    column![
        row![
            text("Load Preset").size(22).color(c.text_primary),
            iced::widget::space().width(Length::Fill),
            styled_button("Back", ButtonKind::Default, c).on_press(Message::CloseLoadDialog),
        ]
        .align_y(iced::Alignment::Center),
        scrollable(preset_grid(app, c)).height(Length::Fill),
    ]
    .spacing(16)
    .padding(24)
    .height(Length::Fill)
    .into()
}

fn preset_grid<'a>(app: &'a CoolCooler, c: &'a AppColors) -> Element<'a, Message> {
    if app.preset_list.is_empty() {
        return column![container(
            text("No presets saved yet")
                .size(14)
                .color(c.text_dim)
                .center()
                .width(Length::Fill),
        )
        .padding(40)]
        .spacing(12)
        .into();
    }

    let mut grid = column![].spacing(12);
    let mut grid_row = row![].spacing(12);
    for (i, entry) in app.preset_list.iter().enumerate() {
        grid_row = grid_row.push(preset_card(entry, c));
        if (i + 1) % 4 == 0 {
            grid = grid.push(grid_row);
            grid_row = row![].spacing(12);
        }
    }
    if !app.preset_list.len().is_multiple_of(4) {
        grid = grid.push(grid_row);
    }
    grid.into()
}

fn preset_card<'a>(
    entry: &'a crate::preset::PresetEntry,
    c: &'a AppColors,
) -> Element<'a, Message> {
    let folder = entry.folder.clone();
    let folder_del = entry.folder.clone();

    mouse_area(
        container(
            column![
                preset_preview(entry, c),
                text(&entry.name)
                    .size(12)
                    .color(c.text_primary)
                    .center()
                    .width(Length::Fill),
                button(text("Delete").size(9).color(c.red).center())
                    .padding([2, 8])
                    .width(Length::Fill)
                    .style(|_: &Theme, _| button::Style {
                        background: None,
                        text_color: c.red,
                        ..Default::default()
                    })
                    .on_press(Message::DeletePreset(folder_del)),
            ]
            .spacing(6)
            .align_x(iced::Alignment::Center),
        )
        .padding(10)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(c.card_bg)),
            border: Border {
                radius: 10.0.into(),
                width: 1.0,
                color: Color::from_rgb(0.2, 0.2, 0.28),
            },
            ..Default::default()
        }),
    )
    .on_press(Message::PresetClicked(folder))
    .into()
}

fn preset_preview<'a>(
    entry: &'a crate::preset::PresetEntry,
    c: &'a AppColors,
) -> Element<'a, Message> {
    if let Some(ref path) = entry.preview_path {
        return container(
            iced::widget::image(iced::widget::image::Handle::from_path(path))
                .width(120)
                .height(120),
        )
        .style(|_: &Theme| container::Style {
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into();
    }

    container(text("No preview").size(10).color(c.text_dim))
        .width(120)
        .height(120)
        .center_x(Length::Shrink)
        .center_y(Length::Shrink)
        .style(|_: &Theme| container::Style {
            background: Some(Background::Color(c.surface)),
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
        .into()
}

fn footer(c: &AppColors) -> Element<'_, Message> {
    text("Made with ♥ by asheriif")
        .size(11)
        .color(c.text_dim)
        .center()
        .width(Length::Fill)
        .into()
}

fn section_label<'a>(label: &'a str, c: &AppColors) -> iced::widget::Text<'a> {
    text(label).size(11).color(c.text_dim)
}

fn card<'a>(
    content: impl Into<Element<'a, Message>>,
    c: &AppColors,
) -> container::Container<'a, Message> {
    let bg = c.card_bg;
    container(content)
        .padding(16)
        .width(Length::Fill)
        .style(move |_theme: &Theme| container::Style {
            background: Some(Background::Color(bg)),
            border: Border {
                radius: 12.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
}

enum ButtonKind {
    Default,
    Danger,
    Disabled,
}

fn styled_button<'a>(
    label: &'a str,
    kind: ButtonKind,
    c: &AppColors,
) -> button::Button<'a, Message> {
    let (bg, text_color) = match kind {
        ButtonKind::Default => (c.surface, c.text_primary),
        ButtonKind::Danger => (c.danger_bg, c.red),
        ButtonKind::Disabled => (c.disabled_bg, c.text_dim),
    };

    button(text(label).size(14).color(text_color).center())
        .padding([10, 20])
        .width(Length::Fill)
        .style(move |_theme: &Theme, _status| button::Style {
            background: Some(Background::Color(bg)),
            text_color,
            border: Border {
                radius: 8.0.into(),
                ..Default::default()
            },
            ..Default::default()
        })
}
