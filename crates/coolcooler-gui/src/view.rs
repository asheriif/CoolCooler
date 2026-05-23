use coolcooler_driver::widgets_allowed;
use iced::widget::{
    button, column, container, mouse_area, pick_list, row, scrollable, slider, text, text_input,
};
use iced::{mouse, window, Background, Border, Color, Element, Length, Theme};
use image::{Rgba, RgbaImage};

use crate::canvas::LayerSelection;
use crate::rendering::circular_preview_from_rgba;
use crate::style::AppColors;
use crate::{widget, CoolCooler, LayerOption, Message};

pub(crate) fn view(app: &CoolCooler, _window_id: window::Id) -> Element<'_, Message> {
    let c = app.colors();

    let title_row = row![
        text("CoolCooler").size(28).color(c.accent),
        iced::widget::space().width(Length::Fill),
        iced::widget::toggler(app.dark_mode)
            .label("Dark")
            .on_toggle(|_| Message::ToggleTheme)
            .size(16)
            .text_size(12),
    ]
    .align_y(iced::Alignment::Center);

    let device_content = if app.driver_connected {
        let info = &app.driver_info;
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
    };

    let lcd_size = app.lcd_size() as f32;

    let handle = app.preview.clone().unwrap_or_else(|| {
        let lcd = app.lcd_size();
        circular_preview_from_rgba(RgbaImage::from_pixel(lcd, lcd, Rgba([0, 0, 0, 255])))
    });

    let is_widget_selected = matches!(app.canvas.active_layer, LayerSelection::Widget(_));
    let cursor_style = if app.dragging {
        mouse::Interaction::Grabbing
    } else if is_widget_selected {
        mouse::Interaction::Move
    } else {
        mouse::Interaction::Grab
    };

    let preview_inner: Element<Message> = row![
        iced::widget::space().width(Length::Fill),
        mouse_area(iced::widget::image(handle).width(lcd_size).height(lcd_size),)
            .on_scroll(Message::Scroll)
            .on_press(Message::DragStart)
            .on_release(Message::DragEnd)
            .on_move(Message::DragMove)
            .interaction(cursor_style),
        iced::widget::space().width(Length::Fill),
    ]
    .into();

    let show_reset = match app.canvas.active_layer {
        LayerSelection::Base => {
            (app.canvas.base_viewport.zoom - 1.0).abs() > 0.01
                || app.canvas.base_viewport.pan != (0.0, 0.0)
        }
        LayerSelection::Widget(id) => app
            .canvas
            .layers
            .iter()
            .find(|l| l.id == id)
            .map(|l| {
                let def = l.widget.descriptor().default_size;
                l.size != def
                    || l.position
                        != (
                            (app.lcd_size() as i32 - def.0 as i32) / 2,
                            (app.lcd_size() as i32 - def.1 as i32) / 2,
                        )
            })
            .unwrap_or(false),
    };

    let canvas_header = if show_reset {
        let info_text = match app.canvas.active_layer {
            LayerSelection::Base => {
                format!("{}%", (app.canvas.base_viewport.zoom * 100.0) as u32)
            }
            LayerSelection::Widget(id) => app
                .canvas
                .layers
                .iter()
                .find(|l| l.id == id)
                .map(|l| format!("{}×{}", l.size.0, l.size.1))
                .unwrap_or_default(),
        };
        row![
            section_label("CANVAS", c),
            iced::widget::space().width(Length::Fill),
            text(info_text).size(11).color(c.text_dim),
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
    } else {
        row![section_label("CANVAS", c)]
    };

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
        .find(|o| o.selection == app.canvas.active_layer)
        .cloned();

    let layer_picker = pick_list(layer_options, active_option, Message::SelectLayer)
        .width(Length::Fill)
        .text_size(13);

    let layer_controls: Element<Message> =
        if let LayerSelection::Widget(id) = app.canvas.active_layer {
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
        };

    let preview_card = card(
        column![canvas_header, preview_inner, layer_controls].spacing(12),
        c,
    );

    let widget_config: Option<Element<Message>> =
        if let LayerSelection::Widget(id) = app.canvas.active_layer {
            app.canvas.layers.iter().find(|l| l.id == id).map(|layer| {
                let opacity_val = layer.opacity as f32 / 255.0;
                let capabilities = layer.widget.capabilities();
                let settings = layer.widget.settings();
                let mut config_items = column![row![
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
                .align_y(iced::Alignment::Center)]
                .spacing(8);

                if capabilities.color {
                    let current_color = settings.color.unwrap_or([255, 255, 255, 255]);
                    let colors: Vec<([u8; 4], &str)> = vec![
                        ([255, 255, 255, 255], "White"),
                        ([220, 220, 220, 255], "Light Gray"),
                        ([80, 255, 80, 255], "Green"),
                        ([255, 80, 60, 255], "Red"),
                        ([0, 180, 255, 255], "Cyan"),
                        ([255, 200, 50, 255], "Gold"),
                        ([255, 140, 0, 255], "Orange"),
                        ([200, 80, 255, 255], "Purple"),
                        ([255, 105, 180, 255], "Pink"),
                    ];

                    let mut swatches = row![text("Color").size(12).color(c.text_dim).width(60)]
                        .spacing(4)
                        .align_y(iced::Alignment::Center);

                    for (color, _name) in &colors {
                        let c = *color;
                        let is_selected = current_color[0..3] == c[0..3];
                        let border_color = if is_selected {
                            Color::WHITE
                        } else {
                            Color::TRANSPARENT
                        };
                        let swatch_color = Color::from_rgb8(c[0], c[1], c[2]);

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
                                .on_press(Message::SetWidgetTextColor(c)),
                        );
                    }

                    config_items = config_items.push(swatches);
                }

                if capabilities.font {
                    let font_names: Vec<String> = widget::fonts::font_names()
                        .into_iter()
                        .map(|s| s.to_string())
                        .collect();
                    let current_font = settings
                        .font_name
                        .clone()
                        .unwrap_or_else(|| widget::fonts::DEFAULT_FONT.to_string());
                    config_items = config_items.push(
                        row![
                            text("Font").size(12).color(c.text_dim).width(60),
                            pick_list(font_names, Some(current_font), Message::SetWidgetFont)
                                .width(Length::Fill)
                                .text_size(12),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    );
                }

                if capabilities.text {
                    let current_text = settings.text.clone().unwrap_or_default();
                    config_items = config_items.push(
                        row![
                            text("Text").size(12).color(c.text_dim).width(60),
                            iced::widget::text_input("Enter text...", &current_text)
                                .on_input(Message::SetWidgetText)
                                .size(12)
                                .width(Length::Fill),
                        ]
                        .spacing(8)
                        .align_y(iced::Alignment::Center),
                    );
                }

                card(config_items, c).into()
            })
        } else {
            None
        };

    let status = text(&app.status_message).size(12).color(c.text_dim);

    let select_btn = if app.loading {
        styled_button("Loading...", ButtonKind::Disabled, c)
    } else {
        styled_button("Select Image", ButtonKind::Default, c).on_press(Message::SelectFile)
    };

    let save_label = if app.current_preset_name.is_some() {
        "Save"
    } else {
        "Save Preset"
    };
    let save_btn =
        styled_button(save_label, ButtonKind::Default, c).on_press(Message::ShowSaveDialog);

    let mut preset_buttons = row![save_btn].spacing(8);
    if app.current_preset_name.is_some() {
        preset_buttons = preset_buttons
            .push(styled_button("Save As", ButtonKind::Default, c).on_press(Message::SavePresetAs));
    }
    preset_buttons = preset_buttons.push(
        styled_button("Load Preset", ButtonKind::Default, c).on_press(Message::ShowLoadDialog),
    );

    let quit_btn = styled_button("Quit", ButtonKind::Danger, c).on_press(Message::Quit);

    let mut left_panel = column![card(device_content, c), preview_card,]
        .spacing(16)
        .width(Length::FillPortion(7))
        .height(Length::Fill);

    if let Some(config) = widget_config {
        left_panel = left_panel.push(config);
    }

    left_panel = left_panel
        .push(status)
        .push(iced::widget::space().height(Length::Fill))
        .push(select_btn)
        .push(preset_buttons)
        .push(quit_btn);

    let right_panel = if !widgets_allowed(app.driver_capability, app.is_animated()) {
        card(
            column![
                text("Widgets").size(18).color(c.text_primary),
                text("Widgets with animated backgrounds are not supported on this device.")
                    .size(13)
                    .color(c.text_dim),
            ]
            .spacing(12),
            c,
        )
        .width(Length::FillPortion(3))
        .height(Length::Fill)
    } else {
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

        let mut catalog_items = column![].spacing(6);
        for (i, spec) in app.widget_catalog.iter().enumerate() {
            let desc = spec.descriptor;
            if desc.category != app.selected_category {
                continue;
            }
            catalog_items = catalog_items.push(
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

        card(
            column![
                text("Widgets").size(18).color(c.text_primary),
                category_picker,
                catalog_items,
            ]
            .spacing(12),
            c,
        )
        .width(Length::FillPortion(3))
        .height(Length::Fill)
    };

    let footer = text("Made with ♥ by asheriif")
        .size(11)
        .color(c.text_dim)
        .center()
        .width(Length::Fill);

    if app.show_save_dialog {
        return container(card(
            column![
                text("Save Preset").size(22).color(c.text_primary),
                text_input("Preset name...", &app.save_name_input)
                    .on_input(Message::SaveNameChanged)
                    .on_submit(Message::SavePreset)
                    .size(14)
                    .padding(10),
                row![
                    styled_button("Save", ButtonKind::Default, c).on_press(Message::SavePreset),
                    styled_button("Cancel", ButtonKind::Danger, c)
                        .on_press(Message::CloseSaveDialog),
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
        .into();
    }

    if app.show_load_dialog {
        let mut preset_grid = column![].spacing(12);

        if app.preset_list.is_empty() {
            preset_grid = preset_grid.push(
                container(
                    text("No presets saved yet")
                        .size(14)
                        .color(c.text_dim)
                        .center()
                        .width(Length::Fill),
                )
                .padding(40),
            );
        } else {
            let mut grid_row = row![].spacing(12);
            for (i, entry) in app.preset_list.iter().enumerate() {
                let folder = entry.folder.clone();
                let folder_del = entry.folder.clone();

                let preview_el: Element<Message> = if let Some(ref handle) = entry.preview {
                    container(iced::widget::image(handle.clone()).width(120).height(120))
                        .style(|_: &Theme| container::Style {
                            border: Border {
                                radius: 8.0.into(),
                                ..Default::default()
                            },
                            ..Default::default()
                        })
                        .into()
                } else {
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
                };

                let preset_card: Element<Message> = mouse_area(
                    container(
                        column![
                            preview_el,
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
                .into();

                grid_row = grid_row.push(preset_card);

                if (i + 1) % 4 == 0 {
                    preset_grid = preset_grid.push(grid_row);
                    grid_row = row![].spacing(12);
                }
            }
            if !app.preset_list.len().is_multiple_of(4) {
                preset_grid = preset_grid.push(grid_row);
            }
        }

        return column![
            row![
                text("Load Preset").size(22).color(c.text_primary),
                iced::widget::space().width(Length::Fill),
                styled_button("Back", ButtonKind::Default, c).on_press(Message::CloseLoadDialog),
            ]
            .align_y(iced::Alignment::Center),
            scrollable(preset_grid).height(Length::Fill),
        ]
        .spacing(16)
        .padding(24)
        .height(Length::Fill)
        .into();
    }

    column![
        title_row,
        row![left_panel, right_panel]
            .spacing(16)
            .height(Length::Fill),
        footer,
    ]
    .spacing(16)
    .padding(24)
    .height(Length::Fill)
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
